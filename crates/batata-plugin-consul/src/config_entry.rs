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
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::{error, info};

use crate::connect_ca::{
    ConsulConnectCAService, IntentionAction, IntentionHTTPPermission, IntentionPermission,
    IntentionRequest, validate_intention_permissions,
};
use crate::constants::CF_CONSUL_CONFIG_ENTRIES;
use crate::raft::{ConsulRaftRequest, ConsulRaftWriter};

use crate::acl::{AclService, ResourceType};
use crate::consul_meta::{ConsulResponseMeta, consul_ok};
use crate::index_provider::{ConsulIndexProvider, ConsulTable};
use crate::model::ConsulError;
use crate::model::ConsulErrorBody;

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
    /// The `kind` field.
    pub kind: String,
    /// The `name` field.
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `namespace` field.
    pub namespace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `partition` field.
    pub partition: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `meta` field.
    pub meta: Option<HashMap<String, String>>,
    /// Additional fields stored as dynamic JSON
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
    /// The `create_index` field.
    pub create_index: u64,
    /// The `modify_index` field.
    pub modify_index: u64,
}

/// Query parameters for config entry list
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ConfigEntryListParams {
    /// The `dc` field.
    pub dc: Option<String>,
    /// The `index` field.
    pub index: Option<u64>,
    /// The `wait` field.
    pub wait: Option<String>,
    /// The `filter` field.
    pub filter: Option<String>,
}

/// Query parameters for config entry apply
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ConfigEntryApplyParams {
    /// The `dc` field.
    pub dc: Option<String>,
    /// The `cas` field.
    pub cas: Option<u64>,
}

/// Query parameters for config entry delete
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ConfigEntryDeleteParams {
    /// The `dc` field.
    pub dc: Option<String>,
    /// The `cas` field.
    pub cas: Option<u64>,
}

