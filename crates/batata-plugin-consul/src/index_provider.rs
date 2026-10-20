use std::sync::Arc;
/// Per-table index tracking for Consul blocking queries and X-Consul-Index headers.
///
/// Matches Consul's original architecture where each data table (kvs, sessions,
/// catalog, etc.) has its own high water mark derived from the Raft log index.
///
/// Blocking queries only wake when the **relevant table** changes, not on
/// any write — this is the key difference from the previous global counter.
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tokio::sync::watch;

/// Maximum blocking query wait time (10 minutes, consistent with Consul).
const MAX_BLOCKING_WAIT: Duration = Duration::from_secs(600);

/// Default blocking query wait time (5 minutes, consistent with Consul).
const DEFAULT_BLOCKING_WAIT: Duration = Duration::from_secs(300);

/// Consul data tables that track independent indexes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConsulTable {
    /// Key-value store (includes tombstones for delete tracking)
    KVS,
    /// Sessions (create, destroy, renew)
    Sessions,
    /// Catalog (services, nodes)
    Catalog,
    /// ACL tokens, policies, roles, auth methods
    ACL,
    /// Config entries (service-defaults, proxy-defaults, etc.)
    ConfigEntries,
    /// Connect CA roots, config, and intentions
    ConnectCA,
    /// Network coordinates
    Coordinates,
    /// Cluster peering
    Peering,
    /// Operator (autopilot, keyring)
    Operator,
    /// Prepared queries
    Queries,
    /// Namespaces
    Namespaces,
    /// Partitions (simplified Enterprise feature)
    Partitions,
    /// User events
    Events,
}

/// Per-table state: high water mark + change notification.
///
/// The `watch` channel carries the index so a waiter can never miss an update:
/// `Notify::notify_waiters()` only wakes waiters registered **at that moment**,
/// so a waiter sitting between "notification consumed" and "re-registered"
/// would sleep until the timeout even though the index had already moved.
/// `watch::Receiver::wait_for` re-checks the latest value instead.
struct TableState {
    /// Highest Raft log index that affected this table.
    /// In standalone mode, this is a local counter.
    index: AtomicU64,
    /// Broadcast channel used to wake blocking queries.
    tx: watch::Sender<u64>,
}

impl TableState {
    fn new(initial_index: u64) -> Self {
        let (tx, _rx) = watch::channel(initial_index);
        Self {
            index: AtomicU64::new(initial_index),
            tx,
        }
    }
}

/// Per-table index tracker for Consul operations.
///
/// Each table has its own index and notification channel.
/// This allows KV blocking queries to wake only on KV changes,
/// not on session or catalog changes.
#[derive(Clone)]
pub struct ConsulTableIndex {
    kvs: Arc<TableState>,
    sessions: Arc<TableState>,
    catalog: Arc<TableState>,
    acl: Arc<TableState>,
    config_entries: Arc<TableState>,
    connect_ca: Arc<TableState>,
    coordinates: Arc<TableState>,
    peering: Arc<TableState>,
    operator: Arc<TableState>,
    queries: Arc<TableState>,
    namespaces: Arc<TableState>,
    partitions: Arc<TableState>,
    events: Arc<TableState>,
}

impl ConsulTableIndex {
    /// Create a new table index with all tables starting at index 1.
    pub fn new() -> Self {
        Self {
            kvs: Arc::new(TableState::new(1)),
            sessions: Arc::new(TableState::new(1)),
            catalog: Arc::new(TableState::new(1)),
            acl: Arc::new(TableState::new(1)),
            config_entries: Arc::new(TableState::new(1)),
            connect_ca: Arc::new(TableState::new(1)),
            coordinates: Arc::new(TableState::new(1)),
            peering: Arc::new(TableState::new(1)),
            operator: Arc::new(TableState::new(1)),
            queries: Arc::new(TableState::new(1)),
            namespaces: Arc::new(TableState::new(1)),
            partitions: Arc::new(TableState::new(1)),
            events: Arc::new(TableState::new(1)),
        }
    }

