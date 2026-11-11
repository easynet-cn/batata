//! Distributed lock service
//!
//! Provides distributed lock operations via gRPC,
//! supporting acquire, release and renew with timeout, plus a client-side
//! renewal watchdog that keeps acquired locks alive until they are released.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use dashmap::DashMap;
use dashmap::mapref::entry::Entry;
use tokio::sync::oneshot;
use tracing::{debug, info};

use batata_api::remote::model::{LockInstance, LockOperationRequest, LockOperationResponse};

use crate::error::Result;
use crate::grpc::GrpcClient;

const LOCK_OP_ACQUIRE: &str = "ACQUIRE";
const LOCK_OP_RELEASE: &str = "RELEASE";
const LOCK_OP_RENEW: &str = "RENEW";

/// Watchdog default TTL when `expiredTime <= 0` (GA uses 30000 ms).
const DEFAULT_LOCK_TTL_MS: u64 = 30_000;
/// Upper bound of the fixed renewal delay (GA `DEFAULT_RENEW_INTERVAL_MS`).
const RENEW_MAX_INTERVAL_MS: u64 = 10_000;
/// Lower bound of the fixed renewal delay.
const RENEW_MIN_INTERVAL_MS: u64 = 1_000;

/// Boxed future returned by a [`RenewFactory`].
type BoxedRenewFuture = Pin<Box<dyn Future<Output = Result<bool>> + Send>>;
/// A clonable handle that issues exactly one RENEW request for a lock.
///
/// Capturing the transport + owner here lets the watchdog run without holding
/// an `Arc<BatataLockService>` (which would form an Arc reference cycle). It
/// is also the seam used by unit tests to inject renew outcomes.
type RenewFactory =
    Arc<dyn Fn(String, i64, String, HashMap<String, String>) -> BoxedRenewFuture + Send + Sync>;

/// Build a `LockOperationRequest` for any lock operation, always filling the
/// stable `owner`. This is the single source of truth for the lock wire format
/// shared by `lock`/`unlock`/`renew` and the watchdog.
fn build_lock_request(
    lock_operation: &str,
    key: &str,
    expired_time: i64,
    lock_type: &str,
    params: HashMap<String, String>,
    owner: &str,
) -> LockOperationRequest {
    let mut req = LockOperationRequest::default();
    req.request.request_id = uuid::Uuid::new_v4().to_string();
    req.module = "lock".to_string();
    req.lock_operation = lock_operation.to_string();
    req.lock_instance = Some(LockInstance {
        key: key.to_string(),
        expired_time,
        lock_type: lock_type.to_string(),
        params,
        owner: Some(owner.to_string()),
    });
    req
}

/// Build the production [`RenewFactory`]: constructs a RENEW request and sends
/// it through the gRPC client. `Ok(true)` means the lock is still held.
fn default_renew_factory(grpc_client: Arc<GrpcClient>, owner: String) -> RenewFactory {
    Arc::new(
        move |key: String,
              expired_time: i64,
              lock_type: String,
              params: HashMap<String, String>|
              -> BoxedRenewFuture {
            let grpc_client = grpc_client.clone();
            let owner = owner.clone();
            Box::pin(async move {
                let req = build_lock_request(
                    LOCK_OP_RENEW,
                    &key,
                    expired_time,
                    &lock_type,
                    params,
                    &owner,
                );
                let resp: LockOperationResponse = grpc_client.request_typed(&req).await?;
                Ok(resp.result)
            })
        },
    )
}

/// Distributed lock service backed by gRPC.
pub struct BatataLockService {
    grpc_client: Arc<GrpcClient>,
    /// Process-unique id (uuid v4) used to derive the lock owner.
    client_id: String,
    /// Stable lock owner (`clientId:threadId`). Cached once: tokio may migrate
    /// a task across worker threads, so recomputing the thread id per call
    /// would make lock/unlock/renew disagree on the owner and break the
    /// server-side owner check (and the watchdog).
    owner: String,
    /// Renewal watchdog for locks acquired through this service.
    watchdog: LockWatchdog,
}

impl BatataLockService {
    /// Create a new lock service with the given gRPC client.
    pub fn new(grpc_client: Arc<GrpcClient>) -> Self {
        let client_id = uuid::Uuid::new_v4().to_string();
        let owner = format!("{}:{:?}", client_id, std::thread::current().id());
        let watchdog = LockWatchdog::new(default_renew_factory(grpc_client.clone(), owner.clone()));
        Self {
            grpc_client,
            client_id,
            owner,
            watchdog,
        }
    }

