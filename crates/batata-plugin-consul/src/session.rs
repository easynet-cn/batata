//! Consul Session API HTTP handlers
//!
//! Implements Consul-compatible session management for distributed locking.

use std::sync::Arc;
use std::time::{Duration, Instant};

use actix_web::{HttpRequest, HttpResponse, web};
use dashmap::DashMap;
use rocksdb::DB;
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};

use batata_consistency::raft::state_machine::CF_CONSUL_SESSIONS;

use crate::acl::{AclService, ResourceType};
use crate::kv::ConsulKVService;
use crate::model::{ConsulError, Session, SessionCreateRequest, SessionCreateResponse};

/// Session storage with TTL tracking
#[derive(Clone)]
pub struct SessionEntry {
    pub session: Session,
    pub created_at: Instant,
    pub ttl: Duration,
}

/// Serializable session data for RocksDB persistence
#[derive(Clone, Serialize, Deserialize)]
struct SessionPersist {
    session: Session,
    created_at_unix: u64,
    ttl_secs: u64,
}

/// Consul Session service
/// Provides distributed locking via session management.
/// When `rocks_db` is `Some`, writes are persisted to RocksDB (write-through cache).
#[derive(Clone)]
pub struct ConsulSessionService {
    sessions: Arc<DashMap<String, SessionEntry>>,
    node_name: String,
    /// Optional RocksDB handle for persistence
    rocks_db: Option<Arc<DB>>,
}

impl ConsulSessionService {
    pub fn new() -> Self {
        let node_name = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "batata-node".to_string());