    /// Get the table state for a given table.
    fn table(&self, table: ConsulTable) -> &Arc<TableState> {
        match table {
            ConsulTable::KVS => &self.kvs,
            ConsulTable::Sessions => &self.sessions,
            ConsulTable::Catalog => &self.catalog,
            ConsulTable::ACL => &self.acl,
            ConsulTable::ConfigEntries => &self.config_entries,
            ConsulTable::ConnectCA => &self.connect_ca,
            ConsulTable::Coordinates => &self.coordinates,
            ConsulTable::Peering => &self.peering,
            ConsulTable::Operator => &self.operator,
            ConsulTable::Queries => &self.queries,
            ConsulTable::Namespaces => &self.namespaces,
            ConsulTable::Partitions => &self.partitions,
            ConsulTable::Events => &self.events,
        }
    }

    /// Get the current index for a single table.
    /// Always returns >= 1 to prevent blocking query busy loops.
    pub fn current_index(&self, table: ConsulTable) -> u64 {
        self.table(table).index.load(Ordering::SeqCst).max(1)
    }

    /// Get the maximum index across multiple tables.
    /// Used for queries that span tables (e.g., KV reads include tombstones).
    pub fn max_index(&self, tables: &[ConsulTable]) -> u64 {
        tables
            .iter()
            .map(|t| self.current_index(*t))
            .max()
            .unwrap_or(1)
    }

    /// Update a table's index after a successful Raft write.
    ///
    /// `raft_log_index` is the Raft log index from `write_with_index()`.
    /// Uses `fetch_max` to ensure monotonic increase (never goes backwards).
    /// Notifies all blocking queries waiting on this table.
    pub fn update(&self, table: ConsulTable, raft_log_index: u64) {
        let state = self.table(table);
        let new_index = state.index.fetch_max(raft_log_index, Ordering::SeqCst);
        let new_index = new_index.max(raft_log_index);
        // A missing receiver is not an error: nobody is waiting, and the
        // atomic above is the source of truth for future waiters.
        let _ = state.tx.send(new_index);
    }

    /// Increment a table's index locally (for standalone mode without Raft).
    /// Returns the new index value.
    pub fn increment(&self, table: ConsulTable) -> u64 {
        let state = self.table(table);
        let new_idx = state.index.fetch_add(1, Ordering::SeqCst) + 1;
        let _ = state.tx.send(new_idx);
        new_idx
    }

    /// Block until the given table's index exceeds `target_index`,
    /// or until the wait duration elapses.
    ///
    /// Returns `true` if the index was reached, `false` on timeout.
    pub async fn wait_for_change(
        &self,
        table: ConsulTable,
        target_index: u64,
        wait: Option<Duration>,
    ) -> bool {
        let state = self.table(table);

        // Already past target — return immediately. `index == 0` means "do not
        // block", matching Consul: a client that has never queried before sends
        // 0 and expects the current value back at once.
        if target_index == 0 || state.index.load(Ordering::SeqCst) > target_index {
            return true;
        }

        let timeout = wait.unwrap_or(DEFAULT_BLOCKING_WAIT).min(MAX_BLOCKING_WAIT);

        let mut rx = state.tx.subscribe();
        tokio::select! {
            changed = rx.wait_for(|i| *i > target_index) => changed.is_ok(),
            _ = tokio::time::sleep(timeout) => state.index.load(Ordering::SeqCst) > target_index,
        }
    }

    /// Parse the `wait` query parameter from Consul/Go duration format.
    ///
    /// Delegates to the unified `parse_go_duration` which supports all
    /// Go `time.Duration` units: `ns`, `us`, `ms`, `s`, `m`, `h` and
    /// compound formats like `1h30m`.
    pub fn parse_wait_duration(wait_str: &str) -> Option<Duration> {
        crate::consul_meta::parse_go_duration(wait_str)
    }
}

