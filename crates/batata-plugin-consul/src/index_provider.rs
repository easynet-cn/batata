// Unified Consul index provider for X-Consul-Index headers.
//
// In Consul, X-Consul-Index is the Raft log index — a monotonically increasing uint64
// that represents the latest modification point. It powers blocking queries and watches.
//
// This provider abstracts the index source:
// - Cluster mode: uses Raft last_applied_index (consistent across all nodes)
// - Single-node mode: uses a local AtomicU64 counter

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use batata_consistency::RaftNode;

/// Provides the current Consul-compatible modification index.
///
/// In cluster mode, returns the Raft last_applied_index which is
/// naturally consistent across all nodes since it comes from the Raft log.
/// In single-node mode, uses a local atomic counter incremented on each write.
#[derive(Clone)]
pub struct ConsulIndexProvider {
    raft_node: Option<Arc<RaftNode>>,
    local_index: Arc<AtomicU64>,
}

impl ConsulIndexProvider {
    /// Create a provider for single-node (in-memory or standalone) mode.
    pub fn new() -> Self {
        Self {
            raft_node: None,
            local_index: Arc::new(AtomicU64::new(1)),
        }
    }

    /// Create a provider backed by Raft for cluster mode.
    pub fn with_raft(raft_node: Arc<RaftNode>) -> Self {
        Self {
            raft_node: Some(raft_node),
            local_index: Arc::new(AtomicU64::new(1)),
        }
    }

    /// Get the current index value for X-Consul-Index headers.
    ///
    /// - In cluster mode: returns Raft last_applied_index (falls back to 1)
    /// - In single-node mode: returns the local atomic counter
    ///
    /// The index is guaranteed to be >= 1 (never 0) to prevent
    /// blocking query busy loops, consistent with Consul's behavior.
    pub fn current_index(&self) -> u64 {
        if let Some(ref raft) = self.raft_node {
            // Use Raft log index — consistent across all cluster nodes
            raft.last_applied_index().unwrap_or(1).max(1)
        } else {
            self.local_index.load(Ordering::SeqCst).max(1)
        }
    }

    /// Increment the local index counter and return the new value.
    /// Called on each naming/catalog write operation in single-node mode.
    /// In cluster mode this is a no-op since Raft index advances automatically.
    pub fn increment(&self) -> u64 {
        if self.raft_node.is_some() {
            // In cluster mode, the Raft log index advances on its own
            // when writes go through Raft. Just return the current value.
            self.current_index()
        } else {
            self.local_index.fetch_add(1, Ordering::SeqCst) + 1
        }
    }
}

impl Default for ConsulIndexProvider {
    fn default() -> Self {
        Self::new()
    }
}