/// Config entry apply request body (incoming JSON)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ConfigEntryRequest {
    /// The `kind` field.
    pub kind: String,
    #[serde(default)]
    /// The `name` field.
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `namespace` field.
    pub namespace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `partition` field.
    pub partition: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `meta` field.
    pub meta: Option<HashMap<String, String>>,
    #[serde(flatten)]
    /// The `extra` field.
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
    /// The `new` associated function.
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
        if let Some(ref db) = self.rocks_db
            && let Some(cf) = db.cf_handle(CF_CONSUL_CONFIG_ENTRIES)
        {
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

    /// Delete an entry from RocksDB
    fn delete_from_rocks(&self, key: &str) {
        if let Some(ref db) = self.rocks_db
            && let Some(cf) = db.cf_handle(CF_CONSUL_CONFIG_ENTRIES)
            && let Err(e) = db.delete_cf(cf, key.as_bytes())
        {
            error!(
                "Failed to delete config entry '{}' from RocksDB: {}",
                key, e
            );
        }
    }

    /// The `list_entries` method.
    pub fn list_entries(&self, kind: &str) -> Vec<ConfigEntry> {
        let prefix = format!("{}/", kind);
        self.entries
            .iter()
            .filter(|r| r.key().starts_with(&prefix))
            .map(|r| r.value().clone())
            .collect()
    }

    /// The `get_entry` method.
    pub fn get_entry(&self, kind: &str, name: &str) -> Option<ConfigEntry> {
        let key = Self::entry_key(kind, name);
        self.entries.get(&key).map(|r| r.value().clone())
    }

    /// The `apply_entry` method.
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

    /// The `delete_entry` method.
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
// Config Entry Semantic Validation
// ============================================================================

/// Valid protocols for service-defaults / proxy-defaults.
const VALID_PROTOCOLS: &[&str] = &["tcp", "http", "http2", "grpc"];

/// Validate a config entry request before persisting.
///
/// Returns `Ok(())` on success or an error string suitable for a 400 response.
/// Unsupported kinds are rejected by the caller before this is invoked.
pub fn validate_config_entry(req: &ConfigEntryRequest) -> Result<(), String> {
    match req.kind.as_str() {
        "service-defaults" => validate_service_defaults(req),
        "proxy-defaults" => validate_proxy_defaults(req),
        "service-router" => validate_service_router(req),
        "service-splitter" => validate_service_splitter(req),
        "service-resolver" => validate_service_resolver(req),
        "service-intentions" => service_intentions_to_intentions(req).map(|_| ()),
        // Kinds without specific structural validation: ingress-gateway,
        // terminating-gateway, mesh, exported-services, api-gateway,
        // http-route, tcp-route, jwt-provider.
        _ => Ok(()),
    }
}

/// Validate `service-defaults` entries.
fn validate_service_defaults(req: &ConfigEntryRequest) -> Result<(), String> {
    if req.name.trim().is_empty() {
        return Err("service-defaults requires a non-empty Name".into());
    }
    if let Some(protocol) = req.extra.get("Protocol").and_then(|v| v.as_str()) {
        if !VALID_PROTOCOLS.contains(&protocol) {
            return Err(format!(
                "service-defaults Protocol '{}' is invalid; must be one of: {}",
                protocol,
                VALID_PROTOCOLS.join(", ")
            ));
        }
    }
    Ok(())
}

/// Validate `proxy-defaults` entries.
fn validate_proxy_defaults(req: &ConfigEntryRequest) -> Result<(), String> {
    // Consul requires proxy-defaults Name to be exactly "global".
    if req.name != "global" {
        return Err("proxy-defaults Name must be 'global'".into());
    }
    if let Some(protocol) = req.extra.get("Protocol").and_then(|v| v.as_str()) {
        if !VALID_PROTOCOLS.contains(&protocol) {
            return Err(format!(
                "proxy-defaults Protocol '{}' is invalid; must be one of: {}",
                protocol,
                VALID_PROTOCOLS.join(", ")
            ));
        }
    }
    Ok(())
}

/// Validate `service-router` entries.
fn validate_service_router(req: &ConfigEntryRequest) -> Result<(), String> {
    if req.name.trim().is_empty() {
        return Err("service-router requires a non-empty Name".into());
    }
    let routes = req.extra.get("Routes");
    match routes {
        Some(serde_json::Value::Array(arr)) if !arr.is_empty() => {
            for (i, route) in arr.iter().enumerate() {
                if !route.is_object() {
                    return Err(format!("service-router Routes[{}] must be an object", i));
                }
            }
            Ok(())
        }
        Some(serde_json::Value::Array(_)) => {
            Err("service-router Routes must contain at least one route".into())
        }
        Some(_) => Err("service-router Routes must be an array".into()),
        None => Ok(()), // Routes is optional
    }
}

/// Validate `service-splitter` entries.
fn validate_service_splitter(req: &ConfigEntryRequest) -> Result<(), String> {
    if req.name.trim().is_empty() {
        return Err("service-splitter requires a non-empty Name".into());
    }
    let splits = req.extra.get("Splits");
    match splits {
        Some(serde_json::Value::Array(arr)) if !arr.is_empty() => {
            let mut total: i64 = 0;
            for (i, split) in arr.iter().enumerate() {
                let weight = split
                    .get("Weight")
                    .and_then(|w| w.as_i64())
                    .ok_or_else(|| {
                        format!("service-splitter Splits[{}] must have an integer Weight", i)
                    })?;
                if weight < 0 {
                    return Err(format!(
                        "service-splitter Splits[{}] Weight must be non-negative",
                        i
                    ));
                }
                total += weight;
            }
            if total != 100 {
                return Err(format!(
                    "service-splitter Splits weights must sum to 100, got {}",
                    total
                ));
            }
            Ok(())
        }
        Some(serde_json::Value::Array(_)) => {
            Err("service-splitter Splits must contain at least one entry".into())
        }
        Some(_) => Err("service-splitter Splits must be an array".into()),
        None => Ok(()), // Splits is optional
    }
}

/// Validate `service-resolver` entries.
fn validate_service_resolver(req: &ConfigEntryRequest) -> Result<(), String> {
    if req.name.trim().is_empty() {
        return Err("service-resolver requires a non-empty Name".into());
    }
    if let Some(serde_json::Value::Object(subsets)) = req.extra.get("Subsets") {
        for (name, subset) in subsets {
            if let Some(filter) = subset.get("Filter").and_then(|f| f.as_str()) {
                if filter.trim().is_empty() {
                    return Err(format!(
                        "service-resolver Subsets.{} Filter must be a non-empty string",
                        name
                    ));
                }
            }
        }
    }
    Ok(())
}

// ============================================================================
// service-intentions → legacy intention store conversion
// ============================================================================

/// Top-level `extra` keys that may accompany a `service-intentions` entry.
/// Read-only index/hash fields are accepted and ignored; JWT is explicitly
/// rejected below because batata has no JWT intention requirement model.
const SERVICE_INTENTIONS_TOP_LEVEL_KEYS: &[&str] = &[
    "Sources",
    "JWT",
    "Hash",
    "RaftIndex",
    "CreateIndex",
    "ModifyIndex",
];

const SERVICE_INTENTIONS_SOURCE_KEYS: &[&str] = &[
    "Name",
    "Peer",
    "Partition",
    "Namespace",
    "SamenessGroup",
    "Action",
    "Permissions",
    "Precedence",
    "Type",
    "Description",
    "LegacyID",
    "LegacyMeta",
    "LegacyCreateTime",
    "LegacyUpdateTime",
];

const SERVICE_INTENTIONS_PERMISSION_KEYS: &[&str] = &["Action", "HTTP"];
const SERVICE_INTENTIONS_HTTP_KEYS: &[&str] = &["PathExact", "PathPrefix", "PathRegex", "Methods"];

const VALID_HTTP_METHODS: &[&str] = &[
    "GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS", "CONNECT", "TRACE",
];

fn reject_unknown_keys(
    value: &serde_json::Value,
    allowed: &[&str],
    context: &str,
) -> Result<(), String> {
    let serde_json::Value::Object(map) = value else {
        return Err(format!("{context} must be an object"));
    };
    for key in map.keys() {
        if !allowed.iter().any(|allowed| *allowed == key) {
            return Err(format!("{context} has unsupported field '{key}'"));
        }
    }
    Ok(())
}

fn required_string(value: &serde_json::Value, key: &str, context: &str) -> Result<String, String> {
    match value.get(key) {
        Some(serde_json::Value::String(s)) if !s.trim().is_empty() => Ok(s.to_string()),
        Some(serde_json::Value::Null) | None => Err(format!("{context}.{key} is required")),
        Some(_) => Err(format!("{context}.{key} must be a non-empty string")),
    }
}

fn optional_string(
    value: &serde_json::Value,
    key: &str,
    context: &str,
) -> Result<Option<String>, String> {
    match value.get(key) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::String(s)) if s.is_empty() => Ok(None),
        Some(serde_json::Value::String(s)) => Ok(Some(s.to_string())),
        Some(_) => Err(format!("{context}.{key} must be a string")),
    }
}

