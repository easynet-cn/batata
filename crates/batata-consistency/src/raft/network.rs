// Raft network layer for gRPC-based inter-node communication
// Implements openraft's network traits using tonic gRPC

use std::future::Future;
use std::time::Duration;

use openraft::error::{
    NetworkError, RPCError, RaftError, ReplicationClosed, StreamingError, Unreachable,
};
use openraft::network::{RPCOption, RaftNetwork};
use openraft::raft::{
    AppendEntriesRequest, AppendEntriesResponse, SnapshotResponse, VoteRequest, VoteResponse,
};
use openraft::{BasicNode, Snapshot, Vote};
use tonic::transport::Channel;
use tracing::{debug, error, warn};

use batata_api::raft::{
    AppendEntriesRequest as ProtoAppendEntriesRequest, Entry as ProtoEntry, LogId as ProtoLogId,
    Vote as ProtoVote, VoteRequest as ProtoVoteRequest, raft_service_client::RaftServiceClient,
};

use super::types::{NodeId, TypeConfig};

/// Network timeout configuration for Raft gRPC connections
#[derive(Clone, Debug)]
pub struct RaftNetworkConfig {
    /// Timeout for regular RPCs (AppendEntries, Vote)
    pub rpc_timeout: Duration,
    /// Timeout for snapshot transfer (larger due to data volume)
    pub snapshot_timeout: Duration,
    /// Connection establishment timeout
    pub connect_timeout: Duration,
}

impl Default for RaftNetworkConfig {
    fn default() -> Self {
        Self {
            rpc_timeout: Duration::from_secs(10),
            snapshot_timeout: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(5),
        }
    }
}

/// Factory for creating Raft network connections
pub struct BatataRaftNetworkFactory {
    network_config: RaftNetworkConfig,
}

impl BatataRaftNetworkFactory {
    pub fn new() -> Self {
        Self {
            network_config: RaftNetworkConfig::default(),
        }
    }

    /// Create a factory with custom network timeouts
    pub fn with_config(network_config: RaftNetworkConfig) -> Self {
        Self { network_config }
    }
}

impl Default for BatataRaftNetworkFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl openraft::network::RaftNetworkFactory<TypeConfig> for BatataRaftNetworkFactory {
    type Network = RaftNetworkConnection;

    async fn new_client(&mut self, _target: NodeId, node: &BasicNode) -> Self::Network {
        RaftNetworkConnection::with_config(node.addr.clone(), self.network_config.clone())
    }
}

/// A network connection to a remote Raft node
pub struct RaftNetworkConnection {
    addr: String,
    client: Option<RaftServiceClient<Channel>>,
    network_config: RaftNetworkConfig,
}

impl RaftNetworkConnection {
    pub fn new(addr: String) -> Self {
        Self {
            addr,
            client: None,
            network_config: RaftNetworkConfig::default(),
        }
    }

    /// Create with custom network configuration
    pub fn with_config(addr: String, network_config: RaftNetworkConfig) -> Self {
        Self {
            addr,
            client: None,
            network_config,
        }
    }

    /// Ensure we have a connected client
    async fn ensure_client(&mut self) -> Result<&mut RaftServiceClient<Channel>, NetworkError> {
        if self.client.is_none() {
            let endpoint = format!("http://{}", self.addr);
            debug!("Connecting to Raft node at {}", endpoint);

            let channel = Channel::from_shared(endpoint.clone())
                .map_err(|e| NetworkError::new(&e))?
                .connect_timeout(self.network_config.connect_timeout)
                .timeout(self.network_config.rpc_timeout)
                .connect()
                .await
                .map_err(|e| {
                    warn!("Failed to connect to {}: {}", endpoint, e);
                    NetworkError::new(&e)
                })?;

            self.client = Some(RaftServiceClient::new(channel));
        }

        // Safe: client is guaranteed to be Some after the above block
        self.client.as_mut().ok_or_else(|| {
            NetworkError::new(&std::io::Error::other(
                "Client connection failed unexpectedly",
            ))
        })
    }

    /// Convert openraft LogId to proto LogId
    fn to_proto_log_id(log_id: Option<openraft::LogId<NodeId>>) -> Option<ProtoLogId> {
        log_id.map(|l| ProtoLogId {
            term: l.leader_id.term,
            index: l.index,
        })
    }

    /// Convert proto LogId to openraft LogId
    fn from_proto_log_id(log_id: Option<ProtoLogId>) -> Option<openraft::LogId<NodeId>> {
        log_id.map(|l| openraft::LogId::new(openraft::CommittedLeaderId::new(l.term, 0), l.index))
    }

