//! Distributed Lock Service Implementation (ADV-002 to ADV-005)
//!
//! Provides:
//! - Lock acquire/release API (ADV-002)
//! - Lock renewal mechanism (ADV-003)
//! - Lock auto-release on timeout (ADV-004)
//! - Raft-based lock implementation (ADV-005)

use async_trait::async_trait;
use dashmap::DashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;
use tokio::time::interval;

use super::model::*;

/// Distributed Lock Service trait (ADV-002)
#[async_trait]
pub trait DistributedLockService: Send + Sync {
    /// Acquire a lock
    async fn acquire(&self, request: LockAcquireRequest) -> anyhow::Result<LockAcquireResult>;

    /// Release a lock
    async fn release(&self, request: LockReleaseRequest) -> anyhow::Result<LockReleaseResult>;

    /// Renew a lock (ADV-003)
    async fn renew(&self, request: LockRenewRequest) -> anyhow::Result<LockRenewResult>;

    /// Get a lock by key
    async fn get(&self, namespace: &str, name: &str) -> anyhow::Result<Option<DistributedLock>>;

    /// List locks
    async fn list(&self, request: LockQueryRequest) -> anyhow::Result<Vec<DistributedLock>>;

    /// Force release a lock (admin operation)
    async fn force_release(&self, namespace: &str, name: &str) -> anyhow::Result<bool>;

    /// Get lock statistics
    async fn get_stats(&self) -> LockStats;
}

/// In-memory lock service implementation
/// For production use with Raft, this would be replaced with RaftLockService
pub struct MemoryLockService {
    /// Lock storage
    locks: Arc<DashMap<String, DistributedLock>>,
    /// Waiting queue for locks (key -> list of waiters)
    waiters: Arc<DashMap<String, Vec<LockWaiter>>>,
    /// Statistics
    stats: Arc<LockStatsCollector>,
    /// Background task handle
    _cleanup_handle: Option<tokio::task::JoinHandle<()>>,
}

struct LockWaiter {
    owner: String,
    tx: mpsc::Sender<LockAcquireResult>,
    deadline: Instant,
    #[allow(dead_code)]
    request: LockAcquireRequest,
}

struct LockStatsCollector {
    total_locks: AtomicU64,
    total_acquisitions: AtomicU64,
    total_releases: AtomicU64,
    total_renewals: AtomicU64,
    expired_locks: AtomicU64,
    failed_acquisitions: AtomicU64,
    total_hold_time_ms: AtomicU64,
    completed_holds: AtomicU64,
}

impl Default for LockStatsCollector {
    fn default() -> Self {
        Self {
            total_locks: AtomicU64::new(0),
            total_acquisitions: AtomicU64::new(0),
            total_releases: AtomicU64::new(0),
            total_renewals: AtomicU64::new(0),
            expired_locks: AtomicU64::new(0),
            failed_acquisitions: AtomicU64::new(0),
            total_hold_time_ms: AtomicU64::new(0),
            completed_holds: AtomicU64::new(0),
        }
    }
}

impl MemoryLockService {
    pub fn new() -> Self {
        Self {
            locks: Arc::new(DashMap::new()),
            waiters: Arc::new(DashMap::new()),
            stats: Arc::new(LockStatsCollector::default()),
            _cleanup_handle: None,
        }
    }

