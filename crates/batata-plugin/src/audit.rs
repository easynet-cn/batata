//! Audit Plugin — Config change audit logging
//!
//! Logs all config change operations for compliance and security.
//! Implements ConfigChangePluginV2 for integration with the enhanced plugin chain.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;

/// Default capacity of the bounded audit queue.
///
/// Audit logging is a side channel: a slow store must never apply backpressure
/// to config publish/remove. The queue is therefore bounded and full queues
/// drop the newest entry instead of growing without limit.
pub const AUDIT_QUEUE_CAPACITY: usize = 1024;

/// Observable counters for the bounded audit queue.
#[derive(Debug, Default)]
pub struct AuditQueueStats {
    /// Entries accepted into the queue (including ones still pending).
    pub enqueued: AtomicU64,
    /// Entries dropped because the queue was full or the consumer stopped.
    pub dropped: AtomicU64,
    /// Entries the consumer failed to append to the store.
    pub append_failed: AtomicU64,
}

impl AuditQueueStats {
    /// Number of entries accepted into the queue.
    pub fn enqueued(&self) -> u64 {
        self.enqueued.load(Ordering::Relaxed)
    }

    /// Number of entries dropped by the explicit drop-newest policy.
    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    /// Number of store append failures observed by the consumer.
    pub fn append_failed(&self) -> u64 {
        self.append_failed.load(Ordering::Relaxed)
    }
}

use crate::spi::{
    ConfigChangePluginV2, ConfigChangeRequest, ConfigChangeResult, ConfigPointcut, ExecuteType,
};

/// Audit log entry
#[derive(Debug, Clone)]
pub struct AuditLogEntry {
    /// Unique identifier for this audit log entry.
    pub id: String,
    /// Event timestamp in milliseconds since the Unix epoch.
    pub timestamp: i64,
    /// Config change pointcut that produced this entry.
    pub pointcut: String,
    /// Execute type (before/after) that produced this entry.
    pub execute_type: String,
    /// Config data ID the operation targeted.
    pub data_id: String,
    /// Config group the operation targeted.
    pub group: String,
    /// Tenant (namespace) the operation targeted.
    pub tenant: String,
    /// Operator that performed the operation.
    pub operator: String,
    /// Client IP address that initiated the operation, if known.
    pub client_ip: Option<String>,
    /// Length in bytes of the config content involved.
    pub content_length: usize,
    /// Arbitrary key-value metadata attached to the operation.
    pub metadata: HashMap<String, String>,
    /// Whether the operation succeeded.
    pub success: bool,
    /// Reason for failure, if the operation was denied or failed.
    pub reason: Option<String>,
}

impl AuditLogEntry {
    /// Build an [`AuditLogEntry`] from a config change request and its outcome.
    pub fn from_request(req: &ConfigChangeRequest, success: bool, reason: Option<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0),
            pointcut: req.pointcut.as_str().to_string(),
            execute_type: req.execute_type.as_str().to_string(),
            data_id: req.data_id.clone(),
            group: req.group.clone(),
            tenant: req.tenant.clone(),
            operator: req.operator.clone(),
            client_ip: req.client_ip.clone(),
            content_length: req.content.len(),
            metadata: req.metadata.clone(),
            success,
            reason,
        }
    }
}

/// Audit log storage trait
#[async_trait::async_trait]
pub trait AuditLogStore: Send + Sync {
    /// Append a single audit log entry to the store.
    async fn append(&self, entry: AuditLogEntry) -> anyhow::Result<()>;
    /// Query audit log entries using optional filters and pagination.
    async fn query(
        &self,
        data_id: Option<&str>,
        group: Option<&str>,
        tenant: Option<&str>,
        operator: Option<&str>,
        start_time: Option<i64>,
        end_time: Option<i64>,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Vec<AuditLogEntry>>;
}

/// In-memory audit log store (default, limited retention)
pub struct InMemoryAuditLogStore {
    logs: Arc<tokio::sync::RwLock<Vec<AuditLogEntry>>>,
    max_size: usize,
}

impl InMemoryAuditLogStore {
    /// Create a new in-memory store retaining at most `max_size` entries.
    pub fn new(max_size: usize) -> Self {
        Self {
            logs: Arc::new(tokio::sync::RwLock::new(Vec::new())),
            max_size,
        }
    }
}

#[async_trait::async_trait]
impl AuditLogStore for InMemoryAuditLogStore {
    async fn append(&self, entry: AuditLogEntry) -> anyhow::Result<()> {
        let mut logs = self.logs.write().await;
        logs.push(entry);
        if logs.len() > self.max_size {
            logs.remove(0);
        }
        Ok(())
    }

