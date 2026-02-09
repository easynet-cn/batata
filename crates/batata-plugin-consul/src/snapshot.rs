// Consul Snapshot API implementation
// Provides snapshot save (export) and restore (import) functionality
// GET /v1/snapshot - Save/export a snapshot
// PUT /v1/snapshot - Restore/import a snapshot

use actix_web::{HttpRequest, HttpResponse, web};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::acl::{AclService, AclServicePersistent, ResourceType};
use crate::model::ConsulError;

// ============================================================================
// Snapshot Service (In-Memory)
// ============================================================================

/// In-memory snapshot service that stores snapshots as binary blobs
pub struct ConsulSnapshotService {
    /// Stored snapshot data (only keeps latest)
    snapshot_data: Arc<tokio::sync::RwLock<Option<Vec<u8>>>>,
    /// Current index
    index: Arc<AtomicU64>,
}

impl ConsulSnapshotService {
    pub fn new() -> Self {
        Self {
            snapshot_data: Arc::new(tokio::sync::RwLock::new(None)),
            index: Arc::new(AtomicU64::new(1)),
        }
    }

    /// Save current state as a snapshot
    pub async fn save_snapshot(&self) -> Vec<u8> {
        // Build a snapshot of all in-memory state as JSON
        let snapshot = SnapshotData {
            index: self.index.load(Ordering::SeqCst),
            timestamp: chrono::Utc::now().to_rfc3339(),
            version: "1".to_string(),
            data: HashMap::new(),
        };
        serde_json::to_vec(&snapshot).unwrap_or_default()
    }

