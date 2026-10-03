//! Consul Connect CA and Intentions API
//!
//! Provides mTLS certificate authority, service intentions,
//! and connection authorization endpoints.

use actix_web::{HttpRequest, HttpResponse, web};
use chrono::Utc;
use dashmap::DashMap;
use rocksdb::DB;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info};

use crate::constants::{CF_CONSUL_CA_ROOTS, CF_CONSUL_INTENTIONS};

use crate::acl::{AclService, ResourceType};
use crate::agent::ConsulAgentService;
use crate::consul_meta::{ConsulResponseMeta, consul_ok};
use crate::index_provider::{ConsulIndexProvider, ConsulTable};
use crate::model::{AgentServiceRegistration, ConsulDatacenterConfig, ConsulError, ConsulErrorBody};
use crate::raft::{ConsulRaftRequest, ConsulRaftWriter};

// ============================================================================
// CA Models
// ============================================================================

/// A CA root certificate
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CARoot {
    #[serde(rename = "ID")]
/// The `id` field.
    pub id: String,
/// The `name` field.
    pub name: String,
/// The `root_cert` field.
    pub root_cert: String,
/// The `active` field.
    pub active: bool,
/// The `create_index` field.
    pub create_index: u64,
/// The `modify_index` field.
    pub modify_index: u64,
}

/// List of CA roots response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CARootList {
    #[serde(rename = "ActiveRootID")]
/// The `active_root_id` field.
    pub active_root_id: String,
/// The `trust_domain` field.
    pub trust_domain: String,
/// The `roots` field.
    pub roots: Vec<CARoot>,
}

/// Helper to deserialize a map that may be null in JSON
fn deserialize_map_or_null<'de, D, K, V>(
    deserializer: D,
) -> Result<std::collections::HashMap<K, V>, D::Error>
where
    D: serde::Deserializer<'de>,
    K: serde::Deserialize<'de> + std::cmp::Eq + std::hash::Hash,
    V: serde::Deserialize<'de>,
{
    let opt = Option::<std::collections::HashMap<K, V>>::deserialize(deserializer)?;
    Ok(opt.unwrap_or_default())
}

/// CA configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CAConfig {
/// The `provider` field.
    pub provider: String,
    #[serde(default, deserialize_with = "deserialize_map_or_null")]
/// The `config` field.
    pub config: std::collections::HashMap<String, serde_json::Value>,
    #[serde(
        default,
        deserialize_with = "deserialize_map_or_null",
        skip_serializing_if = "std::collections::HashMap::is_empty"
    )]
/// The `state` field.
    pub state: std::collections::HashMap<String, String>,
    #[serde(default)]
/// The `force_without_cross_signing` field.
    pub force_without_cross_signing: bool,
    #[serde(default)]
/// The `create_index` field.
    pub create_index: u64,
    #[serde(default)]
/// The `modify_index` field.
    pub modify_index: u64,
}

/// Leaf certificate response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LeafCert {
/// The `serial_number` field.
    pub serial_number: String,
    #[serde(rename = "CertPEM")]
/// The `cert_pem` field.
    pub cert_pem: String,
    #[serde(rename = "PrivateKeyPEM")]
/// The `private_key_pem` field.
    pub private_key_pem: String,
/// The `service` field.
    pub service: String,
    #[serde(rename = "ServiceURI")]
/// The `service_uri` field.
    pub service_uri: String,
/// The `valid_after` field.
    pub valid_after: String,
/// The `valid_before` field.
    pub valid_before: String,
/// The `create_index` field.
    pub create_index: u64,
/// The `modify_index` field.
    pub modify_index: u64,
}

/// Service identity response for `/v1/connect/service/{service_id}`.
///
/// Combines the leaf certificate, CA roots, and SPIFFE identity into a single
/// response so sidecar proxies can obtain all mTLS material in one call.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ServiceIdentity {
/// The `service_name` field.
    pub service_name: String,
    #[serde(rename = "ServiceID")]
/// The `service_id` field.
    pub service_id: String,
    #[serde(rename = "ServiceURI")]
/// The `service_uri` field — SPIFFE ID (e.g. `spiffe://consul/ns/default/dc/dc1/svc/web`).
    pub service_uri: String,
    #[serde(rename = "CertPEM")]
/// The `cert_pem` field — leaf certificate in PEM format.
    pub cert_pem: String,
    #[serde(rename = "PrivateKeyPEM")]
/// The `private_key_pem` field — private key in PEM format.
    pub private_key_pem: String,
/// The `roots` field — list of CA root certificates.
    pub roots: Vec<CARoot>,
/// The `trust_domain` field.
    pub trust_domain: String,
/// The `datacenter` field.
    pub datacenter: String,
/// The `valid_after` field.
    pub valid_after: String,
/// The `valid_before` field.
    pub valid_before: String,
/// The `create_index` field.
    pub create_index: u64,
/// The `modify_index` field.
    pub modify_index: u64,
}

// ============================================================================
// Intention Models
// ============================================================================

/// Intention action
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IntentionAction {
/// The `Allow` variant.
    Allow,
/// The `Deny` variant.
    Deny,
}

/// HTTP permission match
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct IntentionHTTPPermission {
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `path_exact` field.
    pub path_exact: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `path_prefix` field.
    pub path_prefix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `path_regex` field.
    pub path_regex: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
/// The `methods` field.
    pub methods: Vec<String>,
}

/// Intention permission
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct IntentionPermission {
/// The `action` field.
    pub action: IntentionAction,
    #[serde(rename = "HTTP", skip_serializing_if = "Option::is_none")]
/// The `http` field.
    pub http: Option<IntentionHTTPPermission>,
}

/// A service intention
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Intention {
    #[serde(rename = "ID")]
/// The `id` field.
    pub id: String,
    #[serde(default)]
/// The `description` field.
    pub description: String,
    #[serde(rename = "SourceNS", default)]
/// The `source_ns` field.
    pub source_ns: String,
/// The `source_name` field.
    pub source_name: String,
    #[serde(rename = "DestinationNS", default)]
/// The `destination_ns` field.
    pub destination_ns: String,
/// The `destination_name` field.
    pub destination_name: String,
/// The `action` field.
    pub action: IntentionAction,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
/// The `permissions` field.
    pub permissions: Vec<IntentionPermission>,
    #[serde(default)]
/// The `meta` field.
    pub meta: std::collections::HashMap<String, String>,
/// The `precedence` field.
    pub precedence: i32,
/// The `created_at` field.
    pub created_at: String,
/// The `updated_at` field.
    pub updated_at: String,
/// The `create_index` field.
    pub create_index: u64,
/// The `modify_index` field.
    pub modify_index: u64,
}

/// Request to create/update an intention
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct IntentionRequest {
    #[serde(default)]
/// The `description` field.
    pub description: String,
    #[serde(rename = "SourceNS", default)]
/// The `source_ns` field.
    pub source_ns: String,
/// The `source_name` field.
    pub source_name: String,
    #[serde(rename = "DestinationNS", default)]
/// The `destination_ns` field.
    pub destination_ns: String,
/// The `destination_name` field.
    pub destination_name: String,
/// The `action` field.
    pub action: IntentionAction,
    #[serde(default)]
/// The `permissions` field.
    pub permissions: Vec<IntentionPermission>,
    #[serde(default)]
/// The `meta` field.
    pub meta: std::collections::HashMap<String, String>,
}

/// L7 request context used for evaluating HTTP-based intention permissions.
///
/// When `method`/`path` are `None`, the request is treated as L4-only and
/// only permissions without HTTP match conditions will apply.
#[derive(Debug, Clone, Default)]
pub struct L7Request {
    /// HTTP method (e.g. "GET", "POST"). Case-insensitive matching.
    pub method: Option<String>,
    /// HTTP request path (e.g. "/api/v1/health").
    pub path: Option<String>,
}

/// Result of an intention check.
pub struct IntentionCheckResult {
    /// Whether the connection is allowed.
    pub allowed: bool,
    /// Human-readable reason describing which intention (if any) matched.
    pub reason: String,
}

/// Intention check response
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct IntentionCheckResponse {
/// The `allowed` field.
    pub allowed: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
/// The `reason` field.
    pub reason: String,
}

/// Intention match query
#[derive(Debug, Deserialize)]
pub struct IntentionMatchQuery {
/// The `by` field.
    pub by: String,
/// The `name` field.
    pub name: String,
}

/// Query parameters for exact intention lookup
#[derive(Debug, Deserialize)]
pub struct IntentionExactQuery {
/// The `source` field.
    pub source: Option<String>,
/// The `destination` field.
    pub destination: Option<String>,
}

/// Agent authorize request
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AgentAuthorizeRequest {
/// The `target` field.
    pub target: String,
    #[serde(rename = "ClientCertURI")]
/// The `client_cert_uri` field.
    pub client_cert_uri: String,
    #[serde(default)]
/// The `client_cert_serial` field.
    pub client_cert_serial: String,
    /// Optional HTTP method for L7 intention evaluation.
    #[serde(default)]
    pub method: Option<String>,
    /// Optional HTTP path for L7 intention evaluation.
    #[serde(default)]
    pub path: Option<String>,
}

/// Agent authorize response
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct AgentAuthorizeResponse {
/// The `authorized` field.
    pub authorized: bool,
/// The `reason` field.
    pub reason: String,
}

/// Query parameters for intentions
#[derive(Debug, Deserialize)]
pub struct IntentionQueryParams {
/// The `filter` field.
    pub filter: Option<String>,
/// The `source` field.
    pub source: Option<String>,
/// The `destination` field.
    pub destination: Option<String>,
    /// Optional HTTP method for L7 intention evaluation.
    pub method: Option<String>,
    /// Optional HTTP path for L7 intention evaluation.
    pub path: Option<String>,
}

