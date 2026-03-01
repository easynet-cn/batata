//! Consul Session API HTTP handlers
//!
//! Implements Consul-compatible session management for distributed locking.
//! Uses RocksDB as the sole storage backend with unix-timestamp-based TTL.

use std::sync::Arc;

use actix_web::{HttpRequest, HttpResponse, web};
use rocksdb::{DB, WriteBatch};
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use batata_consistency::RaftNode;
use batata_consistency::raft::request::RaftRequest;
use batata_consistency::raft::state_machine::CF_CONSUL_SESSIONS;

use crate::acl::{AclService, ResourceType};
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
    /// Optional Raft node for cluster-mode replication
    raft_node: Option<Arc<RaftNode>>,
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
                let _ = db.write(batch);
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
    pub fn with_raft(db: Arc<DB>, raft_node: Arc<RaftNode>) -> Self {
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

    /// Create a new session
    pub async fn create_session(&self, req: SessionCreateRequest) -> Session {
        let session_id = uuid::Uuid::new_v4().to_string();
        let now = current_unix_secs();

        let ttl_str = req.ttl.clone().unwrap_or_else(|| "15s".to_string());
        let ttl_secs = parse_duration(&ttl_str).unwrap_or(15);

        let session = Session {
            id: session_id.clone(),
            name: req.name.unwrap_or_default(),
            node: req.node.unwrap_or_else(|| self.node_name.clone()),
            lock_delay: parse_duration(&req.lock_delay.unwrap_or_else(|| "15s".to_string()))
                .unwrap_or(15),
            behavior: req.behavior.unwrap_or_else(|| "release".to_string()),
            ttl: ttl_str,
            node_checks: req.node_checks,
            service_checks: req.service_checks,
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
            let stored_json = serde_json::to_string(&stored).unwrap();
            match raft
                .write(RaftRequest::ConsulSessionCreate {
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
                .write(RaftRequest::ConsulSessionDestroy {
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
            if let Some(cf) = self.db.cf_handle(CF_CONSUL_SESSIONS) {
                let _ = self.db.delete_cf(cf, session_id.as_bytes());
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
            let stored_json = serde_json::to_string(&stored).unwrap();
            match raft
                .write(RaftRequest::ConsulSessionRenew {
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
            let _ = self.db.write(batch);
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

    HttpResponse::Ok().json(response)
}

/// PUT /v1/session/destroy/{uuid}
/// Destroys a session
pub async fn destroy_session(
    req: HttpRequest,
    session_service: web::Data<ConsulSessionService>,
    kv_service: web::Data<ConsulKVService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
) -> HttpResponse {
    let session_id = path.into_inner();

    // Check ACL authorization for session write
    let authz = acl_service.authorize_request(&req, ResourceType::Session, &session_id, true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    // Release all KV keys held by this session (Consul "release" behavior)
    kv_service.release_session(&session_id).await;

    let destroyed = session_service.destroy_session(&session_id).await;
    HttpResponse::Ok().json(destroyed)
}

/// GET /v1/session/info/{uuid}
/// Returns information about a specific session
pub async fn get_session_info(
    req: HttpRequest,
    session_service: web::Data<ConsulSessionService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
) -> HttpResponse {
    let session_id = path.into_inner();

    // Check ACL authorization for session read
    let authz = acl_service.authorize_request(&req, ResourceType::Session, &session_id, false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    match session_service.get_session(&session_id) {
        Some(session) => HttpResponse::Ok().json(vec![session]),
        None => HttpResponse::Ok().json(Vec::<Session>::new()),
    }
}

/// GET /v1/session/list
/// Returns all active sessions
pub async fn list_sessions(
    req: HttpRequest,
    session_service: web::Data<ConsulSessionService>,
    acl_service: web::Data<AclService>,
) -> HttpResponse {
    // Check ACL authorization for session read
    let authz = acl_service.authorize_request(&req, ResourceType::Session, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    // Clean up expired sessions first
    session_service.cleanup_expired();

    let sessions = session_service.list_sessions();
    HttpResponse::Ok().json(sessions)
}

/// GET /v1/session/node/{node}
/// Returns sessions for a specific node
pub async fn list_node_sessions(
    req: HttpRequest,
    session_service: web::Data<ConsulSessionService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
) -> HttpResponse {
    let node = path.into_inner();

    // Check ACL authorization for session read
    let authz = acl_service.authorize_request(&req, ResourceType::Session, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let sessions = session_service.list_node_sessions(&node);
    HttpResponse::Ok().json(sessions)
}

/// PUT /v1/session/renew/{uuid}
/// Renews a session TTL
pub async fn renew_session(
    req: HttpRequest,
    session_service: web::Data<ConsulSessionService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
) -> HttpResponse {
    let session_id = path.into_inner();

    // Check ACL authorization for session write
    let authz = acl_service.authorize_request(&req, ResourceType::Session, &session_id, true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    match session_service.renew_session(&session_id).await {
        Some(session) => HttpResponse::Ok().json(vec![session]),
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

        // Get session
        let retrieved = service.get_session(&session.id);
        assert!(retrieved.is_some());

        // Renew session
        let renewed = service.renew_session(&session.id).await;
        assert!(renewed.is_some());

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
        service.create_session(req1).await;
        service.create_session(req2).await;

        let sessions = service.list_sessions();
        assert_eq!(sessions.len(), 2);
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
        assert_eq!(session.ttl, "15s"); // default
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
        assert_eq!(session.lock_delay, 30);
    }

    #[tokio::test]
    async fn test_session_with_checks() {
        let service = ConsulSessionService::new();

        let req = SessionCreateRequest {
            name: Some("checked-session".to_string()),
            ttl: Some("60s".to_string()),
            node_checks: Some(vec!["serfHealth".to_string()]),
            service_checks: Some(vec!["service:web".to_string()]),
            ..Default::default()
        };
        let session = service.create_session(req).await;
        assert_eq!(session.node_checks, Some(vec!["serfHealth".to_string()]));
        assert_eq!(
            session.service_checks,
            Some(vec!["service:web".to_string()])
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
}
