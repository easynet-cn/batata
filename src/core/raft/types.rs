// Raft type configuration for openraft
// Defines all the type aliases needed for the Raft consensus implementation

use std::io::Cursor;

use openraft::BasicNode;
use serde::{Deserialize, Serialize};

use super::request::{RaftRequest, RaftResponse};

/// Node ID type - using u64 for efficient storage and comparison
pub type NodeId = u64;

/// Snapshot data - serialized state machine data
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SnapshotData {
    pub data: Vec<u8>,
}

impl SnapshotData {
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }
}

// Implement AsRef<[u8]> for SnapshotData to allow openraft to work with it
impl AsRef<[u8]> for SnapshotData {
    fn as_ref(&self) -> &[u8] {
        &self.data
    }
}

// openraft requires the ability to create a cursor from snapshot data
impl From<SnapshotData> for Cursor<Vec<u8>> {
    fn from(snapshot: SnapshotData) -> Self {
        Cursor::new(snapshot.data)
    }
}

openraft::declare_raft_types!(
    pub TypeConfig:
        D = RaftRequest,
        R = RaftResponse,
        Node = openraft::BasicNode,
        NodeId = NodeId,
);

/// Type alias for the Raft instance
pub type Raft = openraft::Raft<TypeConfig>;

/// Type alias for log entries
pub type Entry = openraft::Entry<TypeConfig>;

/// Type alias for log ID
pub type LogId = openraft::LogId<NodeId>;

/// Type alias for vote
pub type Vote = openraft::Vote<NodeId>;

/// Type alias for membership
pub type Membership = openraft::Membership<NodeId, openraft::BasicNode>;

/// Type alias for stored membership
pub type StoredMembership = openraft::StoredMembership<NodeId, openraft::BasicNode>;

/// Type alias for snapshot metadata
pub type SnapshotMeta = openraft::SnapshotMeta<NodeId, openraft::BasicNode>;

/// Type alias for metrics
pub type RaftMetrics = openraft::RaftMetrics<NodeId, openraft::BasicNode>;

/// Type alias for server state
pub type ServerState = openraft::ServerState;

/// Helper function to calculate node ID from address
pub fn calculate_node_id(addr: &str) -> NodeId {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    addr.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_node_id() {
        let addr1 = "192.168.1.1:7848";
        let addr2 = "192.168.1.2:7848";

        let id1 = calculate_node_id(addr1);
        let id2 = calculate_node_id(addr2);

        // Same address should produce same ID
        assert_eq!(id1, calculate_node_id(addr1));
        // Different addresses should produce different IDs (with high probability)
        assert_ne!(id1, id2);
    }
}