/// Query parameters for CA root
#[derive(Debug, Deserialize)]
pub struct CARootQueryParams {
/// The `pem` field.
    pub pem: Option<bool>,
    /// Blocking query: minimum index to wait for
    pub index: Option<u64>,
    /// Blocking query: max wait time (e.g. "5s", "30s", "5m")
    pub wait: Option<String>,
}

/// Query parameters for leaf certificate
#[derive(Debug, Default, Deserialize)]
pub struct LeafCertQueryParams {
    /// Blocking query: minimum index to wait for
    pub index: Option<u64>,
    /// Blocking query: max wait time (e.g. "5s", "30s", "5m")
    pub wait: Option<String>,
}

/// Query parameters for the service identity endpoint
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ServiceIdentityQueryParams {
    /// Namespace (defaults to "default")
    pub ns: Option<String>,
}

/// RocksDB key for the CA configuration (stored in CF_CONSUL_CA_ROOTS)
const ROCKS_KEY_CA_CONFIG: &str = "__ca_config__";
/// RocksDB key for the active root ID (stored in CF_CONSUL_CA_ROOTS)
const ROCKS_KEY_ACTIVE_ROOT: &str = "__active_root__";

// ============================================================================
// Service (In-Memory + optional RocksDB persistence)
// ============================================================================

/// Connect CA and Intentions service with optional RocksDB write-through persistence
#[derive(Clone)]
pub struct ConsulConnectCAService {
    /// CA roots
    roots: Arc<DashMap<String, CARoot>>,
    /// Active root ID
    active_root_id: Arc<tokio::sync::RwLock<String>>,
    /// CA configuration
    ca_config: Arc<tokio::sync::RwLock<CAConfig>>,
    /// Intentions
    intentions: Arc<DashMap<String, Intention>>,
    /// Index counter
    index: Arc<std::sync::atomic::AtomicU64>,
    /// Trust domain
    trust_domain: String,
    /// Datacenter name
    datacenter: String,
    /// CA certificate PEM (for signing leaf certs)
    #[allow(dead_code)]
    ca_cert_pem: String,
    /// CA private key PEM (for signing leaf certs)
    ca_key_pem: String,
    /// Optional RocksDB for write-through persistence
    rocks_db: Option<Arc<DB>>,
    /// Optional Raft writer for cluster-mode replication
    raft_node: Option<Arc<ConsulRaftWriter>>,
}

impl ConsulConnectCAService {
/// The `new` associated function.
    pub fn new() -> Self {
        let root_id = uuid::Uuid::new_v4().to_string();

        // Generate a real self-signed CA root certificate
        let (ca_cert_pem, ca_key_pem) = Self::generate_ca_root_cert();

        let root = CARoot {
            id: root_id.clone(),
            name: "Consul CA Root Cert".to_string(),
            root_cert: ca_cert_pem.clone(),
            active: true,
            create_index: 1,
            modify_index: 1,
        };

        let config = CAConfig {
            provider: "consul".to_string(),
            config: {
                let mut m = std::collections::HashMap::new();
                m.insert(
                    "LeafCertTTL".to_string(),
                    serde_json::Value::String("72h".to_string()),
                );
                m.insert(
                    "RootCertTTL".to_string(),
                    serde_json::Value::String("87600h".to_string()),
                );
                m
            },
            state: std::collections::HashMap::new(),
            force_without_cross_signing: false,
            create_index: 1,
            modify_index: 1,
        };

        let roots = DashMap::new();
        roots.insert(root_id.clone(), root);

        Self {
            roots: Arc::new(roots),
            active_root_id: Arc::new(tokio::sync::RwLock::new(root_id)),
            ca_config: Arc::new(tokio::sync::RwLock::new(config)),
            intentions: Arc::new(DashMap::new()),
            index: Arc::new(std::sync::atomic::AtomicU64::new(2)),
            trust_domain: "consul".to_string(),
            datacenter: "dc1".to_string(),
            ca_cert_pem,
            ca_key_pem,
            rocks_db: None,
            raft_node: None,
        }
    }

    /// Create a Connect CA service backed by an existing RocksDB instance.
    /// Loads CA roots from CF_CONSUL_CA_ROOTS and intentions from CF_CONSUL_INTENTIONS.
    pub fn with_rocks(db: Arc<DB>) -> Self {
        let roots = Arc::new(DashMap::new());
        let intentions = Arc::new(DashMap::new());
        let mut max_index = 0u64;
        let mut loaded_active_root_id: Option<String> = None;
        let mut loaded_ca_config: Option<CAConfig> = None;

        // Load CA roots
        if let Some(cf) = db.cf_handle(CF_CONSUL_CA_ROOTS) {
            let iter = db.iterator_cf(cf, rocksdb::IteratorMode::Start);
            let mut root_count = 0u64;

            for item in iter.flatten() {
                let (key_bytes, value_bytes) = item;
                if let Ok(key) = String::from_utf8(key_bytes.to_vec()) {
                    if key == ROCKS_KEY_CA_CONFIG {
                        match serde_json::from_slice::<CAConfig>(&value_bytes) {
                            Ok(config) => {
                                if config.modify_index > max_index {
                                    max_index = config.modify_index;
                                }
                                loaded_ca_config = Some(config);
                            }
                            Err(e) => {
                                error!("Failed to deserialize CA config from RocksDB: {}", e);
                            }
                        }
                    } else if key == ROCKS_KEY_ACTIVE_ROOT {
                        if let Ok(id) = String::from_utf8(value_bytes.to_vec()) {
                            loaded_active_root_id = Some(id);
                        }
                    } else {
                        match serde_json::from_slice::<CARoot>(&value_bytes) {
                            Ok(root) => {
                                if root.modify_index > max_index {
                                    max_index = root.modify_index;
                                }
                                roots.insert(key, root);
                                root_count += 1;
                            }
                            Err(e) => {
                                error!(
                                    "Failed to deserialize CA root '{}' from RocksDB: {}",
                                    key, e
                                );
                            }
                        }
                    }
                }
            }
            info!(
                "Loaded {} CA roots from RocksDB (max_index={})",
                root_count, max_index
            );
        }

        // Load intentions
        if let Some(cf) = db.cf_handle(CF_CONSUL_INTENTIONS) {
            let iter = db.iterator_cf(cf, rocksdb::IteratorMode::Start);
            let mut intention_count = 0u64;

            for item in iter.flatten() {
                let (key_bytes, value_bytes) = item;
                if let Ok(key) = String::from_utf8(key_bytes.to_vec()) {
                    match serde_json::from_slice::<Intention>(&value_bytes) {
                        Ok(intention) => {
                            if intention.modify_index > max_index {
                                max_index = intention.modify_index;
                            }
                            intentions.insert(key, intention);
                            intention_count += 1;
                        }
                        Err(e) => {
                            error!(
                                "Failed to deserialize intention '{}' from RocksDB: {}",
                                key, e
                            );
                        }
                    }
                }
            }
            info!(
                "Loaded {} intentions from RocksDB (max_index={})",
                intention_count, max_index
            );
        }

        // If no roots were loaded from RocksDB, generate a fresh CA root
        let (ca_cert_pem, ca_key_pem) = Self::generate_ca_root_cert();
        if roots.is_empty() {
            let root_id = uuid::Uuid::new_v4().to_string();
            let root = CARoot {
                id: root_id.clone(),
                name: "Consul CA Root Cert".to_string(),
                root_cert: ca_cert_pem.clone(),
                active: true,
                create_index: 1,
                modify_index: 1,
            };
            roots.insert(root_id.clone(), root);
            loaded_active_root_id = Some(root_id);
        }

        let active_root_id = loaded_active_root_id.unwrap_or_else(|| {
            // Fallback: find the first active root
            roots
                .iter()
                .find(|r| r.value().active)
                .map(|r| r.key().clone())
                .unwrap_or_default()
        });

        let ca_config = loaded_ca_config.unwrap_or_else(|| CAConfig {
            provider: "consul".to_string(),
            config: {
                let mut m = std::collections::HashMap::new();
                m.insert(
                    "LeafCertTTL".to_string(),
                    serde_json::Value::String("72h".to_string()),
                );
                m.insert(
                    "RootCertTTL".to_string(),
                    serde_json::Value::String("87600h".to_string()),
                );
                m
            },
            state: std::collections::HashMap::new(),
            force_without_cross_signing: false,
            create_index: 1,
            modify_index: 1,
        });

        Self {
            roots,
            active_root_id: Arc::new(tokio::sync::RwLock::new(active_root_id)),
            ca_config: Arc::new(tokio::sync::RwLock::new(ca_config)),
            intentions,
            index: Arc::new(std::sync::atomic::AtomicU64::new(max_index + 1)),
            trust_domain: "consul".to_string(),
            datacenter: "dc1".to_string(),
            ca_cert_pem,
            ca_key_pem,
            rocks_db: Some(db),
            raft_node: None,
        }
    }

    /// Create a Connect CA service with Raft-replicated storage (cluster mode).
    pub fn with_raft(db: Arc<DB>, raft_node: Arc<ConsulRaftWriter>) -> Self {
        let mut svc = Self::with_rocks(db);
        svc.raft_node = Some(raft_node);
        svc
    }

/// The `with_datacenter` method.
    pub fn with_datacenter(mut self, datacenter: String) -> Self {
        self.datacenter = datacenter;
        self
    }

    // ========================================================================
    // RocksDB persistence helpers
    // ========================================================================

