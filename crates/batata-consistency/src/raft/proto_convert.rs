//! Public proto <-> openraft type conversion utilities.
//!
//! Shared between Batata Raft and Consul Raft gRPC services.
use std::collections::{BTreeMap, BTreeSet};

use batata_server_api::raft::{
    LogId as ProtoLogId, Membership as ProtoMembership, RaftNodeInfo,
    SnapshotMeta as ProtoSnapshotMeta, Vote as ProtoVote,
};
use openraft::{BasicNode, LogId, Membership, StoredMembership};

use super::types::NodeId;

/// Convert proto LogId to openraft LogId.
pub fn from_proto_log_id(log_id: Option<ProtoLogId>) -> Option<LogId<NodeId>> {
    log_id.map(|l| LogId::new(openraft::CommittedLeaderId::new(l.term, 0), l.index))
}

/// Convert openraft LogId to proto LogId.
pub fn to_proto_log_id(log_id: Option<LogId<NodeId>>) -> Option<ProtoLogId> {
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

/// Resolve the address of a node, falling back to an empty string when the
/// membership carries voter ids without node metadata.
fn node_addr(membership: &Membership<NodeId, BasicNode>, node_id: NodeId) -> String {
    membership
        .get_node(&node_id)
        .map(|n| n.addr.clone())
        .unwrap_or_default()
}

/// Convert openraft `StoredMembership` to the wire `Membership` message.
///
/// Mirrors JRaft's `LocalSnapshotMeta`: the wire form carries only the voter /
/// learner id sets plus their addresses. The membership log id is deliberately
/// **not** transmitted -- the receiving side rebuilds it from the snapshot's
/// `last_log_id`, which is exactly what JRaft does with `lastIncludedIndex`.
///
/// Returns `None` for an empty membership so the field stays unset on the wire.
pub fn to_proto_membership(
    membership: &StoredMembership<NodeId, BasicNode>,
) -> Option<ProtoMembership> {
    let inner = membership.membership();

    let voters: Vec<RaftNodeInfo> = inner
        .voter_ids()
        .map(|id| RaftNodeInfo {
            node_id: id,
            addr: node_addr(inner, id),
        })
        .collect();
    let learners: Vec<RaftNodeInfo> = inner
        .learner_ids()
        .map(|id| RaftNodeInfo {
            node_id: id,
            addr: node_addr(inner, id),
        })
        .collect();

    if voters.is_empty() && learners.is_empty() {
        return None;
    }

    Some(ProtoMembership { voters, learners })
}

/// Convert the wire `Membership` message back into an openraft `StoredMembership`.
///
/// Node ids present in `nodes` but absent from the voter set are learners,
/// matching openraft's own `Membership` invariant.
pub fn from_proto_membership(
    membership: Option<ProtoMembership>,
    log_id: Option<LogId<NodeId>>,
) -> StoredMembership<NodeId, BasicNode> {
    let mut voters = BTreeSet::new();
    let mut nodes: BTreeMap<NodeId, BasicNode> = BTreeMap::new();

    if let Some(m) = membership {
        for n in m.voters {
            voters.insert(n.node_id);
            nodes.insert(n.node_id, BasicNode { addr: n.addr });
        }
        for n in m.learners {
            nodes.insert(n.node_id, BasicNode { addr: n.addr });
        }
    }

    StoredMembership::new(log_id, Membership::new(vec![voters], nodes))
}

/// Convert openraft `SnapshotMeta` to the wire `SnapshotMeta` message.
pub fn to_proto_snapshot_meta(
    meta: &openraft::SnapshotMeta<NodeId, BasicNode>,
) -> ProtoSnapshotMeta {
    ProtoSnapshotMeta {
        last_log_id: to_proto_log_id(meta.last_log_id),
        last_membership: to_proto_membership(&meta.last_membership),
        snapshot_id: meta.snapshot_id.clone(),
    }
}

/// Convert the wire `SnapshotMeta` message back into openraft's `SnapshotMeta`.
///
/// Returns `None` when the sender omitted metadata entirely, which the caller
/// treats as "cannot install this snapshot" rather than installing an empty one.
pub fn from_proto_snapshot_meta(
    meta: Option<ProtoSnapshotMeta>,
) -> Option<openraft::SnapshotMeta<NodeId, BasicNode>> {
    let meta = meta?;
    let last_log_id = from_proto_log_id(meta.last_log_id);

    Some(openraft::SnapshotMeta {
        last_log_id,
        last_membership: from_proto_membership(meta.last_membership, last_log_id),
        snapshot_id: meta.snapshot_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn membership_with(
        voters: &[NodeId],
        learners: &[NodeId],
    ) -> StoredMembership<NodeId, BasicNode> {
        let mut nodes = BTreeMap::new();
        for id in voters.iter().chain(learners.iter()) {
            nodes.insert(
                *id,
                BasicNode {
                    addr: format!("127.0.0.1:{}", 9000 + id),
                },
            );
        }
        let voters = voters.iter().copied().collect::<BTreeSet<_>>();
        StoredMembership::new(
            Some(LogId::new(openraft::CommittedLeaderId::new(3, 0), 7)),
            Membership::new(vec![voters], nodes),
        )
    }

    #[test]
    fn test_membership_round_trip_preserves_voters_and_learners() {
        let original = membership_with(&[1, 2, 3], &[4]);
        let proto = to_proto_membership(&original).expect("non-empty membership");

        assert_eq!(proto.voters.len(), 3);
        assert_eq!(proto.learners.len(), 1);
        assert_eq!(proto.voters[0].addr, "127.0.0.1:9001");
        assert_eq!(proto.learners[0].addr, "127.0.0.1:9004");

        let restored = from_proto_membership(Some(proto), *original.log_id());
        let restored_voters: Vec<NodeId> = restored.membership().voter_ids().collect();
        let restored_learners: Vec<NodeId> = restored.membership().learner_ids().collect();

        assert_eq!(restored_voters, vec![1, 2, 3]);
        assert_eq!(restored_learners, vec![4]);
    }

    #[test]
    fn test_membership_round_trip_keeps_addresses() {
        let original = membership_with(&[1, 2, 3], &[4]);
        let proto = to_proto_membership(&original).expect("non-empty membership");
        let restored = from_proto_membership(Some(proto), *original.log_id());

        let addr: Vec<String> = restored
            .membership()
            .nodes()
            .map(|(_, n)| n.addr.clone())
            .collect();
        assert_eq!(
            addr,
            vec![
                "127.0.0.1:9001".to_string(),
                "127.0.0.1:9002".to_string(),
                "127.0.0.1:9003".to_string(),
                "127.0.0.1:9004".to_string(),
            ]
        );
    }

    #[test]
    fn test_empty_membership_is_not_sent() {
        let empty = StoredMembership::default();
        assert!(to_proto_membership(&empty).is_none());
    }

    #[test]
    fn test_snapshot_meta_round_trip() {
        let membership = membership_with(&[1, 2, 3], &[]);
        let original = openraft::SnapshotMeta {
            last_log_id: Some(LogId::new(openraft::CommittedLeaderId::new(5, 0), 42)),
            last_membership: membership,
            snapshot_id: "snapshot-42-1".to_string(),
        };

        let proto = to_proto_snapshot_meta(&original);
        // The wire form carries the membership; the log id rides along as
        // `last_log_id`, exactly like JRaft's `lastIncludedIndex/Term`.
        assert!(proto.last_membership.is_some());
        assert_eq!(proto.last_log_id.as_ref().unwrap().index, 42);

        let restored = from_proto_snapshot_meta(Some(proto)).expect("meta present");
        assert_eq!(restored.last_log_id, original.last_log_id);
        assert_eq!(restored.snapshot_id, original.snapshot_id);
        assert_eq!(
            restored
                .last_membership
                .membership()
                .voter_ids()
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        // Membership log id is rebuilt from the snapshot log id.
        assert_eq!(restored.last_membership.log_id(), &original.last_log_id);
    }

    #[test]
    fn test_snapshot_meta_missing_returns_none() {
        assert!(from_proto_snapshot_meta(None).is_none());
    }
}
