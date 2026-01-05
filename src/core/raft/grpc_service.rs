// Raft gRPC service implementation
// Handles incoming Raft RPC requests from other cluster nodes

use std::sync::Arc;

use tokio::sync::RwLock;
use tonic::{Request, Response, Status, Streaming};
use tracing::{debug, error, info};

use crate::api::raft::{
    AddLearnerRequest, AddLearnerResponse, AppendEntriesRequest as ProtoAppendEntriesRequest,
    AppendEntriesResponse as ProtoAppendEntriesResponse, ChangeMembershipRequest,
    ChangeMembershipResponse, ClientWriteRequest, ClientWriteResponse, Entry as ProtoEntry,
    GetMetricsRequest, GetMetricsResponse, InstallSnapshotRequest as ProtoInstallSnapshotRequest,
    InstallSnapshotResponse as ProtoInstallSnapshotResponse, LogId as ProtoLogId,
    RaftNodeInfo as ProtoRaftNodeInfo, Vote as ProtoVote, VoteRequest as ProtoVoteRequest,
    VoteResponse as ProtoVoteResponse, raft_management_service_server::RaftManagementService,
    raft_service_server::RaftService,
};

use super::node::RaftNode;
use super::request::RaftRequest;
use super::types::{NodeId, TypeConfig};

/// gRPC service for Raft consensus protocol
pub struct RaftGrpcService {
    raft: Arc<RwLock<Option<Arc<RaftNode>>>>,
}

impl RaftGrpcService {
    pub fn new(raft: Arc<RwLock<Option<Arc<RaftNode>>>>) -> Self {
        Self { raft }
    }

    /// Get the raft node, returning error if not initialized
    async fn get_raft(&self) -> Result<Arc<RaftNode>, Status> {
        self.raft
            .read()
            .await
            .clone()
            .ok_or_else(|| Status::unavailable("Raft node not initialized"))
    }

    /// Convert proto LogId to openraft LogId
    fn from_proto_log_id(log_id: Option<ProtoLogId>) -> Option<openraft::LogId<NodeId>> {
        log_id.map(|l| openraft::LogId::new(openraft::CommittedLeaderId::new(l.term, 0), l.index))
    }

    /// Convert openraft LogId to proto LogId
    fn to_proto_log_id(log_id: Option<openraft::LogId<NodeId>>) -> Option<ProtoLogId> {
        log_id.map(|l| ProtoLogId {
            term: l.leader_id.term,
            index: l.index,
        })
    }

    /// Convert proto Vote to openraft Vote
    fn from_proto_vote(vote: Option<ProtoVote>) -> Option<openraft::Vote<NodeId>> {
        vote.map(|v| {
            if v.committed {
                openraft::Vote::new_committed(v.term, v.leader_id)
            } else {
                openraft::Vote::new(v.term, v.leader_id)
            }
        })
    }

    /// Convert openraft Vote to proto Vote
    fn to_proto_vote(vote: &openraft::Vote<NodeId>) -> ProtoVote {
        ProtoVote {
            leader_id: vote.leader_id().node_id,
            term: vote.leader_id().term,
            committed: vote.is_committed(),
        }
    }