    /// Persist a CA root to RocksDB
    #[allow(dead_code)]
    fn persist_root_to_rocks(&self, id: &str, root: &CARoot) {
        if let Some(ref db) = self.rocks_db
            && let Some(cf) = db.cf_handle(CF_CONSUL_CA_ROOTS)
        {
            match serde_json::to_vec(root) {
                Ok(json_bytes) => {
                    if let Err(e) = db.put_cf(cf, id.as_bytes(), &json_bytes) {
                        error!("Failed to persist CA root '{}' to RocksDB: {}", id, e);
                    }
                }
                Err(e) => {
                    error!("Failed to serialize CA root '{}' for RocksDB: {}", id, e);
                }
            }
        }
    }

    /// Delete a CA root from RocksDB
    #[allow(dead_code)]
    fn delete_root_from_rocks(&self, id: &str) {
        if let Some(ref db) = self.rocks_db
            && let Some(cf) = db.cf_handle(CF_CONSUL_CA_ROOTS)
            && let Err(e) = db.delete_cf(cf, id.as_bytes())
        {
            error!("Failed to delete CA root '{}' from RocksDB: {}", id, e);
        }
    }

    /// Persist the active root ID to RocksDB
    #[allow(dead_code)]
    fn persist_active_root_to_rocks(&self, active_root_id: &str) {
        if let Some(ref db) = self.rocks_db
            && let Some(cf) = db.cf_handle(CF_CONSUL_CA_ROOTS)
            && let Err(e) = db.put_cf(
                cf,
                ROCKS_KEY_ACTIVE_ROOT.as_bytes(),
                active_root_id.as_bytes(),
            )
        {
            error!("Failed to persist active root ID to RocksDB: {}", e);
        }
    }

    /// Persist the CA configuration to RocksDB
    fn persist_ca_config_to_rocks(&self, config: &CAConfig) {
        if let Some(ref db) = self.rocks_db
            && let Some(cf) = db.cf_handle(CF_CONSUL_CA_ROOTS)
        {
            match serde_json::to_vec(config) {
                Ok(json_bytes) => {
                    if let Err(e) = db.put_cf(cf, ROCKS_KEY_CA_CONFIG.as_bytes(), &json_bytes) {
                        error!("Failed to persist CA config to RocksDB: {}", e);
                    }
                }
                Err(e) => {
                    error!("Failed to serialize CA config for RocksDB: {}", e);
                }
            }
        }
    }

    /// Persist an intention to RocksDB
    fn persist_intention_to_rocks(&self, id: &str, intention: &Intention) {
        if let Some(ref db) = self.rocks_db
            && let Some(cf) = db.cf_handle(CF_CONSUL_INTENTIONS)
        {
            match serde_json::to_vec(intention) {
                Ok(json_bytes) => {
                    if let Err(e) = db.put_cf(cf, id.as_bytes(), &json_bytes) {
                        error!("Failed to persist intention '{}' to RocksDB: {}", id, e);
                    }
                }
                Err(e) => {
                    error!("Failed to serialize intention '{}' for RocksDB: {}", id, e);
                }
            }
        }
    }

    /// Delete an intention from RocksDB
    fn delete_intention_from_rocks(&self, id: &str) {
        if let Some(ref db) = self.rocks_db
            && let Some(cf) = db.cf_handle(CF_CONSUL_INTENTIONS)
            && let Err(e) = db.delete_cf(cf, id.as_bytes())
        {
            error!("Failed to delete intention '{}' from RocksDB: {}", id, e);
        }
    }

    // ========================================================================
    // CA certificate generation
    // ========================================================================

    /// Generate a self-signed CA root certificate using rcgen
    fn generate_ca_root_cert() -> (String, String) {
        use rcgen::{BasicConstraints, CertificateParams, DnType, IsCa, KeyPair};

        let mut params = CertificateParams::default();
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        params
            .distinguished_name
            .push(DnType::CommonName, "Consul CA Root");
        params
            .distinguished_name
            .push(DnType::OrganizationName, "Batata Consul");

        let key_pair = KeyPair::generate().expect("Failed to generate CA key pair");
        let cert = params
            .self_signed(&key_pair)
            .expect("Failed to generate CA certificate");

        (cert.pem(), key_pair.serialize_pem())
    }

    // ========================================================================
    // CA operations
    // ========================================================================

/// The `get_roots` method.
    pub async fn get_roots(&self) -> CARootList {
        let active_id = self.active_root_id.read().await.clone();
        let roots: Vec<CARoot> = self.roots.iter().map(|r| r.value().clone()).collect();
        CARootList {
            active_root_id: active_id,
            trust_domain: self.trust_domain.clone(),
            roots,
        }
    }

/// The `get_ca_config` method.
    pub async fn get_ca_config(&self) -> CAConfig {
        self.ca_config.read().await.clone()
    }

/// The `set_ca_config` method.
    pub async fn set_ca_config(&self, mut config: CAConfig) -> Result<(), String> {
        if config.provider.is_empty() {
            return Err("Provider is required".to_string());
        }
        let index = self.index.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        config.modify_index = index;
        if let Some(ref raft) = self.raft_node {
            let config_json = serde_json::to_string(&config).unwrap_or_default();
            match raft
                .write(ConsulRaftRequest::CAConfigUpdate { config_json })
                .await
            {
                Ok(r) if !r.success => {
                    error!("Raft CAConfigUpdate rejected: {:?}", r.message);
                }
                Err(e) => {
                    error!("Raft CAConfigUpdate failed: {}", e);
                }
                _ => {}
            }
        } else {
            self.persist_ca_config_to_rocks(&config);
        }
        *self.ca_config.write().await = config;
        Ok(())
    }

/// The `get_leaf_cert` method.
    pub fn get_leaf_cert(&self, service: &str) -> LeafCert {
        let now = Utc::now();
        let valid_before = now + chrono::Duration::hours(72);
        let index = self.index.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let (cert_pem, private_key_pem) = self.generate_leaf_cert(service);

        LeafCert {
            serial_number: format!(
                "{:02x}:{:02x}:{:02x}:{:02x}",
                rand_byte(),
                rand_byte(),
                rand_byte(),
                rand_byte()
            ),
            cert_pem,
            private_key_pem,
            service: service.to_string(),
            service_uri: format!(
                "spiffe://{}/ns/default/dc/{}/svc/{}",
                self.trust_domain, self.datacenter, service
            ),
            valid_after: now.to_rfc3339(),
            valid_before: valid_before.to_rfc3339(),
            create_index: index,
            modify_index: index,
        }
    }

    /// Get the full service identity (leaf cert + CA roots + SPIFFE ID) for a
    /// registered service. Used by the `/v1/connect/service/{service_id}`
    /// endpoint so sidecar proxies can fetch all mTLS material in one call.
    pub fn get_service_identity(&self, service_id: &str, service_name: &str) -> ServiceIdentity {
        let leaf = self.get_leaf_cert(service_name);
        let roots: Vec<CARoot> = self.roots.iter().map(|r| r.value().clone()).collect();
        ServiceIdentity {
            service_name: service_name.to_string(),
            service_id: service_id.to_string(),
            service_uri: leaf.service_uri,
            cert_pem: leaf.cert_pem,
            private_key_pem: leaf.private_key_pem,
            roots,
            trust_domain: self.trust_domain.clone(),
            datacenter: self.datacenter.clone(),
            valid_after: leaf.valid_after,
            valid_before: leaf.valid_before,
            create_index: leaf.create_index,
            modify_index: leaf.modify_index,
        }
    }

    /// Generate a leaf certificate signed by the CA for a given service
    fn generate_leaf_cert(&self, service: &str) -> (String, String) {
        use rcgen::{BasicConstraints, CertificateParams, DnType, IsCa, Issuer, KeyPair};

        // Generate leaf key pair
        let leaf_key_pair = KeyPair::generate().expect("Failed to generate leaf key pair");

        // Create leaf certificate params
        let mut leaf_params = CertificateParams::default();
        leaf_params
            .distinguished_name
            .push(DnType::CommonName, format!("{}.svc.consul", service));
        leaf_params
            .distinguished_name
            .push(DnType::OrganizationName, "Batata Consul Service");

        // Reconstruct CA params and issuer from stored key for signing
        let ca_key = KeyPair::from_pem(&self.ca_key_pem).expect("Failed to parse CA key");
        let mut ca_params = CertificateParams::default();
        ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        ca_params
            .distinguished_name
            .push(DnType::CommonName, "Consul CA Root");
        ca_params
            .distinguished_name
            .push(DnType::OrganizationName, "Batata Consul");
        let issuer = Issuer::from_params(&ca_params, &ca_key);

        // Sign leaf cert with CA
        let leaf_cert = leaf_params
            .signed_by(&leaf_key_pair, &issuer)
            .expect("Failed to sign leaf certificate");

        (leaf_cert.pem(), leaf_key_pair.serialize_pem())
    }

