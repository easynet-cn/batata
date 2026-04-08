//! Consul Session API HTTP handlers
//!
//! Implements Consul-compatible session management for distributed locking.
//! Uses RocksDB as the sole storage backend with unix-timestamp-based TTL.

use std::sync::Arc;

use actix_web::{HttpRequest, HttpResponse, web};
use rocksdb::{DB, WriteBatch};
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use crate::constants::CF_CONSUL_SESSIONS;
use crate::raft::{ConsulRaftRequest, ConsulRaftWriter};

use crate::acl::{AclService, ResourceType};
use crate::consul_meta::{ConsulResponseMeta, consul_ok};
use crate::index_provider::{ConsulIndexProvider, ConsulTable};
use crate::kv::ConsulKVService;
use crate::model::{ConsulError, Session, SessionCreateRequest, SessionCreateResponse};

/// Serializable session data stored in RocksDB.
/// Uses unix timestamps (survives restarts correctly).
#[derive(Clone, Serialize, Deserialize)]
struct StoredSession {
    session: Session,
    created_at_unix: u64,
    ttl_secs: u64,
    #[serde(default = "default_zero")]
    last_renewed_unix: u64,
}

fn default_zero() -> u64 {
    0
}

impl StoredSession {
    fn is_expired(&self) -> bool {
        // TTL of 0 means no TTL — session is persistent (matches Consul behavior)
        if self.ttl_secs == 0 {
            return false;
        }
        let effective_renewed = if self.last_renewed_unix > 0 {
            self.last_renewed_unix
        } else {
            self.created_at_unix
        };
        current_unix_secs() > effective_renewed + self.ttl_secs
    }
}

/// Consul Session service
/// Uses RocksDB as the sole storage backend.
#[derive(Clone)]
pub struct ConsulSessionService {
    /// RocksDB handle (always present)
    db: Arc<DB>,
    node_name: String,
    /// Keeps temp directory alive for tests/in-memory mode
    _temp_dir: Option<Arc<tempfile::TempDir>>,
    /// Optional Raft writer for cluster-mode replication (via core Raft PluginWrite)
    raft_node: Option<Arc<ConsulRaftWriter>>,
}

impl ConsulSessionService {
    /// Create a new session service with a temporary RocksDB (for tests/in-memory mode).
    pub fn new() -> Self {
        let (db, temp_dir) = crate::kv::open_temp_consul_db();
        let node_name = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "batata-node".to_string());

