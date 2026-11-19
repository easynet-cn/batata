//! Optional leader-side guard for Raft writes.
//!
//! Plugin apply handlers must stay deterministic: a wall-clock decision made
//! inside the state machine would differ between replicas. A write guard runs
//! **before** `client_write` on the node that currently believes it is the
//! leader, including writes forwarded from followers through the leader RPC.
//! It can therefore reject a request based on leader-local state without
//! changing the replicated state machine.

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::raft::request::RaftRequest;

/// Pre-commit validation hook. Returning `Err` rejects the write without
/// appending a log entry; the error text propagates to the HTTP caller.
#[async_trait]
pub trait RaftWriteGuard: Send + Sync {
    /// Validate one write request on the leader.
    async fn validate(&self, _request: &RaftRequest) -> Result<(), String> {
        Ok(())
    }

    /// Run after validation succeeds but before the request is proposed.
    ///
    /// Leader-local side effects that must precede any later write belong
    /// here. Consul lock delay uses it to record a session destroy's held
    /// keys before a concurrent acquire can be committed. Replays must remain
    /// idempotent; this hook is not part of the deterministic state machine.
    async fn on_accepted(&self, _request: &RaftRequest) {}
}

/// Shared slot holding the optional guard. Registered post-construction.
pub type SharedRaftWriteGuard = Arc<RwLock<Option<Arc<dyn RaftWriteGuard>>>>;

/// Create an empty write-guard slot.
pub fn new_shared_write_guard() -> SharedRaftWriteGuard {
    Arc::new(RwLock::new(None))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoopGuard;

    #[async_trait]
    impl RaftWriteGuard for NoopGuard {}

    #[tokio::test]
    async fn slot_starts_empty_and_accepts_registration() {
        let slot = new_shared_write_guard();
        assert!(slot.read().await.is_none());
        *slot.write().await = Some(Arc::new(NoopGuard));
        assert!(slot.read().await.is_some());
    }
}
