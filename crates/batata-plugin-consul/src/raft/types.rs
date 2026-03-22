/// Consul-specific OpenRaft type configuration.
///
/// This is separate from the Nacos Raft type config, giving Consul
/// its own Raft log index space and state machine.
use std::io::Cursor;

use serde::{Deserialize, Serialize};

use super::request::{ConsulRaftRequest, ConsulRaftResponse};

/// Reuse NodeId from consistency crate for cluster compatibility.
pub type NodeId = u64;

/// Snapshot data for the Consul state machine.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConsulSnapshotData {
    pub data: Vec<u8>,
}

impl AsRef<[u8]> for ConsulSnapshotData {
    fn as_ref(&self) -> &[u8] {
        &self.data
    }
}

impl From<ConsulSnapshotData> for Cursor<Vec<u8>> {
    fn from(snapshot: ConsulSnapshotData) -> Self {
        Cursor::new(snapshot.data)
    }
}

openraft::declare_raft_types!(
    pub ConsulTypeConfig:
        D = ConsulRaftRequest,
        R = ConsulRaftResponse,
        Node = openraft::BasicNode,
        NodeId = NodeId,
);

pub type ConsulRaft = openraft::Raft<ConsulTypeConfig>;
pub type ConsulEntry = openraft::Entry<ConsulTypeConfig>;
pub type ConsulLogId = openraft::LogId<NodeId>;
pub type ConsulVote = openraft::Vote<NodeId>;
pub type ConsulMembership = openraft::Membership<NodeId, openraft::BasicNode>;
pub type ConsulStoredMembership = openraft::StoredMembership<NodeId, openraft::BasicNode>;
pub type ConsulSnapshotMeta = openraft::SnapshotMeta<NodeId, openraft::BasicNode>;
pub type ConsulRaftMetrics = openraft::RaftMetrics<NodeId, openraft::BasicNode>;