    /// Process-unique client id (uuid v4) backing the lock owner.
    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    /// Stable lock owner (`clientId:threadId`) used by every operation.
    pub fn owner(&self) -> &str {
        &self.owner
    }

    /// Clonable handle to this service's renewal watchdog.
    pub fn watchdog(&self) -> LockWatchdog {
        self.watchdog.clone()
    }

    /// Stop every renewal task (also done automatically on drop).
    pub fn shutdown(&self) {
        self.watchdog.shutdown();
    }

    /// Acquire a distributed lock.
    ///
    /// # Arguments
    /// * `key` - Lock key to acquire
    /// * `expired_time` - Lock expiration time in milliseconds
    ///
    /// # Returns
    /// * `true` if the lock was acquired, `false` if it's already held
    pub async fn lock(&self, key: &str, expired_time: i64) -> Result<bool> {
        self.lock_with_params(key, expired_time, "mutex", Default::default())
            .await
    }

    /// Acquire a distributed lock with custom parameters.
    pub async fn lock_with_params(
        &self,
        key: &str,
        expired_time: i64,
        lock_type: &str,
        params: HashMap<String, String>,
    ) -> Result<bool> {
        let req = build_lock_request(
            LOCK_OP_ACQUIRE,
            key,
            expired_time,
            lock_type,
            params.clone(),
            &self.owner,
        );
        let resp: LockOperationResponse = self.grpc_client.request_typed(&req).await?;

        debug!("Lock acquire key={}, result={}", key, resp.result);

        if resp.result {
            // Register the watchdog only on a successful acquire. register is
            // idempotent, so reentrant acquires never spawn a second task.
            self.watchdog
                .register_full(key, expired_time, lock_type, params);
        }

        Ok(resp.result)
    }

    /// Release a distributed lock.
    ///
    /// The watchdog is unregistered *before* the RELEASE request is sent, so a
    /// late RENEW cannot race the release. `expired_time` is always 0 here
    /// (RELEASE, never RENEW).
    ///
    /// # Arguments
    /// * `key` - Lock key to release
    ///
    /// # Returns
    /// * `true` if the lock was released, `false` if it wasn't held
    pub async fn unlock(&self, key: &str) -> Result<bool> {
        // Unregister first: avoid a late RENEW resurrecting a released lock.
        self.watchdog.unregister(key);

        let req = build_lock_request(LOCK_OP_RELEASE, key, 0, "", Default::default(), &self.owner);
        let resp: LockOperationResponse = self.grpc_client.request_typed(&req).await?;

        debug!("Lock release key={}, result={}", key, resp.result);

        Ok(resp.result)
    }

    /// Renew a distributed lock (`lock_operation = RENEW`).
    ///
    /// `expired_time` is a duration in milliseconds. Returns the server's raw
    /// `result` (`true` while the lock is still held).
    pub async fn renew(
        &self,
        key: &str,
        expired_time: i64,
        lock_type: &str,
        params: HashMap<String, String>,
    ) -> Result<bool> {
        let req = build_lock_request(
            LOCK_OP_RENEW,
            key,
            expired_time,
            lock_type,
            params,
            &self.owner,
        );
        let resp: LockOperationResponse = self.grpc_client.request_typed(&req).await?;

        debug!("Lock renew key={}, result={}", key, resp.result);

        Ok(resp.result)
    }

    /// Full-form renew alias (mirrors [`BatataLockService::renew`]).
    pub async fn renew_with_params(
        &self,
        key: &str,
        expired_time: i64,
        lock_type: &str,
        params: HashMap<String, String>,
    ) -> Result<bool> {
        self.renew(key, expired_time, lock_type, params).await
    }

    /// Convenient renew using the default `mutex` lock type and empty params.
    pub async fn renew_default(&self, key: &str, expired_time: i64) -> Result<bool> {
        self.renew(key, expired_time, "mutex", Default::default())
            .await
    }

    /// Try to acquire a lock, returning immediately if not available.
    /// Same as `lock()` but semantically indicates a non-blocking try.
    pub async fn try_lock(&self, key: &str, expired_time: i64) -> Result<bool> {
        self.lock(key, expired_time).await
    }