fn reject_present(value: &serde_json::Value, key: &str, context: &str) -> Result<(), String> {
    match value.get(key) {
        None | Some(serde_json::Value::Null) => Ok(()),
        Some(_) => Err(format!("{context}.{key} must be omitted")),
    }
}

fn reject_nonempty(value: &serde_json::Value, key: &str, context: &str) -> Result<(), String> {
    let Some(field) = value.get(key) else {
        return Ok(());
    };
    let empty = match field {
        serde_json::Value::Null => true,
        serde_json::Value::String(s) => s.is_empty(),
        serde_json::Value::Array(a) => a.is_empty(),
        serde_json::Value::Object(o) => o.is_empty(),
        _ => false,
    };
    if empty {
        Ok(())
    } else {
        Err(format!("{context}.{key} is not supported by batata"))
    }
}

fn validate_service_name(name: &str, context: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err(format!("{context} is required"));
    }
    if name.contains('*') && name != "*" {
        return Err(format!(
            "{context} may only be a full '*' wildcard, got '{name}'"
        ));
    }
    Ok(())
}

fn validate_namespace(namespace: &str, context: &str) -> Result<(), String> {
    if namespace.contains('*') {
        return Err(format!("{context} wildcards are not supported by batata"));
    }
    Ok(())
}

fn parse_action(value: &str, context: &str) -> Result<IntentionAction, String> {
    match value {
        "allow" => Ok(IntentionAction::Allow),
        "deny" => Ok(IntentionAction::Deny),
        other => Err(format!(
            "{context} must be 'allow' or 'deny', got '{other}'"
        )),
    }
}

