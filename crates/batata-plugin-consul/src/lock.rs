//! Consul Lock and Semaphore implementation
//!
//! Provides distributed locking primitives compatible with Consul's Lock and Semaphore APIs.
//! Uses KV store + Sessions for coordination.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use actix_web::{HttpRequest, HttpResponse, web};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::acl::{AclService, ResourceType};
use crate::kv::ConsulKVService;
use crate::model::ConsulError;
use crate::session::ConsulSessionService;

// ============================================================================
// Lock Models
// ============================================================================

/// Lock options for acquiring a distributed lock
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockOptions {
    /// Key prefix for the lock (e.g., "service/leader")
    #[serde(rename = "Key")]
    pub key: String,

    /// Session TTL (e.g., "15s")
    #[serde(rename = "SessionTTL", default)]
    pub session_ttl: Option<String>,

    /// Lock delay (e.g., "15s") - time before a released lock can be re-acquired
    #[serde(rename = "LockDelay", default)]
    pub lock_delay: Option<String>,

    /// Value to store with the lock
    #[serde(rename = "Value", default)]
    pub value: Option<String>,

    /// Lock wait time (e.g., "30s") - max time to wait for lock acquisition
    #[serde(rename = "LockWaitTime", default)]
    pub lock_wait_time: Option<String>,

    /// Whether to enable lock monitoring (auto-renewal)
    #[serde(rename = "MonitorRetries", default)]
    pub monitor_retries: Option<u32>,

    /// Retry interval for monitoring
    #[serde(rename = "MonitorRetryTime", default)]
    pub monitor_retry_time: Option<String>,
}

impl Default for LockOptions {
    fn default() -> Self {
        Self {
            key: String::new(),
            session_ttl: Some("15s".to_string()),
            lock_delay: Some("15s".to_string()),
            value: None,
            lock_wait_time: Some("0s".to_string()), // No wait by default
            monitor_retries: Some(3),
            monitor_retry_time: Some("2s".to_string()),
        }
    }
}

/// Lock state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lock {
    /// Lock key
    #[serde(rename = "Key")]
    pub key: String,

    /// Session ID holding the lock
    #[serde(rename = "Session")]
    pub session: String,

    /// Value stored with the lock
    #[serde(rename = "Value", skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,

    /// Lock index (modify index of the KV entry)
    #[serde(rename = "LockIndex")]
    pub lock_index: u64,

    /// Whether the lock is held
    #[serde(rename = "IsHeld")]
    pub is_held: bool,
}

/// Lock acquisition result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockAcquireResult {
    /// Whether lock was acquired
    #[serde(rename = "Acquired")]
    pub acquired: bool,

    /// Lock details if acquired
    #[serde(rename = "Lock", skip_serializing_if = "Option::is_none")]
    pub lock: Option<Lock>,

    /// Session ID created for this lock
    #[serde(rename = "Session", skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
}

// ============================================================================
// Semaphore Models
// ============================================================================

/// Semaphore options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemaphoreOptions {
    /// Key prefix for the semaphore
    #[serde(rename = "Prefix")]
    pub prefix: String,

    /// Maximum number of concurrent holders
    #[serde(rename = "Limit")]
    pub limit: u32,

    /// Session TTL
    #[serde(rename = "SessionTTL", default)]
    pub session_ttl: Option<String>,

    /// Value to store
    #[serde(rename = "Value", default)]
    pub value: Option<String>,

    /// Wait time for acquisition
    #[serde(rename = "SemaphoreWaitTime", default)]
    pub semaphore_wait_time: Option<String>,
}

impl Default for SemaphoreOptions {
    fn default() -> Self {
        Self {
            prefix: String::new(),
            limit: 1,
            session_ttl: Some("15s".to_string()),
            value: None,
            semaphore_wait_time: Some("0s".to_string()),
        }
    }
}

/// Semaphore state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Semaphore {
    /// Semaphore prefix
    #[serde(rename = "Prefix")]
    pub prefix: String,

    /// Limit
    #[serde(rename = "Limit")]
    pub limit: u32,

    /// Current holders
    #[serde(rename = "Holders")]
    pub holders: Vec<SemaphoreHolder>,

    /// Whether this client holds a slot
    #[serde(rename = "IsHeld")]
    pub is_held: bool,

    /// This client's session ID if holding
    #[serde(rename = "Session", skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
}

/// Semaphore holder info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemaphoreHolder {
    /// Session ID of holder
    #[serde(rename = "Session")]
    pub session: String,

    /// Value stored by holder
    #[serde(rename = "Value", skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