    /// Acquire a lock and return a guard that auto-releases on drop.
    pub async fn lock_guard(
        self: &Arc<Self>,
        key: &str,
        expired_time: i64,
    ) -> Result<Option<LockGuard>> {
        let acquired = self.lock(key, expired_time).await?;
        if acquired {
            Ok(Some(LockGuard::new(key.to_string(), self.clone())))
        } else {
            Ok(None)
        }
    }
}

impl Drop for BatataLockService {
    fn drop(&mut self) {
        // Cancel every renewal task even if the caller never called
        // shutdown() explicitly (otherwise stray tasks would hold the
        // transport alive forever).
        self.watchdog.shutdown();
    }
}

/// One active renewal task's bookkeeping.
struct Registration {
    /// Guards `remove_if` so a stale task cannot clobber a newer registration.
    generation: u64,
    /// Dropping this sender cancels the task's `oneshot::Receiver`.
    cancel: oneshot::Sender<()>,
}

struct WatchdogInner {
    renew: RenewFactory,
    registrations: DashMap<String, Registration>,
    next_generation: AtomicU64,
}

/// Client-side renewal watchdog.
///
/// A clonable (Arc-backed) handle. Each registered key owns one tokio task
/// that periodically issues a RENEW; the task stops on `unregister`,
/// `shutdown`, or a `false`/`Err` renew outcome. Registration is idempotent
/// (one task per key) and all tasks are cancelled when the owning
/// [`BatataLockService`] is dropped.
#[derive(Clone)]
pub struct LockWatchdog {
    inner: Arc<WatchdogInner>,
}

impl LockWatchdog {
    fn new(renew: RenewFactory) -> Self {
        Self {
            inner: Arc::new(WatchdogInner {
                renew,
                registrations: DashMap::new(),
                next_generation: AtomicU64::new(1),
            }),
        }
    }

    /// Start renewing `key` on the default `mutex` schedule.
    /// Idempotent: a second call for an already-registered key is a no-op.
    pub fn register(&self, key: &str, expired_time: i64) {
        self.register_full(key, expired_time, "mutex", HashMap::new());
    }

    fn register_full(
        &self,
        key: &str,
        expired_time: i64,
        lock_type: &str,
        params: HashMap<String, String>,
    ) {
        self.spawn_registration(
            key,
            expired_time,
            lock_type,
            params,
            renew_interval_ms(expired_time),
        );
    }

    /// Spawn a single renewal task. `interval_ms` is explicit so tests can use
    /// a short delay while production uses [`renew_interval_ms`].
    fn spawn_registration(
        &self,
        key: &str,
        expired_time: i64,
        lock_type: &str,
        params: HashMap<String, String>,
        interval_ms: u64,
    ) {
        // Atomic dedupe: the entry guard is held while inserting, so two
        // concurrent registers cannot both spawn a task for the same key.
        let entry = match self.inner.registrations.entry(key.to_string()) {
            Entry::Occupied(_) => return,
            Entry::Vacant(vacant) => vacant,
        };
        let generation = self.inner.next_generation.fetch_add(1, Ordering::Relaxed);
        let (cancel, rx) = oneshot::channel();
        entry.insert(Registration { generation, cancel });

        let inner = self.inner.clone();
        let key_owned = key.to_string();
        let lock_type_owned = lock_type.to_string();
        tokio::spawn(async move {
            watchdog_loop(
                inner,
                key_owned,
                expired_time,
                lock_type_owned,
                params,
                generation,
                interval_ms,
                Box::pin(rx),
            )
            .await;
        });
    }

    /// Stop renewing `key` (idempotent). Signals the task explicitly so it
    /// exits promptly (dropping the sender would also cancel it).
    pub fn unregister(&self, key: &str) {
        if let Some((_, reg)) = self.inner.registrations.remove(key) {
            let _ = reg.cancel.send(());
        }
    }

    /// Stop all renewal tasks (idempotent).
    pub fn shutdown(&self) {
        self.inner.registrations.clear();
    }

    /// Whether `key` currently has an active renewal task.
    pub fn is_registered(&self, key: &str) -> bool {
        self.inner.registrations.contains_key(key)
    }

    /// Number of active renewal tasks.
    pub fn registration_count(&self) -> usize {
        self.inner.registrations.len()
    }
}