    /// Convert proto Entry to openraft Entry
    fn from_proto_entry(
        entry: ProtoEntry,
    ) -> Result<openraft::Entry<TypeConfig>, serde_json::Error> {
        let log_id = Self::from_proto_log_id(entry.log_id)
            .unwrap_or_else(|| openraft::LogId::new(openraft::CommittedLeaderId::new(0, 0), 0));

        let payload = match entry.payload_type {
            0 => openraft::EntryPayload::Blank,
            1 => {
                let req: RaftRequest = serde_json::from_slice(&entry.payload)?;
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
impl RaftService for RaftGrpcService {
    async fn append_entries(
        &self,
        request: Request<ProtoAppendEntriesRequest>,
    ) -> Result<Response<ProtoAppendEntriesResponse>, Status> {
        let raft_node = self.get_raft().await?;
        let req = request.into_inner();

        debug!(
            "Received AppendEntries: term={}, leader_id={}, entries={}",
            req.term,
            req.leader_id,
            req.entries.len()
        );

        // Convert proto entries to openraft entries
        let entries: Result<Vec<_>, _> = req
            .entries
            .into_iter()
            .map(Self::from_proto_entry)
            .collect();

        let entries = entries.map_err(|e| {
            error!("Failed to deserialize entries: {}", e);
            Status::invalid_argument(format!("Invalid entry format: {}", e))
        })?;

        // Construct openraft request
        let vote = Self::from_proto_vote(req.vote)
            .unwrap_or_else(|| openraft::Vote::new(req.term, req.leader_id));

        let append_req = openraft::raft::AppendEntriesRequest {
            vote,
            prev_log_id: Self::from_proto_log_id(req.prev_log_id),
            entries,
            leader_commit: Self::from_proto_log_id(req.leader_commit),
        };

        // Call raft node
        let response = raft_node
            .raft()
            .append_entries(append_req)
            .await
            .map_err(|e| {
                error!("AppendEntries failed: {}", e);
                Status::internal(format!("AppendEntries failed: {}", e))
            })?;

        // AppendEntriesResponse is an enum in openraft 0.9
        let (success, conflict) = match response {
            openraft::raft::AppendEntriesResponse::Success => (true, None),
            openraft::raft::AppendEntriesResponse::PartialSuccess(log_id) => {
                let match_log = log_id.map(|l| ProtoLogId {
                    term: l.leader_id.term,
                    index: l.index,
                });
                (true, match_log)
            }
            openraft::raft::AppendEntriesResponse::Conflict => {
                (false, Some(ProtoLogId { term: 0, index: 0 }))
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
        let raft_node = self.get_raft().await?;
        let req = request.into_inner();

        debug!(
            "Received Vote request: term={}, candidate_id={}",
            req.term, req.candidate_id
        );

        let vote = Self::from_proto_vote(req.vote)
            .unwrap_or_else(|| openraft::Vote::new(req.term, req.candidate_id));

        let vote_req = openraft::raft::VoteRequest {
            vote,
            last_log_id: Self::from_proto_log_id(req.last_log_id),
        };

        let response = raft_node.raft().vote(vote_req).await.map_err(|e| {
            error!("Vote failed: {}", e);
            Status::internal(format!("Vote failed: {}", e))
        })?;

        Ok(Response::new(ProtoVoteResponse {
            term: response.vote.leader_id().term,
            vote_granted: response.vote_granted,
            vote: Some(Self::to_proto_vote(&response.vote)),
            last_log_id: Self::to_proto_log_id(response.last_log_id),
        }))
    }

    async fn install_snapshot(
        &self,
        request: Request<Streaming<ProtoInstallSnapshotRequest>>,
    ) -> Result<Response<ProtoInstallSnapshotResponse>, Status> {
        use futures::StreamExt;

        let raft_node = self.get_raft().await?;
        let mut stream = request.into_inner();

        // Collect all chunks from the stream
        let mut total_bytes: usize = 0;
        let mut chunk_count: usize = 0;
        let mut snapshot_id = String::new();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.map_err(|e| {
                error!("Error receiving snapshot chunk: {}", e);
                Status::internal(format!("Failed to receive snapshot chunk: {}", e))
            })?;

            // Extract metadata from first chunk
            if let Some(ref meta) = chunk.meta {
                snapshot_id = meta.snapshot_id.clone();
            }

            total_bytes += chunk.data.len();
            chunk_count += 1;

            if chunk.done {
                break;
            }
        }

        // Note: In this implementation, snapshot data is transferred via the full_snapshot
        // method in RaftNetworkV2. This gRPC endpoint receives the data but the actual
        // state machine update is handled by openraft's internal mechanisms when the
        // snapshot is fully received.
        //
        // The full_snapshot() method in network.rs sends the data, and openraft
        // coordinates the state machine update through its storage traits.

        info!(
            "Snapshot stream received: id={}, chunks={}, bytes={}",
            snapshot_id, chunk_count, total_bytes
        );

        // Return response with current vote
        let metrics = raft_node.metrics();
        let vote = metrics.vote;

        Ok(Response::new(ProtoInstallSnapshotResponse {
            term: vote.leader_id().term,
            vote: Some(Self::to_proto_vote(&vote)),
        }))
    }
}

/// gRPC service for Raft cluster management
pub struct RaftManagementGrpcService {
    raft: Arc<RwLock<Option<Arc<RaftNode>>>>,
}

impl RaftManagementGrpcService {
    pub fn new(raft: Arc<RwLock<Option<Arc<RaftNode>>>>) -> Self {
        Self { raft }
    }

    async fn get_raft(&self) -> Result<Arc<RaftNode>, Status> {
        self.raft
            .read()
            .await
            .clone()
            .ok_or_else(|| Status::unavailable("Raft node not initialized"))
    }
}

#[tonic::async_trait]
impl RaftManagementService for RaftManagementGrpcService {
    async fn write(
        &self,
        request: Request<ClientWriteRequest>,
    ) -> Result<Response<ClientWriteResponse>, Status> {
        let raft_node = self.get_raft().await?;
        let req = request.into_inner();

        // Deserialize the request
        let raft_request: RaftRequest = serde_json::from_slice(&req.data).map_err(|e| {
            error!("Failed to deserialize write request: {}", e);
            Status::invalid_argument(format!("Invalid request format: {}", e))
        })?;

        debug!("Received write request: {}", raft_request.op_type());

        // Forward to raft
        match raft_node.write(raft_request).await {
            Ok(response) => Ok(Response::new(ClientWriteResponse {
                success: response.success,
                data: response.data.unwrap_or_default(),
                message: response.message.unwrap_or_default(),
                leader_id: None,
                leader_addr: None,
            })),
            Err(e) => {
                // Check if we need to redirect to leader
                if let Some(leader_id) = raft_node.leader_id() {
                    Ok(Response::new(ClientWriteResponse {
                        success: false,
                        data: Vec::new(),
                        message: format!("Not leader, redirect to leader {}", leader_id),
                        leader_id: Some(leader_id),
                        leader_addr: None, // Leader address not available in metrics
                    }))
                } else {
                    Err(Status::unavailable(format!("Write failed: {}", e)))
                }
            }
        }
    }

    async fn add_learner(
        &self,
        request: Request<AddLearnerRequest>,
    ) -> Result<Response<AddLearnerResponse>, Status> {
        let raft_node = self.get_raft().await?;
        let req = request.into_inner();

        info!("Adding learner: node_id={}, addr={}", req.node_id, req.addr);

        match raft_node.add_learner(req.node_id, req.addr.clone()).await {
            Ok(_) => Ok(Response::new(AddLearnerResponse {
                success: true,
                message: format!("Learner {} added successfully", req.node_id),
            })),
            Err(e) => {
                error!("Failed to add learner: {}", e);
                Ok(Response::new(AddLearnerResponse {
                    success: false,
                    message: format!("Failed to add learner: {}", e),
                }))
            }
        }
    }

    async fn change_membership(
        &self,
        request: Request<ChangeMembershipRequest>,
    ) -> Result<Response<ChangeMembershipResponse>, Status> {
        let raft_node = self.get_raft().await?;
        let req = request.into_inner();

        info!("Changing membership to: {:?}", req.members);

        let members: std::collections::BTreeSet<_> = req.members.into_iter().collect();

        match raft_node.change_membership(members).await {
            Ok(_) => Ok(Response::new(ChangeMembershipResponse {
                success: true,
                message: "Membership changed successfully".to_string(),
            })),
            Err(e) => {
                error!("Failed to change membership: {}", e);
                Ok(Response::new(ChangeMembershipResponse {
                    success: false,
                    message: format!("Failed to change membership: {}", e),
                }))
            }
        }
    }

    async fn get_metrics(
        &self,
        _request: Request<GetMetricsRequest>,
    ) -> Result<Response<GetMetricsResponse>, Status> {
        let raft_node = self.get_raft().await?;

        let metrics = raft_node.metrics();

        let state = match metrics.state {
            openraft::ServerState::Leader => "leader",
            openraft::ServerState::Follower => "follower",
            openraft::ServerState::Candidate => "candidate",
            openraft::ServerState::Learner => "learner",
            openraft::ServerState::Shutdown => "shutdown",
        };

        let members: Vec<ProtoRaftNodeInfo> = metrics
            .membership_config
            .membership()
            .voter_ids()
            .map(|id| ProtoRaftNodeInfo {
                node_id: id,
                addr: String::new(), // Address not directly available from membership
            })
            .collect();

        Ok(Response::new(GetMetricsResponse {
            node_id: metrics.id,
            state: state.to_string(),
            current_term: metrics.current_term,
            leader_id: metrics.current_leader,
            last_log_index: metrics.last_log_index.unwrap_or(0),
            commit_index: metrics.last_applied.map(|l| l.index).unwrap_or(0), // Using last_applied as commit proxy
            applied_index: metrics.last_applied.map(|l| l.index).unwrap_or(0),
            members,
        }))
    }
}