fn parse_permission(
    value: &serde_json::Value,
    source_index: usize,
    permission_index: usize,
) -> Result<IntentionPermission, String> {
    let context = format!("Sources[{source_index}].Permissions[{permission_index}]");
    reject_unknown_keys(value, SERVICE_INTENTIONS_PERMISSION_KEYS, &context)?;
    let action = parse_action(
        &required_string(value, "Action", &context)?,
        &format!("{context}.Action"),
    )?;

    let Some(http) = value.get("HTTP").filter(|v| !v.is_null()) else {
        return Err(format!("{context}.HTTP is required"));
    };
    let http_context = format!("{context}.HTTP");
    reject_unknown_keys(http, SERVICE_INTENTIONS_HTTP_KEYS, &http_context)?;

    let path_exact = optional_string(http, "PathExact", &http_context)?;
    let path_prefix = optional_string(http, "PathPrefix", &http_context)?;
    let path_regex = optional_string(http, "PathRegex", &http_context)?;

    let mut path_parts = 0;
    if path_exact.is_some() {
        path_parts += 1;
    }
    if path_prefix.is_some() {
        path_parts += 1;
    }
    if path_regex.is_some() {
        path_parts += 1;
    }
    if path_parts > 1 {
        return Err(format!(
            "{http_context} may set at most one of PathExact, PathPrefix, PathRegex"
        ));
    }
    if path_exact.as_deref().is_some_and(|p| !p.starts_with('/'))
        || path_prefix.as_deref().is_some_and(|p| !p.starts_with('/'))
    {
        return Err(format!("{http_context} path matches must start with '/'"));
    }

    let methods = match http.get("Methods") {
        None | Some(serde_json::Value::Null) => Vec::new(),
        Some(serde_json::Value::Array(methods)) => {
            let mut seen = HashSet::new();
            let mut parsed = Vec::with_capacity(methods.len());
            for method in methods {
                let Some(method) = method.as_str() else {
                    return Err(format!("{http_context}.Methods must contain strings"));
                };
                if !VALID_HTTP_METHODS
                    .iter()
                    .any(|valid| valid.eq_ignore_ascii_case(method))
                {
                    return Err(format!(
                        "{http_context}.Methods has invalid method '{method}'"
                    ));
                }
                let upper = method.to_ascii_uppercase();
                if !seen.insert(upper.clone()) {
                    return Err(format!(
                        "{http_context}.Methods contains '{method}' more than once"
                    ));
                }
                parsed.push(upper);
            }
            parsed
        }
        Some(_) => return Err(format!("{http_context}.Methods must be an array")),
    };

    if path_parts == 0 && methods.is_empty() {
        return Err(format!(
            "{http_context} must set a path match or at least one method"
        ));
    }

    Ok(IntentionPermission {
        action,
        http: Some(IntentionHTTPPermission {
            path_exact,
            path_prefix,
            path_regex,
            methods,
        }),
    })
}

