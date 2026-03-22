/// Consul Raft gRPC service implementation.
///
/// Handles incoming Raft RPCs from other cluster nodes for the Consul Raft group.
/// Registered on the cluster gRPC port (9849) alongside the Nacos Raft service.
use std::sync::Arc;

use tokio::sync::RwLock;
use tonic::{Request, Response, Status, Streaming};
use tracing::{debug, error};

use batata_api::raft::{
    AppendEntriesRequest as ProtoAppendEntriesRequest,
    AppendEntriesResponse as ProtoAppendEntriesResponse,
    Entry as ProtoEntry,
    InstallSnapshotRequest as ProtoInstallSnapshotRequest,
    InstallSnapshotResponse as ProtoInstallSnapshotResponse,
    VoteRequest as ProtoVoteRequest, VoteResponse as ProtoVoteResponse,
    consul_raft_service_server::ConsulRaftService,
};
use batata_consistency::raft::proto_convert;

use super::node::ConsulRaftNode;
use super::request::ConsulRaftRequest;
use super::types::*;

/// gRPC service for Consul Raft consensus protocol.
pub struct ConsulRaftGrpcService {
    raft_node: Arc<RwLock<Option<Arc<ConsulRaftNode>>>>,
}

impl ConsulRaftGrpcService {
    pub fn new() -> Self {
        Self {
            raft_node: Arc::new(RwLock::new(None)),
        }
    }

    /// Create a clone that shares the same inner state.
    pub fn clone_service(&self) -> Self {
        Self {
            raft_node: self.raft_node.clone(),
        }
    }

    /// Create a management service sharing the same raft_node holder.
    pub fn management_service(&self) -> ConsulRaftManagementGrpcService {
        ConsulRaftManagementGrpcService::new(self.raft_node.clone())
    }

    pub async fn set_raft_node(&self, node: Arc<ConsulRaftNode>) {
        *self.raft_node.write().await = Some(node);
    }

    async fn get_raft(&self) -> Result<Arc<ConsulRaftNode>, Status> {
        self.raft_node
            .read()
            .await
            .clone()
            .ok_or_else(|| Status::unavailable("Consul Raft node not initialized"))
    }

    /// Convert proto Entry to Consul openraft Entry
    fn from_proto_entry(
        entry: ProtoEntry,
    ) -> Result<ConsulEntry, serde_json::Error> {
        let log_id = proto_convert::from_proto_log_id(entry.log_id)
            .unwrap_or_else(|| openraft::LogId::new(openraft::CommittedLeaderId::new(0, 0), 0));

        let payload = match entry.payload_type {
            0 => openraft::EntryPayload::Blank,
            1 => {
                let req: ConsulRaftRequest = serde_json::from_slice(&entry.payload)?;
                openraft::EntryPayload::Normal(req)
            }
            2 => {
                let membership: openraft::Membership<NodeId, openraft::BasicNode> =
                    serde_json::from_slice(&entry.payload)?;
                openraft::EntryPayload::Membership(membership)
            }
            _ => openraft::EntryPayload::Blank,
        };

        Ok(openraft::Entry { log_id, payload })
    }
}

#[tonic::async_trait]
impl ConsulRaftService for ConsulRaftGrpcService {
    async fn append_entries(
        &self,
        request: Request<ProtoAppendEntriesRequest>,
    ) -> Result<Response<ProtoAppendEntriesResponse>, Status> {
        let raft = self.get_raft().await?;
        let req = request.into_inner();

        debug!(
            "Consul Raft: AppendEntries term={}, leader={}, entries={}",
            req.term, req.leader_id, req.entries.len()
        );

        let entries: Result<Vec<_>, _> = req
            .entries
            .into_iter()
            .map(Self::from_proto_entry)
            .collect();

        let entries = entries.map_err(|e| {
            error!("Failed to deserialize Consul entries: {}", e);
            Status::invalid_argument(format!("Invalid entry: {}", e))
        })?;

        let vote = proto_convert::from_proto_vote(req.vote)
            .unwrap_or_else(|| openraft::Vote::new(req.term, req.leader_id));

        let append_req = openraft::raft::AppendEntriesRequest {
            vote,
            prev_log_id: proto_convert::from_proto_log_id(req.prev_log_id),
            entries,
            leader_commit: proto_convert::from_proto_log_id(req.leader_commit),
        };

        let response = raft.raft().append_entries(append_req).await.map_err(|e| {
            error!("Consul Raft AppendEntries failed: {}", e);
            Status::internal(e.to_string())
        })?;

        let (success, conflict) = match response {
            openraft::raft::AppendEntriesResponse::Success => (true, None),
            openraft::raft::AppendEntriesResponse::PartialSuccess(log_id) => {
                let match_log = log_id.map(|l| batata_api::raft::LogId {
                    term: l.leader_id.term,
                    index: l.index,
                });
                (true, match_log)
            }
            openraft::raft::AppendEntriesResponse::Conflict => {
                (false, Some(batata_api::raft::LogId { term: 0, index: 0 }))
            }
            openraft::raft::AppendEntriesResponse::HigherVote(_) => (false, None),
        };

        Ok(Response::new(ProtoAppendEntriesResponse {
            term: vote.leader_id().term,
            success,
            match_log_id: None,
            conflict,
        }))
    }