/// Semaphore contender entry stored in KV
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SemaphoreContender {
    session: String,
    value: Option<String>,
    acquired_at: u64,
}

/// Semaphore lock entry stored at .lock key
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SemaphoreLockEntry {
    limit: u32,
    holders: HashMap<String, u64>, // session -> acquire_time
}

// ============================================================================
// Lock Service
// ============================================================================

/// Active lock tracking
#[allow(dead_code)]
struct ActiveLock {
    key: String,
    session: String,
    acquired_at: Instant,
    last_renewed: Instant,
}

/// Consul Lock Service
/// Provides distributed locking using KV + Sessions
#[derive(Clone)]
pub struct ConsulLockService {
    kv_service: Arc<ConsulKVService>,
    session_service: Arc<ConsulSessionService>,
    /// Track active locks for monitoring/renewal
    active_locks: Arc<DashMap<String, ActiveLock>>,
    /// Lock contention queue
    lock_waiters: Arc<RwLock<HashMap<String, Vec<tokio::sync::oneshot::Sender<bool>>>>>,
}

impl ConsulLockService {
    pub fn new(
        kv_service: Arc<ConsulKVService>,
        session_service: Arc<ConsulSessionService>,
    ) -> Self {
        Self {
            kv_service,
            session_service,
            active_locks: Arc::new(DashMap::new()),
            lock_waiters: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Acquire a distributed lock
    pub async fn lock(&self, opts: &LockOptions) -> LockAcquireResult {
        // Create a session for this lock
        let session_req = crate::model::SessionCreateRequest {
            name: Some(format!("lock:{}", opts.key)),
            ttl: opts.session_ttl.clone(),
            lock_delay: opts.lock_delay.clone(),
            behavior: Some("release".to_string()),
            ..Default::default()
        };

        let session = self.session_service.create_session(session_req);
        let session_id = session.id.clone();

        // Try to acquire the lock using CAS
        let lock_key = format!("{}.lock", opts.key);
        let lock_value = opts.value.clone().unwrap_or_default();

        // Check if key exists and is locked
        if let Some(existing) = self.kv_service.get(&lock_key) {
            if existing.session.is_some() {
                // Lock is held by another session
                // Try waiting if wait time is specified
                if let Some(wait_time) = &opts.lock_wait_time {
                    let wait_ms = parse_duration_ms(wait_time).unwrap_or(0);
                    if wait_ms > 0 {
                        // Add to wait queue
                        let (tx, rx) = tokio::sync::oneshot::channel();
                        {
                            let mut waiters = self.lock_waiters.write().await;
                            waiters
                                .entry(lock_key.clone())
                                .or_default()
                                .push(tx);
                        }

                        // Wait for notification or timeout
                        let result = tokio::time::timeout(
                            Duration::from_millis(wait_ms),
                            rx,
                        )
                        .await;

                        if result.is_ok() {
                            // Retry lock acquisition
                            return self.try_acquire_lock(&lock_key, &session_id, &lock_value).await;
                        }
                    }
                }

                // Could not acquire lock
                // Destroy the session we created
                self.session_service.destroy_session(&session_id);
                return LockAcquireResult {
                    acquired: false,
                    lock: None,
                    session: None,
                };
            }
        }

        self.try_acquire_lock(&lock_key, &session_id, &lock_value).await
    }

    async fn try_acquire_lock(
        &self,
        lock_key: &str,
        session_id: &str,
        value: &str,
    ) -> LockAcquireResult {
        // Create lock entry with session
        let pair = self.kv_service.put(lock_key.to_string(), value, None);

        // Simulate session locking by storing session ID in a special way
        // In a full implementation, we'd modify KVPair to track session
        let lock = Lock {
            key: lock_key.to_string(),
            session: session_id.to_string(),
            value: Some(value.to_string()),
            lock_index: pair.modify_index,
            is_held: true,
        };

        // Track active lock
        self.active_locks.insert(
            lock_key.to_string(),
            ActiveLock {
                key: lock_key.to_string(),
                session: session_id.to_string(),
                acquired_at: Instant::now(),
                last_renewed: Instant::now(),
            },
        );

        LockAcquireResult {
            acquired: true,
            lock: Some(lock),
            session: Some(session_id.to_string()),
        }
    }

    /// Release a distributed lock
    pub async fn unlock(&self, key: &str, session_id: &str) -> bool {
        let lock_key = format!("{}.lock", key);

        // Verify the session holds the lock
        if let Some(active) = self.active_locks.get(&lock_key) {
            if active.session != session_id {
                return false; // Not the lock holder
            }
        } else {
            return false; // No active lock
        }

        // Remove the lock
        self.active_locks.remove(&lock_key);
        self.kv_service.delete(&lock_key);

        // Destroy the session
        self.session_service.destroy_session(session_id);

        // Notify waiters
        self.notify_waiters(&lock_key).await;

        true
    }

    /// Destroy a lock (cleanup all resources)
    pub async fn destroy(&self, key: &str) -> bool {
        let lock_key = format!("{}.lock", key);

        // Remove from active locks
        if let Some((_, active)) = self.active_locks.remove(&lock_key) {
            // Destroy the session
            self.session_service.destroy_session(&active.session);
        }

        // Delete the KV entry
        self.kv_service.delete(&lock_key);

        // Clear any waiters
        {
            let mut waiters = self.lock_waiters.write().await;
            waiters.remove(&lock_key);
        }

        true
    }

    /// Get lock info
    pub fn get_lock_info(&self, key: &str) -> Option<Lock> {
        let lock_key = format!("{}.lock", key);

        if let Some(active) = self.active_locks.get(&lock_key) {
            let pair = self.kv_service.get(&lock_key)?;
            Some(Lock {
                key: lock_key,
                session: active.session.clone(),
                value: pair.decoded_value(),
                lock_index: pair.modify_index,
                is_held: true,
            })
        } else {
            None
        }
    }

    async fn notify_waiters(&self, key: &str) {
        let mut waiters = self.lock_waiters.write().await;
        if let Some(mut queue) = waiters.remove(key) {
            // Notify the first waiter
            if let Some(tx) = queue.pop() {
                let _ = tx.send(true);
            }
            // Put remaining waiters back
            if !queue.is_empty() {
                waiters.insert(key.to_string(), queue);
            }
        }
    }

    /// Renew the lock's session TTL
    pub fn renew_lock(&self, key: &str, session_id: &str) -> bool {
        let lock_key = format!("{}.lock", key);

        // Verify lock holder
        if let Some(mut active) = self.active_locks.get_mut(&lock_key) {
            if active.session != session_id {
                return false;
            }
            active.last_renewed = Instant::now();
        } else {
            return false;
        }

        // Renew the session
        self.session_service.renew_session(session_id).is_some()
    }
}

// ============================================================================
// Semaphore Service
// ============================================================================

/// Consul Semaphore Service
/// Provides N-way distributed semaphore using KV + Sessions
#[derive(Clone)]
pub struct ConsulSemaphoreService {
    kv_service: Arc<ConsulKVService>,
    session_service: Arc<ConsulSessionService>,
}

impl ConsulSemaphoreService {
    pub fn new(
        kv_service: Arc<ConsulKVService>,
        session_service: Arc<ConsulSessionService>,
    ) -> Self {
        Self {
            kv_service,
            session_service,
        }
    }

    /// Acquire a semaphore slot
    pub async fn acquire(&self, opts: &SemaphoreOptions) -> Option<Semaphore> {
        // Create a session for this semaphore holder
        let session_req = crate::model::SessionCreateRequest {
            name: Some(format!("semaphore:{}", opts.prefix)),
            ttl: opts.session_ttl.clone(),
            behavior: Some("delete".to_string()), // Delete contender entry on session loss
            ..Default::default()
        };

        let session = self.session_service.create_session(session_req);
        let session_id = session.id.clone();

        let lock_key = format!("{}/.lock", opts.prefix);
        let contender_key = format!("{}/{}", opts.prefix, session_id);

        // Register as a contender
        let contender = SemaphoreContender {
            session: session_id.clone(),
            value: opts.value.clone(),
            acquired_at: current_unix_time(),
        };
        let contender_json = serde_json::to_string(&contender).ok()?;
        self.kv_service.put(contender_key.clone(), &contender_json, None);

        // Try to acquire a slot
        let acquired = self.try_acquire_slot(&lock_key, &session_id, opts.limit).await;

        if acquired {
            let holders = self.get_holders(&opts.prefix, opts.limit);
            Some(Semaphore {
                prefix: opts.prefix.clone(),
                limit: opts.limit,
                holders,
                is_held: true,
                session: Some(session_id),
            })
        } else {
            // Cleanup: remove contender entry and destroy session
            self.kv_service.delete(&contender_key);
            self.session_service.destroy_session(&session_id);
            None
        }
    }

    async fn try_acquire_slot(&self, lock_key: &str, session_id: &str, limit: u32) -> bool {
        // Get or create lock entry
        let mut lock_entry = if let Some(pair) = self.kv_service.get(lock_key) {
            serde_json::from_str::<SemaphoreLockEntry>(&pair.decoded_value().unwrap_or_default())
                .unwrap_or(SemaphoreLockEntry {
                    limit,
                    holders: HashMap::new(),
                })
        } else {
            SemaphoreLockEntry {
                limit,
                holders: HashMap::new(),
            }
        };

        // Check if there's room
        if lock_entry.holders.len() >= limit as usize {
            return false;
        }

        // Add this session as a holder
        lock_entry.holders.insert(session_id.to_string(), current_unix_time());

        // Update the lock entry
        let lock_json = serde_json::to_string(&lock_entry).unwrap_or_default();
        self.kv_service.put(lock_key.to_string(), &lock_json, None);

        true
    }

    fn get_holders(&self, prefix: &str, _limit: u32) -> Vec<SemaphoreHolder> {
        let lock_key = format!("{}/.lock", prefix);

        if let Some(pair) = self.kv_service.get(&lock_key) {
            if let Ok(entry) = serde_json::from_str::<SemaphoreLockEntry>(
                &pair.decoded_value().unwrap_or_default(),
            ) {
                return entry
                    .holders
                    .keys()
                    .map(|session| {
                        // Get contender info for value
                        let contender_key = format!("{}/{}", prefix, session);
                        let value = self
                            .kv_service
                            .get(&contender_key)
                            .and_then(|p| p.decoded_value())
                            .and_then(|v| serde_json::from_str::<SemaphoreContender>(&v).ok())
                            .and_then(|c| c.value);

                        SemaphoreHolder {
                            session: session.clone(),
                            value,
                        }
                    })
                    .collect();
            }
        }

        Vec::new()
    }

    /// Release a semaphore slot
    pub async fn release(&self, prefix: &str, session_id: &str) -> bool {
        let lock_key = format!("{}/.lock", prefix);
        let contender_key = format!("{}/{}", prefix, session_id);

        // Remove from holders
        if let Some(pair) = self.kv_service.get(&lock_key) {
            if let Ok(mut entry) = serde_json::from_str::<SemaphoreLockEntry>(
                &pair.decoded_value().unwrap_or_default(),
            ) {
                entry.holders.remove(session_id);
                let lock_json = serde_json::to_string(&entry).unwrap_or_default();
                self.kv_service.put(lock_key, &lock_json, None);
            }
        }

        // Remove contender entry
        self.kv_service.delete(&contender_key);

        // Destroy session
        self.session_service.destroy_session(session_id);

        true
    }

    /// Get semaphore info
    pub fn get_info(&self, prefix: &str) -> Option<Semaphore> {
        let lock_key = format!("{}/.lock", prefix);

        if let Some(pair) = self.kv_service.get(&lock_key) {
            if let Ok(entry) = serde_json::from_str::<SemaphoreLockEntry>(
                &pair.decoded_value().unwrap_or_default(),
            ) {
                let holders = self.get_holders(prefix, entry.limit);
                return Some(Semaphore {
                    prefix: prefix.to_string(),
                    limit: entry.limit,
                    holders,
                    is_held: false,
                    session: None,
                });
            }
        }

        None
    }
}

// ============================================================================
// HTTP Handlers
// ============================================================================

/// POST /v1/lock/acquire
pub async fn acquire_lock(
    req: HttpRequest,
    lock_service: web::Data<ConsulLockService>,
    acl_service: web::Data<AclService>,
    body: web::Json<LockOptions>,
) -> HttpResponse {
    // Check ACL authorization for key write
    let authz = acl_service.authorize_request(&req, ResourceType::Key, &body.key, true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let result = lock_service.lock(&body).await;
    HttpResponse::Ok().json(result)
}

/// PUT /v1/lock/release/{key}
pub async fn release_lock(
    req: HttpRequest,
    lock_service: web::Data<ConsulLockService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    query: web::Query<LockReleaseQuery>,
) -> HttpResponse {
    let key = path.into_inner();

    // Check ACL authorization for key write
    let authz = acl_service.authorize_request(&req, ResourceType::Key, &key, true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let session_id = match &query.session {
        Some(s) => s.clone(),
        None => return HttpResponse::BadRequest().json(ConsulError::new("session parameter required")),
    };

    let success = lock_service.unlock(&key, &session_id).await;
    HttpResponse::Ok().json(success)
}

#[derive(Debug, Deserialize)]
pub struct LockReleaseQuery {
    session: Option<String>,
}

/// DELETE /v1/lock/{key}
pub async fn destroy_lock(
    req: HttpRequest,
    lock_service: web::Data<ConsulLockService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
) -> HttpResponse {
    let key = path.into_inner();

    // Check ACL authorization for key write
    let authz = acl_service.authorize_request(&req, ResourceType::Key, &key, true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let success = lock_service.destroy(&key).await;
    HttpResponse::Ok().json(success)
}

/// GET /v1/lock/{key}
pub async fn get_lock(
    req: HttpRequest,
    lock_service: web::Data<ConsulLockService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
) -> HttpResponse {
    let key = path.into_inner();

    // Check ACL authorization for key read
    let authz = acl_service.authorize_request(&req, ResourceType::Key, &key, false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    match lock_service.get_lock_info(&key) {
        Some(lock) => HttpResponse::Ok().json(lock),
        None => HttpResponse::NotFound().json(ConsulError::new("Lock not found")),
    }
}

/// PUT /v1/lock/renew/{key}
pub async fn renew_lock(
    req: HttpRequest,
    lock_service: web::Data<ConsulLockService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    query: web::Query<LockReleaseQuery>,
) -> HttpResponse {
    let key = path.into_inner();

    // Check ACL authorization for session write
    let authz = acl_service.authorize_request(&req, ResourceType::Session, &key, true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let session_id = match &query.session {
        Some(s) => s.clone(),
        None => return HttpResponse::BadRequest().json(ConsulError::new("session parameter required")),
    };

    let success = lock_service.renew_lock(&key, &session_id);
    HttpResponse::Ok().json(success)
}

/// POST /v1/semaphore/acquire
pub async fn acquire_semaphore(
    req: HttpRequest,
    semaphore_service: web::Data<ConsulSemaphoreService>,
    acl_service: web::Data<AclService>,
    body: web::Json<SemaphoreOptions>,
) -> HttpResponse {
    // Check ACL authorization for key write
    let authz = acl_service.authorize_request(&req, ResourceType::Key, &body.prefix, true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    match semaphore_service.acquire(&body).await {
        Some(sem) => HttpResponse::Ok().json(sem),
        None => HttpResponse::Conflict().json(ConsulError::new("Failed to acquire semaphore slot")),
    }
}

/// PUT /v1/semaphore/release/{prefix}
pub async fn release_semaphore(
    req: HttpRequest,
    semaphore_service: web::Data<ConsulSemaphoreService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    query: web::Query<LockReleaseQuery>,
) -> HttpResponse {
    let prefix = path.into_inner();

    // Check ACL authorization for key write
    let authz = acl_service.authorize_request(&req, ResourceType::Key, &prefix, true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let session_id = match &query.session {
        Some(s) => s.clone(),
        None => return HttpResponse::BadRequest().json(ConsulError::new("session parameter required")),
    };

    let success = semaphore_service.release(&prefix, &session_id).await;
    HttpResponse::Ok().json(success)
}

/// GET /v1/semaphore/{prefix}
pub async fn get_semaphore(
    req: HttpRequest,
    semaphore_service: web::Data<ConsulSemaphoreService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
) -> HttpResponse {
    let prefix = path.into_inner();

    // Check ACL authorization for key read
    let authz = acl_service.authorize_request(&req, ResourceType::Key, &prefix, false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    match semaphore_service.get_info(&prefix) {
        Some(sem) => HttpResponse::Ok().json(sem),
        None => HttpResponse::NotFound().json(ConsulError::new("Semaphore not found")),
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

fn parse_duration_ms(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    let (num_str, multiplier) = if let Some(stripped) = s.strip_suffix("ms") {
        (stripped, 1u64)
    } else if let Some(stripped) = s.strip_suffix('s') {
        (stripped, 1000u64)
    } else if let Some(stripped) = s.strip_suffix('m') {
        (stripped, 60_000u64)
    } else if let Some(stripped) = s.strip_suffix('h') {
        (stripped, 3_600_000u64)
    } else {
        (s, 1000u64) // default to seconds
    };

    num_str.parse::<u64>().ok().map(|n| n * multiplier)
}

fn current_unix_time() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration_ms() {
        assert_eq!(parse_duration_ms("15s"), Some(15000));
        assert_eq!(parse_duration_ms("1m"), Some(60000));
        assert_eq!(parse_duration_ms("500ms"), Some(500));
        assert_eq!(parse_duration_ms("1h"), Some(3600000));
    }

    #[test]
    fn test_lock_options_default() {
        let opts = LockOptions::default();
        assert_eq!(opts.session_ttl, Some("15s".to_string()));
        assert_eq!(opts.lock_delay, Some("15s".to_string()));
    }

    #[test]
    fn test_semaphore_options_default() {
        let opts = SemaphoreOptions::default();
        assert_eq!(opts.limit, 1);
        assert_eq!(opts.session_ttl, Some("15s".to_string()));
    }
}