/// Validate and convert a `service-intentions` config entry into the legacy
/// intention rows consumed by the CA/xDS bridge.
///
/// Unsupported upstream features (JWT requirements, peers, sameness groups,
/// partitions, legacy bridge fields and HTTP header permissions) are rejected
/// rather than silently ignored, so an accepted entry has an exact local
/// representation.
pub(crate) fn service_intentions_to_intentions(
    req: &ConfigEntryRequest,
) -> Result<Vec<IntentionRequest>, String> {
    let extra = serde_json::Value::Object(
        req.extra
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
    );
    reject_unknown_keys(
        &extra,
        SERVICE_INTENTIONS_TOP_LEVEL_KEYS,
        "service-intentions",
    )?;
    reject_present(&extra, "JWT", "service-intentions")?;

    let destination_name = req.name.trim();
    validate_service_name(destination_name, "service-intentions Name")?;
    let destination_ns = req.namespace.as_deref().unwrap_or("default");
    if destination_ns.is_empty() {
        return Err("service-intentions Namespace must not be empty".into());
    }
    validate_namespace(destination_ns, "service-intentions Namespace")?;
    let wildcard_destination = destination_name == "*";

    let Some(sources) = req.extra.get("Sources").and_then(|v| v.as_array()) else {
        return Err("service-intentions requires a non-empty Sources array".into());
    };
    if sources.is_empty() {
        return Err("service-intentions requires at least one source".into());
    }

    let meta = req.meta.clone().unwrap_or_default();
    let mut converted = Vec::with_capacity(sources.len());
    for (index, source) in sources.iter().enumerate() {
        let context = format!("Sources[{index}]");
        reject_unknown_keys(source, SERVICE_INTENTIONS_SOURCE_KEYS, &context)?;
        reject_present(source, "LegacyID", &context)?;
        reject_present(source, "LegacyMeta", &context)?;
        reject_present(source, "LegacyCreateTime", &context)?;
        reject_present(source, "LegacyUpdateTime", &context)?;
        reject_nonempty(source, "Peer", &context)?;
        reject_nonempty(source, "Partition", &context)?;
        reject_nonempty(source, "SamenessGroup", &context)?;

        if let Some(source_type) = optional_string(source, "Type", &context)?
            && source_type != "consul"
        {
            return Err(format!(
                "{context}.Type must be 'consul' or omitted, got '{source_type}'"
            ));
        }

        let source_name = required_string(source, "Name", &context)?;
        validate_service_name(&source_name, &format!("{context}.Name"))?;
        let source_ns = optional_string(source, "Namespace", &context)?
            .unwrap_or_else(|| destination_ns.to_string());
        validate_namespace(&source_ns, &format!("{context}.Namespace"))?;
        let description = optional_string(source, "Description", &context)?.unwrap_or_default();

        let action_value = optional_string(source, "Action", &context)?;
        let permissions_value = source.get("Permissions").filter(|v| !v.is_null());

        let (action, permissions) = match (action_value, permissions_value) {
            (Some(_), Some(_)) => {
                return Err(format!(
                    "{context}.Action must be omitted when Permissions are specified"
                ));
            }
            (Some(action), None) => (parse_action(&action, &format!("{context}.Action"))?, vec![]),
            (None, Some(serde_json::Value::Array(permissions))) => {
                if wildcard_destination {
                    return Err(format!(
                        "{context}.Permissions cannot be used with a wildcard destination"
                    ));
                }
                let parsed = permissions
                    .iter()
                    .enumerate()
                    .map(|(i, p)| parse_permission(p, index, i))
                    .collect::<Result<Vec<_>, _>>()?;
                // Upstream stores no top-level action for L7 intentions. The
                // legacy model requires one; ALLOW is an inert placeholder
                // because permission evaluation ignores it.
                (IntentionAction::Allow, parsed)
            }
            (None, Some(_)) => {
                return Err(format!("{context}.Permissions must be an array"));
            }
            (None, None) => {
                return Err(format!("{context} requires either Action or Permissions"));
            }
        };

        validate_intention_permissions(&permissions)?;
        converted.push(IntentionRequest {
            description,
            source_ns,
            source_name,
            destination_ns: destination_ns.to_string(),
            destination_name: destination_name.to_string(),
            action,
            permissions,
            meta: meta.clone(),
        });
    }

    Ok(converted)
}

/// Apply a `service-intentions` config entry to the legacy intention store.
/// Returns the number of legacy intentions written.
pub(crate) async fn sync_service_intentions(
    ca_service: &ConsulConnectCAService,
    req: &ConfigEntryRequest,
) -> Result<usize, String> {
    let intentions = service_intentions_to_intentions(req)?;
    let destination_ns = req.namespace.as_deref().unwrap_or("default");
    Ok(ca_service
        .replace_intentions_for_destination(destination_ns, req.name.trim(), intentions)
        .await)
}