impl Default for ConsulTableIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// Backward-compatible alias. Old code references `ConsulIndexProvider`;
/// it now resolves to `ConsulTableIndex`. Callers need to update to
/// use per-table methods (`current_index(table)` instead of `current_index()`).
pub type ConsulIndexProvider = ConsulTableIndex;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_wait_duration() {
        assert_eq!(
            ConsulTableIndex::parse_wait_duration("5m"),
            Some(Duration::from_secs(300))
        );
        assert_eq!(
            ConsulTableIndex::parse_wait_duration("30s"),
            Some(Duration::from_secs(30))
        );
        assert_eq!(
            ConsulTableIndex::parse_wait_duration("100ms"),
            Some(Duration::from_millis(100))
        );
        assert_eq!(
            ConsulTableIndex::parse_wait_duration("60"),
            Some(Duration::from_secs(60))
        );
        assert_eq!(ConsulTableIndex::parse_wait_duration(""), None);
        assert_eq!(ConsulTableIndex::parse_wait_duration("abc"), None);
    }

    #[test]
    fn test_current_index_minimum_is_one() {
        let idx = ConsulTableIndex::new();
        assert_eq!(idx.current_index(ConsulTable::KVS), 1);
        assert_eq!(idx.current_index(ConsulTable::Sessions), 1);
        assert_eq!(idx.current_index(ConsulTable::Catalog), 1);
    }

    #[test]
    fn test_update_monotonic() {
        let idx = ConsulTableIndex::new();
        idx.update(ConsulTable::KVS, 10);
        assert_eq!(idx.current_index(ConsulTable::KVS), 10);

        // Lower value should not decrease
        idx.update(ConsulTable::KVS, 5);
        assert_eq!(idx.current_index(ConsulTable::KVS), 10);

        // Higher value should increase
        idx.update(ConsulTable::KVS, 20);
        assert_eq!(idx.current_index(ConsulTable::KVS), 20);
    }

    #[test]
    fn test_tables_independent() {
        let idx = ConsulTableIndex::new();
        idx.update(ConsulTable::KVS, 100);
        idx.update(ConsulTable::Sessions, 50);
        idx.update(ConsulTable::Catalog, 75);

        assert_eq!(idx.current_index(ConsulTable::KVS), 100);
        assert_eq!(idx.current_index(ConsulTable::Sessions), 50);
        assert_eq!(idx.current_index(ConsulTable::Catalog), 75);
    }

    #[test]
    fn test_max_index() {
        let idx = ConsulTableIndex::new();
        idx.update(ConsulTable::KVS, 100);
        idx.update(ConsulTable::Sessions, 50);

        assert_eq!(
            idx.max_index(&[ConsulTable::KVS, ConsulTable::Sessions]),
            100
        );
    }

    #[test]
    fn test_increment_standalone() {
        let idx = ConsulTableIndex::new();
        assert_eq!(idx.increment(ConsulTable::KVS), 2); // 1 -> 2
        assert_eq!(idx.increment(ConsulTable::KVS), 3); // 2 -> 3
        assert_eq!(idx.current_index(ConsulTable::Sessions), 1); // unaffected
    }

    #[tokio::test]
    async fn test_wait_already_past() {
        let idx = ConsulTableIndex::new();
        idx.update(ConsulTable::KVS, 10);
        assert!(idx.wait_for_change(ConsulTable::KVS, 5, None).await);
    }

    #[tokio::test]
    async fn test_wait_timeout() {
        let idx = ConsulTableIndex::new();
        let reached = idx
            .wait_for_change(ConsulTable::KVS, 100, Some(Duration::from_millis(50)))
            .await;
        assert!(!reached);
    }

    #[tokio::test]
    async fn test_wait_notified() {
        let idx = ConsulTableIndex::new();
        let idx2 = idx.clone();

        let handle = tokio::spawn(async move {
            idx2.wait_for_change(ConsulTable::KVS, 1, Some(Duration::from_secs(5)))
                .await
        });

        tokio::time::sleep(Duration::from_millis(10)).await;
        idx.update(ConsulTable::KVS, 5);

        assert!(handle.await.unwrap());
    }

    #[tokio::test]
    async fn test_wait_only_wakes_on_relevant_table() {
        let idx = ConsulTableIndex::new();
        let idx2 = idx.clone();

        // Wait on KVS
        let handle = tokio::spawn(async move {
            idx2.wait_for_change(ConsulTable::KVS, 1, Some(Duration::from_millis(100)))
                .await
        });

        tokio::time::sleep(Duration::from_millis(10)).await;
        // Update Sessions — should NOT wake the KVS waiter
        idx.update(ConsulTable::Sessions, 100);

        let reached = handle.await.unwrap();
        assert!(!reached, "KVS wait should NOT be woken by Sessions update");
    }
}