    // ========================================================================
    // Intention operations
    // ========================================================================

/// The `create_intention` method.
    pub async fn create_intention(&self, req: IntentionRequest) -> Intention {
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let index = self.index.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let precedence = Self::compute_precedence(&req.source_name, &req.destination_name);

        let intention = Intention {
            id: id.clone(),
            description: req.description,
            source_ns: if req.source_ns.is_empty() {
                "default".to_string()
            } else {
                req.source_ns
            },
            source_name: req.source_name,
            destination_ns: if req.destination_ns.is_empty() {
                "default".to_string()
            } else {
                req.destination_ns
            },
            destination_name: req.destination_name,
            action: req.action,
            permissions: req.permissions,
            meta: req.meta,
            precedence,
            created_at: now.clone(),
            updated_at: now,
            create_index: index,
            modify_index: index,
        };

        self.intentions.insert(id.clone(), intention.clone());
        if let Some(ref raft) = self.raft_node {
            let intention_json = serde_json::to_string(&intention).unwrap_or_default();
            match raft
                .write(ConsulRaftRequest::IntentionUpsert {
                    id: id.clone(),
                    intention_json,
                })
                .await
            {
                Ok(r) if !r.success => {
                    error!("Raft IntentionUpsert rejected: {:?}", r.message);
                }
                Err(e) => {
                    error!("Raft IntentionUpsert failed: {}", e);
                }
                _ => {}
            }
        } else {
            self.persist_intention_to_rocks(&id, &intention);
        }
        intention
    }

/// The `get_intention` method.
    pub fn get_intention(&self, id: &str) -> Option<Intention> {
        self.intentions.get(id).map(|r| r.value().clone())
    }

/// The `update_intention` method.
    pub async fn update_intention(&self, id: &str, req: IntentionRequest) -> Option<Intention> {
        if let Some(mut entry) = self.intentions.get_mut(id) {
            let index = self.index.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            entry.description = req.description;
            entry.source_ns = if req.source_ns.is_empty() {
                "default".to_string()
            } else {
                req.source_ns
            };
            entry.source_name = req.source_name.clone();
            entry.destination_ns = if req.destination_ns.is_empty() {
                "default".to_string()
            } else {
                req.destination_ns
            };
            entry.destination_name = req.destination_name.clone();
            entry.action = req.action;
            entry.permissions = req.permissions;
            entry.meta = req.meta;
            entry.precedence = Self::compute_precedence(&req.source_name, &req.destination_name);
            entry.updated_at = Utc::now().to_rfc3339();
            entry.modify_index = index;
            let updated = entry.clone();
            if let Some(ref raft) = self.raft_node {
                let intention_json = serde_json::to_string(&updated).unwrap_or_default();
                match raft
                    .write(ConsulRaftRequest::IntentionUpsert {
                        id: id.to_string(),
                        intention_json,
                    })
                    .await
                {
                    Ok(r) if !r.success => {
                        error!("Raft IntentionUpsert rejected: {:?}", r.message);
                    }
                    Err(e) => {
                        error!("Raft IntentionUpsert failed: {}", e);
                    }
                    _ => {}
                }
            } else {
                self.persist_intention_to_rocks(id, &updated);
            }
            Some(updated)
        } else {
            None
        }
    }

/// The `delete_intention` method.
    pub async fn delete_intention(&self, id: &str) -> bool {
        let removed = self.intentions.remove(id).is_some();
        if removed {
            if let Some(ref raft) = self.raft_node {
                match raft
                    .write(ConsulRaftRequest::IntentionDelete { id: id.to_string() })
                    .await
                {
                    Ok(r) if !r.success => {
                        error!("Raft IntentionDelete rejected: {:?}", r.message);
                    }
                    Err(e) => {
                        error!("Raft IntentionDelete failed: {}", e);
                    }
                    _ => {}
                }
            } else {
                self.delete_intention_from_rocks(id);
            }
        }
        removed
    }

/// The `list_intentions` method.
    pub fn list_intentions(&self) -> Vec<Intention> {
        let mut intentions: Vec<Intention> =
            self.intentions.iter().map(|r| r.value().clone()).collect();
        // Sort by precedence (highest first)
        intentions.sort_by(|a, b| b.precedence.cmp(&a.precedence));
        intentions
    }

/// The `check_intention` method.
    pub fn check_intention(
        &self,
        source: &str,
        destination: &str,
        l7: Option<&L7Request>,
    ) -> IntentionCheckResult {
        // Find the highest-precedence matching intention
        let mut best: Option<Intention> = None;
        let mut best_precedence = i32::MIN;

        for entry in self.intentions.iter() {
            let i = entry.value();
            let source_match = i.source_name == "*" || i.source_name == source;
            let dest_match = i.destination_name == "*" || i.destination_name == destination;

            if source_match && dest_match && i.precedence > best_precedence {
                best = Some(i.clone());
                best_precedence = i.precedence;
            }
        }

        let Some(intention) = best else {
            // Default: allow if no intention matches
            return IntentionCheckResult {
                allowed: true,
                reason: format!(
                    "No intention matched (source={}, destination={}); default allow",
                    source, destination
                ),
            };
        };

        // L7 mode: intention has permissions — evaluate them against the request.
        if !intention.permissions.is_empty() {
            return evaluate_intention_permissions(&intention, source, destination, l7);
        }

        // L4 mode (legacy): use the top-level action.
        let allowed = intention.action == IntentionAction::Allow;
        let verb = if allowed { "allows" } else { "denies" };
        IntentionCheckResult {
            allowed,
            reason: format!(
                "Intention {} (source={}, destination={}) {} connection",
                intention.id, intention.source_name, intention.destination_name, verb
            ),
        }
    }

/// The `match_intentions` method.
    pub fn match_intentions(&self, by: &str, name: &str) -> Vec<Intention> {
        let mut matched: Vec<Intention> = self
            .intentions
            .iter()
            .filter(|r| {
                let i = r.value();
                match by {
                    "source" => i.source_name == name || i.source_name == "*",
                    "destination" => i.destination_name == name || i.destination_name == "*",
                    _ => false,
                }
            })
            .map(|r| r.value().clone())
            .collect();
        matched.sort_by(|a, b| b.precedence.cmp(&a.precedence));
        matched
    }

    fn compute_precedence(source: &str, destination: &str) -> i32 {
        match (source, destination) {
            ("*", "*") => 1,
            (_, "*") => 2,
            ("*", _) => 3,
            (_, _) => 4,
        }
    }

    /// Find an intention by exact source and destination name match
    pub fn get_intention_exact(&self, source: &str, destination: &str) -> Option<Intention> {
        self.intentions
            .iter()
            .find(|r| r.value().source_name == source && r.value().destination_name == destination)
            .map(|r| r.value().clone())
    }

    /// Delete an intention by exact source and destination name match
    pub async fn delete_intention_exact(&self, source: &str, destination: &str) -> bool {
        let key_to_remove = self
            .intentions
            .iter()
            .find(|r| r.value().source_name == source && r.value().destination_name == destination)
            .map(|r| r.key().clone());
        if let Some(key) = key_to_remove {
            self.intentions.remove(&key);
            if let Some(ref raft) = self.raft_node {
                match raft
                    .write(ConsulRaftRequest::IntentionDelete {
                        id: key.to_string(),
                    })
                    .await
                {
                    Ok(r) if !r.success => {
                        error!("Raft IntentionDelete rejected: {:?}", r.message);
                    }
                    Err(e) => {
                        error!("Raft IntentionDelete failed: {}", e);
                    }
                    _ => {}
                }
            } else {
                self.delete_intention_from_rocks(&key);
            }
            true
        } else {
            false
        }
    }

    /// Create or update an intention by exact source and destination name match
    pub async fn upsert_intention_exact(&self, req: IntentionRequest) -> Intention {
        // Find existing intention with same source/destination
        let existing_id = self
            .intentions
            .iter()
            .find(|r| {
                r.value().source_name == req.source_name
                    && r.value().destination_name == req.destination_name
            })
            .map(|r| r.key().clone());

        if let Some(id) = existing_id {
            // Update existing
            self.update_intention(&id, req).await.unwrap()
        } else {
            // Create new
            self.create_intention(req).await
        }
    }

/// The `authorize` method.
    pub fn authorize(
        &self,
        target: &str,
        client_cert_uri: &str,
        l7: Option<&L7Request>,
    ) -> AgentAuthorizeResponse {
        // Extract source service from SPIFFE URI
        let source = client_cert_uri.rsplit('/').next().unwrap_or("unknown");

        let result = self.check_intention(source, target, l7);
        AgentAuthorizeResponse {
            authorized: result.allowed,
            reason: result.reason,
        }
    }
}