        Self {
            db,
            node_name,
            _temp_dir: Some(temp_dir),
            raft_node: None,
        }
    }

    /// Override the node name (e.g., from datacenter config).
    pub fn with_node_name(mut self, node_name: String) -> Self {
        self.node_name = node_name;
        self
    }

    /// Create a new session service backed by an existing RocksDB instance.
    /// Cleans up expired sessions on startup.
    pub fn with_rocks(db: Arc<DB>) -> Self {
        let node_name = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "batata-node".to_string());

        // Clean up expired sessions on startup
        if let Some(cf) = db.cf_handle(CF_CONSUL_SESSIONS) {
            let iter = db.iterator_cf(cf, rocksdb::IteratorMode::Start);
            let mut expired_keys = Vec::new();
            let mut loaded = 0u64;

            for item in iter.flatten() {
                let (key_bytes, value_bytes) = item;
                let key = match String::from_utf8(key_bytes.to_vec()) {
                    Ok(k) => k,
                    Err(_) => continue,
                };

                // Skip session-to-key index entries
                if key.starts_with("kidx:") {
                    continue;
                }

                if let Ok(stored) = serde_json::from_slice::<StoredSession>(&value_bytes) {
                    if stored.is_expired() {
                        expired_keys.push(key);
                    } else {
                        loaded += 1;
                    }
                }
            }

            if !expired_keys.is_empty() {
                let count = expired_keys.len();
                let mut batch = WriteBatch::default();
                for key in &expired_keys {
                    batch.delete_cf(cf, key.as_bytes());
                }
                if let Err(e) = db.write(batch) {
                    tracing::error!("Failed to clean up expired sessions from RocksDB: {}", e);
                }
                info!(
                    "Loaded {} sessions from RocksDB ({} expired sessions cleaned up)",
                    loaded, count
                );
            } else {
                info!("Loaded {} sessions from RocksDB", loaded);
            }
        }

        Self {
            db,
            node_name,
            _temp_dir: None,
            raft_node: None,
        }
    }

    /// Create a new session service backed by a Raft-replicated RocksDB.
    /// Does NOT clean up expired sessions locally on startup — cleanup goes through Raft.
    pub fn with_raft(db: Arc<DB>, raft_node: Arc<ConsulRaftWriter>) -> Self {
        let node_name = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "batata-node".to_string());

        // In Raft mode, skip startup cleanup — expired session cleanup is leader-only via Raft
        if let Some(cf) = db.cf_handle(CF_CONSUL_SESSIONS) {
            let iter = db.iterator_cf(cf, rocksdb::IteratorMode::Start);
            let mut count = 0u64;
            for item in iter.flatten() {
                let (key_bytes, value_bytes) = item;
                let key = match String::from_utf8(key_bytes.to_vec()) {
                    Ok(k) => k,
                    Err(_) => continue,
                };
                if key.starts_with("kidx:") {
                    continue;
                }
                if let Ok(stored) = serde_json::from_slice::<StoredSession>(&value_bytes)
                    && !stored.is_expired()
                {
                    count += 1;
                }
            }
            info!("Loaded {} sessions from Raft RocksDB (Raft mode)", count);
        }

        Self {
            db,
            node_name,
            _temp_dir: None,
            raft_node: Some(raft_node),
        }
    }

    // ========================================================================
    // Internal helpers
    // ========================================================================

    /// Read a stored session from RocksDB
    fn get_stored(&self, session_id: &str) -> Option<StoredSession> {
        let cf = self.db.cf_handle(CF_CONSUL_SESSIONS)?;
        let bytes = self.db.get_cf(cf, session_id.as_bytes()).ok()??;
        serde_json::from_slice(&bytes).ok()
    }

    /// Write a stored session to RocksDB
    fn put_stored(&self, session_id: &str, stored: &StoredSession) {
        if let Some(cf) = self.db.cf_handle(CF_CONSUL_SESSIONS) {
            match serde_json::to_vec(stored) {
                Ok(bytes) => {
                    if let Err(e) = self.db.put_cf(cf, session_id.as_bytes(), &bytes) {
                        error!("Failed to write session '{}': {}", session_id, e);
                    }
                }
                Err(e) => error!("Failed to serialize session '{}': {}", session_id, e),
            }
        }
    }

    // ========================================================================
    // Public API
    // ========================================================================

    /// Create a new session.
    ///
    /// Default values match the original Consul implementation:
    /// - TTL: empty (persistent session, no auto-expiry)
    /// - LockDelay: 15s
    /// - Behavior: "release"
    /// - NodeChecks: ["serfHealth"] (Consul's default health check)
    pub async fn create_session(&self, req: SessionCreateRequest) -> Session {
        let session_id = uuid::Uuid::new_v4().to_string();
        let now = current_unix_secs();

        // Consul defaults TTL to empty string (persistent session).
        // Only parse TTL if explicitly provided.
        let ttl_str = req.ttl.clone().unwrap_or_default();
        let ttl_secs = if ttl_str.is_empty() {
            0 // persistent session
        } else {
            crate::consul_meta::parse_go_duration_secs(&ttl_str).unwrap_or(0)
        };

        // Consul defaults NodeChecks to ["serfHealth"] if none specified
        let default_checks = Some(vec!["serfHealth".to_string()]);
        let node_checks = if req.node_checks.is_some() {
            req.node_checks.clone()
        } else {
            default_checks.clone()
        };

        let session = Session {
            id: session_id.clone(),
            name: req.name.unwrap_or_default(),
            node: req.node.unwrap_or_else(|| self.node_name.clone()),
            // Consul Go SDK expects LockDelay in nanoseconds (time.Duration).
            // Default is 15s = 15_000_000_000ns.
            lock_delay: crate::consul_meta::parse_go_duration(
                &req.lock_delay.unwrap_or_else(|| "15s".to_string()),
            )
            .unwrap_or(std::time::Duration::from_secs(15))
            .as_nanos() as u64,
            behavior: req.behavior.unwrap_or_else(|| "release".to_string()),
            ttl: ttl_str,
            checks: node_checks.clone(), // backward compatibility
            node_checks,
            service_checks: req.service_checks,
            namespace: None,
            create_index: now,
            modify_index: now,
        };

        let stored = StoredSession {
            session: session.clone(),
            created_at_unix: now,
            ttl_secs,
            last_renewed_unix: now,
        };

        if let Some(ref raft) = self.raft_node {
            let stored_json = serde_json::to_string(&stored).unwrap_or_default();
            match raft
                .write(ConsulRaftRequest::SessionCreate {
                    session_id: session_id.clone(),
                    stored_session_json: stored_json,
                })
                .await
            {
                Ok(r) if !r.success => {
                    error!("Raft ConsulSessionCreate rejected: {:?}", r.message);
                }
                Err(e) => {
                    error!("Raft ConsulSessionCreate failed: {}", e);
                }
                _ => {}
            }
        } else {
            self.put_stored(&session_id, &stored);
        }

        session
    }

    /// Destroy a session
    pub async fn destroy_session(&self, session_id: &str) -> bool {
        if let Some(ref raft) = self.raft_node {
            match raft
                .write(ConsulRaftRequest::SessionDestroy {
                    session_id: session_id.to_string(),
                })
                .await
            {
                Ok(r) => r.success,
                Err(e) => {
                    error!("Raft ConsulSessionDestroy failed: {}", e);
                    false
                }
            }
        } else {
            let cf = match self.db.cf_handle(CF_CONSUL_SESSIONS) {
                Some(cf) => cf,
                None => return false,
            };

            // Check if session exists
            match self.db.get_cf(cf, session_id.as_bytes()) {
                Ok(Some(_)) => {
                    if let Err(e) = self.db.delete_cf(cf, session_id.as_bytes()) {
                        error!("Failed to delete session '{}': {}", session_id, e);
                        return false;
                    }
                    true
                }
                _ => false,
            }
        }
    }

    /// Get session info (with lazy expiration)
    pub fn get_session(&self, session_id: &str) -> Option<Session> {
        let stored = self.get_stored(session_id)?;

        if stored.is_expired() {
            // Lazy expire: delete the expired session directly from local DB
            // In Raft mode, the leader's cleanup task will handle proper replication
            if let Some(cf) = self.db.cf_handle(CF_CONSUL_SESSIONS)
                && let Err(e) = self.db.delete_cf(cf, session_id.as_bytes())
            {
                tracing::error!(
                    "Failed to delete expired session '{}' from RocksDB: {}",
                    session_id,
                    e
                );
            }
            return None;
        }

        Some(stored.session)
    }

    /// Renew a session (reset last_renewed_unix)
    pub async fn renew_session(&self, session_id: &str) -> Option<Session> {
        let mut stored = self.get_stored(session_id)?;

        if stored.is_expired() {
            self.destroy_session(session_id).await;
            return None;
        }

        stored.last_renewed_unix = current_unix_secs();

        if let Some(ref raft) = self.raft_node {
            let stored_json = serde_json::to_string(&stored).unwrap_or_default();
            match raft
                .write(ConsulRaftRequest::SessionRenew {
                    session_id: session_id.to_string(),
                    stored_session_json: stored_json,
                })
                .await
            {
                Ok(r) if !r.success => {
                    error!("Raft ConsulSessionRenew rejected: {:?}", r.message);
                }
                Err(e) => {
                    error!("Raft ConsulSessionRenew failed: {}", e);
                    return None;
                }
                _ => {}
            }
        } else {
            self.put_stored(session_id, &stored);
        }

        Some(stored.session)
    }

    /// Invalidate (destroy) all sessions linked to a check that became Critical.
    ///
    /// Matches Consul's `checkSessionsTxn()`: when a check enters Critical state,
    /// all sessions that reference that check (via NodeChecks or ServiceChecks)
    /// are automatically destroyed. The session's Behavior determines how KV
    /// locks are handled (release vs delete), but lock handling is delegated to
    /// the caller since this service doesn't hold a KV reference.
    ///
    /// Returns the list of destroyed session IDs (so the caller can release KV locks).
    pub fn invalidate_sessions_for_check(&self, check_id: &str) -> Vec<Session> {
        let sessions = self.list_sessions();
        let mut invalidated = Vec::new();

        for session in sessions {
            let linked = session
                .node_checks
                .as_ref()
                .is_some_and(|checks| checks.iter().any(|c| c == check_id))
                || session
                    .service_checks
                    .as_ref()
                    .is_some_and(|checks| checks.iter().any(|c| c.id == check_id));

            if linked {
                info!(
                    "Invalidating session '{}' (name='{}') because check '{}' became critical",
                    session.id, session.name, check_id
                );

                // Delete the session from storage
                if let Some(cf) = self.db.cf_handle(CF_CONSUL_SESSIONS) {
                    let _ = self.db.delete_cf(cf, session.id.as_bytes());
                }

                invalidated.push(session);
            }
        }

        invalidated
    }

    /// List all active sessions (skips kidx: index entries and expired sessions)
    pub fn list_sessions(&self) -> Vec<Session> {
        let Some(cf) = self.db.cf_handle(CF_CONSUL_SESSIONS) else {
            return Vec::new();
        };

        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);
        let mut sessions = Vec::new();
        let mut expired_keys = Vec::new();

        for item in iter.flatten() {
            let (key_bytes, value_bytes) = item;
            let key = match String::from_utf8(key_bytes.to_vec()) {
                Ok(k) => k,
                Err(_) => continue,
            };

            // Skip session-to-key index entries
            if key.starts_with("kidx:") {
                continue;
            }

            if let Ok(stored) = serde_json::from_slice::<StoredSession>(&value_bytes) {
                if stored.is_expired() {
                    expired_keys.push(key);
                } else {
                    sessions.push(stored.session);
                }
            }
        }

        // Clean up expired sessions
        if !expired_keys.is_empty() {
            let mut batch = WriteBatch::default();
            for key in &expired_keys {
                batch.delete_cf(cf, key.as_bytes());
            }
            if let Err(e) = self.db.write(batch) {
                tracing::error!("Failed to clean up expired sessions from RocksDB: {}", e);
            }
        }

        sessions
    }

    /// List sessions for a specific node
    pub fn list_node_sessions(&self, node: &str) -> Vec<Session> {
        self.list_sessions()
            .into_iter()
            .filter(|s| s.node == node)
            .collect()
    }

    /// Clean up expired sessions
    pub fn cleanup_expired(&self) {
        let Some(cf) = self.db.cf_handle(CF_CONSUL_SESSIONS) else {
            return;
        };

        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);
        let mut batch = WriteBatch::default();
        let mut count = 0u64;

        for item in iter.flatten() {
            let (key_bytes, value_bytes) = item;
            let key = match String::from_utf8(key_bytes.to_vec()) {
                Ok(k) => k,
                Err(_) => continue,
            };

            // Skip session-to-key index entries
            if key.starts_with("kidx:") {
                continue;
            }

            if let Ok(stored) = serde_json::from_slice::<StoredSession>(&value_bytes)
                && stored.is_expired()
            {
                batch.delete_cf(cf, key.as_bytes());
                count += 1;
            }
        }

        if count > 0
            && let Err(e) = self.db.write(batch)
        {
            error!("Failed to cleanup expired sessions: {}", e);
        }
    }

    /// Scan and return IDs of expired sessions (read-only, for leader-only Raft cleanup)
    pub fn scan_expired_session_ids(&self) -> Vec<String> {
        let Some(cf) = self.db.cf_handle(CF_CONSUL_SESSIONS) else {
            return Vec::new();
        };

        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);
        let mut expired = Vec::new();

        for item in iter.flatten() {
            let (key_bytes, value_bytes) = item;
            let key = match String::from_utf8(key_bytes.to_vec()) {
                Ok(k) => k,
                Err(_) => continue,
            };
            if key.starts_with("kidx:") {
                continue;
            }
            if let Ok(stored) = serde_json::from_slice::<StoredSession>(&value_bytes)
                && stored.is_expired()
            {
                expired.push(key);
            }
        }

        expired
    }
}