    async fn query(
        &self,
        data_id: Option<&str>,
        group: Option<&str>,
        tenant: Option<&str>,
        operator: Option<&str>,
        start_time: Option<i64>,
        end_time: Option<i64>,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Vec<AuditLogEntry>> {
        let logs = self.logs.read().await;
        let mut filtered: Vec<_> = logs
            .iter()
            .filter(|e| {
                data_id.is_none_or(|d| e.data_id == d)
                    && group.is_none_or(|g| e.group == g)
                    && tenant.is_none_or(|t| e.tenant == t)
                    && operator.is_none_or(|o| e.operator == o)
                    && start_time.is_none_or(|st| e.timestamp >= st)
                    && end_time.is_none_or(|et| e.timestamp <= et)
            })
            .cloned()
            .collect();
        filtered.reverse();
        let start = ((page_no.saturating_sub(1)) as usize).saturating_mul(page_size as usize);
        let end = (start + page_size as usize).min(filtered.len());
        Ok(filtered[start..end].to_vec())
    }
}

/// Default audit plugin implementation.
///
/// Entries are handed to a single background consumer through a bounded
/// channel. The explicit drop policy is **drop-newest**: when the queue is
/// full, the new entry is discarded immediately and counted in
/// [`AuditQueueStats::dropped`]. The config request path never awaits the
/// queue, so a slow or stalled audit store cannot stall config operations or
/// grow the process heap without bound.
pub struct AuditPlugin {
    store: Arc<dyn AuditLogStore>,
    log_tx: mpsc::Sender<AuditLogEntry>,
    capacity: usize,
    stats: Arc<AuditQueueStats>,
}

impl AuditPlugin {
    /// Create a new audit plugin backed by the given log store, using
    /// [`AUDIT_QUEUE_CAPACITY`].
    pub fn new(store: Arc<dyn AuditLogStore>) -> Self {
        Self::new_with_capacity(store, AUDIT_QUEUE_CAPACITY)
    }

    /// Create a new audit plugin with an explicit queue capacity (clamped to
    /// at least 1). Exposed mainly for tests and for deployments that need a
    /// different bounded-queue budget.
    pub fn new_with_capacity(store: Arc<dyn AuditLogStore>, capacity: usize) -> Self {
        let capacity = capacity.max(1);
        let (log_tx, mut log_rx) = mpsc::channel::<AuditLogEntry>(capacity);
        let store_clone = store.clone();
        let stats = Arc::new(AuditQueueStats::default());
        let consumer_stats = stats.clone();
        tokio::spawn(async move {
            while let Some(entry) = log_rx.recv().await {
                if let Err(e) = store_clone.append(entry).await {
                    consumer_stats.append_failed.fetch_add(1, Ordering::Relaxed);
                    tracing::warn!(target: "audit_plugin", "Failed to append audit log: {}", e);
                }
            }
        });
        Self {
            store,
            log_tx,
            capacity,
            stats,
        }
    }

    /// Create a new audit plugin backed by an in-memory store of the given size.
    pub fn with_memory_store(max_size: usize) -> Self {
        Self::new(Arc::new(InMemoryAuditLogStore::new(max_size)))
    }

    /// Create a new audit plugin backed by an in-memory store, with an
    /// explicit queue capacity.
    pub fn with_memory_store_and_capacity(max_size: usize, capacity: usize) -> Self {
        Self::new_with_capacity(Arc::new(InMemoryAuditLogStore::new(max_size)), capacity)
    }

    /// Return the underlying audit log store.
    pub fn store(&self) -> Arc<dyn AuditLogStore> {
        self.store.clone()
    }

