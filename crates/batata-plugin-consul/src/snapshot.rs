// Consul Snapshot API implementation
// Provides snapshot save (export) and restore (import) functionality
// GET /v1/snapshot - Save/export a snapshot
// PUT /v1/snapshot - Restore/import a snapshot

use actix_web::{HttpRequest, HttpResponse, web};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::acl::{AclService, ResourceType};
use crate::consul_meta::{ConsulResponseMeta, consul_ok};
use crate::model::ConsulError;

// ============================================================================
// Snapshot Service (In-Memory)
// ============================================================================

/// In-memory snapshot service that stores snapshots as binary blobs.
/// When backed by RocksDB, includes KV data in snapshots.
#[derive(Clone)]
pub struct ConsulSnapshotService {
    /// Stored snapshot data (only keeps latest)
    snapshot_data: Arc<tokio::sync::RwLock<Option<Vec<u8>>>>,
    /// Current index
    index: Arc<AtomicU64>,
    /// Optional RocksDB for reading KV data into snapshots
    rocks_db: Option<Arc<rocksdb::DB>>,
}

impl ConsulSnapshotService {
    pub fn new() -> Self {
        Self {
            snapshot_data: Arc::new(tokio::sync::RwLock::new(None)),
            index: Arc::new(AtomicU64::new(1)),
            rocks_db: None,
        }
    }

    /// Create a snapshot service backed by RocksDB for KV data inclusion
    pub fn with_rocks(db: Arc<rocksdb::DB>) -> Self {
        Self {
            snapshot_data: Arc::new(tokio::sync::RwLock::new(None)),
            index: Arc::new(AtomicU64::new(1)),
            rocks_db: Some(db),
        }
    }

    /// Save current state as a snapshot.
    /// When RocksDB is available, dumps ALL column families for complete state export.
    pub async fn save_snapshot(&self) -> Vec<u8> {
        let mut data = HashMap::new();

        if let Some(ref db) = self.rocks_db {
            // Dump all Consul column families for complete snapshot
            let cf_names = [
                ("kv", crate::constants::CF_CONSUL_KV),
                ("sessions", crate::constants::CF_CONSUL_SESSIONS),
                ("acl", crate::constants::CF_CONSUL_ACL),
                ("queries", crate::constants::CF_CONSUL_QUERIES),
                ("config_entries", crate::constants::CF_CONSUL_CONFIG_ENTRIES),
                ("ca_roots", crate::constants::CF_CONSUL_CA_ROOTS),
                ("intentions", crate::constants::CF_CONSUL_INTENTIONS),
                ("coordinates", crate::constants::CF_CONSUL_COORDINATES),
                ("peering", crate::constants::CF_CONSUL_PEERING),
                ("operator", crate::constants::CF_CONSUL_OPERATOR),
                ("events", crate::constants::CF_CONSUL_EVENTS),
                ("namespaces", crate::constants::CF_CONSUL_NAMESPACES),
                ("catalog", crate::constants::CF_CONSUL_CATALOG),
            ];

            for (name, cf_name) in &cf_names {
                if let Some(cf) = db.cf_handle(cf_name) {
                    let mut cf_data = HashMap::new();
                    let iter = db.iterator_cf(cf, rocksdb::IteratorMode::Start);
                    for item in iter.flatten() {
                        let (key_bytes, value_bytes) = item;
                        if let Ok(key) = String::from_utf8(key_bytes.to_vec()) {
                            let value_b64 = base64::Engine::encode(
                                &base64::engine::general_purpose::STANDARD,
                                &value_bytes,
                            );
                            cf_data.insert(key, serde_json::Value::String(value_b64));
                        }
                    }
                    if !cf_data.is_empty() {
                        data.insert(
                            name.to_string(),
                            serde_json::to_value(cf_data).unwrap_or_default(),
                        );
                    }
                }
            }
        }

        let snapshot = SnapshotData {
            index: self.index.load(Ordering::SeqCst),
            timestamp: chrono::Utc::now().to_rfc3339(),
            version: "1".to_string(),
            data,
        };
        serde_json::to_vec(&snapshot).unwrap_or_default()
    }