/// Compute the fixed renewal delay: `max(1000, min(ttl / 3, 10000))`, with
/// `ttl = expired_time > 0 ? expired_time : 30000`.
fn renew_interval_ms(expired_time: i64) -> u64 {
    let ttl = if expired_time > 0 {
        expired_time as u64
    } else {
        DEFAULT_LOCK_TTL_MS
    };
    (ttl / 3).clamp(RENEW_MIN_INTERVAL_MS, RENEW_MAX_INTERVAL_MS)
}

/// The renewal loop, mirroring GA `NacosLockWatchdog.scheduleWithFixedDelay`:
/// wait, renew, repeat. On `false`/`Err` it unregisters itself and stops.
async fn watchdog_loop(
    inner: Arc<WatchdogInner>,
    key: String,
    expired_time: i64,
    lock_type: String,
    params: HashMap<String, String>,
    generation: u64,
    interval_ms: u64,
    mut cancel: Pin<Box<oneshot::Receiver<()>>>,
) {
    loop {
        // `biased` gives a pending unregister priority, so no NEW renew is
        // started after the lock has been released.
        tokio::select! {
            biased;
            _ = &mut cancel => break,
            _ = tokio::time::sleep(Duration::from_millis(interval_ms)) => {}
        }
        let outcome =
            (inner.renew)(key.clone(), expired_time, lock_type.clone(), params.clone()).await;
        if !matches!(outcome, Ok(true)) {
            // Lock lost (or request failed): unregister only our own
            // generation, then stop the task.
            inner
                .registrations
                .remove_if(&key, |_, r| r.generation == generation);
            break;
        }
    }
}

/// RAII lock guard that automatically releases the lock when dropped
pub struct LockGuard {
    key: String,
    lock_service: Arc<BatataLockService>,
    released: bool,
}

impl LockGuard {
    fn new(key: String, lock_service: Arc<BatataLockService>) -> Self {
        Self {
            key,
            lock_service,
            released: false,
        }
    }

    /// Explicitly release the lock
    pub async fn release(&mut self) -> Result<bool> {
        if !self.released {
            self.released = true;
            self.lock_service.unlock(&self.key).await
        } else {
            Ok(true)
        }
    }

