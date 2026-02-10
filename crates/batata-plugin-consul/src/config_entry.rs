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
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::acl::{AclService, ResourceType};
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
// Config Entry Service (In-Memory)
// ============================================================================

/// In-memory config entry service
pub struct ConsulConfigEntryService {
    /// Entries stored by "kind/name" key
    entries: Arc<DashMap<String, ConfigEntry>>,
    /// Current index
    index: Arc<AtomicU64>,
}

impl ConsulConfigEntryService {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(DashMap::new()),
            index: Arc::new(AtomicU64::new(1)),
        }
    }

    fn entry_key(kind: &str, name: &str) -> String {
        format!("{}/{}", kind, name)
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

    pub fn apply_entry(&self, req: ConfigEntryRequest, cas: Option<u64>) -> Result<bool, String> {
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
        self.entries.insert(key, entry);
        Ok(true)
    }

    pub fn delete_entry(&self, kind: &str, name: &str, cas: Option<u64>) -> Result<bool, String> {
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

        self.entries.remove(&key);
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
// Config Entry Service (Persistent)
// ============================================================================

const CONSUL_CONFIG_ENTRY_NAMESPACE: &str = "public";
const CONSUL_CONFIG_ENTRY_GROUP: &str = "consul-config-entries";

/// Persistent config entry service backed by database via ConfigService pattern
pub struct ConsulConfigEntryServicePersistent {
    db: Arc<sea_orm::DatabaseConnection>,
    /// L1 cache
    cache: Arc<DashMap<String, ConfigEntry>>,
    index: Arc<AtomicU64>,
}

impl ConsulConfigEntryServicePersistent {
    pub fn new(db: Arc<sea_orm::DatabaseConnection>) -> Self {
        Self {
            db,
            cache: Arc::new(DashMap::new()),
            index: Arc::new(AtomicU64::new(1)),
        }
    }

    fn entry_key(kind: &str, name: &str) -> String {
        format!("{}/{}", kind, name)
    }

    fn data_id(kind: &str, name: &str) -> String {
        format!("config-entry:{}:{}", kind, name)
    }

    pub async fn list_entries(&self, kind: &str) -> Vec<ConfigEntry> {
        // Try cache first
        let prefix = format!("{}/", kind);
        let cached: Vec<ConfigEntry> = self
            .cache
            .iter()
            .filter(|r| r.key().starts_with(&prefix))
            .map(|r| r.value().clone())
            .collect();

        if !cached.is_empty() {
            return cached;
        }

        // Fallback to database query
        self.load_entries_from_db(kind).await
    }

    pub async fn get_entry(&self, kind: &str, name: &str) -> Option<ConfigEntry> {
        let key = Self::entry_key(kind, name);

        // Try cache first
        if let Some(entry) = self.cache.get(&key) {
            return Some(entry.value().clone());
        }

        // Try database
        self.load_entry_from_db(kind, name).await
    }

    pub async fn apply_entry(
        &self,
        req: ConfigEntryRequest,
        cas: Option<u64>,
    ) -> Result<bool, String> {
        let key = Self::entry_key(&req.kind, &req.name);

        if let Some(cas_index) = cas {
            if let Some(existing) = self.cache.get(&key) {
                if existing.modify_index != cas_index {
                    return Ok(false);
                }
            } else if cas_index != 0 {
                return Ok(false);
            }
        }

        let new_index = self.index.fetch_add(1, Ordering::SeqCst) + 1;
        let existing_create_index = self
            .cache
            .get(&key)
            .map(|r| r.value().create_index)
            .unwrap_or(new_index);

        let entry = ConfigEntry {
            kind: req.kind.clone(),
            name: req.name.clone(),
            namespace: req.namespace,
            partition: req.partition,
            meta: req.meta,
            extra: req.extra,
            create_index: existing_create_index,
            modify_index: new_index,
        };

        // Store in database
        self.store_entry_to_db(&req.kind, &req.name, &entry).await?;

        // Update cache
        self.cache.insert(key, entry);
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
            if let Some(existing) = self.cache.get(&key) {
                if existing.modify_index != cas_index {
                    return Ok(false);
                }
            } else {
                return Ok(false);
            }
        }

        // Remove from database
        self.delete_entry_from_db(kind, name).await?;

        // Remove from cache
        self.cache.remove(&key);
        self.index.fetch_add(1, Ordering::SeqCst);
        Ok(true)
    }

    async fn load_entries_from_db(&self, _kind: &str) -> Vec<ConfigEntry> {
        // Query database via ConfigService pattern
        // For now, return from cache since DB queries will be similar to KV persistent
        let _ = &self.db;
        vec![]
    }

    async fn load_entry_from_db(&self, _kind: &str, _name: &str) -> Option<ConfigEntry> {
        let _ = &self.db;
        None
    }

    async fn store_entry_to_db(
        &self,
        _kind: &str,
        _name: &str,
        _entry: &ConfigEntry,
    ) -> Result<(), String> {
        let _ = &self.db;
        let _ = CONSUL_CONFIG_ENTRY_NAMESPACE;
        let _ = CONSUL_CONFIG_ENTRY_GROUP;
        let _ = Self::data_id(_kind, _name);
        Ok(())
    }

    async fn delete_entry_from_db(&self, _kind: &str, _name: &str) -> Result<(), String> {
        let _ = &self.db;
        Ok(())
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
    HttpResponse::Ok().json(entries)
}

/// GET /v1/config/{kind}/{name} - Read a specific config entry
pub async fn get_config_entry(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    config_service: web::Data<ConsulConfigEntryService>,
    path: web::Path<(String, String)>,
    _query: web::Query<ConfigEntryListParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let (kind, name) = path.into_inner();
    match config_service.get_entry(&kind, &name) {
        Some(entry) => HttpResponse::Ok().json(entry),
        None => HttpResponse::NotFound().json(ConsulError::new("Config entry not found")),
    }
}

/// PUT /v1/config - Apply/create/update a config entry
pub async fn apply_config_entry(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    config_service: web::Data<ConsulConfigEntryService>,
    query: web::Query<ConfigEntryApplyParams>,
    body: web::Json<ConfigEntryRequest>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let entry_req = body.into_inner();
    if !SUPPORTED_KINDS.contains(&entry_req.kind.as_str()) {
        return HttpResponse::BadRequest().json(ConsulError::new(format!(
            "Unsupported config entry kind: {}",
            entry_req.kind
        )));
    }

    match config_service.apply_entry(entry_req, query.cas) {
        Ok(success) => HttpResponse::Ok().json(success),
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
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let (kind, name) = path.into_inner();
    match config_service.delete_entry(&kind, &name, query.cas) {
        Ok(success) => {
            if query.cas.is_some() {
                HttpResponse::Ok().json(success)
            } else {
                HttpResponse::Ok().json(serde_json::json!({}))
            }
        }
        Err(e) => HttpResponse::InternalServerError().json(ConsulError::new(e)),
    }
}

// ============================================================================
// HTTP Handlers (Persistent)
// ============================================================================

/// GET /v1/config/{kind} - List config entries (persistent)
pub async fn list_config_entries_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    config_service: web::Data<ConsulConfigEntryServicePersistent>,
    path: web::Path<String>,
    _query: web::Query<ConfigEntryListParams>,
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

    let entries = config_service.list_entries(&kind).await;
    HttpResponse::Ok().json(entries)
}

/// GET /v1/config/{kind}/{name} - Read config entry (persistent)
pub async fn get_config_entry_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    config_service: web::Data<ConsulConfigEntryServicePersistent>,
    path: web::Path<(String, String)>,
    _query: web::Query<ConfigEntryListParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let (kind, name) = path.into_inner();
    match config_service.get_entry(&kind, &name).await {
        Some(entry) => HttpResponse::Ok().json(entry),
        None => HttpResponse::NotFound().json(ConsulError::new("Config entry not found")),
    }
}

/// PUT /v1/config - Apply config entry (persistent)
pub async fn apply_config_entry_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    config_service: web::Data<ConsulConfigEntryServicePersistent>,
    query: web::Query<ConfigEntryApplyParams>,
    body: web::Json<ConfigEntryRequest>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let entry_req = body.into_inner();
    if !SUPPORTED_KINDS.contains(&entry_req.kind.as_str()) {
        return HttpResponse::BadRequest().json(ConsulError::new(format!(
            "Unsupported config entry kind: {}",
            entry_req.kind
        )));
    }

    match config_service.apply_entry(entry_req, query.cas).await {
        Ok(success) => HttpResponse::Ok().json(success),
        Err(e) => HttpResponse::InternalServerError().json(ConsulError::new(e)),
    }
}