    async fn vote(
        &self,
        request: Request<ProtoVoteRequest>,
    ) -> Result<Response<ProtoVoteResponse>, Status> {
        let raft = self.get_raft().await?;
        let req = request.into_inner();

        debug!("Consul Raft: Vote term={}, candidate={}", req.term, req.candidate_id);

        let vote = proto_convert::from_proto_vote(req.vote)
            .unwrap_or_else(|| openraft::Vote::new(req.term, req.candidate_id));

        let vote_req = openraft::raft::VoteRequest {
            vote,
            last_log_id: proto_convert::from_proto_log_id(req.last_log_id),
        };

        let response = raft.raft().vote(vote_req).await.map_err(|e| {
            error!("Consul Raft Vote failed: {}", e);
            Status::internal(e.to_string())
        })?;

        Ok(Response::new(ProtoVoteResponse {
            term: response.vote.leader_id().term,
            vote_granted: response.vote_granted,
            vote: Some(proto_convert::to_proto_vote(&response.vote)),
            last_log_id: proto_convert::to_proto_log_id(response.last_log_id),
        }))
    }

    async fn install_snapshot(
        &self,
        _request: Request<Streaming<ProtoInstallSnapshotRequest>>,
    ) -> Result<Response<ProtoInstallSnapshotResponse>, Status> {
        Err(Status::unimplemented("Consul Raft snapshot not yet implemented"))
    }
}

// ============================================================================
// Management service — handles forwarded writes from non-leader nodes
// ============================================================================

use batata_api::raft::{
    ClientWriteRequest, ClientWriteResponse,
    consul_raft_management_service_server::ConsulRaftManagementService,
    AddLearnerRequest, AddLearnerResponse,
    ChangeMembershipRequest, ChangeMembershipResponse,
    GetMetricsRequest, GetMetricsResponse,
};

/// gRPC management service for Consul Raft — handles forwarded client writes.
pub struct ConsulRaftManagementGrpcService {
    raft_node: Arc<RwLock<Option<Arc<ConsulRaftNode>>>>,
}

impl ConsulRaftManagementGrpcService {
    pub fn new(raft_node: Arc<RwLock<Option<Arc<ConsulRaftNode>>>>) -> Self {
        Self { raft_node }
    }

    async fn get_raft(&self) -> Result<Arc<ConsulRaftNode>, Status> {
        self.raft_node
            .read()
            .await
            .clone()
            .ok_or_else(|| Status::unavailable("Consul Raft node not initialized"))
    }
}

#[tonic::async_trait]
impl ConsulRaftManagementService for ConsulRaftManagementGrpcService {
    async fn write(
        &self,
        request: Request<ClientWriteRequest>,
    ) -> Result<Response<ClientWriteResponse>, Status> {
        let raft = self.get_raft().await?;
        let data = request.into_inner().data;

        let consul_req: ConsulRaftRequest = serde_json::from_slice(&data)
            .map_err(|e| Status::invalid_argument(format!("Invalid request: {}", e)))?;

        match raft.write_with_index(consul_req).await {
            Ok((resp, _log_index)) => {
                let resp_data = serde_json::to_vec(&resp).unwrap_or_default();
                Ok(Response::new(ClientWriteResponse {
                    success: resp.success,
                    data: resp_data,
                    message: resp.message.unwrap_or_default(),
                    leader_id: None,
                    leader_addr: None,
                }))
            }
            Err(e) => Ok(Response::new(ClientWriteResponse {
                success: false,
                data: vec![],
                message: e.to_string(),
                leader_id: None,
                leader_addr: None,
            })),
        }
    }

    async fn add_learner(
        &self,
        _request: Request<AddLearnerRequest>,
    ) -> Result<Response<AddLearnerResponse>, Status> {
        Err(Status::unimplemented("Not yet implemented"))
    }

    async fn change_membership(
        &self,
        _request: Request<ChangeMembershipRequest>,
    ) -> Result<Response<ChangeMembershipResponse>, Status> {
        Err(Status::unimplemented("Not yet implemented"))
    }

    async fn get_metrics(
        &self,
        _request: Request<GetMetricsRequest>,
    ) -> Result<Response<GetMetricsResponse>, Status> {
        Err(Status::unimplemented("Not yet implemented"))
    }
}