impl Default for ConsulConnectCAService {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// L7 Intention Evaluation
// ============================================================================

/// Evaluate all permissions of an L7 intention against the request.
///
/// Returns an `IntentionCheckResult` describing whether the connection is
/// allowed and which permission (if any) matched.
///
/// Consul semantics:
/// - Permissions are evaluated in order; the first permission whose HTTP
///   match conditions are satisfied decides the outcome.
/// - If no permission matches, the connection is denied (L7 default-deny).
fn evaluate_intention_permissions(
    intention: &Intention,
    source: &str,
    destination: &str,
    l7: Option<&L7Request>,
) -> IntentionCheckResult {
    for permission in &intention.permissions {
        if permission_http_matches(permission, l7) {
            let allowed = permission.action == IntentionAction::Allow;
            return IntentionCheckResult {
                allowed,
                reason: format!(
                    "Intention {} (source={}, destination={}) permission action={:?} matched",
                    intention.id, source, destination, permission.action
                ),
            };
        }
    }

    // No permission matched → deny (L7 default-deny).
    IntentionCheckResult {
        allowed: false,
        reason: format!(
            "Intention {} (source={}, destination={}) has permissions but none matched the request; deny",
            intention.id, source, destination
        ),
    }
}

/// Check whether a permission's HTTP match conditions are satisfied by the
/// request. A permission without HTTP conditions matches all requests.
fn permission_http_matches(permission: &IntentionPermission, l7: Option<&L7Request>) -> bool {
    let Some(http) = &permission.http else {
        return true;
    };
    evaluate_http_permission(http, l7)
}

/// Evaluate an `IntentionHTTPPermission` against the L7 request context.
///
/// All defined conditions are ANDed together. If a condition is absent it is
/// treated as "match any". If no L7 request context is provided, only
/// permissions without HTTP conditions match (handled by the caller).
fn evaluate_http_permission(http: &IntentionHTTPPermission, l7: Option<&L7Request>) -> bool {
    // Method check (case-insensitive).
    if !http.methods.is_empty() {
        let req_method = l7.and_then(|r| r.method.as_deref()).unwrap_or("");
        if !http
            .methods
            .iter()
            .any(|m| m.eq_ignore_ascii_case(req_method))
        {
            return false;
        }
    }

    let req_path = l7.and_then(|r| r.path.as_deref());

    // Path checks: only one of exact/prefix/regex should be set, but we
    // evaluate whichever is present.
    if let Some(prefix) = &http.path_prefix {
        let path = req_path.unwrap_or("");
        if !path.starts_with(prefix.as_str()) {
            return false;
        }
    }

    if let Some(exact) = &http.path_exact {
        let path = req_path.unwrap_or("");
        if path != exact.as_str() {
            return false;
        }
    }

    if let Some(regex) = &http.path_regex {
        let path = req_path.unwrap_or("");
        match regex::Regex::new(regex) {
            Ok(re) => {
                if !re.is_match(path) {
                    return false;
                }
            }
            Err(_) => {
                // Invalid regex should have been rejected at creation time.
                return false;
            }
        }
    }

    true
}

/// Validate the `permissions` of an intention request.
///
/// Checks that:
/// - Each permission's HTTP `methods` (if any) are valid HTTP methods.
/// - Each permission's HTTP `path_regex` (if any) compiles successfully.
///
/// Returns an error message describing the first validation failure, or
/// `Ok(())` if all permissions are valid.
fn validate_intention_permissions(
    permissions: &[IntentionPermission],
) -> Result<(), String> {
    const VALID_METHODS: &[&str] = &[
        "GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS", "CONNECT", "TRACE",
    ];

    for (idx, perm) in permissions.iter().enumerate() {
        let Some(http) = &perm.http else {
            continue;
        };

        for method in &http.methods {
            if !VALID_METHODS
                .iter()
                .any(|m| m.eq_ignore_ascii_case(method))
            {
                return Err(format!(
                    "permission[{}] has invalid HTTP method: {}",
                    idx, method
                ));
            }
        }

        if let Some(regex) = &http.path_regex {
            if regex::Regex::new(regex).is_err() {
                return Err(format!(
                    "permission[{}] has invalid path_regex: {}",
                    idx, regex
                ));
            }
        }
    }

    Ok(())
}

/// Generate a random byte for serial numbers
fn rand_byte() -> u8 {
    rand::random::<u8>()
}

// ============================================================================
// HTTP Handlers (In-Memory)
// ============================================================================

/// GET /v1/connect/ca/roots - List CA root certificates
pub async fn get_ca_roots(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    query: web::Query<CARootQueryParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    // Support blocking queries (Watch API)
    if let Some(target_index) = query.index {
        let timeout = query
            .wait
            .as_deref()
            .and_then(crate::index_provider::ConsulIndexProvider::parse_wait_duration);
        index_provider
            .wait_for_change(ConsulTable::Catalog, target_index, timeout)
            .await;
    }

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ConnectCA));

    // Support ?pem=true — return raw PEM certificate chain
    // (used by Envoy proxies for certificate validation)
    if query.pem.unwrap_or(false) {
        let roots = ca_service.get_roots().await;
        let mut pem_chain = String::new();
        for root in &roots.roots {
            if !root.root_cert.is_empty() {
                pem_chain.push_str(&root.root_cert);
                if !root.root_cert.ends_with('\n') {
                    pem_chain.push('\n');
                }
            }
        }
        return consul_ok(&meta)
            .content_type("application/pem-certificate-chain")
            .body(pem_chain);
    }

    consul_ok(&meta).json(ca_service.get_roots().await)
}

/// GET /v1/connect/ca/configuration - Get CA configuration
pub async fn get_ca_configuration(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ConnectCA));
    consul_ok(&meta).json(ca_service.get_ca_config().await)
}

/// PUT /v1/connect/ca/configuration - Set CA configuration
pub async fn set_ca_configuration(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    body: web::Json<CAConfig>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    match ca_service.set_ca_config(body.into_inner()).await {
        Ok(()) => {
            let meta =
                ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ConnectCA));
            consul_ok(&meta).finish()
        }
        Err(e) => HttpResponse::BadRequest().consul_error(ConsulError::new(e)),
    }
}

/// GET /v1/agent/connect/ca/leaf/{service} - Get leaf certificate
pub async fn get_leaf_cert(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    path: web::Path<String>,
    query: web::Query<LeafCertQueryParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    // Support blocking queries (Watch API)
    if let Some(target_index) = query.index {
        let timeout = query
            .wait
            .as_deref()
            .and_then(crate::index_provider::ConsulIndexProvider::parse_wait_duration);
        index_provider
            .wait_for_change(ConsulTable::Catalog, target_index, timeout)
            .await;
    }

    let service = path.into_inner();
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ConnectCA));
    consul_ok(&meta).json(ca_service.get_leaf_cert(&service))
}

/// GET /v1/connect/service/{service_id} - Get service identity (leaf cert + roots + SPIFFE ID)
///
/// Looks up the registered service by ID, generates a leaf certificate, and
/// returns the full identity bundle (SPIFFE ID, cert, private key, CA roots)
/// so a sidecar proxy can bootstrap mTLS in a single call.
pub async fn get_service_identity(
    req: HttpRequest,
    agent: web::Data<ConsulAgentService>,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    index_provider: web::Data<ConsulIndexProvider>,
    path: web::Path<String>,
    query: web::Query<ServiceIdentityQueryParams>,
) -> HttpResponse {
    let service_id = path.into_inner();
    let namespace = dc_config.resolve_ns(&query.ns);

    // ACL: service identity contains private key material — require service:read.
    let authz = acl_service.authorize_request(&req, ResourceType::Service, &service_id, false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    // Resolve the service registration by ID to obtain its service name.
    let Some(data) = agent.naming_store().get_by_service_id(&namespace, &service_id) else {
        return HttpResponse::NotFound()
            .consul_error(ConsulError::new(format!("Service not found: {}", service_id)));
    };
    let Ok(reg) = serde_json::from_slice::<AgentServiceRegistration>(&data) else {
        return HttpResponse::InternalServerError()
            .consul_error(ConsulError::new("Failed to decode service registration"));
    };
    let service_name = reg.name.clone();

    let identity = ca_service.get_service_identity(&service_id, &service_name);
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ConnectCA));
    consul_ok(&meta).json(identity)
}

/// GET /v1/connect/intentions - List intentions
pub async fn list_intentions(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    _query: web::Query<IntentionQueryParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ConnectCA));
    consul_ok(&meta).json(ca_service.list_intentions())
}

/// POST /v1/connect/intentions - Create intention
pub async fn create_intention(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    body: web::Json<IntentionRequest>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let req_body = body.into_inner();
    if let Err(msg) = validate_intention_permissions(&req_body.permissions) {
        return HttpResponse::BadRequest().consul_error(ConsulError::new(&msg));
    }

    let intention = ca_service.create_intention(req_body).await;
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ConnectCA));
    consul_ok(&meta).json(serde_json::json!({ "ID": intention.id }))
}

/// GET /v1/connect/intentions/{id} - Read intention
pub async fn get_intention(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    path: web::Path<String>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let id = path.into_inner();
    match ca_service.get_intention(&id) {
        Some(intention) => {
            let meta =
                ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ConnectCA));
            consul_ok(&meta).json(intention)
        }
        None => HttpResponse::NotFound().consul_error(ConsulError::new("Intention not found")),
    }
}

/// PUT /v1/connect/intentions/{id} - Update intention
pub async fn update_intention(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    path: web::Path<String>,
    body: web::Json<IntentionRequest>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let id = path.into_inner();
    let req_body = body.into_inner();
    if let Err(msg) = validate_intention_permissions(&req_body.permissions) {
        return HttpResponse::BadRequest().consul_error(ConsulError::new(&msg));
    }
    match ca_service.update_intention(&id, req_body).await {
        Some(_) => {
            let meta =
                ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ConnectCA));
            consul_ok(&meta).json(serde_json::json!({ "ID": id }))
        }
        None => HttpResponse::NotFound().consul_error(ConsulError::new("Intention not found")),
    }
}

/// DELETE /v1/connect/intentions/{id} - Delete intention
pub async fn delete_intention(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    path: web::Path<String>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let id = path.into_inner();
    if ca_service.delete_intention(&id).await {
        let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ConnectCA));
        consul_ok(&meta).json(true)
    } else {
        HttpResponse::NotFound().consul_error(ConsulError::new("Intention not found"))
    }
}

/// GET /v1/connect/intentions/check - Check intention access
pub async fn check_intention(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    query: web::Query<IntentionQueryParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let source = query.source.as_deref().unwrap_or("*");
    let destination = query.destination.as_deref().unwrap_or("*");
    let l7 = build_l7_request(query.method.as_deref(), query.path.as_deref());
    let result = ca_service.check_intention(source, destination, l7.as_ref());
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ConnectCA));
    consul_ok(&meta).json(IntentionCheckResponse {
        allowed: result.allowed,
        reason: result.reason,
    })
}

/// Build an `L7Request` from optional method/path strings.
fn build_l7_request(method: Option<&str>, path: Option<&str>) -> Option<L7Request> {
    if method.is_none() && path.is_none() {
        return None;
    }
    Some(L7Request {
        method: method.map(|s| s.to_string()),
        path: path.map(|s| s.to_string()),
    })
}

/// GET /v1/connect/intentions/match - Match intentions for service
pub async fn match_intentions(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    query: web::Query<IntentionMatchQuery>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let matched = ca_service.match_intentions(&query.by, &query.name);
    let mut result = std::collections::HashMap::new();
    result.insert(query.name.clone(), matched);
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ConnectCA));
    consul_ok(&meta).json(result)
}

/// POST /v1/agent/connect/authorize - Authorize a connection
pub async fn connect_authorize(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    body: web::Json<AgentAuthorizeRequest>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let auth_req = body.into_inner();
    let l7 = build_l7_request(auth_req.method.as_deref(), auth_req.path.as_deref());
    let response = ca_service.authorize(&auth_req.target, &auth_req.client_cert_uri, l7.as_ref());
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ConnectCA));
    consul_ok(&meta).json(response)
}