    /// Convert openraft Vote to proto Vote
    fn to_proto_vote(vote: &openraft::Vote<NodeId>) -> ProtoVote {
        ProtoVote {
            leader_id: vote.leader_id().node_id,
            term: vote.leader_id().term,
            committed: vote.is_committed(),
        }
    }

    /// Convert proto Vote to openraft Vote
    fn from_proto_vote(vote: Option<ProtoVote>) -> Option<openraft::Vote<NodeId>> {
        vote.map(|v| {
            let leader_id = openraft::CommittedLeaderId::new(v.term, v.leader_id);
            if v.committed {
                openraft::Vote::new_committed(leader_id.term, leader_id.node_id)
            } else {
                openraft::Vote::new(leader_id.term, leader_id.node_id)
            }
        })
    }

    /// Convert openraft Entry to proto Entry
    ///
    /// Serializes just the inner payload data (not the EntryPayload enum wrapper),
    /// matching the deserialization in grpc_service.rs which deserializes by payload_type.
    fn to_proto_entry(entry: &openraft::Entry<TypeConfig>) -> ProtoEntry {
        let (payload_type, payload) = match &entry.payload {
            openraft::EntryPayload::Blank => (0u32, Vec::new()),
            openraft::EntryPayload::Normal(req) => {
                let data = serde_json::to_vec(req).unwrap_or_else(|e| {
                    error!(
                        error = %e,
                        log_id = ?entry.log_id,
                        "Failed to serialize Raft Normal entry payload"
                    );
                    Vec::new()
                });
                (1, data)
            }
            openraft::EntryPayload::Membership(membership) => {
                let data = serde_json::to_vec(membership).unwrap_or_else(|e| {
                    error!(
                        error = %e,
                        log_id = ?entry.log_id,
                        "Failed to serialize Raft Membership entry payload"
                    );
                    Vec::new()
                });
                (2, data)
            }
        };

        ProtoEntry {
            log_id: Self::to_proto_log_id(Some(entry.log_id)),
            payload_type,
            payload,
        }
    }
}

impl RaftNetwork<TypeConfig> for RaftNetworkConnection {
    async fn append_entries(
        &mut self,
        req: AppendEntriesRequest<TypeConfig>,
        _option: RPCOption,
    ) -> Result<AppendEntriesResponse<NodeId>, RPCError<NodeId, BasicNode, RaftError<NodeId>>> {
        let client = self
            .ensure_client()
            .await
            .map_err(|e| RPCError::Unreachable(Unreachable::new(&e)))?;

        let proto_req = ProtoAppendEntriesRequest {
            term: req.vote.leader_id().term,
            leader_id: req.vote.leader_id().node_id,
            prev_log_id: Self::to_proto_log_id(req.prev_log_id),
            entries: req.entries.iter().map(Self::to_proto_entry).collect(),
            leader_commit: Self::to_proto_log_id(req.leader_commit),
            vote: Some(Self::to_proto_vote(&req.vote)),
        };

        let response = client
            .append_entries(proto_req)
            .await
            .map_err(|e| {
                error!("AppendEntries RPC failed: {}", e);
                RPCError::Unreachable(Unreachable::new(&e))
            })?
            .into_inner();

        // Parse response - AppendEntriesResponse is an enum in openraft 0.9
        if response.success {
            Ok(AppendEntriesResponse::Success)
        } else if response.conflict.is_some() {
            Ok(AppendEntriesResponse::Conflict)
        } else {
            // Higher vote or conflict - default to conflict
            Ok(AppendEntriesResponse::Conflict)
        }
    }

    async fn install_snapshot(
        &mut self,
        rpc: openraft::raft::InstallSnapshotRequest<TypeConfig>,
        _option: RPCOption,
    ) -> Result<
        openraft::raft::InstallSnapshotResponse<NodeId>,
        RPCError<NodeId, BasicNode, RaftError<NodeId, openraft::error::InstallSnapshotError>>,
    > {
        // For incremental snapshot installation, acknowledge receipt
        // The actual data transfer happens via full_snapshot
        debug!("Install snapshot request received, offset: {}", rpc.offset);
        Ok(openraft::raft::InstallSnapshotResponse { vote: rpc.vote })
    }

