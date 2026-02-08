//! Consul Session API HTTP handlers
//!
//! Implements Consul-compatible session management for distributed locking.

use std::sync::Arc;
use std::time::{Duration, Instant};

use actix_web::{HttpRequest, HttpResponse, web};
use dashmap::DashMap;
use serde::Deserialize;

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

/// Consul Session service
/// Provides distributed locking via session management
#[derive(Clone)]
pub struct ConsulSessionService {
    sessions: Arc<DashMap<String, SessionEntry>>,
    node_name: String,
}

impl ConsulSessionService {
    pub fn new() -> Self {
        let node_name = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "batata-node".to_string());

        Self {
            sessions: Arc::new(DashMap::new()),
            node_name,
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

        self.sessions.insert(session_id, entry);
        session
    }

    /// Destroy a session
    pub fn destroy_session(&self, session_id: &str) -> bool {
        self.sessions.remove(session_id).is_some()
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
        self.sessions.get_mut(session_id).and_then(|mut entry| {
            if entry.created_at.elapsed() > entry.ttl {
                None
            } else {
                entry.created_at = Instant::now();
                Some(entry.session.clone())
            }
        })
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
        self.sessions
            .retain(|_, entry| now.duration_since(entry.created_at) <= entry.ttl);
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

// ============================================================================
// Persistent Session Service (Using ConfigService)
// ============================================================================

use std::sync::Arc as StdArc;

use sea_orm::DatabaseConnection;
use serde::Serialize;

/// Constants for ConfigService mapping
const CONSUL_SESSION_NAMESPACE: &str = "public";
const CONSUL_SESSION_GROUP: &str = "consul-sessions";

/// Stored session metadata for persistent storage
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionMetadata {
    session: Session,
    created_at_unix: u64,
    ttl_secs: u64,
}

/// Consul Session service with database persistence
/// Uses Batata's ConfigService for storage
#[derive(Clone)]
pub struct ConsulSessionServicePersistent {
    db: StdArc<DatabaseConnection>,
    node_name: String,
    /// In-memory cache for sessions
    session_cache: StdArc<DashMap<String, SessionMetadata>>,
}

impl ConsulSessionServicePersistent {
    pub fn new(db: StdArc<DatabaseConnection>) -> Self {
        let node_name = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "batata-node".to_string());

        Self {
            db,
            node_name,
            session_cache: StdArc::new(DashMap::new()),
        }
    }

    fn session_data_id(session_id: &str) -> String {
        format!("session:{}", session_id.replace('/', ":"))
    }

    fn current_unix_time() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    async fn save_session(&self, metadata: &SessionMetadata) -> Result<(), String> {
        let content =
            serde_json::to_string(metadata).map_err(|e| format!("Serialization error: {}", e))?;
        let data_id = Self::session_data_id(&metadata.session.id);

        batata_config::service::config::create_or_update(
            &self.db,
            &data_id,
            CONSUL_SESSION_GROUP,
            CONSUL_SESSION_NAMESPACE,
            &content,
            "consul-session",
            "system",
            "127.0.0.1",
            "",
            &format!("Consul Session: {}", metadata.session.name),
            "",
            "",
            "json",
            "",
            "",
        )
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        // Update cache
        self.session_cache
            .insert(metadata.session.id.clone(), metadata.clone());
        Ok(())
    }

    async fn delete_session_from_db(&self, session_id: &str) -> bool {
        let data_id = Self::session_data_id(session_id);
        self.session_cache.remove(session_id);
        batata_config::service::config::delete(
            &self.db,
            &data_id,
            CONSUL_SESSION_GROUP,
            CONSUL_SESSION_NAMESPACE,
            "",
            "127.0.0.1",
            "system",
        )
        .await
        .unwrap_or(false)
    }

    fn is_session_expired(metadata: &SessionMetadata) -> bool {
        let now = Self::current_unix_time();
        now > metadata.created_at_unix + metadata.ttl_secs
    }

    /// Create a new session
    pub async fn create_session(&self, req: SessionCreateRequest) -> Result<Session, String> {
        let session_id = uuid::Uuid::new_v4().to_string();
        let now = Self::current_unix_time();

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

        let metadata = SessionMetadata {
            session: session.clone(),
            created_at_unix: now,
            ttl_secs,
        };

        self.save_session(&metadata).await?;
        Ok(session)
    }

    /// Destroy a session
    pub async fn destroy_session(&self, session_id: &str) -> bool {
        self.delete_session_from_db(session_id).await
    }

    /// Get session info
    pub async fn get_session(&self, session_id: &str) -> Option<Session> {
        // Check cache first
        if let Some(metadata) = self.session_cache.get(session_id) {
            if Self::is_session_expired(&metadata) {
                // Clean up expired session
                self.session_cache.remove(session_id);
                let _ = self.delete_session_from_db(session_id).await;
                return None;
            }
            return Some(metadata.session.clone());
        }

        // Query from database
        let data_id = Self::session_data_id(session_id);
        if let Ok(Some(config)) = batata_config::service::config::find_one(
            &self.db,
            &data_id,
            CONSUL_SESSION_GROUP,
            CONSUL_SESSION_NAMESPACE,
        )
        .await
            && let Ok(metadata) = serde_json::from_str::<SessionMetadata>(
                &config.config_info.config_info_base.content,
            )
        {
            if Self::is_session_expired(&metadata) {
                let _ = self.delete_session_from_db(session_id).await;
                return None;
            }
            self.session_cache
                .insert(session_id.to_string(), metadata.clone());
            return Some(metadata.session);
        }

        None
    }

    /// Renew a session
    pub async fn renew_session(&self, session_id: &str) -> Option<Session> {
        // Get current session
        let metadata = if let Some(m) = self.session_cache.get(session_id) {
            m.clone()
        } else {
            let data_id = Self::session_data_id(session_id);
            if let Ok(Some(config)) = batata_config::service::config::find_one(
                &self.db,
                &data_id,
                CONSUL_SESSION_GROUP,
                CONSUL_SESSION_NAMESPACE,
            )
            .await
                && let Ok(m) = serde_json::from_str::<SessionMetadata>(
                    &config.config_info.config_info_base.content,
                )
            {
                m
            } else {
                return None;
            }
        };

        if Self::is_session_expired(&metadata) {
            let _ = self.delete_session_from_db(session_id).await;
            return None;
        }

        // Renew by updating created_at
        let now = Self::current_unix_time();
        let mut updated = metadata;
        updated.created_at_unix = now;
        updated.session.modify_index = now;

        if self.save_session(&updated).await.is_ok() {
            Some(updated.session)
        } else {
            None
        }
    }

    /// List all sessions
    pub async fn list_sessions(&self) -> Vec<Session> {
        let mut sessions = Vec::new();

        if let Ok(page) = batata_config::service::config::search_page(
            &self.db,
            1,
            1000,
            CONSUL_SESSION_NAMESPACE,
            "session:*",
            CONSUL_SESSION_GROUP,
            "",
            vec![],
            vec![],
            "",
        )
        .await
        {
            for info in page.page_items {
                if let Ok(Some(config)) = batata_config::service::config::find_one(
                    &self.db,
                    &info.data_id,
                    CONSUL_SESSION_GROUP,
                    CONSUL_SESSION_NAMESPACE,
                )
                .await
                    && let Ok(metadata) = serde_json::from_str::<SessionMetadata>(
                        &config.config_info.config_info_base.content,
                    )
                    && !Self::is_session_expired(&metadata)
                {
                    sessions.push(metadata.session);
                }
            }
        }

        sessions
    }

    /// List sessions for a specific node
    pub async fn list_node_sessions(&self, node: &str) -> Vec<Session> {
        self.list_sessions()
            .await
            .into_iter()
            .filter(|s| s.node == node)
            .collect()
    }

    /// Clean up expired sessions
    pub async fn cleanup_expired(&self) {
        // Clean cache
        let expired_ids: Vec<String> = self
            .session_cache
            .iter()
            .filter(|e| Self::is_session_expired(e.value()))
            .map(|e| e.key().clone())
            .collect();

        for id in expired_ids {
            self.session_cache.remove(&id);
            let _ = self.delete_session_from_db(&id).await;
        }

        // Also clean database entries not in cache
        if let Ok(page) = batata_config::service::config::search_page(
            &self.db,
            1,
            1000,
            CONSUL_SESSION_NAMESPACE,
            "session:*",
            CONSUL_SESSION_GROUP,
            "",
            vec![],
            vec![],
            "",
        )
        .await
        {
            for info in page.page_items {
                if let Ok(Some(config)) = batata_config::service::config::find_one(
                    &self.db,
                    &info.data_id,
                    CONSUL_SESSION_GROUP,
                    CONSUL_SESSION_NAMESPACE,
                )
                .await
                    && let Ok(metadata) = serde_json::from_str::<SessionMetadata>(
                        &config.config_info.config_info_base.content,
                    )
                    && Self::is_session_expired(&metadata)
                {
                    let _ = self.delete_session_from_db(&metadata.session.id).await;
                }
            }
        }
    }
}

// ============================================================================
// Persistent HTTP Handlers
// ============================================================================

/// PUT /v1/session/create (Persistent)
/// Creates a new session with database persistence
pub async fn create_session_persistent(
    req: HttpRequest,
    session_service: web::Data<ConsulSessionServicePersistent>,
    acl_service: web::Data<AclService>,
    body: web::Json<SessionCreateRequest>,
) -> HttpResponse {
    // Check ACL authorization for session write
    let authz = acl_service.authorize_request(&req, ResourceType::Session, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    match session_service.create_session(body.into_inner()).await {
        Ok(session) => {
            let response = SessionCreateResponse {
                id: session.id.clone(),
            };
            HttpResponse::Ok().json(response)
        }
        Err(e) => HttpResponse::InternalServerError().json(ConsulError::new(&e)),
    }
}

/// PUT /v1/session/destroy/{uuid} (Persistent)
/// Destroys a session with database persistence
pub async fn destroy_session_persistent(
    req: HttpRequest,
    session_service: web::Data<ConsulSessionServicePersistent>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
) -> HttpResponse {
    let session_id = path.into_inner();

    // Check ACL authorization for session write
    let authz = acl_service.authorize_request(&req, ResourceType::Session, &session_id, true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let destroyed = session_service.destroy_session(&session_id).await;
    HttpResponse::Ok().json(destroyed)
}

/// GET /v1/session/info/{uuid} (Persistent)
/// Returns information about a specific session with database persistence
pub async fn get_session_info_persistent(
    req: HttpRequest,
    session_service: web::Data<ConsulSessionServicePersistent>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
) -> HttpResponse {
    let session_id = path.into_inner();

    // Check ACL authorization for session read
    let authz = acl_service.authorize_request(&req, ResourceType::Session, &session_id, false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    match session_service.get_session(&session_id).await {
        Some(session) => HttpResponse::Ok().json(vec![session]),
        None => HttpResponse::Ok().json(Vec::<Session>::new()),
    }
}

/// GET /v1/session/list (Persistent)
/// Returns all active sessions with database persistence
pub async fn list_sessions_persistent(
    req: HttpRequest,
    session_service: web::Data<ConsulSessionServicePersistent>,
    acl_service: web::Data<AclService>,
) -> HttpResponse {
    // Check ACL authorization for session read
    let authz = acl_service.authorize_request(&req, ResourceType::Session, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let sessions = session_service.list_sessions().await;
    HttpResponse::Ok().json(sessions)
}

/// GET /v1/session/node/{node} (Persistent)
/// Returns sessions for a specific node with database persistence
pub async fn list_node_sessions_persistent(
    req: HttpRequest,
    session_service: web::Data<ConsulSessionServicePersistent>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
) -> HttpResponse {
    let node = path.into_inner();

    // Check ACL authorization for session read
    let authz = acl_service.authorize_request(&req, ResourceType::Session, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let sessions = session_service.list_node_sessions(&node).await;
    HttpResponse::Ok().json(sessions)
}

/// PUT /v1/session/renew/{uuid} (Persistent)
/// Renews a session TTL with database persistence
pub async fn renew_session_persistent(
    req: HttpRequest,
    session_service: web::Data<ConsulSessionServicePersistent>,
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
}
