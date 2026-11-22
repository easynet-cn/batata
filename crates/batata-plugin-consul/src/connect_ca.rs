//! Consul Connect CA and Intentions API
//!
//! Provides mTLS certificate authority, service intentions,
//! and connection authorization endpoints.

use actix_web::{HttpRequest, HttpResponse, web};
use batata_common::datetime::{DateTimeExt, Utc};
use dashmap::DashMap;
use rocksdb::DB;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info};

use crate::constants::{CF_CONSUL_CA_ROOTS, CF_CONSUL_INTENTIONS, ROCKS_KEY_CA_KEY};

use crate::acl::{AclService, ResourceType};
use crate::agent::ConsulAgentService;
use crate::consul_meta::{ConsulResponseMeta, consul_ok};
use crate::index_provider::{ConsulIndexProvider, ConsulTable};
use crate::model::{
    AgentServiceRegistration, ConsulDatacenterConfig, ConsulError, ConsulErrorBody,
};
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
/// CA config state entry holding the fingerprint of the active CA private key.
///
/// Stored alongside the config so a follower can tell whether the key it
/// loaded from RocksDB actually belongs to the loaded CA root.
const CA_KEY_ID_STATE_KEY: &str = "ca_key_id";
/// Default leaf certificate lifetime used when the CA config does not carry
/// a parseable `LeafCertTTL`.
const DEFAULT_LEAF_CERT_TTL: batata_common::datetime::Duration =
    batata_common::datetime::Duration::hours(72);
/// Fraction of the leaf lifetime after which a cached leaf is rotated.
const LEAF_SOFT_EXPIRY_RATIO: f64 = 0.6;

/// Complete mTLS material for one service, assembled from the leaf cache and
/// the active CA root chain. Returned by [`ConsulConnectCAService::get_connect_material_sync`].
#[derive(Debug, Clone)]
pub struct ConnectMaterial {
    /// Leaf certificate in PEM format.
    pub leaf_cert_pem: String,
    /// Private key matching the leaf certificate, in PEM format.
    pub private_key_pem: String,
    /// Concatenated CA root PEMs (active root first), each ending with a newline.
    pub root_pem: String,
    /// SPIFFE ID encoded as the leaf's URI SAN.
    pub service_uri: String,
    /// Leaf expiry timestamp (RFC3339), used by SDS to schedule rotation.
    pub valid_before: String,
}

/// A cached leaf plus the conditions under which it may still be served.
#[derive(Debug, Clone)]
struct CachedLeaf {
    /// The last generated leaf response.
    leaf: LeafCert,
    /// Moment after which the leaf must be regenerated (60% of its TTL).
    soft_expiry: batata_common::datetime::DateTimeUtc,
    /// Fingerprint of the CA key that signed this leaf. A changed CA key
    /// invalidates the cached leaf immediately.
    ca_key_id: String,
}

// ============================================================================
// Service (In-Memory + optional RocksDB persistence)
// ============================================================================

/// Connect CA and Intentions service with optional RocksDB write-through persistence
#[derive(Clone)]
pub struct ConsulConnectCAService {
    /// CA roots
    roots: Arc<DashMap<String, CARoot>>,
    /// Active root ID
    active_root_id: Arc<std::sync::RwLock<String>>,
    /// CA configuration
    ca_config: Arc<std::sync::RwLock<CAConfig>>,
    /// Intentions
    intentions: Arc<DashMap<String, Intention>>,
    /// Index counter shared by intentions and legacy table indexing
    index: Arc<std::sync::atomic::AtomicU64>,
    /// Revision bumped whenever CA material (root/key/config/leaf) changes.
    /// Kept separate from `index` so xDS can watch CA state independently.
    ca_revision: Arc<std::sync::atomic::AtomicU64>,
    /// Cached leaf certificates, keyed by service name
    leaf_cache: Arc<DashMap<String, CachedLeaf>>,
    /// Trust domain
    trust_domain: String,
    /// Datacenter name
    datacenter: String,
    /// CA certificate PEM of the active root (kept in sync for followers).
    /// Leaf signing derives issuer details from the key, so this is currently
    /// only written, not read.
    #[allow(dead_code)]
    ca_cert_pem: Arc<std::sync::RwLock<String>>,
    /// CA private key PEM (for signing leaf certs)
    ca_key_pem: Arc<std::sync::RwLock<String>>,
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

        let mut config = Self::default_ca_config();
        config.state.insert(
            CA_KEY_ID_STATE_KEY.to_string(),
            Self::ca_key_id(&ca_key_pem),
        );

        let roots = DashMap::new();
        roots.insert(root_id.clone(), root);