    /// Return the live queue counters.
    pub fn queue_stats(&self) -> Arc<AuditQueueStats> {
        self.stats.clone()
    }

    /// Return the configured (maximum) queue capacity.
    pub fn queue_capacity(&self) -> usize {
        self.capacity
    }

    fn log(&self, entry: AuditLogEntry) {
        match self.log_tx.try_send(entry) {
            Ok(()) => {
                self.stats.enqueued.fetch_add(1, Ordering::Relaxed);
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                let dropped = self.stats.dropped.fetch_add(1, Ordering::Relaxed) + 1;
                // Rate-limited: a stalled store can produce thousands of
                // entries per second, and one warning per drop is worse than
                // the problem being reported.
                if dropped == 1 || dropped.is_multiple_of(100) {
                    tracing::warn!(
                        target: "audit_plugin",
                        dropped,
                        capacity = self.capacity,
                        "audit queue full; dropping newest entry (drop-newest policy)"
                    );
                }
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                self.stats.dropped.fetch_add(1, Ordering::Relaxed);
                tracing::warn!(
                    target: "audit_plugin",
                    "audit consumer stopped; dropping entry"
                );
            }
        }
    }
}

#[async_trait::async_trait]
impl ConfigChangePluginV2 for AuditPlugin {
    fn name(&self) -> &str {
        "audit"
    }

    fn order(&self) -> i32 {
        100 // Run after most Before plugins, before After side-effect plugins
    }

    fn interested_pointcuts(&self) -> Vec<ConfigPointcut> {
        vec![
            ConfigPointcut::Publish,
            ConfigPointcut::Remove,
            ConfigPointcut::Get,
            ConfigPointcut::Import,
            ConfigPointcut::Export,
        ]
    }

    fn interested_execute_types(&self) -> Vec<ExecuteType> {
        vec![ExecuteType::Before, ExecuteType::After]
    }

