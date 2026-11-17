//! Public proto <-> openraft type conversion utilities.
//!
//! Shared between Batata Raft and Consul Raft gRPC services.
use std::collections::{BTreeMap, BTreeSet};

use batata_server_api::raft::{
    LogId as ProtoLogId, Membership as ProtoMembership, RaftNodeInfo,
    SnapshotMeta as ProtoSnapshotMeta, Vote as ProtoVote,
};
use openraft::BasicNode;

use super::types::{
    CommittedLeaderId, LogId, Membership, NodeId, SnapshotMeta, StoredMembership, Vote,
};

/// Convert proto LogId to openraft LogId.
///
/// The proposing leader's node id rides on the wire as `leader_id`. openraft
/// 0.10 uses advanced leader ids, so `node_id` is part of a log's identity and
/// participates in `Ord`; reconstructing it as `0` makes every remote log
/// compare older than the local one and blocks vote grants in a fresh cluster.
pub fn from_proto_log_id(log_id: Option<ProtoLogId>) -> Option<LogId> {
    log_id.map(|l| {
        LogId::new(
            CommittedLeaderId {
                term: l.term,
                node_id: l.leader_id,
            },
            l.index,
        )
    })
}

/// Convert openraft LogId to proto LogId.
pub fn to_proto_log_id(log_id: Option<LogId>) -> Option<ProtoLogId> {
    log_id.map(|l| ProtoLogId {
        term: l.leader_id.term,
        index: l.index,
        leader_id: l.leader_id.node_id,
    })
}

/// Convert proto Vote to openraft Vote.
pub fn from_proto_vote(vote: Option<ProtoVote>) -> Option<Vote> {
    vote.map(|v| {
        if v.committed {
            Vote::new_committed(v.term, v.leader_id)
        } else {
            Vote::new(v.term, v.leader_id)
        }
    })
}

/// Convert openraft Vote to proto Vote.
pub fn to_proto_vote(vote: &Vote) -> ProtoVote {
    ProtoVote {
        leader_id: vote.leader_id().node_id,
        term: vote.leader_id().term,
        committed: vote.is_committed(),
    }
}

