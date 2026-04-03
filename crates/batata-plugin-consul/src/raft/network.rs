/// Consul Raft network layer using gRPC.
///
/// Sends Raft protocol messages (AppendEntries, Vote, InstallSnapshot)
/// to other cluster nodes via the `ConsulRaftService` gRPC service
/// on the cluster port (9849).
use std::collections::HashMap;
use std::sync::Arc;

use openraft::BasicNode;
use openraft::error::{InstallSnapshotError, RPCError, RaftError, Unreachable};
use openraft::network::{RPCOption, RaftNetwork, RaftNetworkFactory};
use openraft::raft::{
    AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest, InstallSnapshotResponse,
    VoteRequest, VoteResponse,
};
use tokio::sync::RwLock;
use tonic::transport::Channel;
use tracing::{debug, warn};

use batata_api::raft::{self as proto, consul_raft_service_client::ConsulRaftServiceClient};
use batata_consistency::raft::proto_convert;

use super::types::*;

pub struct ConsulRaftNetworkFactory {
    channels: Arc<RwLock<HashMap<NodeId, Channel>>>,
}

impl ConsulRaftNetworkFactory {
    pub fn new() -> Self {
        Self {
            channels: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for ConsulRaftNetworkFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl RaftNetworkFactory<ConsulTypeConfig> for ConsulRaftNetworkFactory {
    type Network = ConsulRaftNetworkConnection;

    async fn new_client(&mut self, target: NodeId, node: &BasicNode) -> Self::Network {
        ConsulRaftNetworkConnection {
            target,
            target_addr: node.addr.clone(),
            channels: self.channels.clone(),
        }
    }
}

pub struct ConsulRaftNetworkConnection {
    target: NodeId,
    target_addr: String,
    channels: Arc<RwLock<HashMap<NodeId, Channel>>>,
}

impl ConsulRaftNetworkConnection {
    /// Get or create a gRPC channel to the target node's cluster port.
    async fn get_channel(&self) -> Result<Channel, Unreachable> {
        if let Some(ch) = self.channels.read().await.get(&self.target) {
            return Ok(ch.clone());
        }

        // Derive cluster gRPC port: main_port + 1001 (same as Nacos Raft)
        let grpc_addr = raft_grpc_addr(&self.target_addr);
        let endpoint = format!("http://{}", grpc_addr);

        debug!(
            "Consul Raft: connecting to node {} at {}",
            self.target, endpoint
        );

        let channel = Channel::from_shared(endpoint)
            .map_err(|e| Unreachable::new(&e))?
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(10))
            .tcp_keepalive(Some(std::time::Duration::from_secs(10)))
            .tcp_nodelay(true)
            .http2_keep_alive_interval(std::time::Duration::from_secs(10))
            .keep_alive_timeout(std::time::Duration::from_secs(5))
            .connect()
            .await
            .map_err(|e| {
                warn!(
                    "Consul Raft: failed to connect to node {}: {}",
                    self.target, e
                );
                Unreachable::new(&e)
            })?;

        self.channels
            .write()
            .await
            .insert(self.target, channel.clone());
        Ok(channel)
    }

    /// Convert openraft Entry to proto Entry.
    fn entry_to_proto(entry: &openraft::Entry<ConsulTypeConfig>) -> proto::Entry {
        let (payload_type, payload) = match &entry.payload {
            openraft::EntryPayload::Blank => (0u32, vec![]),
            openraft::EntryPayload::Normal(req) => (1, serde_json::to_vec(req).unwrap_or_default()),
            openraft::EntryPayload::Membership(m) => (2, serde_json::to_vec(m).unwrap_or_default()),
        };

        proto::Entry {
            log_id: proto_convert::to_proto_log_id(Some(entry.log_id)),
            payload_type,
            payload,
        }
    }
}

/// Get the gRPC address for Consul Raft communication.
///
/// The ConsulRaftService is registered on the same Raft gRPC port as
/// the Nacos RaftService. Member addresses in the Raft membership
/// already contain the correct Raft port (e.g., 127.0.0.1:7848).
fn raft_grpc_addr(member_addr: &str) -> String {
    member_addr.to_string()
}

/// Default RPC timeout for Consul Raft RPCs.
const DEFAULT_RAFT_RPC_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

impl RaftNetwork<ConsulTypeConfig> for ConsulRaftNetworkConnection {
    async fn append_entries(
        &mut self,
        rpc: AppendEntriesRequest<ConsulTypeConfig>,
        option: RPCOption,
    ) -> Result<AppendEntriesResponse<NodeId>, RPCError<NodeId, BasicNode, RaftError<NodeId>>> {
        let ttl = option.hard_ttl();
        let timeout = if ttl.is_zero() {
            DEFAULT_RAFT_RPC_TIMEOUT
        } else {
            ttl
        };
        let channel = self.get_channel().await.map_err(RPCError::Unreachable)?;
        let mut client = ConsulRaftServiceClient::new(channel);

        let vote_proto = proto_convert::to_proto_vote(&rpc.vote);
        let entries: Vec<proto::Entry> = rpc.entries.iter().map(Self::entry_to_proto).collect();

        let req = proto::AppendEntriesRequest {
            term: rpc.vote.leader_id().term,
            leader_id: rpc.vote.leader_id().node_id,
            prev_log_id: proto_convert::to_proto_log_id(rpc.prev_log_id),
            entries,
            leader_commit: proto_convert::to_proto_log_id(rpc.leader_commit),
            vote: Some(vote_proto),
        };

        let resp = tokio::time::timeout(timeout, client.append_entries(req))
            .await
            .map_err(|_| {
                RPCError::Unreachable(Unreachable::new(&std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "Consul Raft AppendEntries RPC timed out",
                )))
            })?
            .map_err(|e| RPCError::Unreachable(Unreachable::new(&e)))?
            .into_inner();

        if resp.success {
            Ok(AppendEntriesResponse::Success)
        } else {
            Ok(AppendEntriesResponse::Conflict)
        }
    }

    async fn install_snapshot(
        &mut self,
        rpc: InstallSnapshotRequest<ConsulTypeConfig>,
        option: RPCOption,
    ) -> Result<
        InstallSnapshotResponse<NodeId>,
        RPCError<NodeId, BasicNode, RaftError<NodeId, InstallSnapshotError>>,
    > {
        let channel = self.get_channel().await.map_err(RPCError::Unreachable)?;
        let mut client = ConsulRaftServiceClient::new(channel);

        let meta = Some(proto::SnapshotMeta {
            last_log_id: proto_convert::to_proto_log_id(rpc.meta.last_log_id),
            last_membership: None,
            snapshot_id: rpc.meta.snapshot_id.clone(),
        });

        let vote_proto = proto_convert::to_proto_vote(&rpc.vote);
        let data = rpc.data.to_vec();

        // Send snapshot as a single chunk (Consul state is typically small)
        let req = proto::InstallSnapshotRequest {
            term: rpc.vote.leader_id().term,
            leader_id: rpc.vote.leader_id().node_id,
            meta,
            offset: rpc.offset,
            data,
            done: rpc.done,
            vote: Some(vote_proto),
        };

        let resp = client
            .install_snapshot(futures::stream::once(futures::future::ready(req)))
            .await
            .map_err(|e| {
                warn!(
                    "Consul Raft: snapshot transfer to node {} failed: {}",
                    self.target, e
                );
                RPCError::Unreachable(Unreachable::new(&e))
            })?
            .into_inner();

        let vote = proto_convert::from_proto_vote(resp.vote)
            .unwrap_or_else(|| openraft::Vote::new(resp.term, 0));

        Ok(InstallSnapshotResponse { vote })
    }

    async fn vote(
        &mut self,
        rpc: VoteRequest<NodeId>,
        option: RPCOption,
    ) -> Result<VoteResponse<NodeId>, RPCError<NodeId, BasicNode, RaftError<NodeId>>> {
        let ttl = option.hard_ttl();
        let timeout = if ttl.is_zero() {
            DEFAULT_RAFT_RPC_TIMEOUT
        } else {
            ttl
        };
        let channel = self.get_channel().await.map_err(RPCError::Unreachable)?;
        let mut client = ConsulRaftServiceClient::new(channel);

        let vote_proto = proto_convert::to_proto_vote(&rpc.vote);
        let req = proto::VoteRequest {
            term: rpc.vote.leader_id().term,
            candidate_id: rpc.vote.leader_id().node_id,
            last_log_id: proto_convert::to_proto_log_id(rpc.last_log_id),
            vote: Some(vote_proto),
        };

        let resp = tokio::time::timeout(timeout, client.vote(req))
            .await
            .map_err(|_| {
                RPCError::Unreachable(Unreachable::new(&std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "Consul Raft Vote RPC timed out",
                )))
            })?
            .map_err(|e| RPCError::Unreachable(Unreachable::new(&e)))?
            .into_inner();

        let vote = proto_convert::from_proto_vote(resp.vote)
            .unwrap_or_else(|| openraft::Vote::new(resp.term, 0));

        Ok(VoteResponse {
            vote,
            vote_granted: resp.vote_granted,
            last_log_id: proto_convert::from_proto_log_id(resp.last_log_id),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raft_grpc_addr() {
        assert_eq!(raft_grpc_addr("127.0.0.1:7848"), "127.0.0.1:7848");
        assert_eq!(raft_grpc_addr("10.0.0.1:7858"), "10.0.0.1:7858");
    }
}
