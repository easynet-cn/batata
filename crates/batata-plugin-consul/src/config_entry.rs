// Consul Config Entries API implementation
// Provides centralized configuration management for service mesh and other features
//
// Endpoints:
// - GET    /v1/config/{kind}         - List entries of a kind
// - GET    /v1/config/{kind}/{name}  - Read specific entry
// - PUT    /v1/config                - Apply/create/update entry
// - DELETE /v1/config/{kind}/{name}  - Delete entry

use actix_web::{HttpRequest, HttpResponse, web};
use dashmap::DashMap;
use rocksdb::DB;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::{error, info};

use crate::constants::CF_CONSUL_CONFIG_ENTRIES;
use crate::raft::{ConsulRaftRequest, ConsulRaftWriter};

use crate::acl::{AclService, ResourceType};
use crate::consul_meta::{ConsulResponseMeta, consul_ok};
use crate::index_provider::{ConsulIndexProvider, ConsulTable};
use crate::model::ConsulError;

// ============================================================================
// Models
// ============================================================================

/// Supported config entry kinds
pub const SUPPORTED_KINDS: &[&str] = &[
    "service-defaults",
    "proxy-defaults",
    "service-router",
    "service-splitter",
    "service-resolver",
    "ingress-gateway",
    "terminating-gateway",
    "service-intentions",
    "mesh",
    "exported-services",
    "api-gateway",
    "http-route",
    "tcp-route",
    "jwt-provider",
];

/// Config entry (generic JSON-based)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ConfigEntry {
    pub kind: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partition: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<HashMap<String, String>>,
    /// Additional fields stored as dynamic JSON
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
    pub create_index: u64,
    pub modify_index: u64,
}

/// Query parameters for config entry list
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ConfigEntryListParams {
    pub dc: Option<String>,
    pub index: Option<u64>,
    pub wait: Option<String>,
    pub filter: Option<String>,
}

/// Query parameters for config entry apply
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ConfigEntryApplyParams {
    pub dc: Option<String>,
    pub cas: Option<u64>,
}

/// Query parameters for config entry delete
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ConfigEntryDeleteParams {
    pub dc: Option<String>,
    pub cas: Option<u64>,
}

/// Config entry apply request body (incoming JSON)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ConfigEntryRequest {
    pub kind: String,
    #[serde(default)]
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partition: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<HashMap<String, String>>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

// ============================================================================
// Config Entry Service (In-Memory + optional RocksDB persistence)
// ============================================================================

/// Config entry service with optional RocksDB write-through persistence
#[derive(Clone)]
pub struct ConsulConfigEntryService {
    /// Entries stored by "kind/name" key
    entries: Arc<DashMap<String, ConfigEntry>>,
    /// Current index
    index: Arc<AtomicU64>,
    /// Optional RocksDB for write-through persistence
    rocks_db: Option<Arc<DB>>,
    /// Optional Raft writer for cluster mode
    raft_node: Option<Arc<ConsulRaftWriter>>,
}