/// GET /v1/connect/intentions/exact - Get intention by exact source/destination
pub async fn get_intention_exact(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    query: web::Query<IntentionExactQuery>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let source = query.source.as_deref().unwrap_or("*");
    let destination = query.destination.as_deref().unwrap_or("*");
    match ca_service.get_intention_exact(source, destination) {
        Some(intention) => {
            let meta =
                ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ConnectCA));
            consul_ok(&meta).json(intention)
        }
        None => HttpResponse::NotFound().consul_error(ConsulError::new("Intention not found")),
    }
}

/// PUT /v1/connect/intentions/exact - Upsert intention by exact source/destination
pub async fn upsert_intention_exact(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    body: web::Json<IntentionRequest>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let intention = ca_service.upsert_intention_exact(body.into_inner()).await;
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ConnectCA));
    consul_ok(&meta).json(serde_json::json!({ "Created": true, "ID": intention.id }))
}

/// DELETE /v1/connect/intentions/exact - Delete intention by exact source/destination
pub async fn delete_intention_exact(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    query: web::Query<IntentionExactQuery>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let source = query.source.as_deref().unwrap_or("*");
    let destination = query.destination.as_deref().unwrap_or("*");
    if ca_service.delete_intention_exact(source, destination).await {
        let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ConnectCA));
        consul_ok(&meta).json(true)
    } else {
        HttpResponse::NotFound().consul_error(ConsulError::new("Intention not found"))
    }
}

// ============================================================================
// HTTP Handlers (Persistent)
// ============================================================================

/// GET /v1/connect/ca/roots (persistent)
pub async fn get_ca_roots_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    query: web::Query<CARootQueryParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    // Support blocking queries (Watch API)
    if let Some(target_index) = query.index {
        let timeout = query
            .wait
            .as_deref()
            .and_then(crate::index_provider::ConsulIndexProvider::parse_wait_duration);
        index_provider
            .wait_for_change(ConsulTable::Catalog, target_index, timeout)
            .await;
    }

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ConnectCA));

    // Support ?pem=true — return raw PEM certificate chain
    if query.pem.unwrap_or(false) {
        let roots = ca_service.get_roots().await;
        let mut pem_chain = String::new();
        for root in &roots.roots {
            if !root.root_cert.is_empty() {
                pem_chain.push_str(&root.root_cert);
                if !root.root_cert.ends_with('\n') {
                    pem_chain.push('\n');
                }
            }
        }
        return consul_ok(&meta)
            .content_type("application/pem-certificate-chain")
            .body(pem_chain);
    }

    consul_ok(&meta).json(ca_service.get_roots().await)
}

/// GET /v1/connect/ca/configuration (persistent)
pub async fn get_ca_configuration_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ConnectCA));
    consul_ok(&meta).json(ca_service.get_ca_config().await)
}

/// PUT /v1/connect/ca/configuration (persistent)
pub async fn set_ca_configuration_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    body: web::Json<CAConfig>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    match ca_service.set_ca_config(body.into_inner()).await {
        Ok(()) => {
            let meta =
                ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ConnectCA));
            consul_ok(&meta).finish()
        }
        Err(e) => HttpResponse::BadRequest().consul_error(ConsulError::new(e)),
    }
}

/// GET /v1/agent/connect/ca/leaf/{service} (persistent)
pub async fn get_leaf_cert_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    path: web::Path<String>,
    query: web::Query<LeafCertQueryParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    // Support blocking queries (Watch API)
    if let Some(target_index) = query.index {
        let timeout = query
            .wait
            .as_deref()
            .and_then(crate::index_provider::ConsulIndexProvider::parse_wait_duration);
        index_provider
            .wait_for_change(ConsulTable::Catalog, target_index, timeout)
            .await;
    }

    let service = path.into_inner();
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ConnectCA));
    consul_ok(&meta).json(ca_service.get_leaf_cert(&service))
}

/// GET /v1/connect/intentions (persistent)
pub async fn list_intentions_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    _query: web::Query<IntentionQueryParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ConnectCA));
    consul_ok(&meta).json(ca_service.list_intentions())
}

/// POST /v1/connect/intentions (persistent)
pub async fn create_intention_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    body: web::Json<IntentionRequest>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let req_body = body.into_inner();
    if let Err(msg) = validate_intention_permissions(&req_body.permissions) {
        return HttpResponse::BadRequest().consul_error(ConsulError::new(&msg));
    }

    let intention = ca_service.create_intention(req_body).await;
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ConnectCA));
    consul_ok(&meta).json(serde_json::json!({ "ID": intention.id }))
}

/// GET /v1/connect/intentions/{id} (persistent)
pub async fn get_intention_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    path: web::Path<String>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let id = path.into_inner();
    match ca_service.get_intention(&id) {
        Some(intention) => {
            let meta =
                ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ConnectCA));
            consul_ok(&meta).json(intention)
        }
        None => HttpResponse::NotFound().consul_error(ConsulError::new("Intention not found")),
    }
}

/// PUT /v1/connect/intentions/{id} (persistent)
pub async fn update_intention_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    path: web::Path<String>,
    body: web::Json<IntentionRequest>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let id = path.into_inner();
    let req_body = body.into_inner();
    if let Err(msg) = validate_intention_permissions(&req_body.permissions) {
        return HttpResponse::BadRequest().consul_error(ConsulError::new(&msg));
    }
    match ca_service.update_intention(&id, req_body).await {
        Some(_) => {
            let meta =
                ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ConnectCA));
            consul_ok(&meta).json(serde_json::json!({ "ID": id }))
        }
        None => HttpResponse::NotFound().consul_error(ConsulError::new("Intention not found")),
    }
}

/// DELETE /v1/connect/intentions/{id} (persistent)
pub async fn delete_intention_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    path: web::Path<String>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let id = path.into_inner();
    if ca_service.delete_intention(&id).await {
        let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ConnectCA));
        consul_ok(&meta).json(true)
    } else {
        HttpResponse::NotFound().consul_error(ConsulError::new("Intention not found"))
    }
}

/// GET /v1/connect/intentions/check (persistent)
pub async fn check_intention_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    query: web::Query<IntentionQueryParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let source = query.source.as_deref().unwrap_or("*");
    let destination = query.destination.as_deref().unwrap_or("*");
    let l7 = build_l7_request(query.method.as_deref(), query.path.as_deref());
    let result = ca_service.check_intention(source, destination, l7.as_ref());
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ConnectCA));
    consul_ok(&meta).json(IntentionCheckResponse {
        allowed: result.allowed,
        reason: result.reason,
    })
}

/// GET /v1/connect/intentions/match (persistent)
pub async fn match_intentions_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    query: web::Query<IntentionMatchQuery>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let matched = ca_service.match_intentions(&query.by, &query.name);
    let mut result = std::collections::HashMap::new();
    result.insert(query.name.clone(), matched);
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ConnectCA));
    consul_ok(&meta).json(result)
}

/// POST /v1/agent/connect/authorize (persistent)
pub async fn connect_authorize_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    body: web::Json<AgentAuthorizeRequest>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let auth_req = body.into_inner();
    let l7 = build_l7_request(auth_req.method.as_deref(), auth_req.path.as_deref());
    let response = ca_service.authorize(&auth_req.target, &auth_req.client_cert_uri, l7.as_ref());
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ConnectCA));
    consul_ok(&meta).json(response)
}

/// GET /v1/connect/intentions/exact (persistent)
pub async fn get_intention_exact_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    query: web::Query<IntentionExactQuery>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let source = query.source.as_deref().unwrap_or("*");
    let destination = query.destination.as_deref().unwrap_or("*");
    match ca_service.get_intention_exact(source, destination) {
        Some(intention) => {
            let meta =
                ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ConnectCA));
            consul_ok(&meta).json(intention)
        }
        None => HttpResponse::NotFound().consul_error(ConsulError::new("Intention not found")),
    }
}

/// PUT /v1/connect/intentions/exact (persistent)
pub async fn upsert_intention_exact_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    body: web::Json<IntentionRequest>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let intention = ca_service.upsert_intention_exact(body.into_inner()).await;
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ConnectCA));
    consul_ok(&meta).json(serde_json::json!({ "Created": true, "ID": intention.id }))
}