    /// Get the lock key
    pub fn key(&self) -> &str {
        &self.key
    }
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        if !self.released {
            let key = self.key.clone();
            let service = self.lock_service.clone();
            tokio::spawn(async move {
                if let Err(e) = service.unlock(&key).await {
                    tracing::error!("Failed to release lock {} on drop: {}", key, e);
                } else {
                    info!("Auto-released lock {} on drop", key);
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    use crate::error::ClientError;
    use crate::grpc::GrpcClientConfig;

    fn test_service() -> BatataLockService {
        let config = GrpcClientConfig::default();
        let client = Arc::new(GrpcClient::new(config).unwrap());
        BatataLockService::new(client)
    }

    /// A factory that always returns the given boolean.
    fn bool_factory(value: bool) -> RenewFactory {
        Arc::new(
            move |_key: String,
                  _ttl: i64,
                  _lt: String,
                  _params: HashMap<String, String>|
                  -> BoxedRenewFuture { Box::pin(async move { Ok(value) }) },
        )
    }

    fn err_factory() -> RenewFactory {
        Arc::new(
            move |_key: String,
                  _ttl: i64,
                  _lt: String,
                  _params: HashMap<String, String>|
                  -> BoxedRenewFuture {
                Box::pin(async move { Err(ClientError::NotConnected) })
            },
        )
    }

    #[test]
    fn test_lock_constants() {
        assert_eq!(LOCK_OP_ACQUIRE, "ACQUIRE");
        assert_eq!(LOCK_OP_RELEASE, "RELEASE");
        assert_eq!(LOCK_OP_RENEW, "RENEW");
    }

    #[tokio::test]
    async fn test_lock_guard_key() {
        let lock_service = Arc::new(test_service());
        let guard = LockGuard::new("test-key".to_string(), lock_service);
        assert_eq!(guard.key(), "test-key");
    }

    #[tokio::test]
    async fn test_lock_guard_released_flag() {
        let lock_service = Arc::new(test_service());
        let guard = LockGuard::new("test-key".to_string(), lock_service);
        assert!(!guard.released);
    }

    #[test]
    fn test_lock_operation_request_acquire() {
        let req = LockOperationRequest {
            module: "lock".to_string(),
            lock_operation: LOCK_OP_ACQUIRE.to_string(),
            lock_instance: Some(LockInstance {
                key: "my-lock".to_string(),
                expired_time: 30000,
                lock_type: "mutex".to_string(),
                params: Default::default(),
                owner: None,
            }),
            ..Default::default()
        };
        assert_eq!(req.lock_operation, "ACQUIRE");
        assert_eq!(req.module, "lock");
        let instance = req.lock_instance.unwrap();
        assert_eq!(instance.key, "my-lock");
        assert_eq!(instance.expired_time, 30000);
        assert_eq!(instance.lock_type, "mutex");
    }

    #[test]
    fn test_lock_operation_request_release() {
        let req = LockOperationRequest {
            module: "lock".to_string(),
            lock_operation: LOCK_OP_RELEASE.to_string(),
            lock_instance: Some(LockInstance {
                key: "my-lock".to_string(),
                expired_time: 0,
                lock_type: String::new(),
                params: Default::default(),
                owner: None,
            }),
            ..Default::default()
        };
        assert_eq!(req.lock_operation, "RELEASE");
        let instance = req.lock_instance.unwrap();
        assert_eq!(instance.key, "my-lock");
        assert_eq!(instance.expired_time, 0);
    }

    #[test]
    fn test_lock_with_custom_params() {
        let mut params = HashMap::new();
        params.insert("owner".to_string(), "client-1".to_string());
        params.insert("priority".to_string(), "high".to_string());

        let instance = LockInstance {
            key: "resource-lock".to_string(),
            expired_time: 60000,
            lock_type: "reentrant".to_string(),
            params: params.clone(),
            owner: None,
        };

        assert_eq!(instance.params.get("owner").unwrap(), "client-1");
        assert_eq!(instance.params.get("priority").unwrap(), "high");
        assert_eq!(instance.lock_type, "reentrant");
    }

    #[test]
    fn test_renew_request_wire() {
        let req = build_lock_request(
            LOCK_OP_RENEW,
            "my-lock",
            30000,
            "mutex",
            HashMap::new(),
            "client-1:ThreadId(1)",
        );
        assert_eq!(req.lock_operation, "RENEW");
        assert_eq!(req.module, "lock");
        let inst = req.lock_instance.as_ref().unwrap();
        assert_eq!(inst.key, "my-lock");
        assert_eq!(inst.expired_time, 30000);
        assert_eq!(inst.owner.as_deref(), Some("client-1:ThreadId(1)"));
        // camelCase wire must carry the RENEW op and the owner.
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"lockOperation\":\"RENEW\""), "{json}");
        assert!(
            json.contains("\"owner\":\"client-1:ThreadId(1)\""),
            "{json}"
        );
    }

    #[test]
    fn test_unlock_request_wire_is_release_with_zero_ttl() {
        // unlock() always sends RELEASE with expired_time 0; it must never be
        // mistaken for a RENEW (whose expired_time 0 is rejected server-side).
        let req = build_lock_request(LOCK_OP_RELEASE, "my-lock", 0, "", HashMap::new(), "owner");
        assert_eq!(req.lock_operation, "RELEASE");
        assert_ne!(req.lock_operation, LOCK_OP_RENEW);
        assert_eq!(req.lock_instance.as_ref().unwrap().expired_time, 0);
        assert_eq!(
            req.lock_instance.as_ref().unwrap().owner.as_deref(),
            Some("owner")
        );
    }

    #[test]
    fn test_owner_is_stable_and_process_unique() {
        let svc = test_service();
        assert!(!svc.client_id().is_empty());
        let owner = svc.owner().to_string();
        assert!(owner.starts_with(svc.client_id()));
        assert!(owner.contains(':'), "owner should be clientId:threadId");
        // A second service has a different owner (uuid v4 client id).
        let svc2 = test_service();
        assert_ne!(svc.owner(), svc2.owner());
    }

    #[test]
    fn test_renew_interval_ms() {
        assert_eq!(renew_interval_ms(30000), 10000); // min(10000, ...) capped
        assert_eq!(renew_interval_ms(60000), 10000);
        assert_eq!(renew_interval_ms(100000), 10000);
        assert_eq!(renew_interval_ms(3000), 1000); // ttl/3
        assert_eq!(renew_interval_ms(900), 1000); // floored at 1000
        assert_eq!(renew_interval_ms(0), 10000); // default ttl 30000
        assert_eq!(renew_interval_ms(-1), 10000);
    }

    #[tokio::test]
    async fn test_watchdog_register_dedupe_and_unregister() {
        let calls = Arc::new(AtomicUsize::new(0));
        let c = calls.clone();
        let factory: RenewFactory = Arc::new(
            move |_key: String,
                  _ttl: i64,
                  _lt: String,
                  _params: HashMap<String, String>|
                  -> BoxedRenewFuture {
                c.fetch_add(1, Ordering::Relaxed);
                Box::pin(async move { Ok(true) })
            },
        );
        let wd = LockWatchdog::new(factory);

        // Short interval keeps the test fast.
        wd.spawn_registration("k", 30000, "mutex", HashMap::new(), 10);
        assert!(wd.is_registered("k"));
        assert_eq!(wd.registration_count(), 1);
        // A duplicate register (public API) must NOT spawn a second task.
        wd.register("k", 30000);
        assert_eq!(wd.registration_count(), 1);

        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(
            calls.load(Ordering::Relaxed) >= 2,
            "watchdog must renew periodically"
        );

        wd.unregister("k");
        assert!(!wd.is_registered("k"));
        assert_eq!(wd.registration_count(), 0);
        let before = calls.load(Ordering::Relaxed);
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(
            calls.load(Ordering::Relaxed),
            before,
            "no renew after unregister"
        );
        wd.shutdown();
    }

    #[tokio::test]
    async fn test_watchdog_false_unregisters() {
        let wd = LockWatchdog::new(bool_factory(false));
        wd.spawn_registration("k", 30000, "mutex", HashMap::new(), 10);
        assert!(wd.is_registered("k"));
        tokio::time::sleep(Duration::from_millis(60)).await;
        assert!(
            !wd.is_registered("k"),
            "a false renew must unregister and stop"
        );
        assert_eq!(wd.registration_count(), 0);
        wd.shutdown();
    }

    #[tokio::test]
    async fn test_watchdog_err_unregisters() {
        let wd = LockWatchdog::new(err_factory());
        wd.spawn_registration("k", 30000, "mutex", HashMap::new(), 10);
        assert!(wd.is_registered("k"));
        tokio::time::sleep(Duration::from_millis(60)).await;
        assert!(
            !wd.is_registered("k"),
            "an Err renew must unregister and stop"
        );
        wd.shutdown();
    }

    #[tokio::test]
    async fn test_watchdog_shutdown() {
        let wd = LockWatchdog::new(bool_factory(true));
        wd.spawn_registration("a", 30000, "mutex", HashMap::new(), 10);
        wd.spawn_registration("b", 30000, "mutex", HashMap::new(), 10);
        assert_eq!(wd.registration_count(), 2);
        wd.shutdown();
        assert_eq!(wd.registration_count(), 0);
        assert!(!wd.is_registered("a"));
        assert!(!wd.is_registered("b"));
    }

    #[tokio::test]
    async fn test_unlock_unregisters_before_release() {
        let svc = test_service();
        // Simulate a lock() success having registered a watchdog task.
        svc.watchdog().register("k", 30000);
        assert!(svc.watchdog().is_registered("k"));
        // No connection -> unlock() returns Err, but it must unregister first.
        let result = svc.unlock("k").await;
        assert!(result.is_err());
        assert!(
            !svc.watchdog().is_registered("k"),
            "unlock must unregister before releasing"
        );
    }

    #[tokio::test]
    async fn test_lock_failure_does_not_register_watchdog() {
        let svc = test_service();
        let result = svc.lock("k", 30000).await;
        assert!(result.is_err()); // no connection
        assert!(
            !svc.watchdog().is_registered("k"),
            "a failed acquire must not register the watchdog"
        );
    }

    #[tokio::test]
    async fn test_renew_methods_callable_without_connection() {
        let svc = test_service();
        assert!(
            svc.renew("k", 30000, "mutex", HashMap::new())
                .await
                .is_err()
        );
        assert!(
            svc.renew_with_params("k", 30000, "mutex", HashMap::new())
                .await
                .is_err()
        );
        assert!(svc.renew_default("k", 30000).await.is_err());
    }
}