    async fn full_snapshot(
        &mut self,
        vote: Vote<NodeId>,
        snapshot: Snapshot<TypeConfig>,
        cancel: impl Future<Output = ReplicationClosed> + Send + 'static,
        _option: RPCOption,
    ) -> Result<SnapshotResponse<NodeId>, StreamingError<TypeConfig, openraft::error::Fatal<NodeId>>>
    {
        use futures::stream;
        use std::io::Read;
        use tokio::select;

        let client = match self.ensure_client().await {
            Ok(c) => c,
            Err(e) => return Err(StreamingError::Unreachable(Unreachable::new(&e))),
        };

        // Read snapshot data
        let mut snapshot_box = snapshot.snapshot;
        let mut data = Vec::new();
        if let Err(e) = snapshot_box.read_to_end(&mut data) {
            return Err(StreamingError::Unreachable(Unreachable::new(&e)));
        }

        // Create snapshot metadata
        let meta = Some(batata_api::raft::SnapshotMeta {
            last_log_id: Self::to_proto_log_id(snapshot.meta.last_log_id),
            last_membership: None, // Membership is serialized separately in the snapshot data
            snapshot_id: snapshot.meta.snapshot_id.clone(),
        });

        // Split data into chunks and create streaming requests
        const CHUNK_SIZE: usize = 1024 * 1024; // 1MB chunks
        let chunks: Vec<_> = data.chunks(CHUNK_SIZE).collect();
        let total_chunks = chunks.len();

        let requests: Vec<batata_api::raft::InstallSnapshotRequest> = chunks
            .into_iter()
            .enumerate()
            .map(|(i, chunk)| batata_api::raft::InstallSnapshotRequest {
                term: vote.leader_id().term,
                leader_id: vote.leader_id().node_id,
                meta: if i == 0 { meta.clone() } else { None },
                offset: (i * CHUNK_SIZE) as u64,
                data: chunk.to_vec(),
                done: i == total_chunks - 1,
                vote: Some(Self::to_proto_vote(&vote)),
            })
            .collect();

        // Send snapshot stream with cancellation support
        let response = select! {
            result = client.install_snapshot(stream::iter(requests)) => {
                match result {
                    Ok(resp) => resp.into_inner(),
                    Err(e) => {
                        error!("Full snapshot transfer failed: {}", e);
                        return Err(StreamingError::Unreachable(Unreachable::new(&e)));
                    }
                }
            }
            _ = cancel => {
                warn!("Snapshot transfer cancelled");
                return Err(StreamingError::Unreachable(Unreachable::new(
                    &std::io::Error::new(std::io::ErrorKind::Interrupted, "cancelled")
                )));
            }
        };

        Ok(SnapshotResponse {
            vote: Self::from_proto_vote(response.vote).unwrap_or(vote),
        })
    }

    async fn vote(
        &mut self,
        req: VoteRequest<NodeId>,
        _option: RPCOption,
    ) -> Result<VoteResponse<NodeId>, RPCError<NodeId, BasicNode, RaftError<NodeId>>> {
        let client = self
            .ensure_client()
            .await
            .map_err(|e| RPCError::Unreachable(Unreachable::new(&e)))?;

        let proto_req = ProtoVoteRequest {
            term: req.vote.leader_id().term,
            candidate_id: req.vote.leader_id().node_id,
            last_log_id: Self::to_proto_log_id(req.last_log_id),
            vote: Some(Self::to_proto_vote(&req.vote)),
        };

        let response = client
            .vote(proto_req)
            .await
            .map_err(|e| {
                error!("Vote RPC failed: {}", e);
                RPCError::Unreachable(Unreachable::new(&e))
            })?
            .into_inner();

        Ok(VoteResponse {
            vote: Self::from_proto_vote(response.vote).unwrap_or(req.vote),
            vote_granted: response.vote_granted,
            last_log_id: Self::from_proto_log_id(response.last_log_id),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proto_log_id_conversion() {
        let log_id = openraft::LogId::new(openraft::CommittedLeaderId::new(5, 100), 42);
        let proto = RaftNetworkConnection::to_proto_log_id(Some(log_id));
        assert!(proto.is_some());
        let proto = proto.unwrap();
        assert_eq!(proto.term, 5);
        assert_eq!(proto.index, 42);
    }

    #[test]
    fn test_proto_vote_conversion() {
        let vote = openraft::Vote::new(3, 100);
        let proto = RaftNetworkConnection::to_proto_vote(&vote);
        assert_eq!(proto.term, 3);
        assert_eq!(proto.leader_id, 100);
        assert!(!proto.committed);
    }
}