impl ConsulConfigEntryService {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(DashMap::new()),
            index: Arc::new(AtomicU64::new(1)),
            rocks_db: None,
            raft_node: None,
        }
    }

    /// Create a config entry service backed by an existing RocksDB instance.
    /// Loads existing entries from the CF_CONSUL_CONFIG_ENTRIES column family.
    pub fn with_rocks(db: Arc<DB>) -> Self {
        let entries = Arc::new(DashMap::new());
        let mut max_index = 0u64;

        if let Some(cf) = db.cf_handle(CF_CONSUL_CONFIG_ENTRIES) {
            let iter = db.iterator_cf(cf, rocksdb::IteratorMode::Start);
            let mut count = 0u64;

            for item in iter.flatten() {
                let (key_bytes, value_bytes) = item;
                if let Ok(key) = String::from_utf8(key_bytes.to_vec()) {
                    match serde_json::from_slice::<ConfigEntry>(&value_bytes) {
                        Ok(entry) => {
                            if entry.modify_index > max_index {
                                max_index = entry.modify_index;
                            }
                            entries.insert(key, entry);
                            count += 1;
                        }
                        Err(e) => {
                            error!(
                                "Failed to deserialize config entry '{}' from RocksDB: {}",
                                key, e
                            );
                        }
                    }
                }
            }
            info!(
                "Loaded {} config entries from RocksDB (max_index={})",
                count, max_index
            );
        }

        Self {
            entries,
            index: Arc::new(AtomicU64::new(max_index + 1)),
            rocks_db: Some(db),
            raft_node: None,
        }
    }

    /// Create a config entry service with Raft-replicated storage (cluster mode).
    pub fn with_raft(db: Arc<DB>, raft_node: Arc<ConsulRaftWriter>) -> Self {
        let mut svc = Self::with_rocks(db);
        svc.raft_node = Some(raft_node);
        svc
    }

    /// Get the shared entries DashMap for the plugin handler.
    pub fn entries_arc(&self) -> Arc<DashMap<String, ConfigEntry>> {
        self.entries.clone()
    }

    fn entry_key(kind: &str, name: &str) -> String {
        format!("{}/{}", kind, name)
    }

    /// Persist an entry to RocksDB (write-through)
    fn persist_to_rocks(&self, key: &str, entry: &ConfigEntry) {
        if let Some(ref db) = self.rocks_db {
            if let Some(cf) = db.cf_handle(CF_CONSUL_CONFIG_ENTRIES) {
                match serde_json::to_vec(entry) {
                    Ok(json_bytes) => {
                        if let Err(e) = db.put_cf(cf, key.as_bytes(), &json_bytes) {
                            error!("Failed to persist config entry '{}' to RocksDB: {}", key, e);
                        }
                    }
                    Err(e) => {
                        error!(
                            "Failed to serialize config entry '{}' for RocksDB: {}",
                            key, e
                        );
                    }
                }
            }
        }
    }

    /// Delete an entry from RocksDB
    fn delete_from_rocks(&self, key: &str) {
        if let Some(ref db) = self.rocks_db {
            if let Some(cf) = db.cf_handle(CF_CONSUL_CONFIG_ENTRIES) {
                if let Err(e) = db.delete_cf(cf, key.as_bytes()) {
                    error!(
                        "Failed to delete config entry '{}' from RocksDB: {}",
                        key, e
                    );
                }
            }
        }
    }

    pub fn list_entries(&self, kind: &str) -> Vec<ConfigEntry> {
        let prefix = format!("{}/", kind);
        self.entries
            .iter()
            .filter(|r| r.key().starts_with(&prefix))
            .map(|r| r.value().clone())
            .collect()
    }

    pub fn get_entry(&self, kind: &str, name: &str) -> Option<ConfigEntry> {
        let key = Self::entry_key(kind, name);
        self.entries.get(&key).map(|r| r.value().clone())
    }

    pub async fn apply_entry(
        &self,
        mut req: ConfigEntryRequest,
        cas: Option<u64>,
    ) -> Result<bool, String> {
        // Global entries (mesh, proxy-defaults) use kind as name if name is empty
        if req.name.is_empty() {
            req.name = req.kind.clone();
        }
        let key = Self::entry_key(&req.kind, &req.name);

        if let Some(cas_index) = cas {
            if let Some(existing) = self.entries.get(&key) {
                if existing.modify_index != cas_index {
                    return Ok(false);
                }
            } else if cas_index != 0 {
                return Ok(false);
            }
        }

        let new_index = self.index.fetch_add(1, Ordering::SeqCst) + 1;
        let existing_create_index = self
            .entries
            .get(&key)
            .map(|r| r.value().create_index)
            .unwrap_or(new_index);

        let entry = ConfigEntry {
            kind: req.kind,
            name: req.name,
            namespace: req.namespace,
            partition: req.partition,
            meta: req.meta,
            extra: req.extra,
            create_index: existing_create_index,
            modify_index: new_index,
        };

        if let Some(ref raft) = self.raft_node {
            let entry_json =
                serde_json::to_string(&entry).map_err(|e| format!("serialize error: {}", e))?;
            match raft
                .write(ConsulRaftRequest::ConfigEntryApply {
                    key: key.clone(),
                    entry_json,
                })
                .await
            {
                Ok(r) if r.success => {
                    self.entries.insert(key, entry);
                }
                Ok(r) => return Err(r.message.unwrap_or_else(|| "Raft write rejected".into())),
                Err(e) => return Err(format!("Raft write error: {}", e)),
            }
        } else {
            self.entries.insert(key.clone(), entry.clone());
            self.persist_to_rocks(&key, &entry);
        }
        Ok(true)
    }

    pub async fn delete_entry(
        &self,
        kind: &str,
        name: &str,
        cas: Option<u64>,
    ) -> Result<bool, String> {
        let key = Self::entry_key(kind, name);

        if let Some(cas_index) = cas {
            if let Some(existing) = self.entries.get(&key) {
                if existing.modify_index != cas_index {
                    return Ok(false);
                }
            } else {
                return Ok(false);
            }
        }

        if let Some(ref raft) = self.raft_node {
            match raft
                .write(ConsulRaftRequest::ConfigEntryDelete { key: key.clone() })
                .await
            {
                Ok(r) if r.success => {
                    self.entries.remove(&key);
                }
                Ok(r) => return Err(r.message.unwrap_or_else(|| "Raft write rejected".into())),
                Err(e) => return Err(format!("Raft write error: {}", e)),
            }
        } else {
            self.entries.remove(&key);
            self.delete_from_rocks(&key);
        }
        self.index.fetch_add(1, Ordering::SeqCst);
        Ok(true)
    }
}