impl Default for ConsulSessionService {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse duration string (e.g., "15s", "1m", "1h")
#[cfg(test)]
fn parse_duration(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    let (num_str, multiplier) = if let Some(stripped) = s.strip_suffix("ms") {
        (stripped, 1u64) // milliseconds, divide by 1000 later
    } else if let Some(stripped) = s.strip_suffix('s') {
        (stripped, 1000u64)
    } else if let Some(stripped) = s.strip_suffix('m') {
        (stripped, 60_000u64)
    } else if let Some(stripped) = s.strip_suffix('h') {
        (stripped, 3_600_000u64)
    } else {
        (s, 1000u64) // default to seconds
    };

    let num: u64 = num_str.parse().ok()?;

    // Convert to seconds (multiplier is in milliseconds)
    Some(num * multiplier / 1000)
}

fn current_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Query parameters for session endpoints
#[derive(Debug, Clone, Deserialize, Default)]
pub struct SessionQueryParams {
    pub dc: Option<String>,
    pub ns: Option<String>,
}

// ============================================================================
// HTTP Handlers
// ============================================================================

/// PUT /v1/session/create
/// Creates a new session
pub async fn create_session(
    req: HttpRequest,
    session_service: web::Data<ConsulSessionService>,
    acl_service: web::Data<AclService>,
    body: web::Json<SessionCreateRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    // Check ACL authorization for session write
    let authz = acl_service.authorize_request(&req, ResourceType::Session, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let session = session_service.create_session(body.into_inner()).await;
    let response = SessionCreateResponse {
        id: session.id.clone(),
    };

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Sessions));
    consul_ok(&meta).json(response)
}