        Self {
            sessions: Arc::new(DashMap::new()),
            node_name,
            rocks_db: None,
        }
    }

    /// Create a new session service with RocksDB persistence.
    /// Loads existing sessions from RocksDB, skipping expired ones.
    pub fn with_rocks(db: Arc<DB>) -> Self {
        let node_name = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "batata-node".to_string());

        let sessions = Arc::new(DashMap::new());
        let now_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if let Some(cf) = db.cf_handle(CF_CONSUL_SESSIONS) {
            let iter = db.iterator_cf(cf, rocksdb::IteratorMode::Start);
            let mut loaded = 0u64;
            let mut expired = 0u64;

            for item in iter.flatten() {
                let (key_bytes, value_bytes) = item;
                if let Ok(persist) = serde_json::from_slice::<SessionPersist>(&value_bytes) {
                    // Skip expired sessions
                    if now_unix > persist.created_at_unix + persist.ttl_secs {
                        expired += 1;
                        // Delete expired entry from RocksDB
                        let _ = db.delete_cf(cf, &key_bytes);
                        continue;
                    }

                    // Reconstruct Instant: approximate by computing remaining TTL
                    let elapsed = now_unix.saturating_sub(persist.created_at_unix);
                    let created_at = Instant::now() - Duration::from_secs(elapsed);

                    let entry = SessionEntry {
                        session: persist.session.clone(),
                        created_at,
                        ttl: Duration::from_secs(persist.ttl_secs),
                    };
                    sessions.insert(persist.session.id.clone(), entry);
                    loaded += 1;
                } else if let Ok(key) = String::from_utf8(key_bytes.to_vec()) {
                    warn!("Failed to deserialize session entry: {}", key);
                }
            }
            info!(
                "Loaded {} sessions from RocksDB ({} expired sessions cleaned up)",
                loaded, expired
            );
        }

        Self {
            sessions,
            node_name,
            rocks_db: Some(db),
        }
    }

    /// Persist a session to RocksDB
    fn persist_session(&self, session_id: &str, entry: &SessionEntry) {
        if let Some(ref db) = self.rocks_db
            && let Some(cf) = db.cf_handle(CF_CONSUL_SESSIONS)
        {
            let now_unix = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let elapsed = entry.created_at.elapsed().as_secs();
            let created_at_unix = now_unix.saturating_sub(elapsed);

            let persist = SessionPersist {
                session: entry.session.clone(),
                created_at_unix,
                ttl_secs: entry.ttl.as_secs(),
            };
            match serde_json::to_vec(&persist) {
                Ok(bytes) => {
                    if let Err(e) = db.put_cf(cf, session_id.as_bytes(), &bytes) {
                        error!("Failed to persist session '{}': {}", session_id, e);
                    }
                }
                Err(e) => {
                    error!("Failed to serialize session '{}': {}", session_id, e);
                }
            }
        }
    }

    /// Delete a session from RocksDB
    fn delete_session_rocks(&self, session_id: &str) {
        if let Some(ref db) = self.rocks_db
            && let Some(cf) = db.cf_handle(CF_CONSUL_SESSIONS)
            && let Err(e) = db.delete_cf(cf, session_id.as_bytes())
        {
            error!(
                "Failed to delete session '{}' from RocksDB: {}",
                session_id, e
            );
        }
    }

    /// Create a new session
    pub fn create_session(&self, req: SessionCreateRequest) -> Session {
        let session_id = uuid::Uuid::new_v4().to_string();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

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

        let entry = SessionEntry {
            session: session.clone(),
            created_at: Instant::now(),
            ttl: Duration::from_secs(ttl_secs),
        };

        self.sessions.insert(session_id.clone(), entry.clone());
        self.persist_session(&session_id, &entry);
        session
    }

    /// Destroy a session
    pub fn destroy_session(&self, session_id: &str) -> bool {
        let removed = self.sessions.remove(session_id).is_some();
        if removed {
            self.delete_session_rocks(session_id);
        }
        removed
    }

    /// Get session info
    pub fn get_session(&self, session_id: &str) -> Option<Session> {
        self.sessions.get(session_id).map(|e| {
            // Check if session has expired
            if e.created_at.elapsed() > e.ttl {
                None
            } else {
                Some(e.session.clone())
            }
        })?
    }

    /// Renew a session
    pub fn renew_session(&self, session_id: &str) -> Option<Session> {
        let result = self.sessions.get_mut(session_id).and_then(|mut entry| {
            if entry.created_at.elapsed() > entry.ttl {
                None
            } else {
                entry.created_at = Instant::now();
                Some(entry.clone())
            }
        });
        if let Some(ref entry) = result {
            self.persist_session(session_id, entry);
        }
        result.map(|e| e.session)
    }

    /// List all sessions
    pub fn list_sessions(&self) -> Vec<Session> {
        let now = Instant::now();
        self.sessions
            .iter()
            .filter(|e| now.duration_since(e.created_at) <= e.ttl)
            .map(|e| e.session.clone())
            .collect()
    }

    /// List sessions for a specific node
    pub fn list_node_sessions(&self, node: &str) -> Vec<Session> {
        let now = Instant::now();
        self.sessions
            .iter()
            .filter(|e| e.session.node == node && now.duration_since(e.created_at) <= e.ttl)
            .map(|e| e.session.clone())
            .collect()
    }

    /// Clean up expired sessions
    pub fn cleanup_expired(&self) {
        let now = Instant::now();
        let expired_ids: Vec<String> = self
            .sessions
            .iter()
            .filter(|e| now.duration_since(e.created_at) > e.ttl)
            .map(|e| e.key().clone())
            .collect();
        for id in &expired_ids {
            self.sessions.remove(id);
            self.delete_session_rocks(id);
        }
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

    let session = session_service.create_session(body.into_inner());
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
    kv_service.release_session(&session_id);

    let destroyed = session_service.destroy_session(&session_id);
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

    match session_service.renew_session(&session_id) {
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

    #[test]
    fn test_session_lifecycle() {
        let service = ConsulSessionService::new();

        // Create session
        let req = SessionCreateRequest {
            name: Some("test-session".to_string()),
            ttl: Some("60s".to_string()),
            ..Default::default()
        };
        let session = service.create_session(req);
        assert!(!session.id.is_empty());
        assert_eq!(session.name, "test-session");

        // Get session
        let retrieved = service.get_session(&session.id);
        assert!(retrieved.is_some());

        // Renew session
        let renewed = service.renew_session(&session.id);
        assert!(renewed.is_some());

        // Destroy session
        let destroyed = service.destroy_session(&session.id);
        assert!(destroyed);

        // Should not exist anymore
        let gone = service.get_session(&session.id);
        assert!(gone.is_none());
    }

    #[test]
    fn test_destroy_nonexistent_session() {
        let service = ConsulSessionService::new();
        assert!(!service.destroy_session("nonexistent-id"));
    }

    #[test]
    fn test_get_nonexistent_session() {
        let service = ConsulSessionService::new();
        assert!(service.get_session("nonexistent-id").is_none());
    }

    #[test]
    fn test_renew_nonexistent_session() {
        let service = ConsulSessionService::new();
        assert!(service.renew_session("nonexistent-id").is_none());
    }

    #[test]
    fn test_list_sessions() {
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
        service.create_session(req1);
        service.create_session(req2);

        let sessions = service.list_sessions();
        assert_eq!(sessions.len(), 2);
    }

    #[test]
    fn test_list_node_sessions() {
        let service = ConsulSessionService::new();
        let node_name = &service.node_name.clone();

        let req = SessionCreateRequest {
            name: Some("node-session".to_string()),
            ttl: Some("60s".to_string()),
            node: Some(node_name.clone()),
            ..Default::default()
        };
        service.create_session(req);

        let node_sessions = service.list_node_sessions(node_name);
        assert_eq!(node_sessions.len(), 1);

        let other = service.list_node_sessions("other-node");
        assert!(other.is_empty());
    }

    #[test]
    fn test_session_behavior_release() {
        let service = ConsulSessionService::new();

        let req = SessionCreateRequest {
            name: Some("release-session".to_string()),
            behavior: Some("release".to_string()),
            ttl: Some("60s".to_string()),
            ..Default::default()
        };
        let session = service.create_session(req);
        assert_eq!(session.behavior, "release");
    }

    #[test]
    fn test_session_behavior_delete() {
        let service = ConsulSessionService::new();

        let req = SessionCreateRequest {
            name: Some("delete-session".to_string()),
            behavior: Some("delete".to_string()),
            ttl: Some("60s".to_string()),
            ..Default::default()
        };
        let session = service.create_session(req);
        assert_eq!(session.behavior, "delete");
    }

    #[test]
    fn test_session_default_ttl() {
        let service = ConsulSessionService::new();

        let req = SessionCreateRequest {
            name: Some("default-ttl".to_string()),
            ..Default::default()
        };
        let session = service.create_session(req);
        assert_eq!(session.ttl, "15s"); // default
    }

    #[test]
    fn test_session_lock_delay() {
        let service = ConsulSessionService::new();

        let req = SessionCreateRequest {
            name: Some("delayed".to_string()),
            lock_delay: Some("30s".to_string()),
            ttl: Some("60s".to_string()),
            ..Default::default()
        };
        let session = service.create_session(req);
        assert_eq!(session.lock_delay, 30);
    }

    #[test]
    fn test_session_with_checks() {
        let service = ConsulSessionService::new();

        let req = SessionCreateRequest {
            name: Some("checked-session".to_string()),
            ttl: Some("60s".to_string()),
            node_checks: Some(vec!["serfHealth".to_string()]),
            service_checks: Some(vec!["service:web".to_string()]),
            ..Default::default()
        };
        let session = service.create_session(req);
        assert_eq!(session.node_checks, Some(vec!["serfHealth".to_string()]));
        assert_eq!(
            session.service_checks,
            Some(vec!["service:web".to_string()])
        );
    }

    #[test]
    fn test_session_unique_ids() {
        let service = ConsulSessionService::new();

        let ids: Vec<String> = (0..10)
            .map(|_| {
                let req = SessionCreateRequest {
                    ttl: Some("60s".to_string()),
                    ..Default::default()
                };
                service.create_session(req).id
            })
            .collect();

        // All IDs should be unique
        let mut deduped = ids.clone();
        deduped.sort();
        deduped.dedup();
        assert_eq!(ids.len(), deduped.len());
    }

    #[test]
    fn test_destroy_then_list() {
        let service = ConsulSessionService::new();

        let session = service.create_session(SessionCreateRequest {
            name: Some("to-destroy".to_string()),
            ttl: Some("60s".to_string()),
            ..Default::default()
        });
        assert_eq!(service.list_sessions().len(), 1);

        service.destroy_session(&session.id);
        assert_eq!(service.list_sessions().len(), 0);
    }
}