/// Remove the legacy intentions owned by a deleted `service-intentions`
/// config entry.
pub(crate) async fn remove_service_intentions(
    ca_service: &ConsulConnectCAService,
    destination_ns: &str,
    destination_name: &str,
) -> usize {
    ca_service
        .delete_intentions_for_destination(destination_ns, destination_name)
        .await
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
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let kind = path.into_inner();
    if !SUPPORTED_KINDS.contains(&kind.as_str()) {
        return HttpResponse::BadRequest().consul_error(ConsulError::new(format!(
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
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let (kind, name) = path.into_inner();
    match config_service.get_entry(&kind, &name) {
        Some(entry) => {
            let meta =
                ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ConfigEntries));
            consul_ok(&meta).json(entry)
        }
        None => HttpResponse::NotFound().consul_error(ConsulError::new("Config entry not found")),
    }
}

/// PUT /v1/config - Apply/create/update a config entry
pub async fn apply_config_entry(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    config_service: web::Data<ConsulConfigEntryService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    body: web::Bytes,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let entry_req: ConfigEntryRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            return HttpResponse::BadRequest()
                .consul_error(ConsulError::new(format!("Invalid request body: {}", e)));
        }
    };

    if !SUPPORTED_KINDS.contains(&entry_req.kind.as_str()) {
        return HttpResponse::BadRequest().consul_error(ConsulError::new(format!(
            "Unsupported config entry kind: {}",
            entry_req.kind
        )));
    }

    // Semantic validation per kind
    if let Err(msg) = validate_config_entry(&entry_req) {
        return HttpResponse::BadRequest().consul_error(ConsulError::new(msg));
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

    let entry_for_sync = entry_req.clone();
    match config_service.apply_entry(entry_req, cas).await {
        Ok(success) => {
            if success && entry_for_sync.kind == "service-intentions" {
                if let Err(e) = sync_service_intentions(&ca_service, &entry_for_sync).await {
                    return HttpResponse::InternalServerError().consul_error(ConsulError::new(e));
                }
                index_provider.increment(ConsulTable::ConnectCA);
            }
            let meta =
                ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ConfigEntries));
            consul_ok(&meta).json(success)
        }
        Err(e) => HttpResponse::InternalServerError().consul_error(ConsulError::new(e)),
    }
}