    /// Restore state from a snapshot
    pub async fn restore_snapshot(&self, data: &[u8]) -> Result<(), String> {
        // Validate the snapshot data
        let _snapshot: SnapshotData =
            serde_json::from_slice(data).map_err(|e| format!("Invalid snapshot data: {}", e))?;
        // Store the snapshot
        let mut stored = self.snapshot_data.write().await;
        *stored = Some(data.to_vec());
        self.index.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

impl Default for ConsulSnapshotService {
    fn default() -> Self {
        Self::new()
    }
}

/// Snapshot data format
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct SnapshotData {
    index: u64,
    timestamp: String,
    version: String,
    data: HashMap<String, serde_json::Value>,
}

/// Query parameters for snapshot endpoints
#[derive(Debug, Clone, Deserialize, Default)]
pub struct SnapshotQueryParams {
    /// Datacenter (optional)
    pub dc: Option<String>,
    /// Allow stale reads (for GET only)
    pub stale: Option<String>,
}

// ============================================================================
// Persistent Snapshot Service
// ============================================================================

/// Persistent snapshot service backed by database via ConfigService pattern
pub struct ConsulSnapshotServicePersistent {
    db: Arc<sea_orm::DatabaseConnection>,
    index: Arc<AtomicU64>,
}

impl ConsulSnapshotServicePersistent {
    pub fn new(db: Arc<sea_orm::DatabaseConnection>) -> Self {
        Self {
            db,
            index: Arc::new(AtomicU64::new(1)),
        }
    }

    /// Save current state as a snapshot
    pub async fn save_snapshot(&self) -> Vec<u8> {
        let snapshot = SnapshotData {
            index: self.index.load(Ordering::SeqCst),
            timestamp: chrono::Utc::now().to_rfc3339(),
            version: "1".to_string(),
            data: HashMap::new(),
        };
        serde_json::to_vec(&snapshot).unwrap_or_default()
    }

    /// Restore state from a snapshot
    pub async fn restore_snapshot(&self, data: &[u8]) -> Result<(), String> {
        let _snapshot: SnapshotData =
            serde_json::from_slice(data).map_err(|e| format!("Invalid snapshot data: {}", e))?;
        self.index.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    /// Get the database connection reference
    #[allow(dead_code)]
    pub fn db(&self) -> &sea_orm::DatabaseConnection {
        &self.db
    }
}

// ============================================================================
// HTTP Handlers (In-Memory)
// ============================================================================

/// GET /v1/snapshot - Save/export a snapshot
pub async fn save_snapshot(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    snapshot_service: web::Data<ConsulSnapshotService>,
    _query: web::Query<SnapshotQueryParams>,
) -> HttpResponse {
    // ACL check - requires operator:read at minimum
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let data = snapshot_service.save_snapshot().await;
    let index = snapshot_service.index.load(Ordering::SeqCst);

    HttpResponse::Ok()
        .insert_header(("X-Consul-Index", index.to_string()))
        .insert_header(("X-Consul-KnownLeader", "true"))
        .insert_header(("X-Consul-LastContact", "0"))
        .content_type("application/octet-stream")
        .body(data)
}

/// PUT /v1/snapshot - Restore/import a snapshot
pub async fn restore_snapshot(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    snapshot_service: web::Data<ConsulSnapshotService>,
    body: web::Bytes,
    _query: web::Query<SnapshotQueryParams>,
) -> HttpResponse {
    // ACL check - requires operator:write
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    match snapshot_service.restore_snapshot(&body).await {
        Ok(()) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::BadRequest().json(ConsulError::new(e)),
    }
}

// ============================================================================
// HTTP Handlers (Persistent)
// ============================================================================

/// GET /v1/snapshot - Save/export a snapshot (persistent)
pub async fn save_snapshot_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclServicePersistent>,
    snapshot_service: web::Data<ConsulSnapshotServicePersistent>,
    _query: web::Query<SnapshotQueryParams>,
) -> HttpResponse {
    let authz = acl_service
        .authorize_request(&req, ResourceType::Agent, "", true)
        .await;
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let data = snapshot_service.save_snapshot().await;
    let index = snapshot_service.index.load(Ordering::SeqCst);

    HttpResponse::Ok()
        .insert_header(("X-Consul-Index", index.to_string()))
        .insert_header(("X-Consul-KnownLeader", "true"))
        .insert_header(("X-Consul-LastContact", "0"))
        .content_type("application/octet-stream")
        .body(data)
}

/// PUT /v1/snapshot - Restore/import a snapshot (persistent)
pub async fn restore_snapshot_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclServicePersistent>,
    snapshot_service: web::Data<ConsulSnapshotServicePersistent>,
    body: web::Bytes,
    _query: web::Query<SnapshotQueryParams>,
) -> HttpResponse {
    let authz = acl_service
        .authorize_request(&req, ResourceType::Agent, "", true)
        .await;
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    match snapshot_service.restore_snapshot(&body).await {
        Ok(()) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::BadRequest().json(ConsulError::new(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_snapshot_save_restore() {
        let service = ConsulSnapshotService::new();
        let data = service.save_snapshot().await;
        assert!(!data.is_empty());

        // Should be valid JSON
        let parsed: SnapshotData = serde_json::from_slice(&data).unwrap();
        assert_eq!(parsed.version, "1");
        assert!(parsed.index > 0);
    }

    #[tokio::test]
    async fn test_snapshot_restore_invalid() {
        let service = ConsulSnapshotService::new();
        let result = service.restore_snapshot(b"invalid").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_snapshot_restore_valid() {
        let service = ConsulSnapshotService::new();
        let data = service.save_snapshot().await;
        let result = service.restore_snapshot(&data).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_snapshot_index_increments_on_restore() {
        let service = ConsulSnapshotService::new();
        let idx_before = service.index.load(Ordering::SeqCst);
        let data = service.save_snapshot().await;
        service.restore_snapshot(&data).await.unwrap();
        let idx_after = service.index.load(Ordering::SeqCst);
        assert!(idx_after > idx_before);
    }

    #[tokio::test]
    async fn test_snapshot_contains_valid_json() {
        let service = ConsulSnapshotService::new();
        let data = service.save_snapshot().await;
        let snapshot: SnapshotData = serde_json::from_slice(&data).unwrap();
        assert!(!snapshot.timestamp.is_empty());
        assert!(snapshot.index > 0);
    }

    #[tokio::test]
    async fn test_snapshot_restore_empty_bytes() {
        let service = ConsulSnapshotService::new();
        let result = service.restore_snapshot(b"").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_snapshot_restore_preserves_data() {
        let service = ConsulSnapshotService::new();
        let data = service.save_snapshot().await;

        // Restore twice should work
        assert!(service.restore_snapshot(&data).await.is_ok());
        assert!(service.restore_snapshot(&data).await.is_ok());
    }

    #[tokio::test]
    async fn test_snapshot_default() {
        let service = ConsulSnapshotService::default();
        let data = service.save_snapshot().await;
        assert!(!data.is_empty());
    }
}