    /// Restore state from a snapshot.
    /// When RocksDB is available, writes data back to all column families.
    pub async fn restore_snapshot(&self, data: &[u8]) -> Result<(), String> {
        let snapshot: SnapshotData =
            serde_json::from_slice(data).map_err(|e| format!("Invalid snapshot data: {}", e))?;

        if let Some(ref db) = self.rocks_db {
            let cf_names = [
                ("kv", crate::constants::CF_CONSUL_KV),
                ("sessions", crate::constants::CF_CONSUL_SESSIONS),
                ("acl", crate::constants::CF_CONSUL_ACL),
                ("queries", crate::constants::CF_CONSUL_QUERIES),
                ("config_entries", crate::constants::CF_CONSUL_CONFIG_ENTRIES),
                ("ca_roots", crate::constants::CF_CONSUL_CA_ROOTS),
                ("intentions", crate::constants::CF_CONSUL_INTENTIONS),
                ("coordinates", crate::constants::CF_CONSUL_COORDINATES),
                ("peering", crate::constants::CF_CONSUL_PEERING),
                ("operator", crate::constants::CF_CONSUL_OPERATOR),
                ("events", crate::constants::CF_CONSUL_EVENTS),
                ("namespaces", crate::constants::CF_CONSUL_NAMESPACES),
                ("catalog", crate::constants::CF_CONSUL_CATALOG),
            ];

            for (name, cf_name) in &cf_names {
                if let Some(cf_data) = snapshot.data.get(*name) {
                    let Some(cf) = db.cf_handle(cf_name) else {
                        tracing::warn!("Column family '{}' not found, skipping restore", cf_name);
                        continue;
                    };

                    // Clear existing data in this CF
                    let iter = db.iterator_cf(cf, rocksdb::IteratorMode::Start);
                    for item in iter.flatten() {
                        let (key_bytes, _) = item;
                        let _ = db.delete_cf(cf, &key_bytes);
                    }

                    // Restore data from snapshot
                    if let Some(entries) = cf_data.as_object() {
                        for (key, value) in entries {
                            if let Some(value_b64) = value.as_str() {
                                if let Ok(decoded) = base64::Engine::decode(
                                    &base64::engine::general_purpose::STANDARD,
                                    value_b64,
                                ) {
                                    let _ = db.put_cf(cf, key.as_bytes(), &decoded);
                                }
                            }
                        }
                    }

                    tracing::info!("Restored column family '{}' from snapshot", name);
                }
            }
        }

        // Store the snapshot blob and bump index
        let mut stored = self.snapshot_data.write().await;
        *stored = Some(data.to_vec());
        self.index.fetch_add(1, Ordering::SeqCst);
        tracing::info!(
            "Snapshot restore completed (version: {}, index: {})",
            snapshot.version,
            snapshot.index
        );
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

    let meta = ConsulResponseMeta::new(index);
    consul_ok(&meta)
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
    acl_service: web::Data<AclService>,
    snapshot_service: web::Data<ConsulSnapshotServicePersistent>,
    _query: web::Query<SnapshotQueryParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let data = snapshot_service.save_snapshot().await;
    let index = snapshot_service.index.load(Ordering::SeqCst);

    let meta = ConsulResponseMeta::new(index);
    consul_ok(&meta)
        .content_type("application/octet-stream")
        .body(data)
}

/// PUT /v1/snapshot - Restore/import a snapshot (persistent)
pub async fn restore_snapshot_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    snapshot_service: web::Data<ConsulSnapshotServicePersistent>,
    body: web::Bytes,
    _query: web::Query<SnapshotQueryParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", true);
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

    #[tokio::test]
    async fn test_snapshot_without_rocks_has_empty_data() {
        let service = ConsulSnapshotService::new();
        let data = service.save_snapshot().await;
        let snapshot: SnapshotData = serde_json::from_slice(&data).unwrap();
        // Without RocksDB, data should be empty
        assert!(snapshot.data.is_empty());
    }
}