    /// Start with background cleanup task (ADV-004)
    pub fn with_cleanup(self, interval_ms: u64) -> Self {
        let locks = self.locks.clone();
        let waiters = self.waiters.clone();
        let stats = self.stats.clone();

        let handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(interval_ms));
            loop {
                interval.tick().await;
                Self::cleanup_expired_locks(&locks, &waiters, &stats);
            }
        });

        Self {
            locks: self.locks,
            waiters: self.waiters,
            stats: self.stats,
            _cleanup_handle: Some(handle),
        }
    }

    fn cleanup_expired_locks(
        locks: &Arc<DashMap<String, DistributedLock>>,
        waiters: &Arc<DashMap<String, Vec<LockWaiter>>>,
        stats: &Arc<LockStatsCollector>,
    ) {
        let now = current_timestamp();
        let mut expired_keys = Vec::new();

        // Find expired locks
        for entry in locks.iter() {
            if let Some(expires_at) = entry.expires_at
                && now >= expires_at
                && entry.state == LockState::Locked
            {
                expired_keys.push(entry.key.clone());
            }
        }

        // Expire and process waiters
        for key in expired_keys {
            if let Some(mut lock) = locks.get_mut(&key) {
                lock.expire();
                stats.expired_locks.fetch_add(1, Ordering::Relaxed);

                // Try to give lock to first waiter
                if let Some(mut waiter_list) = waiters.get_mut(&key) {
                    while !waiter_list.is_empty() {
                        let waiter = waiter_list.remove(0);
                        if waiter.deadline > Instant::now() {
                            // Give lock to waiter
                            if lock.acquire(&waiter.owner) {
                                let result = LockAcquireResult {
                                    acquired: true,
                                    lock: Some(lock.clone()),
                                    fence_token: lock.fence_token,
                                    ..Default::default()
                                };
                                let _ = waiter.tx.try_send(result);
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    fn generate_key(namespace: &str, name: &str) -> String {
        format!("{}::{}", namespace, name)
    }
}

impl Default for MemoryLockService {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DistributedLockService for MemoryLockService {
    async fn acquire(&self, request: LockAcquireRequest) -> anyhow::Result<LockAcquireResult> {
        let key = Self::generate_key(&request.namespace, &request.name);

        // Try to acquire immediately
        let mut lock = self.locks.entry(key.clone()).or_insert_with(|| {
            self.stats.total_locks.fetch_add(1, Ordering::Relaxed);
            let mut lock = DistributedLock::new(&request.namespace, &request.name);
            lock.ttl_ms = request.ttl_ms;
            lock.auto_renew = request.auto_renew;
            lock.max_renewals = request.max_renewals;
            lock
        });

        // Check for expired lock
        if lock.is_expired() {
            lock.expire();
            self.stats.expired_locks.fetch_add(1, Ordering::Relaxed);
        }

        // Try to acquire
        if lock.acquire(&request.owner) {
            lock.owner_metadata = request.owner_metadata.clone();
            self.stats
                .total_acquisitions
                .fetch_add(1, Ordering::Relaxed);

            return Ok(LockAcquireResult {
                acquired: true,
                lock: Some(lock.clone()),
                fence_token: lock.fence_token,
                ..Default::default()
            });
        }

        // Lock is held by someone else
        let current_owner = lock.owner.clone();
        drop(lock);

        // If no wait time, return immediately
        if request.wait_ms == 0 {
            self.stats
                .failed_acquisitions
                .fetch_add(1, Ordering::Relaxed);
            return Ok(LockAcquireResult {
                acquired: false,
                current_owner,
                error: Some("Lock is held by another owner".to_string()),
                ..Default::default()
            });
        }

        // Wait for lock
        let (tx, mut rx) = mpsc::channel(1);
        let deadline = Instant::now() + Duration::from_millis(request.wait_ms);

        let waiter = LockWaiter {
            owner: request.owner.clone(),
            tx,
            deadline,
            request: request.clone(),
        };

        self.waiters.entry(key.clone()).or_default().push(waiter);

        // Wait with timeout
        let timeout = Duration::from_millis(request.wait_ms);
        match tokio::time::timeout(timeout, rx.recv()).await {
            Ok(Some(result)) => {
                if result.acquired {
                    self.stats
                        .total_acquisitions
                        .fetch_add(1, Ordering::Relaxed);
                } else {
                    self.stats
                        .failed_acquisitions
                        .fetch_add(1, Ordering::Relaxed);
                }
                Ok(result)
            }
            Ok(None) | Err(_) => {
                // Timeout - remove from waiters
                if let Some(mut waiters) = self.waiters.get_mut(&key) {
                    waiters.retain(|w| w.owner != request.owner);
                }
                self.stats
                    .failed_acquisitions
                    .fetch_add(1, Ordering::Relaxed);
                Ok(LockAcquireResult {
                    acquired: false,
                    current_owner,
                    error: Some("Lock acquisition timeout".to_string()),
                    ..Default::default()
                })
            }
        }
    }

    async fn release(&self, request: LockReleaseRequest) -> anyhow::Result<LockReleaseResult> {
        let key = Self::generate_key(&request.namespace, &request.name);

        let mut lock = match self.locks.get_mut(&key) {
            Some(lock) => lock,
            None => {
                return Ok(LockReleaseResult {
                    released: false,
                    error: Some("Lock not found".to_string()),
                });
            }
        };

        // Check fence token if provided
        if let Some(expected_token) = request.fence_token
            && lock.fence_token != expected_token
        {
            return Ok(LockReleaseResult {
                released: false,
                error: Some("Fence token mismatch".to_string()),
            });
        }

        // Calculate hold time
        if let Some(acquired_at) = lock.acquired_at {
            let hold_time = current_timestamp() - acquired_at;
            self.stats
                .total_hold_time_ms
                .fetch_add(hold_time as u64, Ordering::Relaxed);
            self.stats.completed_holds.fetch_add(1, Ordering::Relaxed);
        }

        if !lock.release(&request.owner) {
            return Ok(LockReleaseResult {
                released: false,
                error: Some("Not the lock owner".to_string()),
            });
        }

        self.stats.total_releases.fetch_add(1, Ordering::Relaxed);

        // Notify first waiter
        if let Some(mut waiters) = self.waiters.get_mut(&key) {
            while !waiters.is_empty() {
                let waiter = waiters.remove(0);
                if waiter.deadline > Instant::now() && lock.acquire(&waiter.owner) {
                    let result = LockAcquireResult {
                        acquired: true,
                        lock: Some(lock.clone()),
                        fence_token: lock.fence_token,
                        ..Default::default()
                    };
                    let _ = waiter.tx.try_send(result);
                    break;
                }
            }
        }

        Ok(LockReleaseResult {
            released: true,
            error: None,
        })
    }

    async fn renew(&self, request: LockRenewRequest) -> anyhow::Result<LockRenewResult> {
        let key = Self::generate_key(&request.namespace, &request.name);

        let mut lock = match self.locks.get_mut(&key) {
            Some(lock) => lock,
            None => {
                return Ok(LockRenewResult {
                    renewed: false,
                    expires_at: None,
                    renewal_count: 0,
                    error: Some("Lock not found".to_string()),
                });
            }
        };

        // Update TTL if provided
        if let Some(new_ttl) = request.ttl_ms {
            lock.ttl_ms = new_ttl;
        }

        if !lock.renew(&request.owner) {
            return Ok(LockRenewResult {
                renewed: false,
                expires_at: lock.expires_at,
                renewal_count: lock.renewal_count,
                error: Some("Cannot renew lock - not owner or max renewals reached".to_string()),
            });
        }

        self.stats.total_renewals.fetch_add(1, Ordering::Relaxed);

        Ok(LockRenewResult {
            renewed: true,
            expires_at: lock.expires_at,
            renewal_count: lock.renewal_count,
            error: None,
        })
    }

    async fn get(&self, namespace: &str, name: &str) -> anyhow::Result<Option<DistributedLock>> {
        let key = Self::generate_key(namespace, name);
        Ok(self.locks.get(&key).map(|l| l.clone()))
    }

    async fn list(&self, request: LockQueryRequest) -> anyhow::Result<Vec<DistributedLock>> {
        let mut results = Vec::new();

        for entry in self.locks.iter() {
            let lock = entry.value();

            // Filter by namespace
            if lock.namespace != request.namespace {
                continue;
            }

            // Filter by name if specified
            if let Some(ref name) = request.name
                && &lock.name != name
            {
                continue;
            }

            // Filter by owner if specified
            if let Some(ref owner) = request.owner
                && lock.owner.as_ref() != Some(owner)
            {
                continue;
            }

            // Filter by state if specified
            if let Some(ref state) = request.state
                && &lock.state != state
            {
                continue;
            }

            // Filter expired
            if !request.include_expired && lock.is_expired() {
                continue;
            }

            results.push(lock.clone());

            if results.len() >= request.limit {
                break;
            }
        }

        Ok(results)
    }

    async fn force_release(&self, namespace: &str, name: &str) -> anyhow::Result<bool> {
        let key = Self::generate_key(namespace, name);

        if let Some(mut lock) = self.locks.get_mut(&key) {
            lock.force_release();
            self.stats.total_releases.fetch_add(1, Ordering::Relaxed);
            return Ok(true);
        }

        Ok(false)
    }

    async fn get_stats(&self) -> LockStats {
        let active_locks = self.locks.iter().filter(|l| l.is_locked()).count() as u32;

        let completed = self.stats.completed_holds.load(Ordering::Relaxed);
        let total_hold = self.stats.total_hold_time_ms.load(Ordering::Relaxed);
        let avg_hold = if completed > 0 {
            total_hold / completed
        } else {
            0
        };

        LockStats {
            total_locks: self.stats.total_locks.load(Ordering::Relaxed),
            active_locks,
            total_acquisitions: self.stats.total_acquisitions.load(Ordering::Relaxed),
            total_releases: self.stats.total_releases.load(Ordering::Relaxed),
            total_renewals: self.stats.total_renewals.load(Ordering::Relaxed),
            expired_locks: self.stats.expired_locks.load(Ordering::Relaxed),
            failed_acquisitions: self.stats.failed_acquisitions.load(Ordering::Relaxed),
            avg_hold_time_ms: avg_hold,
        }
    }
}

/// Auto-renewal task for locks with auto_renew enabled (ADV-003)
pub struct AutoRenewalTask {
    service: Arc<dyn DistributedLockService>,
    namespace: String,
    name: String,
    owner: String,
    interval_ms: u64,
    stop_tx: mpsc::Sender<()>,
}

impl AutoRenewalTask {
    pub fn start(
        service: Arc<dyn DistributedLockService>,
        namespace: String,
        name: String,
        owner: String,
        ttl_ms: u64,
    ) -> (Self, mpsc::Receiver<()>) {
        let (stop_tx, stop_rx) = mpsc::channel(1);
        let interval_ms = ttl_ms / 3; // Renew at 1/3 of TTL

        let task = Self {
            service,
            namespace,
            name,
            owner,
            interval_ms,
            stop_tx,
        };

        (task, stop_rx)
    }

    pub async fn run(self, mut stop_rx: mpsc::Receiver<()>) {
        let mut interval = interval(Duration::from_millis(self.interval_ms));

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let request = LockRenewRequest {
                        namespace: self.namespace.clone(),
                        name: self.name.clone(),
                        owner: self.owner.clone(),
                        ttl_ms: None,
                    };

                    match self.service.renew(request).await {
                        Ok(result) => {
                            if !result.renewed {
                                tracing::warn!(
                                    "Lock renewal failed for {}::{}: {:?}",
                                    self.namespace,
                                    self.name,
                                    result.error
                                );
                                break;
                            }
                        }
                        Err(e) => {
                            tracing::error!(
                                "Lock renewal error for {}::{}: {}",
                                self.namespace,
                                self.name,
                                e
                            );
                            break;
                        }
                    }
                }
                _ = stop_rx.recv() => {
                    break;
                }
            }
        }
    }

    pub fn stop(&self) {
        let _ = self.stop_tx.try_send(());
    }
}

fn current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_lock_acquire_release() {
        let service = MemoryLockService::new();

        // Acquire lock
        let request = LockAcquireRequest {
            namespace: "test".to_string(),
            name: "my-lock".to_string(),
            owner: "client-1".to_string(),
            ttl_ms: 30000,
            ..Default::default()
        };

        let result = service.acquire(request).await.unwrap();
        assert!(result.acquired);
        assert!(result.lock.is_some());
        assert!(result.fence_token > 0);

        // Try to acquire same lock with different owner
        let request2 = LockAcquireRequest {
            namespace: "test".to_string(),
            name: "my-lock".to_string(),
            owner: "client-2".to_string(),
            ttl_ms: 30000,
            ..Default::default()
        };

        let result2 = service.acquire(request2).await.unwrap();
        assert!(!result2.acquired);
        assert_eq!(result2.current_owner, Some("client-1".to_string()));

        // Release lock
        let release = LockReleaseRequest {
            namespace: "test".to_string(),
            name: "my-lock".to_string(),
            owner: "client-1".to_string(),
            fence_token: None,
        };

        let release_result = service.release(release).await.unwrap();
        assert!(release_result.released);

        // Now client-2 can acquire
        let request3 = LockAcquireRequest {
            namespace: "test".to_string(),
            name: "my-lock".to_string(),
            owner: "client-2".to_string(),
            ttl_ms: 30000,
            ..Default::default()
        };

        let result3 = service.acquire(request3).await.unwrap();
        assert!(result3.acquired);
    }

    #[tokio::test]
    async fn test_lock_renewal() {
        let service = MemoryLockService::new();

        // Acquire lock
        let request = LockAcquireRequest {
            namespace: "test".to_string(),
            name: "renew-lock".to_string(),
            owner: "client-1".to_string(),
            ttl_ms: 1000,
            max_renewals: 2,
            ..Default::default()
        };

        let result = service.acquire(request).await.unwrap();
        assert!(result.acquired);

        // Renew lock
        let renew_request = LockRenewRequest {
            namespace: "test".to_string(),
            name: "renew-lock".to_string(),
            owner: "client-1".to_string(),
            ttl_ms: None,
        };

        let renew_result = service.renew(renew_request.clone()).await.unwrap();
        assert!(renew_result.renewed);
        assert_eq!(renew_result.renewal_count, 1);

        // Second renewal
        let renew_result2 = service.renew(renew_request.clone()).await.unwrap();
        assert!(renew_result2.renewed);
        assert_eq!(renew_result2.renewal_count, 2);

        // Third renewal should fail (max_renewals = 2)
        let renew_result3 = service.renew(renew_request).await.unwrap();
        assert!(!renew_result3.renewed);
    }

    #[tokio::test]
    async fn test_fence_token() {
        let service = MemoryLockService::new();

        // Acquire lock
        let request = LockAcquireRequest {
            namespace: "test".to_string(),
            name: "fence-lock".to_string(),
            owner: "client-1".to_string(),
            ttl_ms: 30000,
            ..Default::default()
        };

        let result = service.acquire(request).await.unwrap();
        let token = result.fence_token;

        // Try to release with wrong fence token
        let release_wrong = LockReleaseRequest {
            namespace: "test".to_string(),
            name: "fence-lock".to_string(),
            owner: "client-1".to_string(),
            fence_token: Some(token + 1), // Wrong token
        };

        let release_result = service.release(release_wrong).await.unwrap();
        assert!(!release_result.released);

        // Release with correct fence token
        let release_correct = LockReleaseRequest {
            namespace: "test".to_string(),
            name: "fence-lock".to_string(),
            owner: "client-1".to_string(),
            fence_token: Some(token),
        };

        let release_result2 = service.release(release_correct).await.unwrap();
        assert!(release_result2.released);
    }

    #[tokio::test]
    async fn test_stats() {
        let service = MemoryLockService::new();

        // Acquire and release a few locks
        for i in 0..5 {
            let request = LockAcquireRequest {
                namespace: "test".to_string(),
                name: format!("lock-{}", i),
                owner: "client".to_string(),
                ttl_ms: 30000,
                ..Default::default()
            };
            service.acquire(request).await.unwrap();
        }

        let stats = service.get_stats().await;
        assert_eq!(stats.total_locks, 5);
        assert_eq!(stats.total_acquisitions, 5);
        assert_eq!(stats.active_locks, 5);
    }
}