    async fn execute(&self, ctx: &mut ConfigChangeRequest) -> ConfigChangeResult {
        let entry = AuditLogEntry::from_request(ctx, ctx.proceed, ctx.deny_reason.clone());
        self.log(entry);
        ConfigChangeResult::allow()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_request(pointcut: ConfigPointcut, execute_type: ExecuteType) -> ConfigChangeRequest {
        let mut req = ConfigChangeRequest::new(
            "test-data",
            "test-group",
            "public",
            "test content here",
            pointcut,
            execute_type,
            "test_user",
        );
        req.client_ip = Some("127.0.0.1".to_string());
        req
    }

    #[tokio::test]
    async fn test_audit_plugin_always_allows() {
        let plugin = AuditPlugin::with_memory_store(100);
        let mut req = make_request(ConfigPointcut::Publish, ExecuteType::Before);

        let result = plugin.execute(&mut req).await;
        assert!(result.allowed, "audit plugin should always allow");
    }

    #[tokio::test]
    async fn test_audit_plugin_logs_publish() {
        let plugin = AuditPlugin::with_memory_store(100);
        let store = plugin.store();
        let mut req = make_request(ConfigPointcut::Publish, ExecuteType::After);

        let _ = plugin.execute(&mut req).await;

        // Give the async logger a moment to process
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let logs = store
            .query(None, None, None, None, None, None, 1, 100)
            .await
            .unwrap();
        assert!(!logs.is_empty(), "should have logged the publish event");

        let entry = &logs[0];
        assert_eq!(entry.data_id, "test-data");
        assert_eq!(entry.group, "test-group");
        assert_eq!(entry.tenant, "public");
        assert_eq!(entry.operator, "test_user");
        assert_eq!(entry.client_ip.as_deref(), Some("127.0.0.1"));
        assert_eq!(entry.content_length, "test content here".len());
        assert_eq!(entry.pointcut, "publish");
        assert_eq!(entry.execute_type, "after");
    }

    #[tokio::test]
    async fn test_audit_plugin_logs_remove() {
        let plugin = AuditPlugin::with_memory_store(100);
        let store = plugin.store();
        let mut req = make_request(ConfigPointcut::Remove, ExecuteType::Before);

        let _ = plugin.execute(&mut req).await;

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let logs = store
            .query(None, None, None, None, None, None, 1, 100)
            .await
            .unwrap();
        assert!(!logs.is_empty());
        assert_eq!(logs[0].pointcut, "remove");
        assert_eq!(logs[0].execute_type, "before");
    }

    #[tokio::test]
    async fn test_audit_plugin_logs_get() {
        let plugin = AuditPlugin::with_memory_store(100);
        let store = plugin.store();
        let mut req = make_request(ConfigPointcut::Get, ExecuteType::After);

        let _ = plugin.execute(&mut req).await;

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let logs = store
            .query(None, None, None, None, None, None, 1, 100)
            .await
            .unwrap();
        assert!(!logs.is_empty());
        assert_eq!(logs[0].pointcut, "get");
    }

    #[tokio::test]
    async fn test_in_memory_store_max_size() {
        let store = InMemoryAuditLogStore::new(3);

        for i in 0..5 {
            let entry = AuditLogEntry {
                id: format!("id-{}", i),
                timestamp: i as i64,
                pointcut: "publish".to_string(),
                execute_type: "after".to_string(),
                data_id: format!("data-{}", i),
                group: "group".to_string(),
                tenant: "public".to_string(),
                operator: "user".to_string(),
                client_ip: None,
                content_length: 0,
                metadata: HashMap::new(),
                success: true,
                reason: None,
            };
            store.append(entry).await.unwrap();
        }

        let logs = store
            .query(None, None, None, None, None, None, 1, 100)
            .await
            .unwrap();
        assert_eq!(logs.len(), 3, "should only keep max_size entries");
        // Newest first (reverse order), so we should see data-4, data-3, data-2
        assert_eq!(logs[0].data_id, "data-4");
        assert_eq!(logs[1].data_id, "data-3");
        assert_eq!(logs[2].data_id, "data-2");
    }

    #[tokio::test]
    async fn test_in_memory_store_query_filters() {
        let store = InMemoryAuditLogStore::new(100);

        for i in 0..5 {
            let entry = AuditLogEntry {
                id: format!("id-{}", i),
                timestamp: 1000 + i as i64,
                pointcut: if i % 2 == 0 { "publish" } else { "remove" }.to_string(),
                execute_type: "after".to_string(),
                data_id: format!("data-{}", i),
                group: if i < 3 { "group-a" } else { "group-b" }.to_string(),
                tenant: "public".to_string(),
                operator: format!("user-{}", i % 2),
                client_ip: None,
                content_length: 100,
                metadata: HashMap::new(),
                success: true,
                reason: None,
            };
            store.append(entry).await.unwrap();
        }

        // Filter by data_id
        let logs = store
            .query(Some("data-0"), None, None, None, None, None, 1, 100)
            .await
            .unwrap();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].data_id, "data-0");

        // Filter by group
        let logs = store
            .query(None, Some("group-a"), None, None, None, None, 1, 100)
            .await
            .unwrap();
        assert_eq!(logs.len(), 3);

        // Filter by operator
        let logs = store
            .query(None, None, None, Some("user-0"), None, None, 1, 100)
            .await
            .unwrap();
        assert_eq!(logs.len(), 3);

        // Filter by time range
        let logs = store
            .query(None, None, None, None, Some(1001), Some(1003), 1, 100)
            .await
            .unwrap();
        assert_eq!(logs.len(), 3); // timestamps 1001, 1002, 1003
    }

    #[tokio::test]
    async fn test_in_memory_store_pagination() {
        let store = InMemoryAuditLogStore::new(100);

        for i in 0..10 {
            let entry = AuditLogEntry {
                id: format!("id-{}", i),
                timestamp: i as i64,
                pointcut: "publish".to_string(),
                execute_type: "after".to_string(),
                data_id: format!("data-{}", i),
                group: "group".to_string(),
                tenant: "public".to_string(),
                operator: "user".to_string(),
                client_ip: None,
                content_length: 0,
                metadata: HashMap::new(),
                success: true,
                reason: None,
            };
            store.append(entry).await.unwrap();
        }

        // Page 1 (newest first)
        let page1 = store
            .query(None, None, None, None, None, None, 1, 3)
            .await
            .unwrap();
        assert_eq!(page1.len(), 3);
        assert_eq!(page1[0].data_id, "data-9");
        assert_eq!(page1[2].data_id, "data-7");

        // Page 2
        let page2 = store
            .query(None, None, None, None, None, None, 2, 3)
            .await
            .unwrap();
        assert_eq!(page2.len(), 3);
        assert_eq!(page2[0].data_id, "data-6");
        assert_eq!(page2[2].data_id, "data-4");
    }