/// DELETE /v1/config/{kind}/{name} - Delete a config entry
pub async fn delete_config_entry(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    config_service: web::Data<ConsulConfigEntryService>,
    ca_service: web::Data<ConsulConnectCAService>,
    path: web::Path<(String, String)>,
    query: web::Query<ConfigEntryDeleteParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let (kind, name) = path.into_inner();
    let existing = config_service.get_entry(&kind, &name);
    match config_service.delete_entry(&kind, &name, query.cas).await {
        Ok(success) => {
            if success && kind == "service-intentions" {
                let namespace = existing
                    .and_then(|entry| entry.namespace)
                    .unwrap_or_else(|| "default".to_string());
                let removed = remove_service_intentions(&ca_service, &namespace, &name).await;
                if removed > 0 {
                    index_provider.increment(ConsulTable::ConnectCA);
                }
            }
            let meta =
                ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ConfigEntries));
            if query.cas.is_some() {
                consul_ok(&meta).json(success)
            } else {
                consul_ok(&meta).json(serde_json::json!({}))
            }
        }
        Err(e) => HttpResponse::InternalServerError().consul_error(ConsulError::new(e)),
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

    // ========================================================================
    // Validation failure-path tests
    // ========================================================================

    fn req(
        kind: &str,
        name: &str,
        extra: HashMap<String, serde_json::Value>,
    ) -> ConfigEntryRequest {
        ConfigEntryRequest {
            kind: kind.to_string(),
            name: name.to_string(),
            namespace: None,
            partition: None,
            meta: None,
            extra,
        }
    }

    #[test]
    fn validate_service_defaults_empty_name_rejected() {
        let r = req("service-defaults", "  ", HashMap::new());
        assert!(validate_config_entry(&r).is_err());
    }

    #[test]
    fn validate_service_defaults_invalid_protocol_rejected() {
        let mut extra = HashMap::new();
        extra.insert(
            "Protocol".to_string(),
            serde_json::Value::String("ftp".to_string()),
        );
        let r = req("service-defaults", "web", extra);
        let err = validate_config_entry(&r).unwrap_err();
        assert!(err.contains("Protocol") && err.contains("invalid"));
    }

    #[test]
    fn validate_service_defaults_valid_protocol_accepted() {
        for proto in ["tcp", "http", "http2", "grpc"] {
            let mut extra = HashMap::new();
            extra.insert(
                "Protocol".to_string(),
                serde_json::Value::String(proto.to_string()),
            );
            let r = req("service-defaults", "web", extra);
            assert!(
                validate_config_entry(&r).is_ok(),
                "protocol '{}' should be accepted",
                proto
            );
        }
    }

    #[test]
    fn validate_proxy_defaults_non_global_name_rejected() {
        let r = req("proxy-defaults", "not-global", HashMap::new());
        assert!(validate_config_entry(&r).is_err());
    }

    #[test]
    fn validate_proxy_defaults_global_name_accepted() {
        let r = req("proxy-defaults", "global", HashMap::new());
        assert!(validate_config_entry(&r).is_ok());
    }

    #[test]
    fn validate_proxy_defaults_invalid_protocol_rejected() {
        let mut extra = HashMap::new();
        extra.insert(
            "Protocol".to_string(),
            serde_json::Value::String("bad".to_string()),
        );
        let r = req("proxy-defaults", "global", extra);
        assert!(validate_config_entry(&r).is_err());
    }

    #[test]
    fn validate_service_router_empty_name_rejected() {
        let r = req("service-router", "", HashMap::new());
        assert!(validate_config_entry(&r).is_err());
    }

    #[test]
    fn validate_service_splitter_weights_not_100_rejected() {
        let mut extra = HashMap::new();
        extra.insert(
            "Splits".to_string(),
            serde_json::json!([
                { "Weight": 60 },
                { "Weight": 30 },
            ]),
        );
        let r = req("service-splitter", "web", extra);
        let err = validate_config_entry(&r).unwrap_err();
        assert!(err.contains("100"), "error should mention 100: {}", err);
    }

    #[test]
    fn validate_service_splitter_negative_weight_rejected() {
        let mut extra = HashMap::new();
        extra.insert(
            "Splits".to_string(),
            serde_json::json!([
                { "Weight": -10 },
                { "Weight": 110 },
            ]),
        );
        let r = req("service-splitter", "web", extra);
        assert!(validate_config_entry(&r).is_err());
    }

    #[test]
    fn validate_service_splitter_weights_sum_100_accepted() {
        let mut extra = HashMap::new();
        extra.insert(
            "Splits".to_string(),
            serde_json::json!([
                { "Weight": 70 },
                { "Weight": 30 },
            ]),
        );
        let r = req("service-splitter", "web", extra);
        assert!(validate_config_entry(&r).is_ok());
    }

    #[test]
    fn validate_service_splitter_empty_splits_rejected() {
        let mut extra = HashMap::new();
        extra.insert("Splits".to_string(), serde_json::json!([]));
        let r = req("service-splitter", "web", extra);
        assert!(validate_config_entry(&r).is_err());
    }

    #[test]
    fn validate_service_resolver_empty_name_rejected() {
        let r = req("service-resolver", "   ", HashMap::new());
        assert!(validate_config_entry(&r).is_err());
    }

    #[test]
    fn validate_unknown_kind_accepted() {
        // Kinds without specific validation should pass through.
        let r = req("ingress-gateway", "gateway", HashMap::new());
        assert!(validate_config_entry(&r).is_ok());
    }

    fn service_intentions(sources: serde_json::Value) -> ConfigEntryRequest {
        let mut extra = HashMap::new();
        extra.insert("Sources".to_string(), sources);
        req("service-intentions", "api", extra)
    }

    #[test]
    fn service_intentions_l4_source_converts_fields() {
        let entry = service_intentions(serde_json::json!([
            { "Name": "web", "Action": "deny", "Description": "block web" }
        ]));
        let converted = service_intentions_to_intentions(&entry).unwrap();
        assert_eq!(converted.len(), 1);
        assert_eq!(converted[0].source_name, "web");
        assert_eq!(converted[0].source_ns, "default");
        assert_eq!(converted[0].destination_name, "api");
        assert_eq!(converted[0].destination_ns, "default");
        assert_eq!(converted[0].action, IntentionAction::Deny);
        assert_eq!(converted[0].description, "block web");
        assert!(converted[0].permissions.is_empty());
        assert!(validate_config_entry(&entry).is_ok());
    }

    #[test]
    fn service_intentions_l7_permissions_convert_with_placeholder_action() {
        let entry = service_intentions(serde_json::json!([
            {
                "Name": "web",
                "Permissions": [
                    { "Action": "deny", "HTTP": { "PathPrefix": "/admin" } },
                    { "Action": "allow", "HTTP": { "PathPrefix": "/", "Methods": ["GET", "post"] } }
                ]
            }
        ]));
        let converted = service_intentions_to_intentions(&entry).unwrap();
        assert_eq!(converted.len(), 1);
        // Permission evaluation ignores the top-level action; the legacy model
        // still requires one, so ALLOW is an inert placeholder.
        assert_eq!(converted[0].action, IntentionAction::Allow);
        assert_eq!(converted[0].permissions.len(), 2);
        assert_eq!(
            converted[0].permissions[0]
                .http
                .as_ref()
                .unwrap()
                .path_prefix
                .as_deref(),
            Some("/admin")
        );
        assert_eq!(
            converted[0].permissions[1].http.as_ref().unwrap().methods,
            vec!["GET".to_string(), "POST".to_string()]
        );
    }

    #[test]
    fn service_intentions_rejects_action_with_permissions() {
        let entry = service_intentions(serde_json::json!([
            {
                "Name": "web",
                "Action": "allow",
                "Permissions": [
                    { "Action": "deny", "HTTP": { "PathPrefix": "/admin" } }
                ]
            }
        ]));
        let err = service_intentions_to_intentions(&entry).unwrap_err();
        assert!(err.contains("must be omitted"), "{err}");
    }

    #[test]
    fn service_intentions_rejects_jwt_and_peer() {
        let mut with_jwt = service_intentions(serde_json::json!([
            { "Name": "web", "Action": "allow" }
        ]));
        with_jwt.extra.insert("JWT".into(), serde_json::json!({}));
        assert!(service_intentions_to_intentions(&with_jwt).is_err());

        let with_peer = service_intentions(serde_json::json!([
            { "Name": "web", "Action": "allow", "Peer": "peer-1" }
        ]));
        let err = service_intentions_to_intentions(&with_peer).unwrap_err();
        assert!(err.contains("not supported"), "{err}");
    }

    #[test]
    fn service_intentions_rejects_partial_wildcard() {
        let entry = service_intentions(serde_json::json!([
            { "Name": "web-*", "Action": "deny" }
        ]));
        let err = service_intentions_to_intentions(&entry).unwrap_err();
        assert!(err.contains("full '*' wildcard"), "{err}");
    }

    #[test]
    fn service_intentions_rejects_invalid_permission_shapes() {
        let missing_http = service_intentions(serde_json::json!([
            { "Name": "web", "Permissions": [{ "Action": "deny" }] }
        ]));
        let err = service_intentions_to_intentions(&missing_http).unwrap_err();
        assert!(err.contains("HTTP is required"), "{err}");

        let multiple_paths = service_intentions(serde_json::json!([
            {
                "Name": "web",
                "Permissions": [{
                    "Action": "allow",
                    "HTTP": { "PathExact": "/health", "PathPrefix": "/" }
                }]
            }
        ]));
        let err = service_intentions_to_intentions(&multiple_paths).unwrap_err();
        assert!(err.contains("at most one"), "{err}");

        let duplicate_methods = service_intentions(serde_json::json!([
            {
                "Name": "web",
                "Permissions": [{
                    "Action": "allow",
                    "HTTP": { "Methods": ["GET", "get"] }
                }]
            }
        ]));
        let err = service_intentions_to_intentions(&duplicate_methods).unwrap_err();
        assert!(err.contains("more than once"), "{err}");
    }

    #[test]
    fn validate_config_entry_dispatches_service_intentions() {
        let entry = service_intentions(serde_json::json!([]));
        assert!(validate_config_entry(&entry).is_err());
    }

    #[tokio::test]
    async fn service_intentions_sync_replaces_and_removes_destination_rows() {
        let ca = ConsulConnectCAService::new();
        let first = service_intentions(serde_json::json!([
            { "Name": "web", "Action": "allow" },
            { "Name": "cron", "Action": "deny" }
        ]));
        assert_eq!(sync_service_intentions(&ca, &first).await.unwrap(), 2);
        assert_eq!(ca.match_intentions("destination", "api").len(), 2);

        let second = service_intentions(serde_json::json!([
            { "Name": "batch", "Action": "deny" }
        ]));
        assert_eq!(sync_service_intentions(&ca, &second).await.unwrap(), 1);
        let remaining = ca.match_intentions("destination", "api");
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].source_name, "batch");

        assert_eq!(remove_service_intentions(&ca, "default", "api").await, 1);
        assert!(ca.match_intentions("destination", "api").is_empty());
    }
}