/// PUT /v1/session/destroy/{uuid}
/// Destroys a session
pub async fn destroy_session(
    req: HttpRequest,
    session_service: web::Data<ConsulSessionService>,
    kv_service: web::Data<ConsulKVService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let session_id = path.into_inner();

    // Check ACL authorization for session write
    let authz = acl_service.authorize_request(&req, ResourceType::Session, &session_id, true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    // Get session behavior before destroying (needed to handle KV keys correctly)
    let behavior = session_service
        .get_session(&session_id)
        .map(|s| s.behavior.clone())
        .unwrap_or_else(|| "release".to_string());

    // Handle KV keys based on session behavior (matches Consul's session.go:378-409)
    if behavior == "delete" {
        // Delete behavior: delete all KV keys held by this session
        kv_service.delete_session_keys(&session_id).await;
    } else {
        // Release behavior (default): release locks but keep keys
        kv_service.release_session(&session_id).await;
    }
    // Increment KVS index to wake blocking queries (e.g., Go SDK Lock monitor)
    index_provider.increment(ConsulTable::KVS);

    let destroyed = session_service.destroy_session(&session_id).await;
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Sessions));
    consul_ok(&meta).json(destroyed)
}

/// GET /v1/session/info/{uuid}
/// Returns information about a specific session
pub async fn get_session_info(
    req: HttpRequest,
    session_service: web::Data<ConsulSessionService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let session_id = path.into_inner();

    // Check ACL authorization for session read
    let authz = acl_service.authorize_request(&req, ResourceType::Session, &session_id, false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Sessions));
    match session_service.get_session(&session_id) {
        Some(session) => consul_ok(&meta).json(vec![session]),
        None => consul_ok(&meta).json(Vec::<Session>::new()),
    }
}