    #[tokio::test]
    async fn test_audit_plugin_name() {
        let plugin = AuditPlugin::with_memory_store(100);
        assert_eq!(plugin.name(), "audit");
    }

    #[tokio::test]
    async fn test_audit_plugin_order() {
        let plugin = AuditPlugin::with_memory_store(100);
        assert_eq!(plugin.order(), 100);
    }

    #[tokio::test]
    async fn test_audit_plugin_interested_pointcuts() {
        let plugin = AuditPlugin::with_memory_store(100);
        let pointcuts = plugin.interested_pointcuts();
        assert!(pointcuts.contains(&ConfigPointcut::Publish));
        assert!(pointcuts.contains(&ConfigPointcut::Remove));
        assert!(pointcuts.contains(&ConfigPointcut::Get));
        assert!(pointcuts.contains(&ConfigPointcut::Import));
        assert!(pointcuts.contains(&ConfigPointcut::Export));
        assert!(!pointcuts.contains(&ConfigPointcut::History));
    }

    #[tokio::test]
    async fn test_audit_plugin_interested_execute_types() {
        let plugin = AuditPlugin::with_memory_store(100);
        let types = plugin.interested_execute_types();
        assert!(types.contains(&ExecuteType::Before));
        assert!(types.contains(&ExecuteType::After));
    }

    #[test]
    fn test_audit_log_entry_from_request_success() {
        let mut req = make_request(ConfigPointcut::Publish, ExecuteType::After);
        req.proceed = true;

        let entry = AuditLogEntry::from_request(&req, true, None);
        assert!(entry.success);
        assert!(entry.reason.is_none());
        assert!(!entry.id.is_empty());
        assert!(entry.timestamp > 0);
    }

    #[test]
    fn test_audit_log_entry_from_request_denied() {
        let mut req = make_request(ConfigPointcut::Publish, ExecuteType::Before);
        req.proceed = false;
        req.deny_reason = Some("not allowed".to_string());

        let entry = AuditLogEntry::from_request(&req, false, Some("not allowed".to_string()));
        assert!(!entry.success);
        assert_eq!(entry.reason.as_deref(), Some("not allowed"));
    }

    struct BlockingStore {
        permits: Arc<tokio::sync::Semaphore>,
        appended: Arc<std::sync::atomic::AtomicUsize>,
    }

    #[async_trait::async_trait]
    impl AuditLogStore for BlockingStore {
        async fn append(&self, _entry: AuditLogEntry) -> anyhow::Result<()> {
            // The test grants permits only after it has verified the request
            // path is not blocked; until then the consumer is stalled.
            self.permits.acquire().await.unwrap().forget();
            self.appended
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok(())
        }

        async fn query(
            &self,
            _data_id: Option<&str>,
            _group: Option<&str>,
            _tenant: Option<&str>,
            _operator: Option<&str>,
            _start_time: Option<i64>,
            _end_time: Option<i64>,
            _page_no: u64,
            _page_size: u64,
        ) -> anyhow::Result<Vec<AuditLogEntry>> {
            Ok(Vec::new())
        }
    }

    struct FailingStore;

    #[async_trait::async_trait]
    impl AuditLogStore for FailingStore {
        async fn append(&self, _entry: AuditLogEntry) -> anyhow::Result<()> {
            anyhow::bail!("store unavailable")
        }

        async fn query(
            &self,
            _data_id: Option<&str>,
            _group: Option<&str>,
            _tenant: Option<&str>,
            _operator: Option<&str>,
            _start_time: Option<i64>,
            _end_time: Option<i64>,
            _page_no: u64,
            _page_size: u64,
        ) -> anyhow::Result<Vec<AuditLogEntry>> {
            Ok(Vec::new())
        }
    }