/// DELETE /v1/connect/intentions/exact (persistent)
pub async fn delete_intention_exact_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    query: web::Query<IntentionExactQuery>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let source = query.source.as_deref().unwrap_or("*");
    let destination = query.destination.as_deref().unwrap_or("*");
    if ca_service.delete_intention_exact(source, destination).await {
        let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ConnectCA));
        consul_ok(&meta).json(true)
    } else {
        HttpResponse::NotFound().consul_error(ConsulError::new("Intention not found"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ca_roots() {
        let service = ConsulConnectCAService::new();
        let roots = service.get_roots().await;
        assert_eq!(roots.roots.len(), 1);
        let root = &roots.roots[0];
        assert!(root.active);
        assert!(!root.id.is_empty());
        assert!(!root.name.is_empty());
        assert!(root.root_cert.contains("CERTIFICATE"));
        assert_eq!(roots.trust_domain, "consul");
    }

    #[tokio::test]
    async fn test_ca_config() {
        let service = ConsulConnectCAService::new();
        let config = service.get_ca_config().await;
        assert_eq!(config.provider, "consul");
        assert!(config.create_index > 0);
        assert!(config.modify_index > 0);
    }

    #[tokio::test]
    async fn test_set_ca_config() {
        let service = ConsulConnectCAService::new();
        let new_config = CAConfig {
            provider: "vault".to_string(),
            config: std::collections::HashMap::new(),
            state: std::collections::HashMap::new(),
            force_without_cross_signing: false,
            create_index: 0,
            modify_index: 0,
        };
        assert!(service.set_ca_config(new_config).await.is_ok());
        let config = service.get_ca_config().await;
        assert_eq!(config.provider, "vault");
    }

    #[test]
    fn test_leaf_cert() {
        let service = ConsulConnectCAService::new();
        let leaf = service.get_leaf_cert("web");
        assert_eq!(leaf.service, "web");
        assert!(leaf.service_uri.contains("web"));
        assert!(!leaf.cert_pem.is_empty());
    }

    #[tokio::test]
    async fn test_create_intention() {
        let service = ConsulConnectCAService::new();
        let intention = service
            .create_intention(IntentionRequest {
                description: "Allow web to api".to_string(),
                source_ns: String::new(),
                source_name: "web".to_string(),
                destination_ns: String::new(),
                destination_name: "api".to_string(),
                action: IntentionAction::Allow,
                permissions: vec![],
                meta: Default::default(),
            })
            .await;
        assert_eq!(intention.source_name, "web");
        assert_eq!(intention.destination_name, "api");
        assert_eq!(intention.action, IntentionAction::Allow);
        assert_eq!(intention.precedence, 4); // specific to specific
    }

    #[tokio::test]
    async fn test_intention_precedence() {
        let service = ConsulConnectCAService::new();
        // Create a deny-all intention (lowest precedence)
        service
            .create_intention(IntentionRequest {
                description: String::new(),
                source_ns: String::new(),
                source_name: "*".to_string(),
                destination_ns: String::new(),
                destination_name: "*".to_string(),
                action: IntentionAction::Deny,
                permissions: vec![],
                meta: Default::default(),
            })
            .await;
        // Create a specific allow
        service
            .create_intention(IntentionRequest {
                description: String::new(),
                source_ns: String::new(),
                source_name: "web".to_string(),
                destination_ns: String::new(),
                destination_name: "api".to_string(),
                action: IntentionAction::Allow,
                permissions: vec![],
                meta: Default::default(),
            })
            .await;

        // Specific allow should win over wildcard deny
        assert!(service.check_intention("web", "api", None).allowed);
        // Unknown services should be denied (wildcard deny)
        assert!(!service.check_intention("unknown", "other", None).allowed);
    }

    #[tokio::test]
    async fn test_list_intentions() {
        let service = ConsulConnectCAService::new();
        service
            .create_intention(IntentionRequest {
                description: String::new(),
                source_ns: String::new(),
                source_name: "a".to_string(),
                destination_ns: String::new(),
                destination_name: "b".to_string(),
                action: IntentionAction::Allow,
                permissions: vec![],
                meta: Default::default(),
            })
            .await;
        service
            .create_intention(IntentionRequest {
                description: String::new(),
                source_ns: String::new(),
                source_name: "*".to_string(),
                destination_ns: String::new(),
                destination_name: "*".to_string(),
                action: IntentionAction::Deny,
                permissions: vec![],
                meta: Default::default(),
            })
            .await;

        let intentions = service.list_intentions();
        assert_eq!(intentions.len(), 2);
        // Higher precedence first
        assert!(intentions[0].precedence >= intentions[1].precedence);
        // Verify specific intention is first (precedence 4 > 1)
        assert_eq!(intentions[0].source_name, "a");
        assert_eq!(intentions[0].destination_name, "b");
        assert_eq!(intentions[1].source_name, "*");
        assert_eq!(intentions[1].destination_name, "*");
    }

    #[tokio::test]
    async fn test_delete_intention() {
        let service = ConsulConnectCAService::new();
        let intention = service
            .create_intention(IntentionRequest {
                description: String::new(),
                source_ns: String::new(),
                source_name: "web".to_string(),
                destination_ns: String::new(),
                destination_name: "api".to_string(),
                action: IntentionAction::Allow,
                permissions: vec![],
                meta: Default::default(),
            })
            .await;
        assert!(service.delete_intention(&intention.id).await);
        assert!(service.get_intention(&intention.id).is_none());
    }

    #[tokio::test]
    async fn test_authorize() {
        let service = ConsulConnectCAService::new();
        service
            .create_intention(IntentionRequest {
                description: String::new(),
                source_ns: String::new(),
                source_name: "web".to_string(),
                destination_ns: String::new(),
                destination_name: "api".to_string(),
                action: IntentionAction::Allow,
                permissions: vec![],
                meta: Default::default(),
            })
            .await;

        let result = service.authorize("api", "spiffe://consul/ns/default/dc/dc1/svc/web", None);
        assert!(result.authorized);
    }

    #[tokio::test]
    async fn test_match_intentions() {
        let service = ConsulConnectCAService::new();
        service
            .create_intention(IntentionRequest {
                description: String::new(),
                source_ns: String::new(),
                source_name: "web".to_string(),
                destination_ns: String::new(),
                destination_name: "api".to_string(),
                action: IntentionAction::Allow,
                permissions: vec![],
                meta: Default::default(),
            })
            .await;

        let matched = service.match_intentions("source", "web");
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].source_name, "web");
        assert_eq!(matched[0].destination_name, "api");

        let matched = service.match_intentions("destination", "api");
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].destination_name, "api");
    }

    #[tokio::test]
    async fn test_delete_nonexistent_intention() {
        let service = ConsulConnectCAService::new();
        assert!(!service.delete_intention("nonexistent-id").await);
    }

    #[test]
    fn test_get_nonexistent_intention() {
        let service = ConsulConnectCAService::new();
        assert!(service.get_intention("nonexistent-id").is_none());
    }

    #[tokio::test]
    async fn test_update_intention() {
        let service = ConsulConnectCAService::new();
        let intention = service
            .create_intention(IntentionRequest {
                description: "original".to_string(),
                source_ns: String::new(),
                source_name: "web".to_string(),
                destination_ns: String::new(),
                destination_name: "api".to_string(),
                action: IntentionAction::Allow,
                permissions: vec![],
                meta: Default::default(),
            })
            .await;

        let updated = service
            .update_intention(
                &intention.id,
                IntentionRequest {
                    description: "updated".to_string(),
                    source_ns: String::new(),
                    source_name: "frontend".to_string(),
                    destination_ns: String::new(),
                    destination_name: "backend".to_string(),
                    action: IntentionAction::Deny,
                    permissions: vec![],
                    meta: Default::default(),
                },
            )
            .await;

        assert!(updated.is_some());
        let u = updated.unwrap();
        assert_eq!(u.description, "updated");
        assert_eq!(u.source_name, "frontend");
        assert_eq!(u.destination_name, "backend");
        assert_eq!(u.action, IntentionAction::Deny);
        assert_eq!(u.id, intention.id); // ID preserved
    }

    #[tokio::test]
    async fn test_update_nonexistent_intention() {
        let service = ConsulConnectCAService::new();
        let result = service
            .update_intention(
                "nonexistent",
                IntentionRequest {
                    description: String::new(),
                    source_ns: String::new(),
                    source_name: "a".to_string(),
                    destination_ns: String::new(),
                    destination_name: "b".to_string(),
                    action: IntentionAction::Allow,
                    permissions: vec![],
                    meta: Default::default(),
                },
            )
            .await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_authorize_deny() {
        let service = ConsulConnectCAService::new();
        service
            .create_intention(IntentionRequest {
                description: String::new(),
                source_ns: String::new(),
                source_name: "malicious".to_string(),
                destination_ns: String::new(),
                destination_name: "api".to_string(),
                action: IntentionAction::Deny,
                permissions: vec![],
                meta: Default::default(),
            })
            .await;

        let result = service.authorize("api", "spiffe://consul/ns/default/dc/dc1/svc/malicious", None);
        assert!(!result.authorized);
        assert!(result.reason.contains("denies"));
    }

    #[test]
    fn test_authorize_no_intentions_allows() {
        let service = ConsulConnectCAService::new();
        // No intentions configured - default allow
        let result = service.authorize("api", "spiffe://consul/ns/default/dc/dc1/svc/web", None);
        assert!(result.authorized);
    }

    #[tokio::test]
    async fn test_intention_precedence_wildcard_vs_specific() {
        let service = ConsulConnectCAService::new();

        // Wildcard deny-all
        service
            .create_intention(IntentionRequest {
                description: String::new(),
                source_ns: String::new(),
                source_name: "*".to_string(),
                destination_ns: String::new(),
                destination_name: "*".to_string(),
                action: IntentionAction::Deny,
                permissions: vec![],
                meta: Default::default(),
            })
            .await;

        // Specific allow
        service
            .create_intention(IntentionRequest {
                description: String::new(),
                source_ns: String::new(),
                source_name: "web".to_string(),
                destination_ns: String::new(),
                destination_name: "api".to_string(),
                action: IntentionAction::Allow,
                permissions: vec![],
                meta: Default::default(),
            })
            .await;

        // Specific rule (precedence 4) should beat wildcard (precedence 1)
        assert!(service.check_intention("web", "api", None).allowed);

        // Other services should be denied by wildcard
        assert!(!service.check_intention("unknown", "api", None).allowed);
    }

    #[test]
    fn test_leaf_cert_fields() {
        let service = ConsulConnectCAService::new();
        let cert = service.get_leaf_cert("my-service");

        assert!(!cert.serial_number.is_empty());
        assert!(cert.cert_pem.contains("CERTIFICATE"));
        assert!(
            cert.private_key_pem.contains("PRIVATE KEY"),
            "Expected PEM private key, got: {}",
            &cert.private_key_pem[..cert.private_key_pem.len().min(50)]
        );
        assert!(cert.service.contains("my-service"));
        assert!(cert.service_uri.contains("my-service"));
        assert!(!cert.valid_after.is_empty());
        assert!(!cert.valid_before.is_empty());
    }

    #[tokio::test]
    async fn test_ca_config_empty_provider() {
        let service = ConsulConnectCAService::new();
        let result = service
            .set_ca_config(CAConfig {
                provider: String::new(),
                config: Default::default(),
                state: Default::default(),
                force_without_cross_signing: false,
                create_index: 0,
                modify_index: 0,
            })
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Provider is required"));
    }

    #[tokio::test]
    async fn test_intention_default_namespace() {
        let service = ConsulConnectCAService::new();
        let intention = service
            .create_intention(IntentionRequest {
                description: String::new(),
                source_ns: String::new(), // empty → default
                source_name: "web".to_string(),
                destination_ns: String::new(),
                destination_name: "api".to_string(),
                action: IntentionAction::Allow,
                permissions: vec![],
                meta: Default::default(),
            })
            .await;

        assert_eq!(intention.source_ns, "default");
        assert_eq!(intention.destination_ns, "default");
    }

    #[tokio::test]
    async fn test_match_intentions_wildcard() {
        let service = ConsulConnectCAService::new();

        service
            .create_intention(IntentionRequest {
                description: String::new(),
                source_ns: String::new(),
                source_name: "*".to_string(),
                destination_ns: String::new(),
                destination_name: "db".to_string(),
                action: IntentionAction::Deny,
                permissions: vec![],
                meta: Default::default(),
            })
            .await;

        // Wildcard source matches any source query
        let matched = service.match_intentions("source", "anything");
        assert_eq!(matched.len(), 1);
    }

    #[tokio::test]
    async fn test_intention_with_meta() {
        let service = ConsulConnectCAService::new();
        let mut meta = std::collections::HashMap::new();
        meta.insert("env".to_string(), "production".to_string());
        meta.insert("team".to_string(), "platform".to_string());

        let intention = service
            .create_intention(IntentionRequest {
                description: "with metadata".to_string(),
                source_ns: String::new(),
                source_name: "web".to_string(),
                destination_ns: String::new(),
                destination_name: "api".to_string(),
                action: IntentionAction::Allow,
                permissions: vec![],
                meta,
            })
            .await;

        assert_eq!(intention.meta.len(), 2);
        assert_eq!(intention.meta.get("env").unwrap(), "production");
    }

    // ========================================================================
    // L7 Intention tests
    // ========================================================================

    fn l7_req(method: &str, path: &str) -> L7Request {
        L7Request {
            method: Some(method.to_string()),
            path: Some(path.to_string()),
        }
    }

    #[tokio::test]
    async fn test_l7_intention_allow_method_match() {
        let service = ConsulConnectCAService::new();
        service
            .create_intention(IntentionRequest {
                description: String::new(),
                source_ns: String::new(),
                source_name: "web".to_string(),
                destination_ns: String::new(),
                destination_name: "api".to_string(),
                action: IntentionAction::Deny, // top-level action ignored when permissions present
                permissions: vec![IntentionPermission {
                    action: IntentionAction::Allow,
                    http: Some(IntentionHTTPPermission {
                        path_exact: None,
                        path_prefix: None,
                        path_regex: None,
                        methods: vec!["GET".to_string()],
                    }),
                }],
                meta: Default::default(),
            })
            .await;

        let req = l7_req("GET", "/anything");
        let result = service.check_intention("web", "api", Some(&req));
        assert!(result.allowed, "GET should be allowed: {}", result.reason);

        let req = l7_req("POST", "/anything");
        let result = service.check_intention("web", "api", Some(&req));
        assert!(!result.allowed, "POST should be denied: {}", result.reason);
    }

    #[tokio::test]
    async fn test_l7_intention_path_prefix_match() {
        let service = ConsulConnectCAService::new();
        service
            .create_intention(IntentionRequest {
                description: String::new(),
                source_ns: String::new(),
                source_name: "web".to_string(),
                destination_ns: String::new(),
                destination_name: "api".to_string(),
                action: IntentionAction::Allow,
                permissions: vec![IntentionPermission {
                    action: IntentionAction::Allow,
                    http: Some(IntentionHTTPPermission {
                        path_exact: None,
                        path_prefix: Some("/api/".to_string()),
                        path_regex: None,
                        methods: vec![],
                    }),
                }],
                meta: Default::default(),
            })
            .await;

        let req = l7_req("GET", "/api/v1/users");
        assert!(
            service.check_intention("web", "api", Some(&req)).allowed,
            "path with /api/ prefix should be allowed"
        );

        let req = l7_req("GET", "/health");
        assert!(
            !service.check_intention("web", "api", Some(&req)).allowed,
            "path without /api/ prefix should be denied (no permission matched)"
        );
    }

    #[tokio::test]
    async fn test_l7_intention_path_regex_match() {
        let service = ConsulConnectCAService::new();
        service
            .create_intention(IntentionRequest {
                description: String::new(),
                source_ns: String::new(),
                source_name: "web".to_string(),
                destination_ns: String::new(),
                destination_name: "api".to_string(),
                action: IntentionAction::Allow,
                permissions: vec![IntentionPermission {
                    action: IntentionAction::Allow,
                    http: Some(IntentionHTTPPermission {
                        path_exact: None,
                        path_prefix: None,
                        path_regex: Some(r"^/api/v\d+/users/\d+$".to_string()),
                        methods: vec![],
                    }),
                }],
                meta: Default::default(),
            })
            .await;

        let req = l7_req("GET", "/api/v2/users/42");
        assert!(
            service.check_intention("web", "api", Some(&req)).allowed,
            "regex-matching path should be allowed"
        );

        let req = l7_req("GET", "/api/v2/users/abc");
        assert!(
            !service.check_intention("web", "api", Some(&req)).allowed,
            "non-matching regex path should be denied"
        );
    }

    #[tokio::test]
    async fn test_l7_intention_no_matching_permission_denies() {
        let service = ConsulConnectCAService::new();
        service
            .create_intention(IntentionRequest {
                description: String::new(),
                source_ns: String::new(),
                source_name: "web".to_string(),
                destination_ns: String::new(),
                destination_name: "api".to_string(),
                action: IntentionAction::Allow, // ignored — permissions present
                permissions: vec![IntentionPermission {
                    action: IntentionAction::Allow,
                    http: Some(IntentionHTTPPermission {
                        path_exact: Some("/admin".to_string()),
                        path_prefix: None,
                        path_regex: None,
                        methods: vec![],
                    }),
                }],
                meta: Default::default(),
            })
            .await;

        // Request to /public does not match the /admin-only permission → deny
        let req = l7_req("GET", "/public");
        let result = service.check_intention("web", "api", Some(&req));
        assert!(!result.allowed, "no matching permission should deny: {}", result.reason);
        assert!(result.reason.contains("none matched"));
    }

    #[tokio::test]
    async fn test_l7_intention_permission_without_http_matches_all() {
        let service = ConsulConnectCAService::new();
        service
            .create_intention(IntentionRequest {
                description: String::new(),
                source_ns: String::new(),
                source_name: "web".to_string(),
                destination_ns: String::new(),
                destination_name: "api".to_string(),
                action: IntentionAction::Deny,
                permissions: vec![IntentionPermission {
                    action: IntentionAction::Allow,
                    http: None, // no HTTP conditions → matches all requests
                }],
                meta: Default::default(),
            })
            .await;

        // L4 request (no method/path) — permission without HTTP matches
        assert!(
            service.check_intention("web", "api", None).allowed,
            "permission without HTTP conditions should match L4 request"
        );

        // L7 request — still matches
        let req = l7_req("DELETE", "/whatever");
        assert!(
            service.check_intention("web", "api", Some(&req)).allowed,
            "permission without HTTP conditions should match any L7 request"
        );
    }

    #[tokio::test]
    async fn test_l4_compatibility_no_permissions() {
        let service = ConsulConnectCAService::new();
        service
            .create_intention(IntentionRequest {
                description: String::new(),
                source_ns: String::new(),
                source_name: "web".to_string(),
                destination_ns: String::new(),
                destination_name: "api".to_string(),
                action: IntentionAction::Allow,
                permissions: vec![], // L4 mode
                meta: Default::default(),
            })
            .await;

        // Both L4 and L7 requests use the top-level action when no permissions.
        assert!(service.check_intention("web", "api", None).allowed);
        let req = l7_req("POST", "/anything");
        assert!(service.check_intention("web", "api", Some(&req)).allowed);
    }

    #[test]
    fn test_validate_intention_permissions_invalid_method() {
        let permissions = vec![IntentionPermission {
            action: IntentionAction::Allow,
            http: Some(IntentionHTTPPermission {
                path_exact: None,
                path_prefix: None,
                path_regex: None,
                methods: vec!["FETCH".to_string()],
            }),
        }];
        let err = validate_intention_permissions(&permissions).unwrap_err();
        assert!(err.contains("invalid HTTP method") && err.contains("FETCH"));
    }

    #[test]
    fn test_validate_intention_permissions_invalid_regex() {
        let permissions = vec![IntentionPermission {
            action: IntentionAction::Allow,
            http: Some(IntentionHTTPPermission {
                path_exact: None,
                path_prefix: None,
                path_regex: Some("[invalid".to_string()),
                methods: vec![],
            }),
        }];
        let err = validate_intention_permissions(&permissions).unwrap_err();
        assert!(err.contains("invalid path_regex"));
    }

    #[test]
    fn test_validate_intention_permissions_valid() {
        let permissions = vec![
            IntentionPermission {
                action: IntentionAction::Allow,
                http: Some(IntentionHTTPPermission {
                    path_exact: None,
                    path_prefix: Some("/api/".to_string()),
                    path_regex: Some(r"^/v\d+/$".to_string()),
                    methods: vec!["GET".to_string(), "post".to_string()],
                }),
            },
            IntentionPermission {
                action: IntentionAction::Deny,
                http: None,
            },
        ];
        assert!(validate_intention_permissions(&permissions).is_ok());
    }
}