        Self {
            roots: Arc::new(roots),
            active_root_id: Arc::new(std::sync::RwLock::new(root_id)),
            ca_config: Arc::new(std::sync::RwLock::new(config)),
            intentions: Arc::new(DashMap::new()),
            index: Arc::new(std::sync::atomic::AtomicU64::new(2)),
            ca_revision: Arc::new(std::sync::atomic::AtomicU64::new(1)),
            leaf_cache: Arc::new(DashMap::new()),
            trust_domain: "consul".to_string(),
            datacenter: "dc1".to_string(),
            ca_cert_pem: Arc::new(std::sync::RwLock::new(ca_cert_pem)),
            ca_key_pem: Arc::new(std::sync::RwLock::new(ca_key_pem)),
            rocks_db: None,
            raft_node: None,
        }
    }

    /// Create a Connect CA service backed by an existing RocksDB instance.
    /// Loads CA roots/config/active root/key from CF_CONSUL_CA_ROOTS and
    /// intentions from CF_CONSUL_INTENTIONS. When roots are present but their
    /// private key is missing or does not match the recorded key id, a fresh
    /// root/key pair is generated and written back to RocksDB.
    pub fn with_rocks(db: Arc<DB>) -> Self {
        let roots = Arc::new(DashMap::new());
        let intentions = Arc::new(DashMap::new());
        let mut max_index = 0u64;
        let mut loaded_active_root_id: Option<String> = None;
        let mut loaded_ca_config: Option<CAConfig> = None;
        let mut loaded_ca_key_pem: Option<String> = None;

        // Load CA roots, config, active root ID and the CA private key
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
                    } else if key == ROCKS_KEY_CA_KEY {
                        match String::from_utf8(value_bytes.to_vec()) {
                            Ok(pem) => loaded_ca_key_pem = Some(pem),
                            Err(e) => error!("Failed to read CA key from RocksDB: {}", e),
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

        // The stored key must parse and match the fingerprint recorded in the
        // CA config. Otherwise the loaded key cannot sign valid leaves.
        let loaded_key_usable = loaded_ca_key_pem
            .as_deref()
            .filter(|pem| rcgen::KeyPair::from_pem(pem).is_ok());
        let recorded_key_id = loaded_ca_config
            .as_ref()
            .and_then(|c| c.state.get(CA_KEY_ID_STATE_KEY))
            .cloned();
        let key_matches = loaded_key_usable
            .map(|pem| match recorded_key_id.as_deref() {
                // Without a recorded id there is nothing to compare against;
                // a parseable key is the best evidence available.
                None => true,
                Some(id) => id == Self::ca_key_id(pem).as_str(),
            })
            .unwrap_or(false);

        // Generate and persist a fresh root/key pair when no usable material
        // was loaded. The previously active roots stay in the bundle but are
        // demoted so there is only ever one active signing root.
        let mut generated_material: Option<(String, CARoot, String, CAConfig)> = None;
        if roots.is_empty() || !key_matches {
            let (cert_pem, key_pem) = Self::generate_ca_root_cert();
            let root_id = uuid::Uuid::new_v4().to_string();
            let mut next_index = max_index.saturating_add(1);
            for mut entry in roots.iter_mut() {
                if entry.value().active {
                    entry.value_mut().active = false;
                    entry.value_mut().modify_index = next_index;
                    next_index = next_index.saturating_add(1);
                }
            }
            let root = CARoot {
                id: root_id.clone(),
                name: "Consul CA Root Cert".to_string(),
                root_cert: cert_pem.clone(),
                active: true,
                create_index: next_index,
                modify_index: next_index,
            };
            roots.insert(root_id.clone(), root.clone());

            let mut config = loaded_ca_config.unwrap_or_else(Self::default_ca_config);
            config
                .state
                .insert(CA_KEY_ID_STATE_KEY.to_string(), Self::ca_key_id(&key_pem));
            config.modify_index = next_index.saturating_add(1);

            loaded_active_root_id = Some(root_id.clone());
            loaded_ca_config = Some(config.clone());
            loaded_ca_key_pem = Some(key_pem.clone());
            generated_material = Some((root_id, root, key_pem, config));
        }

        let active_root_id = loaded_active_root_id
            .filter(|id| roots.contains_key(id))
            .or_else(|| {
                roots
                    .iter()
                    .find(|r| r.value().active)
                    .map(|r| r.key().clone())
            })
            .unwrap_or_default();
        let ca_cert_pem = roots
            .get(&active_root_id)
            .map(|r| r.value().root_cert.clone())
            .unwrap_or_default();
        let ca_key_pem = loaded_ca_key_pem.unwrap_or_default();
        let ca_config = loaded_ca_config.unwrap_or_else(Self::default_ca_config);

        let svc = Self {
            roots,
            active_root_id: Arc::new(std::sync::RwLock::new(active_root_id.clone())),
            ca_config: Arc::new(std::sync::RwLock::new(ca_config)),
            intentions,
            index: Arc::new(std::sync::atomic::AtomicU64::new(
                max_index.saturating_add(2),
            )),
            ca_revision: Arc::new(std::sync::atomic::AtomicU64::new(1)),
            leaf_cache: Arc::new(DashMap::new()),
            trust_domain: "consul".to_string(),
            datacenter: "dc1".to_string(),
            ca_cert_pem: Arc::new(std::sync::RwLock::new(ca_cert_pem)),
            ca_key_pem: Arc::new(std::sync::RwLock::new(ca_key_pem)),
            rocks_db: Some(db),
            raft_node: None,
        };

        if let Some((root_id, root, key_pem, config)) = generated_material {
            // Persist demoted roots too, otherwise they would remain active
            // in RocksDB after a restart.
            for entry in svc.roots.iter() {
                if !entry.value().active {
                    svc.persist_root_to_rocks(entry.key(), entry.value());
                }
            }
            svc.persist_root_to_rocks(&root_id, &root);
            svc.persist_active_root_to_rocks(&root_id);
            svc.persist_ca_key_to_rocks(&key_pem);
            svc.persist_ca_config_to_rocks(&config);
            svc.bump_ca_revision();
        } else {
            // A valid key was loaded; make sure the active root pointer is
            // present in RocksDB for future followers.
            svc.persist_active_root_to_rocks(&active_root_id);
        }

        svc
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

    /// Persist the active CA private key PEM to RocksDB
    fn persist_ca_key_to_rocks(&self, key_pem: &str) {
        if let Some(ref db) = self.rocks_db
            && let Some(cf) = db.cf_handle(CF_CONSUL_CA_ROOTS)
            && let Err(e) = db.put_cf(cf, ROCKS_KEY_CA_KEY.as_bytes(), key_pem.as_bytes())
        {
            error!("Failed to persist CA key to RocksDB: {}", e);
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

    /// Default CA configuration used by in-memory and freshly persisted CAs.
    fn default_ca_config() -> CAConfig {
        CAConfig {
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
        }
    }

    /// Stable identifier for a CA private key, used to detect that a cached
    /// leaf was signed by a key which is no longer active.
    fn ca_key_id(key_pem: &str) -> String {
        use base64::Engine as _;
        use sha2::{Digest, Sha256};
        let digest = Sha256::digest(key_pem.as_bytes());
        base64::engine::general_purpose::STANDARD.encode(digest)
    }

    /// Read the CA key, recovering from a poisoned lock rather than panicking.
    fn read_ca_key_pem(&self) -> String {
        self.ca_key_pem
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Advance the CA revision observed by the xDS sync bridge.
    fn bump_ca_revision(&self) {
        self.ca_revision
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

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
        let active_id = self
            .active_root_id
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let roots: Vec<CARoot> = self.roots.iter().map(|r| r.value().clone()).collect();
        CARootList {
            active_root_id: active_id,
            trust_domain: self.trust_domain.clone(),
            roots,
        }
    }

    /// The `get_ca_config` method.
    pub async fn get_ca_config(&self) -> CAConfig {
        self.ca_config
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
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
        *self.ca_config.write().unwrap_or_else(|e| e.into_inner()) = config;
        self.bump_ca_revision();
        Ok(())
    }

    /// Parse a Consul-style duration string (e.g. "72h", "30m", "1h30m").
    /// Returns `None` when the string has no recognised time component.
    fn parse_duration(value: &str) -> Option<batata_common::datetime::Duration> {
        let bytes = value.as_bytes();
        let mut total_seconds: i64 = 0;
        let mut i = 0;
        let mut found = false;

        while i < bytes.len() {
            if bytes[i].is_ascii_whitespace() {
                i += 1;
                continue;
            }
            let number_start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            if number_start == i {
                return None;
            }
            let amount: i64 = value[number_start..i].parse().ok()?;
            if i >= bytes.len() {
                return None;
            }
            let multiplier = match bytes[i] {
                b's' | b'S' => 1,
                b'm' | b'M' => 60,
                b'h' | b'H' => 3600,
                b'd' | b'D' => 86400,
                _ => return None,
            };
            total_seconds = total_seconds.checked_add(amount.checked_mul(multiplier)?)?;
            found = true;
            i += 1;
        }

        found.then(|| batata_common::datetime::Duration::seconds(total_seconds))
    }

    /// Effective leaf lifetime from the CA configuration.
    fn leaf_cert_ttl(&self) -> batata_common::datetime::Duration {
        let config = self.ca_config.read().unwrap_or_else(|e| e.into_inner());
        config
            .config
            .get("LeafCertTTL")
            .and_then(|v| v.as_str())
            .and_then(Self::parse_duration)
            .unwrap_or(DEFAULT_LEAF_CERT_TTL)
    }

    /// Return the cached leaf while it is valid and signed by the current CA
    /// key; otherwise generate a fresh leaf and refresh the cache.
    ///
    /// Kept synchronous because existing SDS/xDS handlers call it directly.
    pub fn get_leaf_cert(&self, service: &str) -> LeafCert {
        let ca_key_id = Self::ca_key_id(&self.read_ca_key_pem());
        let now = Utc::now();

        if let Some(cached) = self.leaf_cache.get(service)
            && cached.ca_key_id == ca_key_id
            && now < cached.soft_expiry
        {
            return cached.leaf.clone();
        }

        let service_uri = format!(
            "spiffe://{}/ns/default/dc/{}/svc/{}",
            self.trust_domain, self.datacenter, service
        );
        let ttl = self.leaf_cert_ttl();
        let (cert_pem, private_key_pem) = self.generate_leaf_cert(service, &service_uri);
        let index = self.index.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let leaf = LeafCert {
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
            service_uri,
            valid_after: now.to_rfc3339(),
            valid_before: (now + ttl).to_rfc3339(),
            create_index: index,
            modify_index: index,
        };
        let soft_expiry = now
            + batata_common::datetime::Duration::milliseconds(
                (ttl.whole_milliseconds() as f64 * LEAF_SOFT_EXPIRY_RATIO) as i64,
            );
        self.leaf_cache.insert(
            service.to_string(),
            CachedLeaf {
                leaf: leaf.clone(),
                soft_expiry,
                ca_key_id,
            },
        );
        self.bump_ca_revision();
        leaf
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

    /// Generate a leaf certificate signed by the CA for a given service.
    /// The SPIFFE ID is embedded as a URI SAN so proxies can verify identity.
    fn generate_leaf_cert(&self, service: &str, service_uri: &str) -> (String, String) {
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
        leaf_params.subject_alt_names.push(rcgen::SanType::URI(
            service_uri
                .to_string()
                .try_into()
                .expect("SPIFFE ID must be a valid IA5 URI SAN"),
        ));

        // Reconstruct CA params and issuer from stored key for signing
        let ca_key_pem = self.read_ca_key_pem();
        let ca_key = KeyPair::from_pem(&ca_key_pem).expect("Failed to parse CA key");
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

    /// Build the updated intention under the DashMap shard guard.
    ///
    /// This must stay a **synchronous** helper returning an owned value: the
    /// shard guard is scoped to this function and therefore can never be alive
    /// at an `.await` in `update_intention`. Holding it across
    /// `raft.write(...).await` deadlocks the single-threaded actix runtime --
    /// the Raft FSM apply hook runs on the same `main` thread and calls
    /// `refresh_from_rocks()` -> `intentions.clear()`, which needs every shard
    /// write lock. `peering.rs` already drops its guard explicitly for the
    /// same reason; see `docs/raft-timing.md` §6.2 for the measured outage.
    fn build_updated_intention(&self, id: &str, req: IntentionRequest) -> Option<Intention> {
        let mut entry = self.intentions.get_mut(id)?;
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
        Some(entry.clone())
    }

    /// The `update_intention` method.
    pub async fn update_intention(&self, id: &str, req: IntentionRequest) -> Option<Intention> {
        // No DashMap guard is alive at any `.await` below: the mutation happens
        // in a synchronous helper that returns an owned `Intention`.
        let updated = self.build_updated_intention(id, req)?;
        let id = id.to_string();
        if let Some(ref raft) = self.raft_node {
            let intention_json = serde_json::to_string(&updated).unwrap_or_default();
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
            self.persist_intention_to_rocks(&id, &updated);
        }
        Some(updated)
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

    /// Delete every legacy intention targeting one destination.
    ///
    /// `service-intentions` config entries are authoritative for their
    /// destination in upstream Consul, so applying one replaces the legacy
    /// intention rows for that destination instead of merging with them.
    pub async fn delete_intentions_for_destination(
        &self,
        destination_ns: &str,
        destination_name: &str,
    ) -> usize {
        let ids: Vec<String> = self
            .intentions
            .iter()
            .filter(|r| {
                let intention = r.value();
                intention.destination_ns == destination_ns
                    && intention.destination_name == destination_name
            })
            .map(|r| r.key().clone())
            .collect();

        let mut deleted = 0;
        for id in ids {
            if self.delete_intention(&id).await {
                deleted += 1;
            }
        }
        deleted
    }

    /// Replace all legacy intentions for one destination with the converted
    /// `service-intentions` config entry sources.
    ///
    /// Returns the number of intentions written. Existing rows for the same
    /// destination are removed first, including rows created through the
    /// legacy intention API, because the config entry owns that destination.
    pub async fn replace_intentions_for_destination(
        &self,
        destination_ns: &str,
        destination_name: &str,
        requests: Vec<IntentionRequest>,
    ) -> usize {
        self.delete_intentions_for_destination(destination_ns, destination_name)
            .await;

        let mut written = 0;
        for req in requests {
            self.upsert_intention_exact(req).await;
            written += 1;
        }
        written
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

    /// Monotonic revision of CA material. Bumped on root/key/config changes,
    /// fresh leaf generation and follower refreshes so the xDS sync bridge can
    /// rebuild SDS resources when CA state changes.
    pub fn ca_revision(&self) -> u64 {
        self.ca_revision.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Assemble the complete SDS material for a service from the leaf cache
    /// and the CA trust bundle. Returns `None` only when no CA material exists.
    pub fn get_connect_material_sync(&self, service: &str) -> Option<ConnectMaterial> {
        if self.roots.is_empty() || self.read_ca_key_pem().is_empty() {
            return None;
        }

        let leaf = self.get_leaf_cert(service);
        let active_id = self
            .active_root_id
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let mut roots: Vec<CARoot> = self.roots.iter().map(|r| r.value().clone()).collect();
        // Active root first, followed by the rest of the trust bundle.
        roots.sort_by(|a, b| {
            (b.id == active_id)
                .cmp(&(a.id == active_id))
                .then_with(|| a.id.cmp(&b.id))
        });

        let mut root_pem = String::new();
        for root in &roots {
            if root.root_cert.is_empty() {
                continue;
            }
            root_pem.push_str(&root.root_cert);
            if !root_pem.ends_with('\n') {
                root_pem.push('\n');
            }
        }
        if root_pem.is_empty() {
            return None;
        }

        Some(ConnectMaterial {
            leaf_cert_pem: leaf.cert_pem,
            private_key_pem: leaf.private_key_pem,
            root_pem,
            service_uri: leaf.service_uri,
            valid_before: leaf.valid_before,
        })
    }

    /// Re-read CA roots, config, active root, private key and intentions from
    /// RocksDB, replacing the in-memory state on a follower. Missing column
    /// families and corrupt records are skipped rather than causing a panic.
    pub fn refresh_from_rocks(&self) {
        let Some(ref db) = self.rocks_db else {
            return;
        };

        let new_roots = DashMap::new();
        let new_intentions = DashMap::new();
        let mut loaded_active_root_id: Option<String> = None;
        let mut loaded_ca_config: Option<CAConfig> = None;
        let mut loaded_ca_key_pem: Option<String> = None;
        let mut saw_ca_records = false;
        let mut saw_intention_records = false;
        let mut max_index = 0u64;

        if let Some(cf) = db.cf_handle(CF_CONSUL_CA_ROOTS) {
            saw_ca_records = true;
            for item in db.iterator_cf(cf, rocksdb::IteratorMode::Start).flatten() {
                let (key_bytes, value_bytes) = item;
                let Ok(key) = String::from_utf8(key_bytes.to_vec()) else {
                    continue;
                };
                if key == ROCKS_KEY_CA_CONFIG {
                    match serde_json::from_slice::<CAConfig>(&value_bytes) {
                        Ok(config) => {
                            max_index = max_index.max(config.modify_index);
                            loaded_ca_config = Some(config);
                        }
                        Err(e) => error!("Failed to deserialize CA config from RocksDB: {}", e),
                    }
                } else if key == ROCKS_KEY_ACTIVE_ROOT {
                    if let Ok(id) = String::from_utf8(value_bytes.to_vec()) {
                        loaded_active_root_id = Some(id);
                    }
                } else if key == ROCKS_KEY_CA_KEY {
                    if let Ok(pem) = String::from_utf8(value_bytes.to_vec()) {
                        loaded_ca_key_pem = Some(pem);
                    }
                } else {
                    match serde_json::from_slice::<CARoot>(&value_bytes) {
                        Ok(root) => {
                            max_index = max_index.max(root.modify_index);
                            new_roots.insert(key, root);
                        }
                        Err(e) => {
                            error!(
                                "Failed to deserialize CA root '{}' from RocksDB: {}",
                                key, e
                            )
                        }
                    }
                }
            }
        }

        if let Some(cf) = db.cf_handle(CF_CONSUL_INTENTIONS) {
            saw_intention_records = true;
            for item in db.iterator_cf(cf, rocksdb::IteratorMode::Start).flatten() {
                let (key_bytes, value_bytes) = item;
                let Ok(key) = String::from_utf8(key_bytes.to_vec()) else {
                    continue;
                };
                match serde_json::from_slice::<Intention>(&value_bytes) {
                    Ok(intention) => {
                        max_index = max_index.max(intention.modify_index);
                        new_intentions.insert(key, intention);
                    }
                    Err(e) => {
                        error!(
                            "Failed to deserialize intention '{}' from RocksDB: {}",
                            key, e
                        )
                    }
                }
            }
        }

        // Only replace in-memory CA state when the DB actually supplied some;
        // an empty/corrupt DB must not drop the current signing material.
        if saw_ca_records
            && (!new_roots.is_empty() || loaded_ca_key_pem.is_some() || loaded_ca_config.is_some())
        {
            let active_id = loaded_active_root_id
                .filter(|id| new_roots.contains_key(id))
                .or_else(|| {
                    new_roots
                        .iter()
                        .find(|r| r.value().active)
                        .map(|r| r.key().clone())
                })
                .unwrap_or_default();
            if let Some(root) = new_roots.get(&active_id) {
                *self.ca_cert_pem.write().unwrap_or_else(|e| e.into_inner()) =
                    root.value().root_cert.clone();
            }
            if let Some(key_pem) = loaded_ca_key_pem {
                *self.ca_key_pem.write().unwrap_or_else(|e| e.into_inner()) = key_pem;
            }
            if let Some(config) = loaded_ca_config {
                *self.ca_config.write().unwrap_or_else(|e| e.into_inner()) = config;
            }
            *self
                .active_root_id
                .write()
                .unwrap_or_else(|e| e.into_inner()) = active_id;
            self.roots.clear();
            for (id, root) in new_roots.into_iter() {
                self.roots.insert(id, root);
            }
            self.index.fetch_max(
                max_index.saturating_add(1),
                std::sync::atomic::Ordering::SeqCst,
            );
        }

        if saw_intention_records {
            self.intentions.clear();
            for (id, intention) in new_intentions.into_iter() {
                self.intentions.insert(id, intention);
            }
        }

        // Leaves signed by a previous CA key must not be served after refresh.
        self.leaf_cache.clear();
        self.bump_ca_revision();
    }

    /// Monotonic revision incremented on every intention create/update/delete.
    /// The xDS sync bridge uses this to rebuild RBAC filters when intentions
    /// change (naming-store revision alone would miss intention-only changes).
    pub fn intentions_revision(&self) -> u64 {
        self.index.load(std::sync::atomic::Ordering::SeqCst)
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

/// Cached compiled L7 path regexes, keyed by pattern string.
///
/// `evaluate_http_permission` runs on every authorized L7 request; recompiling
/// the same pattern each time is wasteful. Patterns are validated at intention
/// creation time (see `validate_intention_permissions`) and are immutable once
/// stored, so a process-wide cache is safe.
static PATH_REGEX_CACHE: std::sync::OnceLock<DashMap<String, regex::Regex>> =
    std::sync::OnceLock::new();

/// Return a compiled regex for `pattern`, reusing a cached one when possible.
/// Invalid patterns yield `Err(())`, preserving the caller's deny behaviour
/// (invalid regexes are rejected at creation time).
fn cached_path_regex(pattern: &str) -> Result<regex::Regex, ()> {
    let cache = PATH_REGEX_CACHE.get_or_init(DashMap::new);
    if let Some(hit) = cache.get(pattern) {
        return Ok(hit.value().clone());
    }
    let compiled = regex::Regex::new(pattern).map_err(|_| ())?;
    cache.insert(pattern.to_string(), compiled.clone());
    Ok(compiled)
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
        match cached_path_regex(regex) {
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
pub(crate) fn validate_intention_permissions(
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
            if !VALID_METHODS.iter().any(|m| m.eq_ignore_ascii_case(method)) {
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
            .wait_for_change(ConsulTable::ConnectCA, target_index, timeout)
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
    let service = path.into_inner();
    // Leaf certificates contain private key material, so require service:write.
    let authz = acl_service.authorize_request(&req, ResourceType::Service, &service, true);
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
            .wait_for_change(ConsulTable::ConnectCA, target_index, timeout)
            .await;
    }

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
    let Some(data) = agent
        .naming_store()
        .get_by_service_id(&namespace, &service_id)
    else {
        return HttpResponse::NotFound().consul_error(ConsulError::new(format!(
            "Service not found: {}",
            service_id
        )));
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
            .wait_for_change(ConsulTable::ConnectCA, target_index, timeout)
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
    let service = path.into_inner();
    // Leaf certificates contain private key material, so require service:write.
    let authz = acl_service.authorize_request(&req, ResourceType::Service, &service, true);
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
            .wait_for_change(ConsulTable::ConnectCA, target_index, timeout)
            .await;
    }

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

    #[test]
    fn test_cached_path_regex_reuses_compiled_pattern() {
        let pattern = "^/cached-api/[0-9]+$";
        // Unique pattern + removal keeps this test independent of others that
        // may run in parallel against the shared static cache.
        PATH_REGEX_CACHE.get_or_init(DashMap::new).remove(pattern);

        // Success path: a valid pattern compiles and matches as expected.
        let re = cached_path_regex(pattern).expect("valid regex must compile");
        assert!(re.is_match("/cached-api/42"));
        assert!(!re.is_match("/other/42"));

        // Guard: the compiled regex must be cached for reuse. Removing the
        // cache insert in `cached_path_regex` makes this assertion fail.
        assert!(
            PATH_REGEX_CACHE.get().unwrap().contains_key(pattern),
            "compiled path regex must be cached"
        );
        // A second lookup returns the cached regex with identical behaviour.
        let re2 = cached_path_regex(pattern).expect("cached lookup");
        assert!(re2.is_match("/cached-api/7"));
    }

    #[test]
    fn test_cached_path_regex_rejects_invalid() {
        // Invalid patterns keep the deny behaviour (Err, never cached).
        assert!(cached_path_regex("([").is_err());
    }

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

    /// A DashMap shard guard must never be held across the Raft write await.
    /// `update_intention` runs on an actix arbiter while the FSM apply hook
    /// runs on the single-threaded `main` runtime, where `on_ca_change` ->
    /// `refresh_from_rocks()` -> `intentions.clear()` needs every shard lock.
    /// Holding one guard there froze the whole node for 3 x 7s in T6 (219
    /// AppendEntries RPC timeouts, 3 apply_wait_timeout events). The mutation
    /// is deliberately confined to the synchronous `build_updated_intention`
    /// helper so the compiler cannot let a guard survive into an `.await`.
    #[test]
    fn update_intention_has_no_shard_guard_across_await() {
        let src = include_str!("connect_ca.rs");
        let start = src
            .find("pub async fn update_intention(&self")
            .expect("update_intention must exist");
        let rest = &src[start..];
        let end = rest
            .find("    /// The `delete_intention` method.")
            .expect("the method following update_intention must exist");
        let body = &rest[..end];
        assert!(
            !body.contains("get_mut("),
            "update_intention must not hold a DashMap shard guard while awaiting the Raft write"
        );
        assert!(
            body.contains(".await"),
            "the Raft write must still be awaited"
        );
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

        let result = service.authorize(
            "api",
            "spiffe://consul/ns/default/dc/dc1/svc/malicious",
            None,
        );
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
        assert!(
            !result.allowed,
            "no matching permission should deny: {}",
            result.reason
        );
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

    // ========================================================================
    // Connect CA lifecycle tests
    // ========================================================================

    /// Open a temp RocksDB with the two CFs used by the Connect CA service.
    fn temp_ca_db() -> (Arc<DB>, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut db_opts = rocksdb::Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);
        let cf_opts = rocksdb::Options::default();
        let cfs = vec![
            rocksdb::ColumnFamilyDescriptor::new(CF_CONSUL_CA_ROOTS, cf_opts.clone()),
            rocksdb::ColumnFamilyDescriptor::new(CF_CONSUL_INTENTIONS, cf_opts),
        ];
        let db = DB::open_cf_descriptors(&db_opts, dir.path(), cfs).expect("open temp RocksDB");
        (Arc::new(db), dir)
    }

    #[test]
    fn test_ca_root_and_key_persist_across_with_rocks_instances() {
        let (db, _dir) = temp_ca_db();
        let first = ConsulConnectCAService::with_rocks(db.clone());
        let roots_first: Vec<CARoot> = first.roots.iter().map(|r| r.value().clone()).collect();
        assert_eq!(roots_first.len(), 1);
        let key_first = first.read_ca_key_pem();

        let cf = db.cf_handle(CF_CONSUL_CA_ROOTS).expect("CA roots CF");
        let persisted = db
            .get_cf(cf, ROCKS_KEY_CA_KEY.as_bytes())
            .expect("read CA key")
            .expect("CA key persisted");
        assert_eq!(persisted.as_slice(), key_first.as_bytes());

        // A second service must load the same root and the same key instead
        // of generating a new one (the bug this guards against).
        let second = ConsulConnectCAService::with_rocks(db.clone());
        let roots_second: Vec<CARoot> = second.roots.iter().map(|r| r.value().clone()).collect();
        assert_eq!(roots_second.len(), 1);
        assert_eq!(roots_second[0].id, roots_first[0].id);
        assert_eq!(roots_second[0].root_cert, roots_first[0].root_cert);
        assert_eq!(second.read_ca_key_pem(), key_first);
        assert!(second.ca_revision() >= 1);
    }

    #[test]
    fn test_with_rocks_regenerates_root_when_key_missing() {
        let (db, _dir) = temp_ca_db();
        let first = ConsulConnectCAService::with_rocks(db.clone());
        let old_root_id = first
            .active_root_id
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let old_key = first.read_ca_key_pem();

        let cf = db.cf_handle(CF_CONSUL_CA_ROOTS).expect("CA roots CF");
        db.delete_cf(cf, ROCKS_KEY_CA_KEY.as_bytes())
            .expect("delete CA key");

        let second = ConsulConnectCAService::with_rocks(db.clone());
        assert_ne!(
            second
                .active_root_id
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .clone(),
            old_root_id
        );
        assert_ne!(second.read_ca_key_pem(), old_key);
        assert!(
            db.get_cf(cf, ROCKS_KEY_CA_KEY.as_bytes())
                .expect("read CA key")
                .is_some(),
            "regenerated key must be persisted"
        );
        assert_eq!(second.ca_revision(), 2, "generation bumps CA revision");
    }

    #[test]
    fn test_with_rocks_regenerates_root_when_key_id_mismatches() {
        let (db, _dir) = temp_ca_db();
        let first = ConsulConnectCAService::with_rocks(db.clone());
        let old_root_id = first
            .active_root_id
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();

        // Replace the stored key with a different valid key while leaving the
        // recorded key id untouched; the loader must detect the mismatch.
        let (_other_cert, other_key) = ConsulConnectCAService::generate_ca_root_cert();
        let cf = db.cf_handle(CF_CONSUL_CA_ROOTS).expect("CA roots CF");
        db.put_cf(cf, ROCKS_KEY_CA_KEY.as_bytes(), other_key.as_bytes())
            .expect("put mismatched key");

        let second = ConsulConnectCAService::with_rocks(db.clone());
        assert_ne!(
            second
                .active_root_id
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .clone(),
            old_root_id
        );
        assert_ne!(second.read_ca_key_pem(), other_key);
    }

    #[test]
    fn test_leaf_cert_contains_spiffe_uri_san() {
        use base64::{Engine as _, engine::general_purpose::STANDARD};

        let service = ConsulConnectCAService::new();
        let leaf = service.get_leaf_cert("web");
        let payload: String = leaf
            .cert_pem
            .lines()
            .filter(|line| !line.starts_with("-----"))
            .collect();
        let der = STANDARD.decode(payload).expect("leaf PEM must be base64");

        let uri = b"spiffe://consul/ns/default/dc/dc1/svc/web";
        assert!(
            der.windows(uri.len())
                .any(|window| window == uri.as_slice()),
            "leaf DER must carry the SPIFFE ID as a URI SAN"
        );
    }

    #[test]
    fn test_leaf_cache_soft_expiry_and_key_mismatch() {
        let service = ConsulConnectCAService::new();
        let first = service.get_leaf_cert("web");
        let revision = service.ca_revision();

        let cached = service.get_leaf_cert("web");
        assert_eq!(cached.cert_pem, first.cert_pem, "cache hit must reuse leaf");
        assert_eq!(
            service.ca_revision(),
            revision,
            "cache hit must not bump revision"
        );

        // Force the soft-expiry deadline into the past.
        service
            .leaf_cache
            .get_mut("web")
            .expect("cached leaf")
            .soft_expiry = Utc::now() - batata_common::datetime::Duration::seconds(1);
        let rotated = service.get_leaf_cert("web");
        assert_ne!(
            rotated.cert_pem, first.cert_pem,
            "expired leaf must be regenerated"
        );
        assert_eq!(service.ca_revision(), revision + 1);

        // A changed CA key invalidates the cached leaf immediately.
        service
            .leaf_cache
            .get_mut("web")
            .expect("cached leaf")
            .ca_key_id = "stale-key".into();
        let after_mismatch = service.get_leaf_cert("web");
        assert_ne!(
            after_mismatch.cert_pem, rotated.cert_pem,
            "leaf signed by a stale CA key must be regenerated"
        );
        assert_eq!(service.ca_revision(), revision + 2);
    }

    #[tokio::test]
    async fn test_get_connect_material_sync_root_chain() {
        let service = ConsulConnectCAService::new();
        let material = service
            .get_connect_material_sync("api")
            .expect("CA material must exist");

        assert_eq!(
            material.service_uri,
            "spiffe://consul/ns/default/dc/dc1/svc/api"
        );
        assert!(!material.leaf_cert_pem.is_empty());
        assert!(!material.private_key_pem.is_empty());
        assert!(!material.valid_before.is_empty());
        assert!(material.root_pem.ends_with('\n'));

        let roots = service.get_roots().await;
        for root in &roots.roots {
            assert!(
                material.root_pem.contains(root.root_cert.trim_end()),
                "root chain must contain every CA root"
            );
        }

        // No backing DB: refresh must be a harmless no-op.
        let revision = service.ca_revision();
        service.refresh_from_rocks();
        assert!(service.ca_revision() >= revision);
    }

    #[test]
    fn test_refresh_from_rocks_replaces_ca_material() {
        let (db, _dir) = temp_ca_db();
        let service = ConsulConnectCAService::with_rocks(db.clone());
        let old_key = service.read_ca_key_pem();

        // Simulate a leader rotating the CA key and root directly in RocksDB.
        let (new_cert_pem, new_key_pem) = ConsulConnectCAService::generate_ca_root_cert();
        let new_root_id = uuid::Uuid::new_v4().to_string();
        let new_root = CARoot {
            id: new_root_id.clone(),
            name: "Consul CA Root Cert".to_string(),
            root_cert: new_cert_pem,
            active: true,
            create_index: 10,
            modify_index: 10,
        };
        let mut new_config = service
            .ca_config
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        new_config.state.insert(
            CA_KEY_ID_STATE_KEY.to_string(),
            ConsulConnectCAService::ca_key_id(&new_key_pem),
        );
        let cf = db.cf_handle(CF_CONSUL_CA_ROOTS).expect("CA roots CF");
        db.put_cf(
            cf,
            new_root_id.as_bytes(),
            serde_json::to_vec(&new_root).expect("serialize root"),
        )
        .expect("put root");
        db.put_cf(cf, ROCKS_KEY_ACTIVE_ROOT.as_bytes(), new_root_id.as_bytes())
            .expect("put active root");
        db.put_cf(cf, ROCKS_KEY_CA_KEY.as_bytes(), new_key_pem.as_bytes())
            .expect("put key");
        db.put_cf(
            cf,
            ROCKS_KEY_CA_CONFIG.as_bytes(),
            serde_json::to_vec(&new_config).expect("serialize config"),
        )
        .expect("put config");

        let cached_leaf = service.get_leaf_cert("web");
        let revision = service.ca_revision();
        service.refresh_from_rocks();

        // Refresh invalidates every cached leaf, even when the key is unchanged.
        assert!(service.leaf_cache.is_empty());
        // A rotated CA key invalidates leaves signed by the previous key.
        assert_ne!(service.get_leaf_cert("web").cert_pem, cached_leaf.cert_pem);
        assert_eq!(service.read_ca_key_pem(), new_key_pem);
        assert_ne!(service.read_ca_key_pem(), old_key);
        assert_eq!(
            service
                .active_root_id
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .clone(),
            new_root_id
        );
        assert!(service.ca_revision() > revision);
    }
}