/// Resolve the address of a node, falling back to an empty string when the
/// membership carries voter ids without node metadata.
fn node_addr(membership: &Membership, node_id: NodeId) -> String {
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
pub fn to_proto_membership(membership: &StoredMembership) -> Option<ProtoMembership> {
    let inner = membership.membership();

    let voters: Vec<RaftNodeInfo> = inner
        .voter_ids()
        .map(|id| RaftNodeInfo {
            node_id: id,
            addr: node_addr(inner, id),
        })
        .collect();
    let learners: Vec<RaftNodeInfo> = inner
        .non_write_voter_ids()
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
    log_id: Option<LogId>,
) -> StoredMembership {
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

    // 0.10 makes `new()` fallible: every voter must have node metadata. This
    // conversion inserts one `BasicNode` per wire voter, so the invariant
    // always holds. `new_with_defaults()` is deliberately not used: it takes
    // bare node ids and would discard the addresses carried on the wire.
    let membership = Membership::new(vec![voters], nodes)
        .expect("wire conversion inserts node metadata for every voter");
    StoredMembership::new(log_id, membership)
}

/// Convert openraft `SnapshotMeta` to the wire `SnapshotMeta` message.
pub fn to_proto_snapshot_meta(meta: &SnapshotMeta) -> ProtoSnapshotMeta {
    // The wire keeps the 0.9 `snapshot_id` slot so an older peer can still
    // read it; openraft 0.10 dropped the field and identifies a snapshot by
    // the log position it covers.
    let snapshot_id = format!(
        "snapshot-{}",
        meta.last_log_id.map(|l| l.index).unwrap_or(0)
    );
    ProtoSnapshotMeta {
        last_log_id: to_proto_log_id(meta.last_log_id),
        last_membership: to_proto_membership(&meta.last_membership),
        snapshot_id,
    }
}

/// Convert the wire `SnapshotMeta` message back into openraft's `SnapshotMeta`.
///
/// Returns `None` when the sender omitted metadata entirely, which the caller
/// treats as "cannot install this snapshot" rather than installing an empty one.
pub fn from_proto_snapshot_meta(meta: Option<ProtoSnapshotMeta>) -> Option<SnapshotMeta> {
    let meta = meta?;
    let last_log_id = from_proto_log_id(meta.last_log_id);

    // `snapshot_id` is a wire-only field; drop it when rebuilding the 0.10 type.
    Some(SnapshotMeta {
        last_log_id,
        last_membership: from_proto_membership(meta.last_membership, last_log_id),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn membership_with(voters: &[NodeId], learners: &[NodeId]) -> StoredMembership {
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
        // Every voter has a node in `nodes`, matching the production
        // conversion invariant enforced by `Membership::new()`.
        let membership = Membership::new(vec![voters], nodes).unwrap();
        StoredMembership::new(
            Some(LogId::new(
                CommittedLeaderId {
                    term: 3,
                    node_id: 6548153108518673165,
                },
                7,
            )),
            membership,
        )
    }

    #[test]
    fn test_log_id_round_trip_preserves_leader_node_id() {
        // A real cold-start log id uses a non-zero, hash-derived node id. The
        // old conversion hardcoded 0, so this assertion failed but no existing
        // test caught it because every fixture also used node_id: 0.
        let original = LogId::new(
            CommittedLeaderId {
                term: 4,
                node_id: 6548153108518673165,
            },
            19,
        );

        let wire = to_proto_log_id(Some(original)).expect("log id present");
        assert_eq!(wire.term, 4);
        assert_eq!(wire.index, 19);
        assert_eq!(wire.leader_id, 6548153108518673165);

        let restored = from_proto_log_id(Some(wire)).expect("log id present");
        assert_eq!(restored, original);
    }

    #[test]
    fn test_log_id_node_id_participates_in_up_to_date_comparison() {
        // This is exactly the check openraft uses to grant or reject a vote:
        // `candidate.last_log_id >= self.last_log_id`. Two logs with the same
        // term and index but different proposing leaders MUST compare unequal.
        // If the wire zeroes node_id, both collapse to T0-N0.0 and a follower
        // can no longer order candidate logs -- the split-vote root cause.
        let log_a = LogId::new(
            CommittedLeaderId {
                term: 0,
                node_id: 2327545806963820616,
            },
            0,
        );
        let log_b = LogId::new(
            CommittedLeaderId {
                term: 0,
                node_id: 6548153108518673165,
            },
            0,
        );
        assert_ne!(log_a, log_b);
        // Distinct logs must still be strictly ordered (advanced leader ids
        // form a total order), so neither direction collapses to Equal.
        assert_ne!(log_a.cmp(&log_b), std::cmp::Ordering::Equal);

        // Round-tripping each log through the wire must not erase that ordering.
        let restored_a = from_proto_log_id(to_proto_log_id(Some(log_a))).unwrap();
        let restored_b = from_proto_log_id(to_proto_log_id(Some(log_b))).unwrap();
        assert_eq!(restored_a.cmp(&restored_b), log_a.cmp(&log_b));
        assert_ne!(restored_a, restored_b);
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
        let restored_learners: Vec<NodeId> = restored.membership().non_write_voter_ids().collect();

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
        let original = SnapshotMeta {
            last_log_id: Some(LogId::new(
                CommittedLeaderId {
                    term: 5,
                    node_id: 2327545806963820616,
                },
                42,
            )),
            last_membership: membership,
        };

        let proto = to_proto_snapshot_meta(&original);
        // The wire form carries the membership; the log id rides along as
        // `last_log_id`, exactly like JRaft's `lastIncludedIndex/Term`.
        assert!(proto.last_membership.is_some());
        assert_eq!(proto.last_log_id.as_ref().unwrap().index, 42);
        assert_eq!(
            proto.last_log_id.as_ref().unwrap().leader_id,
            2327545806963820616
        );
        assert!(!proto.snapshot_id.is_empty());

        let restored = from_proto_snapshot_meta(Some(proto)).expect("meta present");
        assert_eq!(restored.last_log_id, original.last_log_id);
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