impl Default for ConsulConfigEntryService {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// HTTP Handlers (In-Memory)
// ============================================================================

/// GET /v1/config/{kind} - List config entries of a kind
pub async fn list_config_entries(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    config_service: web::Data<ConsulConfigEntryService>,
    path: web::Path<String>,
    _query: web::Query<ConfigEntryListParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let kind = path.into_inner();
    if !SUPPORTED_KINDS.contains(&kind.as_str()) {
        return HttpResponse::BadRequest().json(ConsulError::new(format!(
            "Unsupported config entry kind: {}",
            kind
        )));
    }

    let entries = config_service.list_entries(&kind);
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ConfigEntries));
    consul_ok(&meta).json(entries)
}

/// GET /v1/config/{kind}/{name} - Read a specific config entry
pub async fn get_config_entry(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    config_service: web::Data<ConsulConfigEntryService>,
    path: web::Path<(String, String)>,
    _query: web::Query<ConfigEntryListParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let (kind, name) = path.into_inner();
    match config_service.get_entry(&kind, &name) {
        Some(entry) => {
            let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ConfigEntries));
            consul_ok(&meta).json(entry)
        }
        None => HttpResponse::NotFound().json(ConsulError::new("Config entry not found")),
    }
}

/// PUT /v1/config - Apply/create/update a config entry
pub async fn apply_config_entry(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    config_service: web::Data<ConsulConfigEntryService>,
    index_provider: web::Data<ConsulIndexProvider>,
    body: web::Bytes,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let entry_req: ConfigEntryRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            return HttpResponse::BadRequest()
                .json(ConsulError::new(format!("Invalid request body: {}", e)));
        }
    };

    if !SUPPORTED_KINDS.contains(&entry_req.kind.as_str()) {
        return HttpResponse::BadRequest().json(ConsulError::new(format!(
            "Unsupported config entry kind: {}",
            entry_req.kind
        )));
    }

    // Parse optional CAS parameter from query string
    let cas: Option<u64> = req.uri().query().and_then(|q| {
        q.split('&').find_map(|pair| {
            let mut kv = pair.splitn(2, '=');
            if kv.next() == Some("cas") {
                kv.next().and_then(|v| v.parse().ok())
            } else {
                None
            }
        })
    });

    match config_service.apply_entry(entry_req, cas).await {
        Ok(success) => {
            let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ConfigEntries));
            consul_ok(&meta).json(success)
        }
        Err(e) => HttpResponse::InternalServerError().json(ConsulError::new(e)),
    }
}

