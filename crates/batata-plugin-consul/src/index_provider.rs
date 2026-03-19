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
use std::time::Duration;

use batata_consistency::RaftNode;

/// Maximum blocking query wait time (10 minutes, consistent with Consul).
const MAX_BLOCKING_WAIT: Duration = Duration::from_secs(600);

/// Default blocking query wait time (5 minutes, consistent with Consul).
const DEFAULT_BLOCKING_WAIT: Duration = Duration::from_secs(300);

/// Provides the current Consul-compatible modification index.
///
/// In cluster mode, returns the Raft last_applied_index which is
/// naturally consistent across all nodes since it comes from the Raft log.
/// In single-node mode, uses a local atomic counter incremented on each write.
#[derive(Clone)]
pub struct ConsulIndexProvider {
    raft_node: Option<Arc<RaftNode>>,
    local_index: Arc<AtomicU64>,
    /// Notify waiters when index changes (for blocking queries)
    notify: Arc<tokio::sync::Notify>,
}

impl ConsulIndexProvider {
    /// Create a provider for single-node (in-memory or standalone) mode.
    pub fn new() -> Self {
        Self {
            raft_node: None,
            local_index: Arc::new(AtomicU64::new(1)),
            notify: Arc::new(tokio::sync::Notify::new()),
        }
    }

    /// Create a provider backed by Raft for cluster mode.
    pub fn with_raft(raft_node: Arc<RaftNode>) -> Self {
        Self {
            raft_node: Some(raft_node),
            local_index: Arc::new(AtomicU64::new(1)),
            notify: Arc::new(tokio::sync::Notify::new()),
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
        let new_idx = if self.raft_node.is_some() {
            // In cluster mode, the Raft log index advances on its own
            // when writes go through Raft. Just return the current value.
            self.current_index()
        } else {
            self.local_index.fetch_add(1, Ordering::SeqCst) + 1
        };
        // Wake up any blocking query waiters
        self.notify.notify_waiters();
        new_idx
    }

    /// Block until the index exceeds `target_index` or until `wait` duration elapses.
    ///
    /// This implements Consul-compatible blocking queries. If `target_index` is 0 or
    /// already exceeded, returns immediately. The `wait` duration is clamped to
    /// `MAX_BLOCKING_WAIT` (10 minutes) consistent with Consul.
    ///
    /// Returns `true` if index was reached, `false` if timed out.
    pub async fn wait_for_index(&self, target_index: u64, wait: Option<Duration>) -> bool {
        // If no target or already past it, return immediately
        if target_index == 0 || self.current_index() > target_index {
            return true;
        }

        let timeout = wait.unwrap_or(DEFAULT_BLOCKING_WAIT).min(MAX_BLOCKING_WAIT);

        tokio::select! {
            _ = async {
                loop {
                    self.notify.notified().await;
                    if self.current_index() > target_index {
                        break;
                    }
                }
            } => true,
            _ = tokio::time::sleep(timeout) => {
                self.current_index() > target_index
            }
        }
    }

    /// Parse the `wait` query parameter from Consul format (e.g. "5m", "30s", "100ms").
    /// Returns None if the parameter is missing or unparseable.
    pub fn parse_wait_duration(wait_str: &str) -> Option<Duration> {
        let s = wait_str.trim();
        if s.is_empty() {
            return None;
        }
        if let Some(ms) = s.strip_suffix("ms") {
            ms.parse::<u64>().ok().map(Duration::from_millis)
        } else if let Some(secs) = s.strip_suffix('s') {
            secs.parse::<u64>().ok().map(Duration::from_secs)
        } else if let Some(mins) = s.strip_suffix('m') {
            mins.parse::<u64>()
                .ok()
                .map(|m| Duration::from_secs(m * 60))
        } else {
            // Treat bare number as seconds (Consul default)
            s.parse::<u64>().ok().map(Duration::from_secs)
        }
    }
}

impl Default for ConsulIndexProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_wait_duration() {
        assert_eq!(
            ConsulIndexProvider::parse_wait_duration("5m"),
            Some(Duration::from_secs(300))
        );
        assert_eq!(
            ConsulIndexProvider::parse_wait_duration("30s"),
            Some(Duration::from_secs(30))
        );
        assert_eq!(
            ConsulIndexProvider::parse_wait_duration("100ms"),
            Some(Duration::from_millis(100))
        );
        assert_eq!(
            ConsulIndexProvider::parse_wait_duration("60"),
            Some(Duration::from_secs(60))
        );
        assert_eq!(ConsulIndexProvider::parse_wait_duration(""), None);
        assert_eq!(ConsulIndexProvider::parse_wait_duration("abc"), None);
    }

    #[tokio::test]
    async fn test_wait_for_index_already_past() {
        let provider = ConsulIndexProvider::new();
        // Index starts at 1, target 0 should return immediately
        assert!(provider.wait_for_index(0, None).await);
    }

    #[tokio::test]
    async fn test_wait_for_index_timeout() {
        let provider = ConsulIndexProvider::new();
        // Index is 1, target 100 — should timeout quickly
        let reached = provider
            .wait_for_index(100, Some(Duration::from_millis(50)))
            .await;
        assert!(!reached);
    }

    #[tokio::test]
    async fn test_wait_for_index_notified() {
        let provider = ConsulIndexProvider::new();
        let p2 = provider.clone();

        let handle =
            tokio::spawn(async move { p2.wait_for_index(1, Some(Duration::from_secs(5))).await });

        // Increment should wake up the waiter
        tokio::time::sleep(Duration::from_millis(10)).await;
        provider.increment();

        let reached = handle.await.unwrap();
        assert!(reached);
    }
}
