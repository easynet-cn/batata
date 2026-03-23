/// Public proto ↔ openraft type conversion utilities.
///
/// Shared between Nacos Raft and Consul Raft gRPC services.
use batata_api::raft::{LogId as ProtoLogId, Vote as ProtoVote};

use super::types::NodeId;

/// Convert proto LogId to openraft LogId.
pub fn from_proto_log_id(log_id: Option<ProtoLogId>) -> Option<openraft::LogId<NodeId>> {
    log_id.map(|l| openraft::LogId::new(openraft::CommittedLeaderId::new(l.term, 0), l.index))
}

/// Convert openraft LogId to proto LogId.
pub fn to_proto_log_id(log_id: Option<openraft::LogId<NodeId>>) -> Option<ProtoLogId> {
    log_id.map(|l| ProtoLogId {
        term: l.leader_id.term,
        index: l.index,
    })
}

/// Convert proto Vote to openraft Vote.
pub fn from_proto_vote(vote: Option<ProtoVote>) -> Option<openraft::Vote<NodeId>> {
    vote.map(|v| {
        if v.committed {
            openraft::Vote::new_committed(v.term, v.leader_id)
        } else {
            openraft::Vote::new(v.term, v.leader_id)
        }
    })
}

/// Convert openraft Vote to proto Vote.
pub fn to_proto_vote(vote: &openraft::Vote<NodeId>) -> ProtoVote {
    ProtoVote {
        leader_id: vote.leader_id().node_id,
        term: vote.leader_id().term,
        committed: vote.is_committed(),
    }
}
