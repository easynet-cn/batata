//! Raft type configuration for openraft.
//!
//! Defines all the type aliases needed for the Raft consensus implementation.

use super::request::{RaftRequest, RaftResponse};
use super::snapshot_file::SnapshotFile;
use super::state_machine::RocksStateMachine;
use openraft::type_config::alias;

/// Node ID type - using u64 for efficient storage and comparison
pub type NodeId = u64;

openraft::declare_raft_types!(
    /// The openraft type configuration used throughout the crate.
    ///
    /// Since openraft 0.10 the snapshot data type is declared by the state
    /// machine, the snapshot builder and the network instead of the type
    /// config. All three use [`SnapshotFile`], a file handle, so that snapshot
    /// transfer streams through disk instead of allocating the whole snapshot.
    pub TypeConfig:
        D = RaftRequest,
        R = RaftResponse,
        Node = openraft::BasicNode,
        NodeId = NodeId,
);

/// Type alias for the Raft instance (0.10 names the state machine type).
pub type Raft = openraft::Raft<TypeConfig, RocksStateMachine>;

/// Snapshot data shared by the state machine, snapshot builder and network.
pub type SnapshotData = SnapshotFile;

/// Type alias for log entries
pub type Entry = alias::EntryOf<TypeConfig>;

/// Type alias for log ID
pub type LogId = alias::LogIdOf<TypeConfig>;

/// Type alias for vote
pub type Vote = alias::VoteOf<TypeConfig>;

/// Type alias for committed leader ID
pub type CommittedLeaderId = alias::CommittedLeaderIdOf<TypeConfig>;

/// Type alias for membership
pub type Membership = alias::MembershipOf<TypeConfig>;

/// Type alias for stored membership
pub type StoredMembership = alias::StoredMembershipOf<TypeConfig>;

/// Type alias for snapshot metadata
pub type SnapshotMeta = alias::SnapshotMetaOf<TypeConfig>;

/// Type alias for a complete snapshot carrying the file-backed data
pub type Snapshot = alias::SnapshotOf<TypeConfig, SnapshotFile>;

/// Type alias for metrics
pub type RaftMetrics = openraft::RaftMetrics<TypeConfig>;

/// Type alias for server state
pub type ServerState = openraft::ServerState;