    async fn wait_for<F: Fn() -> bool>(ready: F) -> bool {
        for _ in 0..50 {
            if ready() {
                return true;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
        ready()
    }

    /// A stalled consumer must not stall config requests or grow the queue
    /// without bound: the excess entries are dropped and counted.
    #[tokio::test]
    async fn test_bounded_audit_queue_drops_when_consumer_is_slow() {
        let permits = Arc::new(tokio::sync::Semaphore::new(0));
        let appended = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let store = Arc::new(BlockingStore {
            permits: permits.clone(),
            appended: appended.clone(),
        });
        let plugin = AuditPlugin::new_with_capacity(store, 2);
        assert_eq!(plugin.queue_capacity(), 2);

        let mut req = make_request(ConfigPointcut::Publish, ExecuteType::After);
        let burst = 20u64;
        let completed = tokio::time::timeout(tokio::time::Duration::from_secs(2), async {
            for _ in 0..burst {
                plugin.execute(&mut req).await;
            }
        })
        .await;
        assert!(
            completed.is_ok(),
            "audit logging must never apply backpressure to config requests"
        );

        let stats = plugin.queue_stats();
        assert!(
            stats.dropped() > 0,
            "a bounded queue must drop entries under a stalled consumer"
        );
        assert_eq!(stats.enqueued() + stats.dropped(), burst);
        assert_eq!(stats.append_failed(), 0);
        assert!(
            stats.enqueued() <= plugin.queue_capacity() as u64 + 1,
            "at most capacity entries plus the one held by the consumer may be queued"
        );

        // Success path: once the store recovers, queued entries are appended.
        permits.add_permits(30);
        assert!(wait_for(|| appended.load(std::sync::atomic::Ordering::Relaxed) > 0).await);
        assert_eq!(stats.append_failed(), 0);
    }

    /// The normal path must count accepted entries and produce no drops or
    /// append failures.
    #[tokio::test]
    async fn test_audit_queue_success_path_counts_enqueued_entries() {
        let plugin = AuditPlugin::with_memory_store_and_capacity(100, 8);
        let store = plugin.store();
        let mut req = make_request(ConfigPointcut::Publish, ExecuteType::After);
        for _ in 0..3 {
            plugin.execute(&mut req).await;
        }
        let mut observed = 0;
        for _ in 0..50 {
            observed = store
                .query(None, None, None, None, None, None, 1, 100)
                .await
                .unwrap()
                .len();
            if observed == 3 {
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
        assert_eq!(observed, 3, "all accepted entries must reach the store");
        let stats = plugin.queue_stats();
        assert_eq!(stats.enqueued(), 3);
        assert_eq!(stats.dropped(), 0);
        assert_eq!(stats.append_failed(), 0);
    }

    /// Store failures are observable instead of being swallowed.
    #[tokio::test]
    async fn test_audit_append_failures_are_counted() {
        let plugin = AuditPlugin::new_with_capacity(Arc::new(FailingStore), 8);
        let mut req = make_request(ConfigPointcut::Remove, ExecuteType::After);
        plugin.execute(&mut req).await;
        let stats = plugin.queue_stats();
        assert!(wait_for(|| stats.append_failed() > 0).await);
        assert_eq!(stats.enqueued(), 1);
        assert_eq!(stats.dropped(), 0);
    }

    /// A stopped consumer is a drop, not a silent success.
    #[tokio::test]
    async fn test_closed_consumer_counts_as_dropped() {
        let (tx, rx) = mpsc::channel::<AuditLogEntry>(1);
        drop(rx);
        let plugin = AuditPlugin {
            store: Arc::new(InMemoryAuditLogStore::new(4)),
            log_tx: tx,
            capacity: 1,
            stats: Arc::new(AuditQueueStats::default()),
        };
        let mut req = make_request(ConfigPointcut::Publish, ExecuteType::After);
        plugin.execute(&mut req).await;
        let stats = plugin.queue_stats();
        assert_eq!(stats.dropped(), 1);
        assert_eq!(stats.enqueued(), 0);
    }
}