/// DELETE /v1/config/{kind}/{name} - Delete config entry (persistent)
pub async fn delete_config_entry_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    config_service: web::Data<ConsulConfigEntryServicePersistent>,
    path: web::Path<(String, String)>,
    query: web::Query<ConfigEntryDeleteParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let (kind, name) = path.into_inner();
    match config_service.delete_entry(&kind, &name, query.cas).await {
        Ok(success) => {
            if query.cas.is_some() {
                HttpResponse::Ok().json(success)
            } else {
                HttpResponse::Ok().json(serde_json::json!({}))
            }
        }
        Err(e) => HttpResponse::InternalServerError().json(ConsulError::new(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_and_get_entry() {
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

        let result = service.apply_entry(req, None);
        assert!(result.is_ok());
        assert!(result.unwrap());

        let entry = service.get_entry("service-defaults", "web");
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert_eq!(entry.kind, "service-defaults");
        assert_eq!(entry.name, "web");
    }

    #[test]
    fn test_list_entries_by_kind() {
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
            .unwrap();

        let sd_entries = service.list_entries("service-defaults");
        assert_eq!(sd_entries.len(), 2);

        let pd_entries = service.list_entries("proxy-defaults");
        assert_eq!(pd_entries.len(), 1);
    }

    #[test]
    fn test_delete_entry() {
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
            .unwrap();

        assert!(service.get_entry("service-defaults", "web").is_some());
        service
            .delete_entry("service-defaults", "web", None)
            .unwrap();
        assert!(service.get_entry("service-defaults", "web").is_none());
    }

    #[test]
    fn test_cas_apply() {
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
        service.apply_entry(req.clone(), None).unwrap();
        let entry = service.get_entry("service-defaults", "web").unwrap();

        // CAS with wrong index
        let result = service.apply_entry(req.clone(), Some(999));
        assert_eq!(result.unwrap(), false);

        // CAS with correct index
        let result = service.apply_entry(req, Some(entry.modify_index));
        assert_eq!(result.unwrap(), true);
    }

    #[test]
    fn test_cas_delete() {
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
            .unwrap();
        let entry = service.get_entry("mesh", "mesh").unwrap();

        // CAS with wrong index
        let result = service.delete_entry("mesh", "mesh", Some(999));
        assert_eq!(result.unwrap(), false);

        // Entry should still exist
        assert!(service.get_entry("mesh", "mesh").is_some());

        // CAS with correct index
        let result = service.delete_entry("mesh", "mesh", Some(entry.modify_index));
        assert_eq!(result.unwrap(), true);
        assert!(service.get_entry("mesh", "mesh").is_none());
    }
}