/// GET /v1/session/list
/// Returns all active sessions
pub async fn list_sessions(
    req: HttpRequest,
    session_service: web::Data<ConsulSessionService>,
    acl_service: web::Data<AclService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    // Check ACL authorization for session read
    let authz = acl_service.authorize_request(&req, ResourceType::Session, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    // Clean up expired sessions first
    session_service.cleanup_expired();

    let sessions = session_service.list_sessions();
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Sessions));
    consul_ok(&meta).json(sessions)
}

/// GET /v1/session/node/{node}
/// Returns sessions for a specific node
pub async fn list_node_sessions(
    req: HttpRequest,
    session_service: web::Data<ConsulSessionService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let node = path.into_inner();

    // Check ACL authorization for session read
    let authz = acl_service.authorize_request(&req, ResourceType::Session, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let sessions = session_service.list_node_sessions(&node);
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Sessions));
    consul_ok(&meta).json(sessions)
}

/// PUT /v1/session/renew/{uuid}
/// Renews a session TTL
pub async fn renew_session(
    req: HttpRequest,
    session_service: web::Data<ConsulSessionService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let session_id = path.into_inner();

    // Check ACL authorization for session write
    let authz = acl_service.authorize_request(&req, ResourceType::Session, &session_id, true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Sessions));
    match session_service.renew_session(&session_id).await {
        Some(session) => consul_ok(&meta).json(vec![session]),
        None => HttpResponse::NotFound().json(ConsulError::new("Session not found or expired")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("15s"), Some(15));
        assert_eq!(parse_duration("1m"), Some(60));
        assert_eq!(parse_duration("1h"), Some(3600));
        assert_eq!(parse_duration("500ms"), Some(0));
        assert_eq!(parse_duration("30"), Some(30));
    }

    #[tokio::test]
    async fn test_session_lifecycle() {
        let service = ConsulSessionService::new();

        // Create session
        let req = SessionCreateRequest {
            name: Some("test-session".to_string()),
            ttl: Some("60s".to_string()),
            ..Default::default()
        };
        let session = service.create_session(req).await;
        assert!(!session.id.is_empty());
        assert_eq!(session.name, "test-session");
        assert_eq!(session.ttl, "60s");
        assert_eq!(session.behavior, "release");
        assert!(!session.node.is_empty());

        // Get session
        let retrieved = service.get_session(&session.id);
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.id, session.id);
        assert_eq!(retrieved.name, "test-session");
        assert_eq!(retrieved.ttl, "60s");

        // Renew session
        let renewed = service.renew_session(&session.id).await;
        assert!(renewed.is_some());
        let renewed = renewed.unwrap();
        assert_eq!(renewed.id, session.id);
        assert_eq!(renewed.name, "test-session");

        // Destroy session
        let destroyed = service.destroy_session(&session.id).await;
        assert!(destroyed);

        // Should not exist anymore
        let gone = service.get_session(&session.id);
        assert!(gone.is_none());
    }

    #[tokio::test]
    async fn test_destroy_nonexistent_session() {
        let service = ConsulSessionService::new();
        assert!(!service.destroy_session("nonexistent-id").await);
    }

    #[test]
    fn test_get_nonexistent_session() {
        let service = ConsulSessionService::new();
        assert!(service.get_session("nonexistent-id").is_none());
    }

    #[tokio::test]
    async fn test_renew_nonexistent_session() {
        let service = ConsulSessionService::new();
        assert!(service.renew_session("nonexistent-id").await.is_none());
    }

    #[tokio::test]
    async fn test_list_sessions() {
        let service = ConsulSessionService::new();

        // Initially empty
        assert!(service.list_sessions().is_empty());

        let req1 = SessionCreateRequest {
            name: Some("session-1".to_string()),
            ttl: Some("60s".to_string()),
            ..Default::default()
        };
        let req2 = SessionCreateRequest {
            name: Some("session-2".to_string()),
            ttl: Some("60s".to_string()),
            ..Default::default()
        };
        let s1 = service.create_session(req1).await;
        let s2 = service.create_session(req2).await;

        let sessions = service.list_sessions();
        assert_eq!(sessions.len(), 2);
        let names: Vec<&str> = sessions.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"session-1"));
        assert!(names.contains(&"session-2"));
        // Verify IDs are distinct
        assert_ne!(s1.id, s2.id);
    }

    #[tokio::test]
    async fn test_list_node_sessions() {
        let service = ConsulSessionService::new();
        let node_name = &service.node_name.clone();

        let req = SessionCreateRequest {
            name: Some("node-session".to_string()),
            ttl: Some("60s".to_string()),
            node: Some(node_name.clone()),
            ..Default::default()
        };
        service.create_session(req).await;

        let node_sessions = service.list_node_sessions(node_name);
        assert_eq!(node_sessions.len(), 1);
        assert_eq!(node_sessions[0].name, "node-session");
        assert_eq!(node_sessions[0].node, *node_name);
        assert_eq!(node_sessions[0].ttl, "60s");

        let other = service.list_node_sessions("other-node");
        assert!(other.is_empty());
    }

    #[tokio::test]
    async fn test_session_behavior_release() {
        let service = ConsulSessionService::new();

        let req = SessionCreateRequest {
            name: Some("release-session".to_string()),
            behavior: Some("release".to_string()),
            ttl: Some("60s".to_string()),
            ..Default::default()
        };
        let session = service.create_session(req).await;
        assert_eq!(session.behavior, "release");
    }

    #[tokio::test]
    async fn test_session_behavior_delete() {
        let service = ConsulSessionService::new();

        let req = SessionCreateRequest {
            name: Some("delete-session".to_string()),
            behavior: Some("delete".to_string()),
            ttl: Some("60s".to_string()),
            ..Default::default()
        };
        let session = service.create_session(req).await;
        assert_eq!(session.behavior, "delete");
    }

    #[tokio::test]
    async fn test_session_default_ttl() {
        let service = ConsulSessionService::new();

        let req = SessionCreateRequest {
            name: Some("default-ttl".to_string()),
            ..Default::default()
        };
        let session = service.create_session(req).await;
        // Consul defaults: TTL is empty (persistent session), NodeChecks = ["serfHealth"]
        assert_eq!(session.ttl, ""); // persistent by default (matches Consul)
        assert_eq!(
            session.node_checks,
            Some(vec!["serfHealth".to_string()]),
            "Default node checks should include serfHealth"
        );
        assert_eq!(session.behavior, "release");
        assert_eq!(session.lock_delay, 15_000_000_000); // 15s in nanoseconds
    }

    #[tokio::test]
    async fn test_session_lock_delay() {
        let service = ConsulSessionService::new();

        let req = SessionCreateRequest {
            name: Some("delayed".to_string()),
            lock_delay: Some("30s".to_string()),
            ttl: Some("60s".to_string()),
            ..Default::default()
        };
        let session = service.create_session(req).await;
        assert_eq!(session.lock_delay, 30_000_000_000); // 30s in nanoseconds
    }

    #[tokio::test]
    async fn test_session_with_checks() {
        let service = ConsulSessionService::new();

        let req = SessionCreateRequest {
            name: Some("checked-session".to_string()),
            ttl: Some("60s".to_string()),
            node_checks: Some(vec!["serfHealth".to_string()]),
            service_checks: Some(vec![crate::model::SessionServiceCheck {
                id: "service:web".to_string(),
                namespace: None,
            }]),
            ..Default::default()
        };
        let session = service.create_session(req).await;
        assert_eq!(session.node_checks, Some(vec!["serfHealth".to_string()]));
        assert_eq!(
            session.service_checks,
            Some(vec![crate::model::SessionServiceCheck {
                id: "service:web".to_string(),
                namespace: None
            }])
        );
    }

    #[tokio::test]
    async fn test_session_unique_ids() {
        let service = ConsulSessionService::new();

        let mut ids = Vec::new();
        for _ in 0..10 {
            let req = SessionCreateRequest {
                ttl: Some("60s".to_string()),
                ..Default::default()
            };
            ids.push(service.create_session(req).await.id);
        }

        // All IDs should be unique
        let mut deduped = ids.clone();
        deduped.sort();
        deduped.dedup();
        assert_eq!(ids.len(), deduped.len());
    }

    #[tokio::test]
    async fn test_destroy_then_list() {
        let service = ConsulSessionService::new();

        let session = service
            .create_session(SessionCreateRequest {
                name: Some("to-destroy".to_string()),
                ttl: Some("60s".to_string()),
                ..Default::default()
            })
            .await;
        assert_eq!(service.list_sessions().len(), 1);

        service.destroy_session(&session.id).await;
        assert_eq!(service.list_sessions().len(), 0);
    }

    #[tokio::test]
    async fn test_invalidate_sessions_for_node_check() {
        let service = ConsulSessionService::new();

        // Create sessions linked to serfHealth (default)
        let s1 = service
            .create_session(SessionCreateRequest {
                name: Some("session-1".to_string()),
                ttl: Some("60s".to_string()),
                ..Default::default() // NodeChecks defaults to ["serfHealth"]
            })
            .await;
        let _s2 = service
            .create_session(SessionCreateRequest {
                name: Some("session-2".to_string()),
                ttl: Some("60s".to_string()),
                node_checks: Some(vec!["custom-check".to_string()]),
                ..Default::default()
            })
            .await;
        assert_eq!(service.list_sessions().len(), 2);

        // Invalidate sessions linked to serfHealth
        let invalidated = service.invalidate_sessions_for_check("serfHealth");
        assert_eq!(invalidated.len(), 1);
        assert_eq!(invalidated[0].id, s1.id);

        // Only session-2 survives (linked to custom-check, not serfHealth)
        let remaining = service.list_sessions();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].name, "session-2");
    }

    #[tokio::test]
    async fn test_invalidate_sessions_for_service_check() {
        let service = ConsulSessionService::new();

        let _s1 = service
            .create_session(SessionCreateRequest {
                name: Some("web-session".to_string()),
                ttl: Some("60s".to_string()),
                node_checks: Some(vec![]),
                service_checks: Some(vec![crate::model::SessionServiceCheck {
                    id: "service:web-1".to_string(),
                    namespace: None,
                }]),
                ..Default::default()
            })
            .await;
        let _s2 = service
            .create_session(SessionCreateRequest {
                name: Some("db-session".to_string()),
                ttl: Some("60s".to_string()),
                node_checks: Some(vec![]),
                service_checks: Some(vec![crate::model::SessionServiceCheck {
                    id: "service:db-1".to_string(),
                    namespace: None,
                }]),
                ..Default::default()
            })
            .await;
        assert_eq!(service.list_sessions().len(), 2);

        // Only web-1 fails
        let invalidated = service.invalidate_sessions_for_check("service:web-1");
        assert_eq!(invalidated.len(), 1);
        assert_eq!(invalidated[0].name, "web-session");

        let remaining = service.list_sessions();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].name, "db-session");
    }

    #[tokio::test]
    async fn test_invalidate_sessions_no_matches() {
        let service = ConsulSessionService::new();

        let _s1 = service
            .create_session(SessionCreateRequest {
                name: Some("session-1".to_string()),
                ttl: Some("60s".to_string()),
                ..Default::default()
            })
            .await;

        let invalidated = service.invalidate_sessions_for_check("nonexistent-check");
        assert!(invalidated.is_empty());
        assert_eq!(service.list_sessions().len(), 1);
    }
}