/// DELETE /v1/config/{kind}/{name} - Delete a config entry
pub async fn delete_config_entry(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    config_service: web::Data<ConsulConfigEntryService>,
    path: web::Path<(String, String)>,
    query: web::Query<ConfigEntryDeleteParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let (kind, name) = path.into_inner();
    match config_service.delete_entry(&kind, &name, query.cas).await {
        Ok(success) => {
            let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ConfigEntries));
            if query.cas.is_some() {
                consul_ok(&meta).json(success)
            } else {
                consul_ok(&meta).json(serde_json::json!({}))
            }
        }
        Err(e) => HttpResponse::InternalServerError().json(ConsulError::new(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_apply_and_get_entry() {
        let service = ConsulConfigEntryService::new();
        let req = ConfigEntryRequest {
            kind: "service-defaults".to_string(),
            name: "web".to_string(),
            namespace: None,
            partition: None,
            meta: None,
            extra: {
                let mut m = HashMap::new();
                m.insert(
                    "Protocol".to_string(),
                    serde_json::Value::String("http".to_string()),
                );
                m
            },
        };

        let result = service.apply_entry(req, None).await;
        assert!(result.is_ok());
        assert!(result.unwrap());

        let entry = service.get_entry("service-defaults", "web");
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert_eq!(entry.kind, "service-defaults");
        assert_eq!(entry.name, "web");
    }

    #[tokio::test]
    async fn test_list_entries_by_kind() {
        let service = ConsulConfigEntryService::new();

        // Add two service-defaults entries
        service
            .apply_entry(
                ConfigEntryRequest {
                    kind: "service-defaults".to_string(),
                    name: "web".to_string(),
                    namespace: None,
                    partition: None,
                    meta: None,
                    extra: HashMap::new(),
                },
                None,
            )
            .await
            .unwrap();

        service
            .apply_entry(
                ConfigEntryRequest {
                    kind: "service-defaults".to_string(),
                    name: "api".to_string(),
                    namespace: None,
                    partition: None,
                    meta: None,
                    extra: HashMap::new(),
                },
                None,
            )
            .await
            .unwrap();

        // Add one proxy-defaults entry
        service
            .apply_entry(
                ConfigEntryRequest {
                    kind: "proxy-defaults".to_string(),
                    name: "global".to_string(),
                    namespace: None,
                    partition: None,
                    meta: None,
                    extra: HashMap::new(),
                },
                None,
            )
            .await
            .unwrap();

        let sd_entries = service.list_entries("service-defaults");
        assert_eq!(sd_entries.len(), 2);

        let pd_entries = service.list_entries("proxy-defaults");
        assert_eq!(pd_entries.len(), 1);
    }

    #[tokio::test]
    async fn test_delete_entry() {
        let service = ConsulConfigEntryService::new();
        service
            .apply_entry(
                ConfigEntryRequest {
                    kind: "service-defaults".to_string(),
                    name: "web".to_string(),
                    namespace: None,
                    partition: None,
                    meta: None,
                    extra: HashMap::new(),
                },
                None,
            )
            .await
            .unwrap();

        assert!(service.get_entry("service-defaults", "web").is_some());
        service
            .delete_entry("service-defaults", "web", None)
            .await
            .unwrap();
        assert!(service.get_entry("service-defaults", "web").is_none());
    }

    #[tokio::test]
    async fn test_cas_apply() {
        let service = ConsulConfigEntryService::new();
        let req = ConfigEntryRequest {
            kind: "service-defaults".to_string(),
            name: "web".to_string(),
            namespace: None,
            partition: None,
            meta: None,
            extra: HashMap::new(),
        };

        // First apply
        service.apply_entry(req.clone(), None).await.unwrap();
        let entry = service.get_entry("service-defaults", "web").unwrap();

        // CAS with wrong index
        let result = service.apply_entry(req.clone(), Some(999)).await;
        assert!(!result.unwrap());

        // CAS with correct index
        let result = service.apply_entry(req, Some(entry.modify_index)).await;
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_cas_delete() {
        let service = ConsulConfigEntryService::new();
        service
            .apply_entry(
                ConfigEntryRequest {
                    kind: "mesh".to_string(),
                    name: "mesh".to_string(),
                    namespace: None,
                    partition: None,
                    meta: None,
                    extra: HashMap::new(),
                },
                None,
            )
            .await
            .unwrap();
        let entry = service.get_entry("mesh", "mesh").unwrap();

        // CAS with wrong index
        let result = service.delete_entry("mesh", "mesh", Some(999)).await;
        assert!(!result.unwrap());

        // Entry should still exist
        assert!(service.get_entry("mesh", "mesh").is_some());

        // CAS with correct index
        let result = service
            .delete_entry("mesh", "mesh", Some(entry.modify_index))
            .await;
        assert!(result.unwrap());
        assert!(service.get_entry("mesh", "mesh").is_none());
    }
}
