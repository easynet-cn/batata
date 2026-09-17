// Consul ACL (Access Control List) implementation
// Provides token-based authentication and authorization for Consul API endpoints
// Supports both in-memory storage and persistent storage via ConfigService

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::LazyLock;

use actix_web::{HttpRequest, HttpResponse, web};
use base64::Engine;
use moka::sync::Cache;
use rocksdb::DB;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, error, info, warn};

use crate::acl_store::AclStore;
use crate::consul_meta::{ConsulResponseMeta, consul_ok};
use crate::index_provider::{ConsulIndexProvider, ConsulTable};
use crate::model::{ConsulDatacenterConfig, ConsulError, ConsulErrorBody};
use crate::raft::{ConsulRaftRequest, ConsulRaftWriter};

// ACL Token header name
/// The `X_CONSUL_TOKEN` constant.
pub const X_CONSUL_TOKEN: &str = "X-Consul-Token";
/// The `CONSUL_TOKEN_QUERY` constant.
pub const CONSUL_TOKEN_QUERY: &str = "token";
/// Well-known accessor ID for the bootstrap management token
pub const BOOTSTRAP_ACCESSOR_ID: &str = "00000000-0000-0000-0000-000000000001";

// Cache for validated tokens (60-second TTL, cluster-safe default)
static TOKEN_CACHE: LazyLock<Cache<String, AclToken>> = LazyLock::new(|| {
    Cache::builder()
        .time_to_live(Duration::from_secs(60))
        .max_capacity(10_000)
        .build()
});

// Cache for parsed policy rules (avoids re-parsing rule strings on every authorize() call)
// Key: policy_id, Value: parsed rules. Invalidated by TTL when policy is updated.
static PARSED_RULES_CACHE: LazyLock<Cache<String, ParsedRules>> = LazyLock::new(|| {
    Cache::builder()
        .time_to_live(Duration::from_secs(60))
        .max_capacity(1_000)
        .build()
});

// Cache for policy lookups (avoids double/triple RocksDB reads on name-based lookups)
static POLICY_CACHE: LazyLock<Cache<String, AclPolicy>> = LazyLock::new(|| {
    Cache::builder()
        .time_to_live(Duration::from_secs(30))
        .max_capacity(1_000)
        .build()
});

/// Clear every ACL cache.
///
/// Called by the Consul Raft apply hook whenever a committed ACL write
/// (token/policy/role/auth-method/binding-rule set or delete) lands on
/// the local node, so followers see policy changes immediately instead
/// of waiting up to 60 s for the moka TTL to expire.
///
/// This is a blunt invalidation — it clears all entries in all three
/// caches rather than trying to identify which specific token/policy
/// changed. The cost is one cold-read per active token/policy after the
/// apply, which is cheap compared to the correctness hazard of stale
/// authorization decisions. Consul itself also rebuilds the full cache
/// on ACL change notifications.
pub fn invalidate_all_caches() {
    TOKEN_CACHE.invalidate_all();
    PARSED_RULES_CACHE.invalidate_all();
    POLICY_CACHE.invalidate_all();
    tracing::debug!("ACL caches invalidated (token + parsed_rules + policy)");
}

/// Expanded ACL token with resolved policies and roles
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AclTokenExpanded {
/// The `expanded_policies` field.
    pub expanded_policies: Vec<AclPolicy>,
/// The `expanded_roles` field.
    pub expanded_roles: Vec<AclRole>,
    #[serde(flatten)]
/// The `token` field.
    pub token: AclToken,
}

/// ACL Token structure
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AclToken {
/// The `accessor_id` field.
    pub accessor_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `secret_id` field.
    pub secret_id: Option<String>,
/// The `description` field.
    pub description: String,
/// The `policies` field.
    pub policies: Vec<PolicyLink>,
/// The `roles` field.
    pub roles: Vec<RoleLink>,
/// The `local` field.
    pub local: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `expiration_time` field.
    pub expiration_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "ExpirationTTL")]
/// The `expiration_ttl` field.
    pub expiration_ttl: Option<u64>,
/// The `create_time` field.
    pub create_time: String,
/// The `modify_time` field.
    pub modify_time: String,
}

/// Policy link in token
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PolicyLink {
    #[serde(rename = "ID", default)]
/// The `id` field.
    pub id: String,
    #[serde(default)]
/// The `name` field.
    pub name: String,
}

/// Role link in token
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct RoleLink {
    #[serde(rename = "ID", default)]
/// The `id` field.
    pub id: String,
    #[serde(default)]
/// The `name` field.
    pub name: String,
}

/// ACL Policy structure
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AclPolicy {
    #[serde(rename = "ID")]
/// The `id` field.
    pub id: String,
/// The `name` field.
    pub name: String,
/// The `description` field.
    pub description: String,
/// The `rules` field.
    pub rules: String,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `datacenters` field.
    pub datacenters: Option<Vec<String>>,
/// The `create_time` field.
    pub create_time: String,
/// The `modify_time` field.
    pub modify_time: String,
}

/// ACL Role structure
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AclRole {
    #[serde(rename = "ID")]
/// The `id` field.
    pub id: String,
/// The `name` field.
    pub name: String,
/// The `description` field.
    pub description: String,
/// The `policies` field.
    pub policies: Vec<PolicyLink>,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `service_identities` field.
    pub service_identities: Option<Vec<ServiceIdentity>>,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `node_identities` field.
    pub node_identities: Option<Vec<NodeIdentity>>,
/// The `create_time` field.
    pub create_time: String,
/// The `modify_time` field.
    pub modify_time: String,
}

/// Service identity for role
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ServiceIdentity {
/// The `service_name` field.
    pub service_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `datacenters` field.
    pub datacenters: Option<Vec<String>>,
}

/// Node identity for role
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct NodeIdentity {
/// The `node_name` field.
    pub node_name: String,
/// The `datacenter` field.
    pub datacenter: String,
}

/// ACL Auth Method structure
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AuthMethod {
/// The `name` field.
    pub name: String,
    #[serde(rename = "Type")]
/// The `method_type` field.
    pub method_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `display_name` field.
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `description` field.
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `max_token_ttl` field.
    pub max_token_ttl: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `token_locality` field.
    pub token_locality: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `config` field.
    pub config: Option<HashMap<String, serde_json::Value>>,
/// The `create_index` field.
    pub create_index: u64,
/// The `modify_index` field.
    pub modify_index: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `namespace` field.
    pub namespace: Option<String>,
}

/// Parsed ACL rules for authorization checks
#[derive(Clone, Debug, Default)]
pub struct ParsedRules {
/// The `agent_rules` field.
    pub agent_rules: Vec<ResourceRule>,
/// The `key_rules` field.
    pub key_rules: Vec<ResourceRule>,
/// The `node_rules` field.
    pub node_rules: Vec<ResourceRule>,
/// The `service_rules` field.
    pub service_rules: Vec<ResourceRule>,
/// The `session_rules` field.
    pub session_rules: Vec<ResourceRule>,
/// The `query_rules` field.
    pub query_rules: Vec<ResourceRule>,
}

/// Single resource rule
#[derive(Clone, Debug)]
pub struct ResourceRule {
/// The `prefix` field.
    pub prefix: String,
/// The `policy` field.
    pub policy: RulePolicy,
}

/// Rule policy type
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RulePolicy {
/// The `Read` variant.
    Read,
/// The `Write` variant.
    Write,
/// The `Deny` variant.
    Deny,
}

impl std::str::FromStr for RulePolicy {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "read" => RulePolicy::Read,
            "write" => RulePolicy::Write,
            "deny" => RulePolicy::Deny,
            _ => RulePolicy::Deny,
        })
    }
}

impl RulePolicy {
/// The `allows_read` method.
    pub fn allows_read(&self) -> bool {
        matches!(self, RulePolicy::Read | RulePolicy::Write)
    }

/// The `allows_write` method.
    pub fn allows_write(&self) -> bool {
        matches!(self, RulePolicy::Write)
    }
}

/// Resource types for authorization
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResourceType {
/// The `Agent` variant.
    Agent,
/// The `Key` variant.
    Key,
/// The `Keyring` variant.
    Keyring,
/// The `Node` variant.
    Node,
/// The `Operator` variant.
    Operator,
/// The `Service` variant.
    Service,
/// The `Session` variant.
    Session,
/// The `Query` variant.
    Query,
}

/// ACL authorization result
#[derive(Clone, Debug)]
pub struct AuthzResult {
/// The `allowed` field.
    pub allowed: bool,
/// The `reason` field.
    pub reason: String,
}

impl AuthzResult {
/// The `allowed` associated function.
    pub fn allowed() -> Self {
        Self {
            allowed: true,
            reason: String::new(),
        }
    }

/// The `denied` associated function.
    pub fn denied(reason: &str) -> Self {
        Self {
            allowed: false,
            reason: reason.to_string(),
        }
    }
}

/// ACL Service for managing tokens and policies.
///
/// Storage is handled by `AclStore`:
/// - `AclStore::Memory`: instance-level DashMaps (no global state)
/// - `AclStore::Persistent`: RocksDB as single source of truth
#[derive(Clone)]
pub struct AclService {
    enabled: bool,
    default_policy: RulePolicy,
    /// Storage backend (Memory or Persistent/RocksDB)
    store: AclStore,
    /// Optional Raft writer for cluster-mode replication
    raft_node: Option<Arc<ConsulRaftWriter>>,
}

impl Default for AclService {
    fn default() -> Self {
        Self::new()
    }
}

impl AclService {
/// The `new` associated function.
    pub fn new() -> Self {
        let store = AclStore::memory();
        let mut svc = Self {
            enabled: true,
            default_policy: RulePolicy::Deny,
            store,
            raft_node: None,
        };
        svc.init_bootstrap(None);
        svc
    }

    /// Create an enabled ACL service with a pre-configured initial management token.
    /// Similar to Consul's `acl.tokens.initial_management` config.
    pub fn with_initial_management_token(token: String) -> Self {
        let store = AclStore::memory();
        let mut svc = Self {
            enabled: true,
            default_policy: RulePolicy::Deny,
            store,
            raft_node: None,
        };
        svc.init_bootstrap(Some(token));
        svc
    }

/// The `disabled` associated function.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            default_policy: RulePolicy::Write,
            store: AclStore::memory(),
            raft_node: None,
        }
    }

    /// Create an enabled ACL service with RocksDB as single source of truth.
    ///
    /// If the bootstrap token is not already in RocksDB, it is created and persisted.
    pub fn with_rocks(db: Arc<DB>) -> Self {
        Self::with_rocks_and_token(db, None)
    }

/// The `with_rocks_and_token` associated function.
    pub fn with_rocks_and_token(db: Arc<DB>, initial_management_token: Option<String>) -> Self {
        let store = AclStore::persistent(db);

        // Check if bootstrap token already exists in RocksDB
        let loaded_bootstrap = store.has_token_by_accessor(BOOTSTRAP_ACCESSOR_ID);

        let mut svc = Self {
            enabled: true,
            default_policy: RulePolicy::Deny,
            store,
            raft_node: None,
        };

        // Only create bootstrap token/policy if not already in RocksDB
        if !loaded_bootstrap {
            svc.init_bootstrap(initial_management_token);
        }

        let counts = svc.store_counts();
        info!(
            "ACL store initialized (RocksDB): {} tokens, {} policies, {} roles, {} auth methods, {} binding rules",
            counts.0, counts.1, counts.2, counts.3, counts.4
        );

        svc
    }

    /// Create an enabled ACL service with Raft-replicated storage (cluster mode).
    pub fn with_raft(db: Arc<DB>, raft_node: Arc<ConsulRaftWriter>) -> Self {
        let mut svc = Self::with_rocks(db);
        svc.raft_node = Some(raft_node);
        svc
    }

/// The `with_raft_and_token` associated function.
    pub fn with_raft_and_token(db: Arc<DB>, raft_node: Arc<ConsulRaftWriter>, initial_management_token: Option<String>) -> Self {
        let mut svc = Self::with_rocks_and_token(db, initial_management_token);
        svc.raft_node = Some(raft_node);
        svc
    }

    /// Count entities in the store (for logging at startup).
    fn store_counts(&self) -> (usize, usize, usize, usize, usize) {
        (
            self.store.list_tokens().len(),
            self.store.list_policies().len(),
            self.store.list_roles().len(),
            self.store.list_auth_methods().len(),
            self.store.list_binding_rules().len(),
        )
    }

    /// Get a reference to the underlying store (for callers that need direct access).
    pub fn store(&self) -> &AclStore {
        &self.store
    }

    fn init_bootstrap(&mut self, initial_management_token: Option<String>) {
        // Create global-management policy
        let mgmt_policy = AclPolicy {
            id: "00000000-0000-0000-0000-000000000001".to_string(),
            name: "global-management".to_string(),
            description: "Builtin global management policy".to_string(),
            rules: r#"
agent_prefix "" { policy = "write" }
key_prefix "" { policy = "write" }
node_prefix "" { policy = "write" }
service_prefix "" { policy = "write" }
session_prefix "" { policy = "write" }
query_prefix "" { policy = "write" }
"#
            .to_string(),
            datacenters: None,
            create_time: chrono::Utc::now().to_rfc3339(),
            modify_time: chrono::Utc::now().to_rfc3339(),
        };
        self.store.put_policy(&mgmt_policy);

        // Use configured initial management token or generate a random UUID
        let bootstrap_secret =
            initial_management_token.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        info!("ACL bootstrap token secret_id: {}", bootstrap_secret);
        let bootstrap_token = AclToken {
            accessor_id: BOOTSTRAP_ACCESSOR_ID.to_string(),
            secret_id: Some(bootstrap_secret.clone()),
            description: "Bootstrap Token (Management)".to_string(),
            policies: vec![PolicyLink {
                id: "00000000-0000-0000-0000-000000000001".to_string(),
                name: "global-management".to_string(),
            }],
            roles: vec![],
            local: false,
            expiration_time: None,
            expiration_ttl: None,
            create_time: chrono::Utc::now().to_rfc3339(),
            modify_time: chrono::Utc::now().to_rfc3339(),
        };
        self.store.put_token(&bootstrap_secret, &bootstrap_token);
    }

    /// Find the bootstrap token by its well-known accessor ID (O(1) via index).
    pub fn find_bootstrap_token(&self) -> Option<AclToken> {
        self.store.get_token_by_accessor(BOOTSTRAP_ACCESSOR_ID)
    }

    /// Check if the bootstrap token has already been created (O(1) via index).
    pub fn is_bootstrapped(&self) -> bool {
        self.store.has_token_by_accessor(BOOTSTRAP_ACCESSOR_ID)
    }

    /// Check if ACL is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Extract token from request.
    ///
    /// Follows the original Consul precedence order (`agent/http.go:parseToken`):
    /// 1. `X-Consul-Token` header
    /// 2. `Authorization: Bearer <token>` header
    /// 3. `?token=` query parameter
    pub fn extract_token(req: &HttpRequest) -> Option<String> {
        // 1. X-Consul-Token header (highest priority)
        if let Some(token) = req.headers().get(X_CONSUL_TOKEN)
            && let Ok(token_str) = token.to_str()
            && !token_str.is_empty()
        {
            return Some(token_str.to_string());
        }

        // 2. Authorization: Bearer <token>
        if let Some(auth) = req.headers().get("Authorization")
            && let Ok(auth_str) = auth.to_str()
            && let Some(token) = auth_str.strip_prefix("Bearer ")
        {
            let token = token.trim();
            if !token.is_empty() {
                return Some(token.to_string());
            }
        }

        // 3. ?token= query parameter
        let query = web::Query::<HashMap<String, String>>::from_query(req.query_string()).ok()?;
        query
            .get(CONSUL_TOKEN_QUERY)
            .filter(|t| !t.is_empty())
            .cloned()
    }

    /// Validate and get token
    pub fn get_token(&self, secret_id: &str) -> Option<AclToken> {
        // Check hot-path cache first
        if let Some(token) = TOKEN_CACHE.get(secret_id) {
            return Some(token);
        }

        // Read from store (Memory DashMap or RocksDB)
        let token = self.store.get_token(secret_id)?;
        TOKEN_CACHE.insert(secret_id.to_string(), token.clone());
        Some(token)
    }

    /// Create a new token
    pub async fn create_token(
        &self,
        description: &str,
        policies: Vec<String>,
        roles: Vec<String>,
        local: bool,
        expiration_ttl: Option<&str>,
    ) -> AclToken {
        let now = chrono::Utc::now();
        let now_str = now.to_rfc3339();
        let accessor_id = uuid::Uuid::new_v4().to_string();
        let secret_id = uuid::Uuid::new_v4().to_string();

        let policy_links: Vec<PolicyLink> = policies
            .iter()
            .filter_map(|p| {
                self.store.get_policy(p).map(|policy| PolicyLink {
                    id: policy.id.clone(),
                    name: policy.name.clone(),
                })
            })
            .collect();

        let role_links: Vec<RoleLink> = roles
            .iter()
            .filter_map(|r| {
                self.store.get_role(r).map(|role| RoleLink {
                    id: role.id.clone(),
                    name: role.name.clone(),
                })
            })
            .collect();

        // Parse expiration TTL (Go duration format: "1h", "30m", "24h", etc.)
        let (expiration_time, expiration_ttl_nanos) =
            if let Some(ttl_str) = expiration_ttl.filter(|s| !s.is_empty()) {
                if let Some(std_dur) = parse_duration(ttl_str) {
                    let chrono_dur = chrono::Duration::from_std(std_dur).unwrap_or_default();
                    let exp = now + chrono_dur;
                    (Some(exp.to_rfc3339()), Some(std_dur.as_nanos() as u64))
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };

        let token = AclToken {
            accessor_id,
            secret_id: Some(secret_id.clone()),
            description: description.to_string(),
            policies: policy_links,
            roles: role_links,
            local,
            expiration_time,
            expiration_ttl: expiration_ttl_nanos,
            create_time: now_str.clone(),
            modify_time: now_str,
        };

        self.store.put_token(&secret_id, &token);
        if let Some(ref raft) = self.raft_node {
            let token_json = serde_json::to_string(&token).unwrap_or_default();
            match raft
                .write(ConsulRaftRequest::ACLTokenSet {
                    accessor_id: token.accessor_id.clone(),
                    token_json,
                })
                .await
            {
                Ok(r) if !r.success => {
                    error!("Raft ACLTokenSet rejected: {:?}", r.message);
                }
                Err(e) => {
                    error!("Raft ACLTokenSet failed: {}", e);
                }
                _ => {}
            }
        }
        token
    }

    /// Delete a token
    pub async fn delete_token(&self, accessor_id: &str) -> bool {
        let removed = self
            .store
            .retain_tokens(|_, token| token.accessor_id != accessor_id);
        if removed.is_empty() {
            return false;
        }
        // Invalidate token cache for removed tokens
        for (secret_id, _) in &removed {
            TOKEN_CACHE.invalidate(secret_id);
        }
        if let Some(ref raft) = self.raft_node {
            match raft
                .write(ConsulRaftRequest::ACLTokenDelete {
                    accessor_id: accessor_id.to_string(),
                })
                .await
            {
                Ok(r) if !r.success => {
                    error!("Raft ACLTokenDelete rejected: {:?}", r.message);
                }
                Err(e) => {
                    error!("Raft ACLTokenDelete failed: {}", e);
                }
                _ => {}
            }
        }
        true
    }

    /// List all tokens
    pub fn list_tokens(&self) -> Vec<AclToken> {
        self.store
            .list_tokens()
            .into_iter()
            .map(|mut token| {
                token.secret_id = None; // Don't expose secret_id in list
                token
            })
            .collect()
    }

    /// Create a new ACL policy.
    ///
    /// Returns `Err` if a policy with the same name already exists
    /// (Consul enforces unique policy names).
    pub async fn create_policy(
        &self,
        name: &str,
        description: &str,
        rules: &str,
        datacenters: Option<Vec<String>>,
    ) -> Result<AclPolicy, String> {
        // Consul enforces unique policy names
        if self.store.policy_name_exists(name) {
            return Err(format!(
                "Invalid Policy: A Policy with Name \"{}\" already exists",
                name
            ));
        }

        let now = chrono::Utc::now().to_rfc3339();
        let policy_id = uuid::Uuid::new_v4().to_string();

        let policy = AclPolicy {
            id: policy_id,
            name: name.to_string(),
            description: description.to_string(),
            rules: rules.to_string(),
            datacenters,
            create_time: now.clone(),
            modify_time: now,
        };

        self.store.put_policy(&policy);
        if let Some(ref raft) = self.raft_node {
            let policy_json = serde_json::to_string(&policy).unwrap_or_default();
            match raft
                .write(ConsulRaftRequest::ACLPolicySet {
                    id: policy.id.clone(),
                    policy_json,
                })
                .await
            {
                Ok(r) if !r.success => {
                    error!("Raft ACLPolicySet rejected: {:?}", r.message);
                }
                Err(e) => {
                    error!("Raft ACLPolicySet failed: {}", e);
                }
                _ => {}
            }
        }
        Ok(policy)
    }

    /// Delete a policy by ID
    pub async fn delete_policy(&self, id: &str) -> bool {
        if let Some(policy) = self.store.remove_policy(id) {
            // Invalidate caches
            POLICY_CACHE.invalidate(id);
            POLICY_CACHE.invalidate(&policy.name);
            PARSED_RULES_CACHE.invalidate(id);

            if let Some(ref raft) = self.raft_node {
                match raft
                    .write(ConsulRaftRequest::ACLPolicyDelete { id: id.to_string() })
                    .await
                {
                    Ok(r) if !r.success => {
                        error!("Raft ACLPolicyDelete rejected: {:?}", r.message);
                    }
                    Err(e) => {
                        error!("Raft ACLPolicyDelete failed: {}", e);
                    }
                    _ => {}
                }
            }
            true
        } else {
            false
        }
    }

    /// Get a policy by ID or name (cached for hot-path authorization)
    pub fn get_policy(&self, id_or_name: &str) -> Option<AclPolicy> {
        if let Some(cached) = POLICY_CACHE.get(id_or_name) {
            return Some(cached);
        }
        let policy = self.store.get_policy(id_or_name)?;
        POLICY_CACHE.insert(id_or_name.to_string(), policy.clone());
        // Also cache by the other key (if looked up by name, cache by id too)
        if id_or_name != policy.id {
            POLICY_CACHE.insert(policy.id.clone(), policy.clone());
        }
        if id_or_name != policy.name {
            POLICY_CACHE.insert(policy.name.clone(), policy.clone());
        }
        Some(policy)
    }

    /// List all policies
    pub fn list_policies(&self) -> Vec<AclPolicy> {
        self.store.list_policies()
    }

    /// Create a new role
    pub async fn create_role(
        &self,
        name: &str,
        description: &str,
        policies: Vec<String>,
    ) -> AclRole {
        let now = chrono::Utc::now().to_rfc3339();
        let role_id = uuid::Uuid::new_v4().to_string();

        let policy_links: Vec<PolicyLink> = policies
            .iter()
            .filter_map(|p| {
                self.store.get_policy(p).map(|policy| PolicyLink {
                    id: policy.id.clone(),
                    name: policy.name.clone(),
                })
            })
            .collect();

        let role = AclRole {
            id: role_id,
            name: name.to_string(),
            description: description.to_string(),
            policies: policy_links,
            service_identities: None,
            node_identities: None,
            create_time: now.clone(),
            modify_time: now,
        };

        self.store.put_role(&role);
        if let Some(ref raft) = self.raft_node {
            let role_json = serde_json::to_string(&role).unwrap_or_default();
            match raft
                .write(ConsulRaftRequest::ACLRoleSet {
                    id: role.id.clone(),
                    role_json,
                })
                .await
            {
                Ok(r) if !r.success => {
                    error!("Raft ACLRoleSet rejected: {:?}", r.message);
                }
                Err(e) => {
                    error!("Raft ACLRoleSet failed: {}", e);
                }
                _ => {}
            }
        }
        role
    }

    /// Get a role by ID or name
    pub fn get_role(&self, id_or_name: &str) -> Option<AclRole> {
        self.store.get_role(id_or_name)
    }

    /// Update a role
    pub async fn update_role(
        &self,
        id: &str,
        name: Option<&str>,
        description: Option<&str>,
        policies: Option<Vec<String>>,
    ) -> Option<AclRole> {
        let mut role = self.get_role(id)?;
        let now = chrono::Utc::now().to_rfc3339();

        // Remove old name mapping if name is changing
        if let Some(new_name) = name
            && new_name != role.name
        {
            self.store.remove_role_name_index(&role.name);
            role.name = new_name.to_string();
        }

        if let Some(desc) = description {
            role.description = desc.to_string();
        }

        if let Some(policy_names) = policies {
            let policy_links: Vec<PolicyLink> = policy_names
                .iter()
                .filter_map(|p| {
                    self.store.get_policy(p).map(|policy| PolicyLink {
                        id: policy.id.clone(),
                        name: policy.name.clone(),
                    })
                })
                .collect();
            role.policies = policy_links;
        }

        role.modify_time = now;

        self.store.put_role(&role);
        if let Some(ref raft) = self.raft_node {
            let role_json = serde_json::to_string(&role).unwrap_or_default();
            match raft
                .write(ConsulRaftRequest::ACLRoleSet {
                    id: role.id.clone(),
                    role_json,
                })
                .await
            {
                Ok(r) if !r.success => {
                    error!("Raft ACLRoleSet rejected: {:?}", r.message);
                }
                Err(e) => {
                    error!("Raft ACLRoleSet failed: {}", e);
                }
                _ => {}
            }
        }
        Some(role)
    }

    /// Delete a role
    pub async fn delete_role(&self, id: &str) -> bool {
        if self.store.remove_role(id).is_some() {
            if let Some(ref raft) = self.raft_node {
                match raft
                    .write(ConsulRaftRequest::ACLRoleDelete { id: id.to_string() })
                    .await
                {
                    Ok(r) if !r.success => {
                        error!("Raft ACLRoleDelete rejected: {:?}", r.message);
                    }
                    Err(e) => {
                        error!("Raft ACLRoleDelete failed: {}", e);
                    }
                    _ => {}
                }
            }
            true
        } else {
            false
        }
    }

    /// List all roles
    pub fn list_roles(&self) -> Vec<AclRole> {
        self.store.list_roles()
    }

    /// Create a new auth method
    #[allow(clippy::too_many_arguments)]
    pub async fn create_auth_method(
        &self,
        name: &str,
        method_type: &str,
        display_name: Option<&str>,
        description: Option<&str>,
        max_token_ttl: Option<&str>,
        token_locality: Option<&str>,
        config: Option<HashMap<String, serde_json::Value>>,
    ) -> AuthMethod {
        use std::sync::atomic::{AtomicU64, Ordering};
        static AUTH_METHOD_INDEX: AtomicU64 = AtomicU64::new(1);

        let index = AUTH_METHOD_INDEX.fetch_add(1, Ordering::SeqCst);

        let method = AuthMethod {
            name: name.to_string(),
            method_type: method_type.to_string(),
            display_name: display_name.map(|s| s.to_string()),
            description: description.map(|s| s.to_string()),
            max_token_ttl: max_token_ttl.map(|s| s.to_string()),
            token_locality: token_locality.map(|s| s.to_string()),
            config,
            create_index: index,
            modify_index: index,
            namespace: None,
        };

        self.store.put_auth_method(&method);
        if let Some(ref raft) = self.raft_node {
            let method_json = serde_json::to_string(&method).unwrap_or_default();
            match raft
                .write(ConsulRaftRequest::ACLAuthMethodSet {
                    name: name.to_string(),
                    method_json,
                })
                .await
            {
                Ok(r) if !r.success => {
                    error!("Raft ACLAuthMethodSet rejected: {:?}", r.message);
                }
                Err(e) => {
                    error!("Raft ACLAuthMethodSet failed: {}", e);
                }
                _ => {}
            }
        }
        method
    }

    /// Get an auth method by name
    pub fn get_auth_method(&self, name: &str) -> Option<AuthMethod> {
        self.store.get_auth_method(name)
    }

    /// Update an auth method
    #[allow(clippy::too_many_arguments)]
    pub async fn update_auth_method(
        &self,
        name: &str,
        method_type: Option<&str>,
        display_name: Option<&str>,
        description: Option<&str>,
        max_token_ttl: Option<&str>,
        token_locality: Option<&str>,
        config: Option<HashMap<String, serde_json::Value>>,
    ) -> Option<AuthMethod> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static AUTH_METHOD_INDEX: AtomicU64 = AtomicU64::new(1);

        let mut method = self.store.get_auth_method(name)?;
        let index = AUTH_METHOD_INDEX.fetch_add(1, Ordering::SeqCst);

        if let Some(t) = method_type {
            method.method_type = t.to_string();
        }
        if let Some(dn) = display_name {
            method.display_name = Some(dn.to_string());
        }
        if let Some(d) = description {
            method.description = Some(d.to_string());
        }
        if let Some(ttl) = max_token_ttl {
            method.max_token_ttl = Some(ttl.to_string());
        }
        if let Some(loc) = token_locality {
            method.token_locality = Some(loc.to_string());
        }
        if config.is_some() {
            method.config = config;
        }
        method.modify_index = index;

        self.store.put_auth_method(&method);
        if let Some(ref raft) = self.raft_node {
            let method_json = serde_json::to_string(&method).unwrap_or_default();
            match raft
                .write(ConsulRaftRequest::ACLAuthMethodSet {
                    name: name.to_string(),
                    method_json,
                })
                .await
            {
                Ok(r) if !r.success => {
                    error!("Raft ACLAuthMethodSet rejected: {:?}", r.message);
                }
                Err(e) => {
                    error!("Raft ACLAuthMethodSet failed: {}", e);
                }
                _ => {}
            }
        }
        Some(method)
    }

    /// Delete an auth method
    pub async fn delete_auth_method(&self, name: &str) -> bool {
        let removed = self.store.remove_auth_method(name);
        if removed && let Some(ref raft) = self.raft_node {
            match raft
                .write(ConsulRaftRequest::ACLAuthMethodDelete {
                    name: name.to_string(),
                })
                .await
            {
                Ok(r) if !r.success => {
                    error!("Raft ACLAuthMethodDelete rejected: {:?}", r.message);
                }
                Err(e) => {
                    error!("Raft ACLAuthMethodDelete failed: {}", e);
                }
                _ => {}
            }
        }
        removed
    }

    /// List all auth methods
    pub fn list_auth_methods(&self) -> Vec<AuthMethod> {
        self.store.list_auth_methods()
    }

    /// Parse policy rules into structured format
    pub fn parse_rules(&self, rules: &str) -> ParsedRules {
        let mut parsed = ParsedRules::default();

        for line in rules.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Parse rules like: agent_prefix "" { policy = "write" }
            if let Some((resource_part, policy_part)) = line.split_once('{') {
                let resource_part = resource_part.trim();
                let policy_part = policy_part.trim().trim_end_matches('}').trim();

                // Extract prefix
                let (resource_type, prefix) =
                    if let Some(rest) = resource_part.strip_prefix("agent_prefix") {
                        ("agent", rest.trim().trim_matches('"'))
                    } else if let Some(rest) = resource_part.strip_prefix("key_prefix") {
                        ("key", rest.trim().trim_matches('"'))
                    } else if let Some(rest) = resource_part.strip_prefix("node_prefix") {
                        ("node", rest.trim().trim_matches('"'))
                    } else if let Some(rest) = resource_part.strip_prefix("service_prefix") {
                        ("service", rest.trim().trim_matches('"'))
                    } else if let Some(rest) = resource_part.strip_prefix("session_prefix") {
                        ("session", rest.trim().trim_matches('"'))
                    } else if let Some(rest) = resource_part.strip_prefix("query_prefix") {
                        ("query", rest.trim().trim_matches('"'))
                    } else {
                        continue;
                    };

                // Extract policy
                let policy = if let Some(policy_str) = policy_part.strip_prefix("policy") {
                    let policy_str = policy_str
                        .trim()
                        .trim_start_matches('=')
                        .trim()
                        .trim_matches('"');
                    policy_str.parse::<RulePolicy>().unwrap_or(RulePolicy::Deny)
                } else {
                    continue;
                };

                let rule = ResourceRule {
                    prefix: prefix.to_string(),
                    policy,
                };

                match resource_type {
                    "agent" => parsed.agent_rules.push(rule),
                    "key" => parsed.key_rules.push(rule),
                    "node" => parsed.node_rules.push(rule),
                    "service" => parsed.service_rules.push(rule),
                    "session" => parsed.session_rules.push(rule),
                    "query" => parsed.query_rules.push(rule),
                    _ => {}
                }
            }
        }

        parsed
    }

    /// Check authorization for a resource
    pub fn authorize(
        &self,
        token: &AclToken,
        resource_type: ResourceType,
        resource_name: &str,
        write: bool,
    ) -> AuthzResult {
        if !self.enabled {
            return AuthzResult::allowed();
        }

        // Get all rules from token's policies (cached to avoid re-parsing on every request)
        let mut all_rules = ParsedRules::default();
        for policy_link in &token.policies {
            if let Some(policy) = self.get_policy(&policy_link.id) {
                let parsed = if let Some(cached) = PARSED_RULES_CACHE.get(&policy.id) {
                    cached
                } else {
                    let fresh = self.parse_rules(&policy.rules);
                    PARSED_RULES_CACHE.insert(policy.id.clone(), fresh.clone());
                    fresh
                };
                all_rules.agent_rules.extend(parsed.agent_rules);
                all_rules.key_rules.extend(parsed.key_rules);
                all_rules.node_rules.extend(parsed.node_rules);
                all_rules.service_rules.extend(parsed.service_rules);
                all_rules.session_rules.extend(parsed.session_rules);
                all_rules.query_rules.extend(parsed.query_rules);
            }
        }

        // Select rules based on resource type
        let rules = match resource_type {
            ResourceType::Agent | ResourceType::Operator | ResourceType::Keyring => {
                &all_rules.agent_rules
            }
            ResourceType::Key => &all_rules.key_rules,
            ResourceType::Node => &all_rules.node_rules,
            ResourceType::Service => &all_rules.service_rules,
            ResourceType::Session => &all_rules.session_rules,
            ResourceType::Query => &all_rules.query_rules,
        };

        // Find the most specific matching rule
        let mut best_match: Option<&ResourceRule> = None;
        let mut best_match_len = 0;

        for rule in rules {
            if resource_name.starts_with(&rule.prefix) && rule.prefix.len() >= best_match_len {
                best_match = Some(rule);
                best_match_len = rule.prefix.len();
            }
        }

        // Check authorization
        if let Some(rule) = best_match {
            if write {
                if rule.policy.allows_write() {
                    AuthzResult::allowed()
                } else {
                    AuthzResult::denied("Permission denied: write access required")
                }
            } else if rule.policy.allows_read() {
                AuthzResult::allowed()
            } else {
                AuthzResult::denied("Permission denied: read access required")
            }
        } else {
            // No matching rule, use default policy
            if self.default_policy.allows_write() || (!write && self.default_policy.allows_read()) {
                AuthzResult::allowed()
            } else {
                AuthzResult::denied("Permission denied: no matching ACL rule")
            }
        }
    }

    /// Authorize from request
    pub fn authorize_request(
        &self,
        req: &HttpRequest,
        resource_type: ResourceType,
        resource_name: &str,
        write: bool,
    ) -> AuthzResult {
        if !self.enabled {
            return AuthzResult::allowed();
        }

        let secret_id = match Self::extract_token(req) {
            Some(t) => t,
            None => return AuthzResult::denied("ACL token required"),
        };

        let token = match self.get_token(&secret_id) {
            Some(t) => t,
            None => return AuthzResult::denied("ACL token not found or invalid"),
        };

        self.authorize(&token, resource_type, resource_name, write)
    }
}

// ============================================================================
// ACL Bootstrap Response
// ============================================================================

/// Bootstrap response with the initial management token
#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct BootstrapResponse {
    #[serde(rename = "ID")]
/// The `id` field.
    pub id: String,
/// The `accessor_id` field.
    pub accessor_id: String,
/// The `secret_id` field.
    pub secret_id: String,
/// The `description` field.
    pub description: String,
/// The `policies` field.
    pub policies: Vec<PolicyLink>,
/// The `local` field.
    pub local: bool,
/// The `create_time` field.
    pub create_time: String,
/// The `hash` field.
    pub hash: String,
}

// ============================================================================
// ACL Login Request/Response
// ============================================================================

/// Login request with auth method
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LoginRequest {
/// The `auth_method` field.
    pub auth_method: String,
/// The `bearer_token` field.
    pub bearer_token: Option<String>,
/// The `meta` field.
    pub meta: Option<HashMap<String, String>>,
}

/// Login response with token
#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct LoginResponse {
/// The `accessor_id` field.
    pub accessor_id: String,
/// The `secret_id` field.
    pub secret_id: String,
/// The `description` field.
    pub description: String,
/// The `policies` field.
    pub policies: Vec<PolicyLink>,
/// The `roles` field.
    pub roles: Vec<RoleLink>,
/// The `local` field.
    pub local: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `auth_method` field.
    pub auth_method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `expiration_time` field.
    pub expiration_time: Option<String>,
/// The `create_time` field.
    pub create_time: String,
}

// ============================================================================
// Token Clone Request
// ============================================================================

/// Clone token request
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CloneTokenRequest {
/// The `description` field.
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `namespace` field.
    pub namespace: Option<String>,
}

/// ACL error response
#[derive(Debug, Clone, Serialize)]
struct AclError {
    error: String,
}

impl AclError {
    fn new(msg: impl Into<String>) -> Self {
        Self { error: msg.into() }
    }
}

// ============================================================================
// ACL API Endpoints
// ============================================================================

/// GET /v1/acl/tokens
/// List all tokens
pub async fn list_tokens(
    acl_service: web::Data<AclService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let tokens = acl_service.list_tokens();
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));
    consul_ok(&meta).json(tokens)
}

/// GET /v1/acl/token/{accessor_id}
pub async fn get_token(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let accessor_id = path.into_inner();

    // Check if expanded=true is in query string
    let expanded = req
        .uri()
        .query()
        .map(|q| q.contains("expanded=true"))
        .unwrap_or(false);

    // Find token by accessor_id
    let token = acl_service
        .store()
        .find_token_by_accessor(&accessor_id)
        .map(|(_, t)| t);

    match token {
        Some(t) => {
            let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));
            if expanded {
                // Resolve policies and roles for expanded response
                let expanded_policies: Vec<AclPolicy> = t
                    .policies
                    .iter()
                    .filter_map(|pl| {
                        let key = if !pl.id.is_empty() { &pl.id } else { &pl.name };
                        acl_service.get_policy(key)
                    })
                    .collect();

                let expanded_roles: Vec<AclRole> = t
                    .roles
                    .iter()
                    .filter_map(|rl| {
                        let key = if !rl.id.is_empty() { &rl.id } else { &rl.name };
                        acl_service.get_role(key)
                    })
                    .collect();

                let response = AclTokenExpanded {
                    expanded_policies,
                    expanded_roles,
                    token: t,
                };
                consul_ok(&meta).json(response)
            } else {
                consul_ok(&meta).json(t)
            }
        }
        None => HttpResponse::NotFound().consul_error("ACL not found"),
    }
}

/// Token creation request
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CreateTokenRequest {
/// The `description` field.
    pub description: Option<String>,
/// The `policies` field.
    pub policies: Option<Vec<PolicyLink>>,
/// The `roles` field.
    pub roles: Option<Vec<RoleLink>>,
/// The `local` field.
    pub local: Option<bool>,
    #[serde(default, rename = "ExpirationTTL")]
/// The `expiration_ttl` field.
    pub expiration_ttl: Option<serde_json::Value>,
}

/// PUT /v1/acl/token
/// Create a new token
pub async fn create_token(
    acl_service: web::Data<AclService>,
    body: web::Json<CreateTokenRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    // Collect policy identifiers - prefer ID, fallback to name
    let policies: Vec<String> = body
        .policies
        .as_ref()
        .map(|p| {
            p.iter()
                .map(|pl| {
                    if !pl.id.is_empty() {
                        pl.id.clone()
                    } else {
                        pl.name.clone()
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    // Collect role identifiers - prefer ID, fallback to name
    let roles: Vec<String> = body
        .roles
        .as_ref()
        .map(|r| {
            r.iter()
                .map(|rl| {
                    if !rl.id.is_empty() {
                        rl.id.clone()
                    } else {
                        rl.name.clone()
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    let local = body.local.unwrap_or(false);

    // ExpirationTTL can be a number (nanoseconds from Go SDK) or string ("1h")
    let expiration_ttl_str: Option<String> = body.expiration_ttl.as_ref().and_then(|v| match v {
        serde_json::Value::Number(n) => {
            // Go's time.Duration serializes as nanoseconds
            n.as_u64().map(|ns| format!("{}s", ns / 1_000_000_000))
        }
        serde_json::Value::String(s) => Some(s.clone()),
        _ => None,
    });

    let token = acl_service
        .create_token(
            body.description.as_deref().unwrap_or(""),
            policies,
            roles,
            local,
            expiration_ttl_str.as_deref(),
        )
        .await;

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));
    consul_ok(&meta).json(token)
}

/// DELETE /v1/acl/token/{accessor_id}
pub async fn delete_token(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let accessor_id = path.into_inner();

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));
    if acl_service.delete_token(&accessor_id).await {
        consul_ok(&meta).json(true)
    } else {
        HttpResponse::NotFound().consul_error("Token not found")
    }
}

/// GET /v1/acl/token/self
/// Returns the token associated with the current request
pub async fn get_token_self(
    acl_service: web::Data<AclService>,
    req: HttpRequest,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let secret_id = match AclService::extract_token(&req) {
        Some(t) => t,
        None => return HttpResponse::Forbidden().consul_error("ACL token required"),
    };

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));
    match acl_service.get_token(&secret_id) {
        Some(token) => consul_ok(&meta).json(token),
        None => HttpResponse::Forbidden().consul_error("ACL token not found or invalid"),
    }
}

/// PUT /v1/acl/token/{accessor_id}/clone
/// Clone an existing token
pub async fn clone_token(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    body: web::Json<CloneTokenRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let accessor_id = path.into_inner();

    // Find the token to clone
    let source_token = acl_service
        .store()
        .find_token_by_accessor(&accessor_id)
        .map(|(_, t)| t);

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));
    match source_token {
        Some(source) => {
            // Create a new token with the same policies
            let policies: Vec<String> = source.policies.iter().map(|p| p.name.clone()).collect();
            let description = body
                .description
                .clone()
                .unwrap_or_else(|| format!("Clone of {}", source.description));
            let roles: Vec<String> = source.roles.iter().map(|r| r.name.clone()).collect();
            let new_token = acl_service
                .create_token(&description, policies, roles, source.local, None)
                .await;
            consul_ok(&meta).json(new_token)
        }
        None => HttpResponse::NotFound().consul_error("Token not found"),
    }
}

/// PUT /v1/acl/bootstrap
/// Bootstrap the ACL system (creates initial management token)
pub async fn acl_bootstrap(
    acl_service: web::Data<AclService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    // Check if already bootstrapped - return 403 error like Consul does
    if acl_service.is_bootstrapped() {
        return HttpResponse::Forbidden().json(AclError::new(
            "ACL bootstrap no longer allowed (reset index: 0)",
        ));
    }

    // NOTE: In a real implementation, init_bootstrap would need &mut self.
    // Since we're behind web::Data (Arc), we can't mutate here.
    // The bootstrap should have been initialized at startup.
    // For now, we just check if the token exists.

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));
    if let Some(token) = acl_service.find_bootstrap_token() {
        let response = BootstrapResponse {
            id: token.accessor_id.clone(),
            accessor_id: token.accessor_id,
            secret_id: token.secret_id.unwrap_or_default(),
            description: token.description,
            policies: token.policies,
            local: token.local,
            create_time: token.create_time,
            hash: base64::engine::general_purpose::STANDARD.encode("bootstrap"),
        };
        consul_ok(&meta).json(response)
    } else {
        HttpResponse::InternalServerError().consul_error("Failed to bootstrap ACL")
    }
}

/// POST /v1/acl/login
/// Login with an auth method to get a token
pub async fn acl_login(
    acl_service: web::Data<AclService>,
    body: web::Json<LoginRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    // Check if auth method exists
    let auth_method = match acl_service.get_auth_method(&body.auth_method) {
        Some(m) => m,
        None => {
            return HttpResponse::NotFound()
                .consul_error(format!("Auth method '{}' not found", body.auth_method));
        }
    };

    // For now, we implement a simple login that creates a token
    // In production, this would validate the bearer_token against the auth method's config
    let now = chrono::Utc::now().to_rfc3339();
    let accessor_id = uuid::Uuid::new_v4().to_string();
    let secret_id = uuid::Uuid::new_v4().to_string();

    // Calculate expiration based on max_token_ttl
    let expiration_time = auth_method.max_token_ttl.as_ref().and_then(|ttl| {
        // Parse TTL like "1h", "30m", "24h"
        parse_duration(ttl).map(|dur| {
            (chrono::Utc::now() + chrono::Duration::from_std(dur).unwrap_or_default()).to_rfc3339()
        })
    });

    let token = AclToken {
        accessor_id: accessor_id.clone(),
        secret_id: Some(secret_id.clone()),
        description: format!("Login token via {}", body.auth_method),
        policies: vec![], // No policies by default; binding rules would add them
        roles: vec![],
        local: auth_method.token_locality.as_deref() == Some("local"),
        expiration_time: expiration_time.clone(),
        expiration_ttl: None,
        create_time: now.clone(),
        modify_time: now.clone(),
    };

    acl_service.store().put_token(&secret_id, &token);

    let response = LoginResponse {
        accessor_id,
        secret_id,
        description: format!("Login token via {}", body.auth_method),
        policies: vec![],
        roles: vec![],
        local: auth_method.token_locality.as_deref() == Some("local"),
        auth_method: Some(body.auth_method.clone()),
        expiration_time,
        create_time: now,
    };

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));
    consul_ok(&meta).json(response)
}

/// POST /v1/acl/logout
/// Logout and invalidate the current token
pub async fn acl_logout(
    acl_service: web::Data<AclService>,
    req: HttpRequest,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let secret_id = match AclService::extract_token(&req) {
        Some(t) => t,
        None => return HttpResponse::Forbidden().consul_error("ACL token required"),
    };

    // Don't allow logging out the bootstrap token
    if let Some(token) = acl_service.get_token(&secret_id)
        && token.accessor_id == BOOTSTRAP_ACCESSOR_ID
    {
        return HttpResponse::Forbidden().consul_error("Cannot logout bootstrap token");
    }

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));

    // Find and delete the token
    if let Some(token) = acl_service.get_token(&secret_id)
        && acl_service.delete_token(&token.accessor_id).await
    {
        consul_ok(&meta).json(true)
    } else {
        HttpResponse::NotFound().consul_error("Token not found")
    }
}

/// Helper function to parse duration strings in Go format.
/// Supports simple formats like "1h", "30m", "24h" and compound formats like "1h0m0s", "2h30m15s".
fn parse_duration(s: &str) -> Option<std::time::Duration> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    // Try compound Go duration format: e.g. "1h0m0s", "2h30m", "45s", "1h30s"
    let mut total_secs: u64 = 0;
    let mut current_num = String::new();
    let mut matched_any = false;

    for ch in s.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            current_num.push(ch);
        } else {
            if current_num.is_empty() {
                return None;
            }
            let num: f64 = current_num.parse().ok()?;
            current_num.clear();
            matched_any = true;
            match ch {
                'h' | 'H' => total_secs += (num * 3600.0) as u64,
                'm' | 'M' => total_secs += (num * 60.0) as u64,
                's' | 'S' => total_secs += num as u64,
                _ => return None,
            }
        }
    }

    // If there is a trailing number with no unit, treat it as seconds
    if !current_num.is_empty() {
        let num: u64 = current_num.parse().ok()?;
        total_secs += num;
        matched_any = true;
    }

    if matched_any {
        Some(std::time::Duration::from_secs(total_secs))
    } else {
        None
    }
}

/// GET /v1/acl/policies
/// List all policies
pub async fn list_policies(
    acl_service: web::Data<AclService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let policies = acl_service.list_policies();
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));
    consul_ok(&meta).json(policies)
}

/// GET /v1/acl/policy/{id}
pub async fn get_policy(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let id = path.into_inner();

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));
    match acl_service.get_policy(&id) {
        Some(policy) => consul_ok(&meta).json(policy),
        None => HttpResponse::NotFound().consul_error("Policy not found"),
    }
}

/// DELETE /v1/acl/policy/{id}
pub async fn delete_policy(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let id = path.into_inner();

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));
    if acl_service.delete_policy(&id).await {
        consul_ok(&meta).json(true)
    } else {
        HttpResponse::NotFound().consul_error("Policy not found")
    }
}

/// Policy creation request
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CreatePolicyRequest {
/// The `name` field.
    pub name: String,
/// The `description` field.
    pub description: Option<String>,
/// The `rules` field.
    pub rules: String,
/// The `datacenters` field.
    pub datacenters: Option<Vec<String>>,
}

/// PUT /v1/acl/policy
/// Create a new policy
pub async fn create_policy(
    acl_service: web::Data<AclService>,
    body: web::Json<CreatePolicyRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    match acl_service
        .create_policy(
            &body.name,
            body.description.as_deref().unwrap_or(""),
            &body.rules,
            body.datacenters.clone(),
        )
        .await
    {
        Ok(policy) => {
            let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));
            consul_ok(&meta).json(policy)
        }
        Err(e) => HttpResponse::InternalServerError().consul_error(ConsulError::new(&e)),
    }
}

// ============================================================================
// ACL Role API Endpoints
// ============================================================================

/// Role creation/update request
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct RoleRequest {
/// The `name` field.
    pub name: String,
/// The `description` field.
    pub description: Option<String>,
/// The `policies` field.
    pub policies: Option<Vec<PolicyLink>>,
/// The `service_identities` field.
    pub service_identities: Option<Vec<ServiceIdentity>>,
/// The `node_identities` field.
    pub node_identities: Option<Vec<NodeIdentity>>,
}

/// GET /v1/acl/roles
/// List all roles
pub async fn list_roles(
    acl_service: web::Data<AclService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let roles = acl_service.list_roles();
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));
    consul_ok(&meta).json(roles)
}

/// PUT /v1/acl/role
/// Create a new role
pub async fn create_role(
    acl_service: web::Data<AclService>,
    body: web::Json<RoleRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let policies: Vec<String> = body
        .policies
        .as_ref()
        .map(|p| {
            p.iter()
                .map(|pl| {
                    if !pl.id.is_empty() {
                        pl.id.clone()
                    } else {
                        pl.name.clone()
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    let role = acl_service
        .create_role(
            &body.name,
            body.description.as_deref().unwrap_or(""),
            policies,
        )
        .await;

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));
    consul_ok(&meta).json(role)
}

/// GET /v1/acl/role/{id}
/// Get a role by ID or name
pub async fn get_role(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let id = path.into_inner();

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));
    match acl_service.get_role(&id) {
        Some(role) => consul_ok(&meta).json(role),
        None => HttpResponse::NotFound().consul_error("Role not found"),
    }
}

/// PUT /v1/acl/role/{id}
/// Update an existing role
pub async fn update_role(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    body: web::Json<RoleRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let id = path.into_inner();

    let policies: Option<Vec<String>> = body
        .policies
        .as_ref()
        .map(|p| p.iter().map(|pl| pl.name.clone()).collect());

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));
    match acl_service
        .update_role(&id, Some(&body.name), body.description.as_deref(), policies)
        .await
    {
        Some(role) => consul_ok(&meta).json(role),
        None => HttpResponse::NotFound().consul_error("Role not found"),
    }
}

/// DELETE /v1/acl/role/{id}
/// Delete a role
pub async fn delete_role(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let id = path.into_inner();

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));
    if acl_service.delete_role(&id).await {
        consul_ok(&meta).json(true)
    } else {
        HttpResponse::NotFound().consul_error("Role not found")
    }
}

// ============================================================================
// ACL Auth Method API Endpoints
// ============================================================================

/// Auth method creation/update request
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AuthMethodRequest {
/// The `name` field.
    pub name: String,
    #[serde(rename = "Type")]
/// The `method_type` field.
    pub method_type: String,
/// The `display_name` field.
    pub display_name: Option<String>,
/// The `description` field.
    pub description: Option<String>,
/// The `max_token_ttl` field.
    pub max_token_ttl: Option<String>,
/// The `token_locality` field.
    pub token_locality: Option<String>,
/// The `config` field.
    pub config: Option<HashMap<String, serde_json::Value>>,
}

/// GET /v1/acl/auth-methods
/// List all auth methods
pub async fn list_auth_methods(
    acl_service: web::Data<AclService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let methods = acl_service.list_auth_methods();
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));
    consul_ok(&meta).json(methods)
}

/// PUT /v1/acl/auth-method
/// Create a new auth method
pub async fn create_auth_method(
    acl_service: web::Data<AclService>,
    body: web::Json<AuthMethodRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let method = acl_service
        .create_auth_method(
            &body.name,
            &body.method_type,
            body.display_name.as_deref(),
            body.description.as_deref(),
            body.max_token_ttl.as_deref(),
            body.token_locality.as_deref(),
            body.config.clone(),
        )
        .await;

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));
    consul_ok(&meta).json(method)
}

/// GET /v1/acl/auth-method/{name}
/// Get an auth method by name
pub async fn get_auth_method(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let name = path.into_inner();

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));
    match acl_service.get_auth_method(&name) {
        Some(method) => consul_ok(&meta).json(method),
        None => HttpResponse::NotFound().consul_error("Auth method not found"),
    }
}

/// PUT /v1/acl/auth-method/{name}
/// Update an existing auth method
pub async fn update_auth_method(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    body: web::Json<AuthMethodRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let name = path.into_inner();

    match acl_service
        .update_auth_method(
            &name,
            Some(&body.method_type),
            body.display_name.as_deref(),
            body.description.as_deref(),
            body.max_token_ttl.as_deref(),
            body.token_locality.as_deref(),
            body.config.clone(),
        )
        .await
    {
        Some(method) => {
            let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));
            consul_ok(&meta).json(method)
        }
        None => HttpResponse::NotFound().consul_error("Auth method not found"),
    }
}

/// DELETE /v1/acl/auth-method/{name}
/// Delete an auth method
pub async fn delete_auth_method(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let name = path.into_inner();

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));
    if acl_service.delete_auth_method(&name).await {
        consul_ok(&meta).json(true)
    } else {
        HttpResponse::NotFound().consul_error("Auth method not found")
    }
}

/// ACL Binding Rule structure
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct BindingRule {
    #[serde(rename = "ID")]
/// The `id` field.
    pub id: String,
/// The `description` field.
    pub description: String,
/// The `auth_method` field.
    pub auth_method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `selector` field.
    pub selector: Option<String>,
/// The `bind_type` field.
    pub bind_type: String,
/// The `bind_name` field.
    pub bind_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `bind_vars` field.
    pub bind_vars: Option<HashMap<String, String>>,
/// The `create_index` field.
    pub create_index: u64,
/// The `modify_index` field.
    pub modify_index: u64,
}

/// Binding Rule create/update request
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct BindingRuleRequest {
    #[serde(default)]
/// The `description` field.
    pub description: Option<String>,
/// The `auth_method` field.
    pub auth_method: String,
    #[serde(default)]
/// The `selector` field.
    pub selector: Option<String>,
/// The `bind_type` field.
    pub bind_type: String,
/// The `bind_name` field.
    pub bind_name: String,
    #[serde(default)]
/// The `bind_vars` field.
    pub bind_vars: Option<HashMap<String, String>>,
}

/// ACL Replication status
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AclReplicationStatus {
/// The `enabled` field.
    pub enabled: bool,
/// The `running` field.
    pub running: bool,
/// The `source_datacenter` field.
    pub source_datacenter: String,
/// The `replication_type` field.
    pub replication_type: String,
/// The `replicated_index` field.
    pub replicated_index: u64,
/// The `replicated_role_index` field.
    pub replicated_role_index: u64,
/// The `replicated_token_index` field.
    pub replicated_token_index: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `last_success` field.
    pub last_success: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `last_error` field.
    pub last_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `last_error_message` field.
    pub last_error_message: Option<String>,
}

/// Templated policy response — matches Consul's `ACLTemplatedPolicyResponse`.
///
/// All fields are always serialized (even when empty) to match Consul
/// behavior, where `Schema`/`Template`/`Description` are plain `string`
/// fields without `omitempty`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TemplatedPolicy {
/// The `template_name` field.
    pub template_name: String,
/// The `schema` field.
    pub schema: String,
/// The `template` field.
    pub template: String,
/// The `description` field.
    pub description: String,
}

/// Templated policy variables — matches Consul's `ACLTemplatedPolicyVariables`.
///
/// Sent as the request body to the preview endpoint. `Name` is optional:
/// templates without a JSON schema (dns, nomad-server, nomad-client) do not
/// require it. The JSON field is lowercase `name` to match Consul's
/// `json:"name,omitempty"` tag.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TemplatedPolicyVariables {
    #[serde(rename = "name", skip_serializing_if = "Option::is_none")]
/// The `name` field.
    pub name: Option<String>,
}

/// Synthetic policy — matches the `ACLPolicy` returned by Consul's preview
/// endpoint. Only the fields populated by `SyntheticPolicy()` are included.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct SyntheticPolicy {
    #[serde(rename = "ID")]
/// The `id` field.
    pub id: String,
/// The `name` field.
    pub name: String,
/// The `description` field.
    pub description: String,
/// The `rules` field.
    pub rules: String,
}

// ============================================================================
// Built-in templated policy templates (byte-identical to Consul's .hcl files)
// ============================================================================

/// JSON schema for templated policies that require a `name` variable
/// (service, node, api-gateway). Byte-identical to Consul's embedded
/// `service.json` / `node.json` / `api-gateway.json` schema files.
const NAME_VARIABLE_SCHEMA: &str = "{\n\t\"type\": \"object\",\n\t\"properties\": {\n\t\t\"name\": { \"type\": \"string\", \"$ref\": \"#/definitions/min-length-one\" }\n\t},\n\t\"required\": [\"name\"],\n\t\"definitions\": {\n\t\t\"min-length-one\": {\n\t\t\t\t\"type\": \"string\",\n\t\t\t\t\"minLength\": 1\n\t\t}\n\t}\n}";

/// builtin/service template — byte-identical to Consul's `service.hcl`.
const SERVICE_TEMPLATE: &str = "\nservice \"{{.Name}}\" {\n\tpolicy = \"write\"\n}\nservice \"{{.Name}}-sidecar-proxy\" {\n\tpolicy = \"write\"\n}\nservice_prefix \"\" {\n\tpolicy = \"read\"\n}\nnode_prefix \"\" {\n\tpolicy = \"read\"\n}";
const SERVICE_DESCRIPTION: &str = "Gives the token or role permissions to register a service and discover services in the Consul catalog. It also gives the specified service's sidecar proxy the permission to discover and route traffic to other services.";

/// builtin/node template — byte-identical to Consul's `node.hcl`.
const NODE_TEMPLATE: &str = "\nnode \"{{.Name}}\" {\n\tpolicy = \"write\"\n}\nservice_prefix \"\" {\n\tpolicy = \"read\"\n}";
const NODE_DESCRIPTION: &str = "Gives the token or role permissions for a register an agent/node into the catalog. A node is typically a consul agent but can also be a physical server, cloud instance or a container.";

/// builtin/dns template — byte-identical to Consul's `dns.hcl`.
const DNS_TEMPLATE: &str = "\nnode_prefix \"\" {\n\tpolicy = \"read\"\n}\nservice_prefix \"\" {\n\tpolicy = \"read\"\n}\nquery_prefix \"\" {\n\tpolicy = \"read\"\n}";
const DNS_DESCRIPTION: &str = "Gives the token or role permissions for the Consul DNS to query services in the network.";

/// builtin/nomad-server template — byte-identical to Consul's `nomad-server.hcl`.
const NOMAD_SERVER_TEMPLATE: &str = "\nacl = \"write\"\nagent_prefix \"\" {\n  policy = \"read\"\n}\nnode_prefix \"\" {\n  policy = \"read\"\n}\nservice_prefix \"\" {\n  policy = \"write\"\n}";
const NOMAD_SERVER_DESCRIPTION: &str = "Gives the token or role permissions required for integration with a nomad server.";

/// builtin/api-gateway template — byte-identical to Consul's `api-gateway.hcl`.
const API_GATEWAY_TEMPLATE: &str = "mesh = \"read\"\nnode_prefix \"\" {\n\tpolicy = \"read\"\n}\nservice_prefix \"\" {\n\tpolicy = \"read\"\n}\nservice \"{{.Name}}\" {\n\tpolicy = \"write\"\n}";
const API_GATEWAY_DESCRIPTION: &str = "Gives the token or role permissions for a Consul api gateway";

/// builtin/nomad-client template — byte-identical to Consul's `nomad-client.hcl`.
const NOMAD_CLIENT_TEMPLATE: &str = "agent_prefix \"\" {\n  policy = \"read\"\n}\nnode_prefix \"\" {\n  policy = \"read\"\n}\nservice_prefix \"\" {\n  policy = \"write\"\n}\nkey_prefix \"\" {\n  policy = \"read\"\n}";
const NOMAD_CLIENT_DESCRIPTION: &str = "Gives the token or role permissions required for integration with a nomad client.";

/// Returns all 6 built-in templated policies as `(template_name, schema,
/// template, description)` tuples, matching Consul's
/// `aclTemplatedPoliciesList` map.
fn builtin_templates() -> Vec<(&'static str, &'static str, &'static str, &'static str)> {
    vec![
        (
            "builtin/service",
            NAME_VARIABLE_SCHEMA,
            SERVICE_TEMPLATE,
            SERVICE_DESCRIPTION,
        ),
        (
            "builtin/node",
            NAME_VARIABLE_SCHEMA,
            NODE_TEMPLATE,
            NODE_DESCRIPTION,
        ),
        ("builtin/dns", "", DNS_TEMPLATE, DNS_DESCRIPTION),
        (
            "builtin/nomad-server",
            "",
            NOMAD_SERVER_TEMPLATE,
            NOMAD_SERVER_DESCRIPTION,
        ),
        (
            "builtin/api-gateway",
            NAME_VARIABLE_SCHEMA,
            API_GATEWAY_TEMPLATE,
            API_GATEWAY_DESCRIPTION,
        ),
        (
            "builtin/nomad-client",
            "",
            NOMAD_CLIENT_TEMPLATE,
            NOMAD_CLIENT_DESCRIPTION,
        ),
    ]
}

/// Render a template by substituting `{{.Name}}` with the provided name.
///
/// Mirrors Consul's Go `text/template` execution: when no name is supplied
/// the template is returned verbatim (the `{{.Name}}` placeholder is left
/// untouched for schema-less templates that never reference it).
fn render_template(template: &str, name: Option<&str>) -> String {
    match name {
        Some(n) => template.replace("{{.Name}}", n),
        None => template.to_string(),
    }
}

/// Validate templated policy variables against the template's JSON schema.
///
/// Mirrors Consul's `ACLTemplatedPolicy.ValidateTemplatedPolicy`:
/// - Empty schema → no validation required.
/// - Non-empty schema requires `name` (minLength 1).
/// - `builtin/service` and `builtin/node` additionally require a valid
///   identity name (lowercase alphanumeric, `-` and `_` only).
fn validate_template_variables(
    template_name: &str,
    schema: &str,
    variables: &TemplatedPolicyVariables,
) -> Result<(), String> {
    if schema.is_empty() {
        return Ok(());
    }

    let name = variables.name.as_deref().unwrap_or("");
    if name.is_empty() {
        return Err("name is required".to_string());
    }

    // Additional identity-name validation for service and node templates.
    // Only lowercase alphanumeric characters, '-' and '_' are allowed.
    if (template_name == "builtin/service" || template_name == "builtin/node")
        && !name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        {
            let kind = if template_name == "builtin/service" {
                "service"
            } else {
                "node"
            };
            return Err(format!(
                "{} identity \"{}\" has an invalid name. Only lowercase alphanumeric characters, '-' and '_' are allowed",
                kind, name
            ));
        }

    Ok(())
}

/// Generate a deterministic synthetic policy ID from the rendered rules.
///
/// Consul uses FNV-128a; batata uses SHA-256 (first 32 hex chars) since a
/// 128-bit FNV implementation is not readily available. The ID only needs to
/// be deterministic for a given rules string, not byte-compatible with
/// Consul.
fn synthetic_policy_id(rules: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(rules.as_bytes());
    let result = hasher.finalize();
    result.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Token update request
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TokenUpdateRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `accessor_id` field.
    pub accessor_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `description` field.
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `policies` field.
    pub policies: Option<Vec<PolicyLink>>,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `roles` field.
    pub roles: Option<Vec<RoleLink>>,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `local` field.
    pub local: Option<bool>,
}

/// Policy update request
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PolicyUpdateRequest {
    #[serde(rename = "ID", skip_serializing_if = "Option::is_none")]
/// The `id` field.
    pub id: Option<String>,
/// The `name` field.
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `description` field.
    pub description: Option<String>,
/// The `rules` field.
    pub rules: String,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `datacenters` field.
    pub datacenters: Option<Vec<String>>,
}

// ============================================================================
// Missing ACL Service methods (in-memory)
// ============================================================================

impl AclService {
    /// Update an existing token
    pub fn update_token(&self, accessor_id: &str, update: TokenUpdateRequest) -> Option<AclToken> {
        let now = chrono::Utc::now().to_rfc3339();
        let (secret_key, mut token) = self.store.find_token_by_accessor(accessor_id)?;

        if let Some(desc) = update.description {
            token.description = desc;
        }
        if let Some(policies) = update.policies {
            token.policies = policies;
        }
        if let Some(roles) = update.roles {
            token.roles = roles;
        }
        if let Some(local) = update.local {
            token.local = local;
        }
        token.modify_time = now;

        self.store.put_token(&secret_key, &token);
        TOKEN_CACHE.invalidate(&secret_key);
        Some(token)
    }

    /// Update an existing policy
    pub fn update_policy(&self, id: &str, update: PolicyUpdateRequest) -> Option<AclPolicy> {
        let now = chrono::Utc::now().to_rfc3339();
        let mut policy = self.store.get_policy(id)?;

        let old_name = policy.name.clone();
        policy.name = update.name;
        if let Some(desc) = update.description {
            policy.description = desc;
        }
        policy.rules = update.rules;
        policy.datacenters = update.datacenters;
        policy.modify_time = now;

        // Update name mapping if name changed
        if old_name != policy.name {
            self.store.remove_policy_name_index(&old_name);
            POLICY_CACHE.invalidate(&old_name);
        }
        self.store.put_policy(&policy);
        // Invalidate caches for this policy
        POLICY_CACHE.invalidate(&policy.id);
        POLICY_CACHE.invalidate(&policy.name);
        PARSED_RULES_CACHE.invalidate(&policy.id);
        Some(policy)
    }

    /// Create a binding rule
    pub fn create_binding_rule(&self, req: BindingRuleRequest) -> BindingRule {
        let rule = BindingRule {
            id: uuid::Uuid::new_v4().to_string(),
            description: req.description.unwrap_or_default(),
            auth_method: req.auth_method,
            selector: req.selector,
            bind_type: req.bind_type,
            bind_name: req.bind_name,
            bind_vars: req.bind_vars,
            create_index: 1,
            modify_index: 1,
        };
        self.store.put_binding_rule(&rule);
        rule
    }

    /// Get a binding rule by ID
    pub fn get_binding_rule(&self, id: &str) -> Option<BindingRule> {
        self.store.get_binding_rule(id)
    }

    /// Update a binding rule
    pub fn update_binding_rule(&self, id: &str, req: BindingRuleRequest) -> Option<BindingRule> {
        let mut rule = self.store.get_binding_rule(id)?;
        if let Some(desc) = req.description {
            rule.description = desc;
        }
        rule.auth_method = req.auth_method;
        rule.selector = req.selector;
        rule.bind_type = req.bind_type;
        rule.bind_name = req.bind_name;
        rule.bind_vars = req.bind_vars;
        rule.modify_index += 1;
        self.store.put_binding_rule(&rule);
        Some(rule)
    }

    /// Delete a binding rule
    pub fn delete_binding_rule(&self, id: &str) -> bool {
        self.store.remove_binding_rule(id)
    }

    /// List binding rules
    pub fn list_binding_rules(&self) -> Vec<BindingRule> {
        self.store.list_binding_rules()
    }
}

// ============================================================================
// Missing ACL HTTP handlers (in-memory)
// ============================================================================

/// PUT /v1/acl/token/{id} - Update token
pub async fn update_token(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    body: web::Json<TokenUpdateRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let accessor_id = path.into_inner();
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));
    match acl_service.update_token(&accessor_id, body.into_inner()) {
        Some(token) => consul_ok(&meta).json(token),
        None => HttpResponse::NotFound().consul_error("Token not found"),
    }
}

/// PUT /v1/acl/policy/{id} - Update policy
pub async fn update_policy(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    body: web::Json<PolicyUpdateRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let id = path.into_inner();
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));
    match acl_service.update_policy(&id, body.into_inner()) {
        Some(policy) => consul_ok(&meta).json(policy),
        None => HttpResponse::NotFound().consul_error("Policy not found"),
    }
}

/// GET /v1/acl/policy/name/{name} - Get policy by name
pub async fn get_policy_by_name(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let name = path.into_inner();
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));
    match acl_service.get_policy(&name) {
        Some(policy) => consul_ok(&meta).json(policy),
        None => HttpResponse::NotFound().consul_error("Policy not found"),
    }
}

/// GET /v1/acl/role/name/{name} - Get role by name
pub async fn get_role_by_name(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let name = path.into_inner();
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));
    match acl_service.get_role(&name) {
        Some(role) => consul_ok(&meta).json(role),
        None => HttpResponse::NotFound().consul_error("Role not found"),
    }
}

/// GET /v1/acl/replication - ACL replication status
pub async fn acl_replication(
    dc_config: web::Data<ConsulDatacenterConfig>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));
    consul_ok(&meta).json(AclReplicationStatus {
        enabled: false,
        running: false,
        source_datacenter: dc_config.primary_datacenter.clone(),
        replication_type: "tokens".to_string(),
        replicated_index: 0,
        replicated_role_index: 0,
        replicated_token_index: 0,
        last_success: None,
        last_error: None,
        last_error_message: None,
    })
}

/// GET /v1/acl/binding-rules - List binding rules
pub async fn list_binding_rules(
    acl_service: web::Data<AclService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));
    consul_ok(&meta).json(acl_service.list_binding_rules())
}

/// PUT /v1/acl/binding-rule - Create binding rule
pub async fn create_binding_rule(
    acl_service: web::Data<AclService>,
    body: web::Json<BindingRuleRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let rule = acl_service.create_binding_rule(body.into_inner());
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));
    consul_ok(&meta).json(rule)
}

/// GET /v1/acl/binding-rule/{id} - Get binding rule
pub async fn get_binding_rule(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let id = path.into_inner();
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));
    match acl_service.get_binding_rule(&id) {
        Some(rule) => consul_ok(&meta).json(rule),
        None => HttpResponse::NotFound().consul_error("Binding rule not found"),
    }
}

/// PUT /v1/acl/binding-rule/{id} - Update binding rule
pub async fn update_binding_rule(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    body: web::Json<BindingRuleRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let id = path.into_inner();
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));
    match acl_service.update_binding_rule(&id, body.into_inner()) {
        Some(rule) => consul_ok(&meta).json(rule),
        None => HttpResponse::NotFound().consul_error("Binding rule not found"),
    }
}

/// DELETE /v1/acl/binding-rule/{id} - Delete binding rule
pub async fn delete_binding_rule(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let id = path.into_inner();
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));
    if acl_service.delete_binding_rule(&id) {
        consul_ok(&meta).json(true)
    } else {
        HttpResponse::NotFound().consul_error("Binding rule not found")
    }
}

/// GET /v1/acl/templated-policies - List templated policies
///
/// Returns a `map[string]ACLTemplatedPolicyResponse` with all 6 built-in
/// templates, matching Consul's `ACLTemplatedPolicyList` handler.
pub async fn list_templated_policies(
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let mut policies = HashMap::new();
    for (name, schema, template, description) in builtin_templates() {
        policies.insert(
            name.to_string(),
            TemplatedPolicy {
                template_name: name.to_string(),
                schema: schema.to_string(),
                template: template.to_string(),
                description: description.to_string(),
            },
        );
    }
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));
    consul_ok(&meta).json(policies)
}

/// GET /v1/acl/templated-policy/name/{name} - Get templated policy by name
///
/// Returns a single `ACLTemplatedPolicyResponse` with `TemplateName`,
/// `Schema`, `Template`, and `Description` (all strings, even when empty).
pub async fn get_templated_policy(
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let name = path.into_inner();
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));

    for (tpl_name, schema, template, description) in builtin_templates() {
        if tpl_name == name {
            return consul_ok(&meta).json(TemplatedPolicy {
                template_name: tpl_name.to_string(),
                schema: schema.to_string(),
                template: template.to_string(),
                description: description.to_string(),
            });
        }
    }

    HttpResponse::BadRequest().consul_error(format!("Invalid templated policy Name: {}", name))
}

/// POST /v1/acl/templated-policy/preview/{name} - Preview rendered template
///
/// Validates the supplied variables against the template's JSON schema,
/// renders the template, and returns a synthetic `ACLPolicy` with a
/// deterministic ID derived from the rendered rules.
pub async fn preview_templated_policy(
    path: web::Path<String>,
    body: web::Json<TemplatedPolicyVariables>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let template_name = path.into_inner();
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));

    // Find the template by name.
    let (schema, template, _description) =
        match builtin_templates()
            .iter()
            .find(|(n, _, _, _)| *n == template_name)
        {
            Some(&(_, schema, template, desc)) => (schema, template, desc),
            None => {
                return HttpResponse::BadRequest().consul_error(format!(
                    "templated policy \"{}\" does not exist",
                    template_name
                ));
            }
        };

    // Validate template variables against the schema.
    if let Err(err) = validate_template_variables(&template_name, schema, &body) {
        return HttpResponse::BadRequest().consul_error(format!(
            "validation error for templated policy: \"{}\": {}",
            template_name, err
        ));
    }

    // Render the template by substituting {{.Name}}.
    let rules = render_template(template, body.name.as_deref());

    // Generate the synthetic policy.
    let id = synthetic_policy_id(&rules);
    let policy = SyntheticPolicy {
        id: id.clone(),
        name: format!("synthetic-policy-{}", id),
        description: format!(
            "synthetic policy generated from templated policy: {}",
            template_name
        ),
        rules,
    };

    consul_ok(&meta).json(policy)
}

// ============================================================================
// ACL Authorize Endpoint
// ============================================================================

/// ACL authorization check request
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AclAuthorizationCheck {
/// The `resource` field.
    pub resource: String,
    #[serde(default)]
/// The `segment` field.
    pub segment: Option<String>,
/// The `access` field.
    pub access: String,
}

/// ACL authorization response
#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct AclAuthorizationResponse {
/// The `allow` field.
    pub allow: bool,
/// The `resource` field.
    pub resource: String,
/// The `access` field.
    pub access: String,
    #[serde(skip_serializing_if = "String::is_empty")]
/// The `error` field.
    pub error: String,
}

/// POST /v1/acl/authorize - Authorize a batch of ACL checks
/// POST /v1/internal/acl/authorize - Same endpoint on internal path
pub async fn acl_authorize(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    body: web::Json<Vec<AclAuthorizationCheck>>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let checks = body.into_inner();

    // Max 64 checks per request
    if checks.len() > 64 {
        return HttpResponse::BadRequest()
            .json(AclError::new("Too many authorization checks (max 64)"));
    }

    let responses: Vec<AclAuthorizationResponse> = checks
        .into_iter()
        .map(|check| {
            let resource_type = match check.resource.as_str() {
                "service" => ResourceType::Service,
                "node" | "agent" => ResourceType::Agent,
                "key" => ResourceType::Key,
                "operator" => ResourceType::Operator,
                "session" => ResourceType::Session,
                "query" => ResourceType::Query,
                _ => ResourceType::Agent,
            };
            let read_only = check.access == "read";
            let authz = acl_service.authorize_request(
                &req,
                resource_type,
                check.segment.as_deref().unwrap_or(""),
                !read_only,
            );

            AclAuthorizationResponse {
                allow: authz.allowed,
                resource: check.resource,
                access: check.access,
                error: if authz.allowed {
                    String::new()
                } else {
                    authz.reason
                },
            }
        })
        .collect();

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));
    consul_ok(&meta).json(responses)
}

// ============================================================================
// OIDC authentication endpoints.
// ============================================================================

/// `POST /v1/acl/oidc/auth-url` request body.
///
/// Corresponds to Consul's `ACLOIDCAuthURLParams` struct.
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct OidcAuthUrlRequest {
    /// The auth method name (must be an auth method of type `oidc`).
    pub auth_method: String,
    /// The callback URI (must be in the auth method's configured `AllowedRedirectURIs`).
    #[serde(rename = "RedirectURI")]
    pub redirect_uri: String,
    /// The client nonce (optional, used for extra request validation).
    #[serde(default)]
    pub client_nonce: Option<String>,
    /// Client metadata (optional).
    #[serde(default)]
    pub meta: Option<HashMap<String, String>>,
}

/// `POST /v1/acl/oidc/auth-url` response body.
#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct OidcAuthUrlResponse {
    /// The generated OIDC authorization URL.
    #[serde(rename = "AuthURL")]
    pub auth_url: String,
}

/// `POST /v1/acl/oidc/callback` request body.
///
/// Corresponds to Consul's `ACLOIDCCallbackParams` struct.
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct OidcCallbackRequest {
    /// The auth method name.
    pub auth_method: String,
    /// The state returned with the authorization URL.
    pub state: String,
    /// The authorization code returned by the OIDC provider.
    pub code: String,
    /// The client nonce (optional, must match the one provided in the auth-url request).
    #[serde(default)]
    pub client_nonce: Option<String>,
}

/// POST /v1/acl/oidc/auth-url
/// Generates the OIDC authorization URL.
///
/// Steps:
/// 1. Look up the auth method and verify its type is oidc.
/// 2. Create or get the cached `OidcAuthenticator` from the auth method config.
/// 3. Call `authenticator.get_auth_url` to generate the authorization URL.
/// 4. Return the AuthURL.
pub async fn oidc_auth_url(
    acl_service: web::Data<AclService>,
    body: web::Json<OidcAuthUrlRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    // Look up the auth method.
    let auth_method = match acl_service.get_auth_method(&body.auth_method) {
        Some(m) => m,
        None => {
            return HttpResponse::NotFound()
                .consul_error(format!("Auth method '{}' not found", body.auth_method));
        }
    };

    // Verify the type is oidc.
    if auth_method.method_type != "oidc" {
        return HttpResponse::BadRequest().consul_error(format!(
            "Auth method '{}' is not of type 'oidc' (got '{}')",
            body.auth_method, auth_method.method_type
        ));
    }

    // Create or get the cached OidcAuthenticator from the config.
    let authenticator = match crate::oidc::get_or_create_authenticator(
        &body.auth_method,
        &auth_method.config,
    ) {
        Ok(a) => a,
        Err(e) => {
            warn!("Failed to create OIDC authenticator for '{}': {}", body.auth_method, e);
            return HttpResponse::BadRequest().consul_error(format!(
                "Invalid OIDC configuration for auth method '{}': {}",
                body.auth_method, e
            ));
        }
    };

    // Generate the authorization URL.
    match authenticator
        .get_auth_url(
            &body.redirect_uri,
            &body.auth_method,
            body.client_nonce.as_deref(),
            body.meta.clone(),
        )
        .await
    {
        Ok(auth_url) => {
            let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));
            consul_ok(&meta).json(OidcAuthUrlResponse { auth_url })
        }
        Err(e) => {
            warn!(
                "Failed to generate OIDC auth URL for '{}': {}",
                body.auth_method, e
            );
            HttpResponse::BadRequest().consul_error(e)
        }
    }
}

/// POST /v1/acl/oidc/callback
/// Exchanges the authorization code for a token.
///
/// Steps:
/// 1. Look up the auth method and verify its type is oidc.
/// 2. Create or get the cached `OidcAuthenticator` from the config.
/// 3. Call `authenticator.exchange_code` to exchange the authorization code.
/// 4. Verify the `client_nonce` (if provided in the request).
/// 5. Apply binding rules to determine policies and roles.
/// 6. Create the ACL token.
/// 7. Return the token.
pub async fn oidc_callback(
    acl_service: web::Data<AclService>,
    body: web::Json<OidcCallbackRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    // Validate required fields.
    if body.state.is_empty() {
        return HttpResponse::BadRequest().consul_error("State parameter is required");
    }
    if body.code.is_empty() {
        return HttpResponse::BadRequest().consul_error("Code parameter is required");
    }

    // Look up the auth method.
    let auth_method = match acl_service.get_auth_method(&body.auth_method) {
        Some(m) => m,
        None => {
            return HttpResponse::NotFound()
                .consul_error(format!("Auth method '{}' not found", body.auth_method));
        }
    };

    // Verify the type is oidc.
    if auth_method.method_type != "oidc" {
        return HttpResponse::BadRequest().consul_error(format!(
            "Auth method '{}' is not of type 'oidc' (got '{}')",
            body.auth_method, auth_method.method_type
        ));
    }

    // Create or get the cached OidcAuthenticator from the config.
    let authenticator = match crate::oidc::get_or_create_authenticator(
        &body.auth_method,
        &auth_method.config,
    ) {
        Ok(a) => a,
        Err(e) => {
            warn!("Failed to create OIDC authenticator for '{}': {}", body.auth_method, e);
            return HttpResponse::BadRequest().consul_error(format!(
                "Invalid OIDC configuration for auth method '{}': {}",
                body.auth_method, e
            ));
        }
    };

    // Exchanges the authorization code for claims.
    let oidc_claims = match authenticator.exchange_code(&body.state, &body.code).await {
        Ok(claims) => claims,
        Err(e) => {
            warn!(
                "OIDC code exchange failed for '{}': {}",
                body.auth_method, e
            );
            return HttpResponse::BadRequest().consul_error(e);
        }
    };

    // Verify the `client_nonce` (if provided in the request).
    if let Some(ref request_nonce) = body.client_nonce
        && oidc_claims.client_nonce.as_ref() != Some(request_nonce) {
            return HttpResponse::BadRequest()
                .consul_error("Client nonce mismatch");
        }

    // Apply binding rules to determine policies and roles.
    let (policies, roles) = apply_oidc_binding_rules(
        &acl_service,
        &body.auth_method,
        &oidc_claims.claims,
    );

    // Compute the token expiry time.
    let expiration_ttl = auth_method.max_token_ttl.as_deref();

    // Determine the token locality.
    let local = auth_method.token_locality.as_deref() == Some("local");

    // Create the ACL token.
    let token = acl_service
        .create_token(
            &format!("OIDC token via {}", body.auth_method),
            policies,
            roles,
            local,
            expiration_ttl,
        )
        .await;

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));
    consul_ok(&meta).json(token)
}

/// Applies OIDC binding rules to determine the policies and roles to grant based on the claims.
///
/// Iterates over all binding rules of the given auth method; for each rule:
/// - If the selector is `None` or empty, it matches everything.
/// - Otherwise, attempt a simple evaluation of the selector expression.
///
/// Returns the two lists `(policies, roles)`.
fn apply_oidc_binding_rules(
    acl_service: &AclService,
    auth_method: &str,
    claims: &HashMap<String, serde_json::Value>,
) -> (Vec<String>, Vec<String>) {
    let mut policies = Vec::new();
    let mut roles = Vec::new();

    // Get all binding rules for this auth method.
    let rules: Vec<BindingRule> = acl_service
        .list_binding_rules()
        .into_iter()
        .filter(|r| r.auth_method == auth_method)
        .collect();

    for rule in &rules {
        // Evaluate the selector.
        let matched = if rule.selector.as_ref().is_none_or(|s| s.is_empty()) {
            // No selector, matches everything.
            true
        } else {
            // Has a selector, attempt a simple evaluation.
            evaluate_binding_rule_selector(rule.selector.as_deref().unwrap_or(""), claims)
        };

        if matched {
            match rule.bind_type.as_str() {
                "policy" => {
                    policies.push(rule.bind_name.clone());
                }
                "role" => {
                    roles.push(rule.bind_name.clone());
                }
                // Other types such as service and node-identity are not handled yet.
                _ => {
                    debug!("OIDC binding rule bind_type '{}' not supported, skipping", rule.bind_type);
                }
            }
        }
    }

    (policies, roles)
}

/// Simply evaluates a binding rule selector expression.
///
/// Supported formats:
/// - `key == "value"` - string equality comparison.
/// - `key in ["a", "b"]` - list containment check.
///
/// Unsupported formats return false.
fn evaluate_binding_rule_selector(
    selector: &str,
    claims: &HashMap<String, serde_json::Value>,
) -> bool {
    let selector = selector.trim();

    // Try to parse the `key == "value"` format.
    if let Some(eq_pos) = selector.find("==") {
        let key = selector[..eq_pos].trim();
        let value_str = selector[eq_pos + 2..].trim();

        // Strip the quotes.
        let value = value_str.trim_matches('"');

        if let Some(claim_value) = claims.get(key) {
            // String comparison.
            if let Some(s) = claim_value.as_str() {
                return s == value;
            }
            // String comparison for other types.
            return claim_value.to_string().trim_matches('"') == value;
        }
        return false;
    }

    // Try to parse the `key in [...]` format.
    if let Some(in_pos) = selector.find(" in ") {
        let key = selector[..in_pos].trim();
        let list_str = selector[in_pos + 4..].trim();

        // Parse the list.
        let list_str = list_str.trim_start_matches('[').trim_end_matches(']');
        let values: Vec<&str> = list_str
            .split(',')
            .map(|v| v.trim().trim_matches('"'))
            .collect();

        if let Some(claim_value) = claims.get(key) {
            // If the claim is an array, check for intersection.
            if let Some(arr) = claim_value.as_array() {
                return arr.iter().any(|v| {
                    if let Some(s) = v.as_str() {
                        values.contains(&s)
                    } else {
                        let s = v.to_string();
                        let trimmed = s.trim_matches('"');
                        values.contains(&trimmed)
                    }
                });
            }
            // If the claim is a string, check whether it is in the list.
            if let Some(s) = claim_value.as_str() {
                return values.contains(&s);
            }
        }
        return false;
    }

    // Unsupported format, does not match.
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acl_service_creation() {
        let service = AclService::new();
        assert!(service.is_enabled());
    }

    #[test]
    fn test_acl_service_disabled() {
        let service = AclService::disabled();
        assert!(!service.is_enabled());
    }

    #[test]
    fn test_bootstrap_token() {
        let service = AclService::new();
        let token = service.find_bootstrap_token();
        assert!(token.is_some());
        let token = token.unwrap();
        assert_eq!(token.description, "Bootstrap Token (Management)");
        assert!(!token.policies.is_empty());
        // Secret ID should be a UUID, not "root"
        let secret_id = token.secret_id.unwrap();
        assert_ne!(secret_id, "root");
        assert!(uuid::Uuid::parse_str(&secret_id).is_ok());
    }

    #[test]
    fn test_global_management_policy() {
        let service = AclService::new();
        let policy = service.get_policy("global-management");
        assert!(policy.is_some());
        let policy = policy.unwrap();
        assert_eq!(policy.name, "global-management");
        assert!(policy.rules.contains("service_prefix"));
    }

    #[test]
    fn test_parse_rules() {
        let service = AclService::new();
        let rules = r#"
            service_prefix "" { policy = "read" }
            service_prefix "web-" { policy = "write" }
            key_prefix "config/" { policy = "read" }
        "#;

        let parsed = service.parse_rules(rules);
        assert_eq!(parsed.service_rules.len(), 2);
        assert_eq!(parsed.key_rules.len(), 1);
        assert_eq!(parsed.key_rules[0].prefix, "config/");
    }

    #[test]
    fn test_rule_policy() {
        assert!(RulePolicy::Write.allows_read());
        assert!(RulePolicy::Write.allows_write());
        assert!(RulePolicy::Read.allows_read());
        assert!(!RulePolicy::Read.allows_write());
        assert!(!RulePolicy::Deny.allows_read());
        assert!(!RulePolicy::Deny.allows_write());
    }

    #[test]
    fn test_authorize_with_bootstrap_token() {
        let service = AclService::new();
        let token = service.find_bootstrap_token().unwrap();

        // Bootstrap token should have full access
        let result = service.authorize(&token, ResourceType::Service, "any-service", true);
        assert!(result.allowed);

        let result = service.authorize(&token, ResourceType::Key, "any/key", true);
        assert!(result.allowed);
    }

    #[tokio::test]
    async fn test_create_token() {
        let service = AclService::new();
        let token = service
            .create_token(
                "Test token",
                vec!["global-management".to_string()],
                vec![],
                false,
                None,
            )
            .await;

        assert!(!token.accessor_id.is_empty());
        assert!(token.secret_id.is_some());
        assert_eq!(token.description, "Test token");
        assert!(!token.policies.is_empty());
        assert_eq!(token.policies[0].name, "global-management");
        assert!(!token.local);
        assert!(token.roles.is_empty());
        assert!(!token.create_time.is_empty());
        assert!(!token.modify_time.is_empty());
    }

    #[tokio::test]
    async fn test_create_policy() {
        let service = AclService::new();
        let policy = service
            .create_policy(
                "test-policy",
                "A test policy",
                r#"service_prefix "test-" { policy = "write" }"#,
                None,
            )
            .await
            .unwrap();

        assert!(!policy.id.is_empty());
        assert_eq!(policy.name, "test-policy");
        assert_eq!(policy.description, "A test policy");
        assert!(policy.rules.contains("service_prefix"));
        assert!(policy.rules.contains("test-"));
        assert!(!policy.create_time.is_empty());
        assert!(!policy.modify_time.is_empty());

        // Verify we can retrieve it
        let retrieved = service.get_policy("test-policy");
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.name, "test-policy");
        assert_eq!(retrieved.description, "A test policy");
    }

    #[test]
    fn test_authz_result() {
        let allowed = AuthzResult::allowed();
        assert!(allowed.allowed);
        assert!(allowed.reason.is_empty());

        let denied = AuthzResult::denied("No permission");
        assert!(!denied.allowed);
        assert_eq!(denied.reason, "No permission");
    }

    #[tokio::test]
    async fn test_delete_token() {
        let service = AclService::new();
        let token = service
            .create_token(
                "to-delete",
                vec!["global-management".to_string()],
                vec![],
                false,
                None,
            )
            .await;
        assert!(service.delete_token(&token.accessor_id).await);
        // Deleting again should return false
        assert!(!service.delete_token(&token.accessor_id).await);
        // Verify token is no longer retrievable by looking up in list
        let tokens = service.list_tokens();
        assert!(
            !tokens.iter().any(|t| t.accessor_id == token.accessor_id),
            "Deleted token should not appear in list"
        );
    }

    #[test]
    fn test_list_tokens_hides_secret() {
        let service = AclService::new();
        let tokens = service.list_tokens();
        assert!(
            !tokens.is_empty(),
            "Should have at least the bootstrap token"
        );
        // All tokens in list should have secret_id = None
        for t in &tokens {
            assert!(t.secret_id.is_none());
            assert!(!t.accessor_id.is_empty());
        }
    }

    #[tokio::test]
    async fn test_create_and_delete_policy() {
        let service = AclService::new();
        let policy = service
            .create_policy(
                "acl-test-del-policy",
                "For deletion test",
                r#"key_prefix "" { policy = "read" }"#,
                None,
            )
            .await
            .unwrap();

        let fetched = service.get_policy(&policy.id);
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().description, "For deletion test");
        assert!(service.delete_policy(&policy.id).await);
        assert!(service.get_policy(&policy.id).is_none());
        // Also verify get by name returns None after deletion
        assert!(service.get_policy("acl-test-del-policy").is_none());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_policy() {
        let service = AclService::new();
        assert!(!service.delete_policy("nonexistent-policy-id").await);
    }

    #[tokio::test]
    async fn test_role_crud() {
        let service = AclService::new();

        // Create policy first (roles link to policies)
        let policy = service
            .create_policy(
                "acl-role-test-policy",
                "Test",
                r#"service_prefix "" { policy = "read" }"#,
                None,
            )
            .await
            .unwrap();

        // Create role
        let role = service
            .create_role("acl-test-role", "Test role", vec![policy.name.clone()])
            .await;
        assert_eq!(role.name, "acl-test-role");
        assert_eq!(role.description, "Test role");
        assert!(!role.id.is_empty());
        assert!(!role.policies.is_empty());
        assert_eq!(role.policies[0].name, policy.name);
        assert!(!role.create_time.is_empty());
        assert!(!role.modify_time.is_empty());

        // Get by ID
        let fetched = service.get_role(&role.id);
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().name, "acl-test-role");

        // Get by name
        let by_name = service.get_role("acl-test-role");
        assert!(by_name.is_some());

        // Update
        let updated = service
            .update_role(
                &role.id,
                Some("acl-renamed-role"),
                Some("Updated desc"),
                None,
            )
            .await;
        assert!(updated.is_some());
        assert_eq!(updated.unwrap().name, "acl-renamed-role");

        // Delete
        assert!(service.delete_role(&role.id).await);
        assert!(service.get_role(&role.id).is_none());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_role() {
        let service = AclService::new();
        assert!(!service.delete_role("nonexistent-role-id").await);
    }

    #[tokio::test]
    async fn test_auth_method_crud() {
        let service = AclService::new();

        let method = service
            .create_auth_method(
                "acl-test-kubernetes",
                "kubernetes",
                Some("K8s Auth"),
                Some("Kubernetes auth method"),
                Some("5m"),
                Some("local"),
                None,
            )
            .await;
        assert_eq!(method.name, "acl-test-kubernetes");
        assert_eq!(method.method_type, "kubernetes");
        assert_eq!(method.display_name.as_deref(), Some("K8s Auth"));
        assert_eq!(
            method.description.as_deref(),
            Some("Kubernetes auth method")
        );

        // Get
        let fetched = service.get_auth_method("acl-test-kubernetes");
        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.name, "acl-test-kubernetes");
        assert_eq!(fetched.method_type, "kubernetes");

        // Get nonexistent
        assert!(service.get_auth_method("nonexistent-method").is_none());
    }

    #[tokio::test]
    async fn test_authorize_with_custom_policy() {
        let service = AclService::new();

        // Create a read-only policy for services
        let policy = service
            .create_policy(
                "acl-test-readonly-svc",
                "Read only",
                r#"service_prefix "" { policy = "read" }"#,
                None,
            )
            .await
            .unwrap();

        // Create token with this policy
        let token = service
            .create_token(
                "readonly-token",
                vec![policy.name.clone()],
                vec![],
                false,
                None,
            )
            .await;

        // Read should be allowed
        let result = service.authorize(&token, ResourceType::Service, "web", false);
        assert!(result.allowed);

        // Write should be denied
        let result = service.authorize(&token, ResourceType::Service, "web", true);
        assert!(!result.allowed);
    }

    #[tokio::test]
    async fn test_authorize_deny_policy() {
        let service = AclService::new();

        let policy = service
            .create_policy(
                "acl-test-deny-policy",
                "Deny",
                r#"key_prefix "secret/" { policy = "deny" }"#,
                None,
            )
            .await
            .unwrap();

        let token = service
            .create_token("deny-token", vec![policy.name.clone()], vec![], false, None)
            .await;

        // Should be denied for both read and write on secret/
        let result = service.authorize(&token, ResourceType::Key, "secret/data", false);
        assert!(!result.allowed);
    }

    #[test]
    fn test_disabled_acl_allows_all() {
        let service = AclService::disabled();

        let token = AclToken::default();
        let result = service.authorize(&token, ResourceType::Service, "anything", true);
        assert!(result.allowed);
    }

    #[test]
    fn test_parse_rules_node_and_session() {
        let service = AclService::new();
        let rules = r#"
            node_prefix "web-" { policy = "write" }
            session_prefix "" { policy = "read" }
            query_prefix "q-" { policy = "write" }
        "#;

        let parsed = service.parse_rules(rules);
        assert_eq!(parsed.node_rules.len(), 1);
        assert_eq!(parsed.session_rules.len(), 1);
        assert_eq!(parsed.query_rules.len(), 1);
    }

    #[test]
    fn test_resource_type_coverage() {
        // Ensure all resource types can be used in authorization
        let service = AclService::new();
        let token = service.find_bootstrap_token().unwrap();

        let types = vec![
            ResourceType::Service,
            ResourceType::Key,
            ResourceType::Keyring,
            ResourceType::Node,
            ResourceType::Session,
            ResourceType::Query,
            ResourceType::Agent,
            ResourceType::Operator,
        ];

        for rt in types {
            let result = service.authorize(&token, rt, "test", false);
            assert!(
                result.allowed,
                "Root token should access all resource types"
            );
        }
    }

    #[test]
    fn test_binding_rule_crud_lifecycle() {
        let service = AclService::new();

        // Create
        let rule = service.create_binding_rule(BindingRuleRequest {
            description: Some("Test binding rule".into()),
            auth_method: "kubernetes".into(),
            selector: Some("serviceaccount.name==web".into()),
            bind_type: "service".into(),
            bind_name: "web-${serviceaccount.name}".into(),
            bind_vars: None,
        });
        assert!(!rule.id.is_empty(), "Rule ID should be generated");
        assert_eq!(rule.auth_method, "kubernetes");
        assert_eq!(rule.bind_type, "service");
        assert_eq!(rule.bind_name, "web-${serviceaccount.name}");
        assert_eq!(rule.description, "Test binding rule");
        assert_eq!(rule.selector, Some("serviceaccount.name==web".into()));

        // Read
        let fetched = service.get_binding_rule(&rule.id);
        assert!(fetched.is_some(), "Should find created rule");
        assert_eq!(fetched.unwrap().bind_name, "web-${serviceaccount.name}");

        // List
        let rules = service.list_binding_rules();
        assert!(
            rules.iter().any(|r| r.id == rule.id),
            "List should contain created rule"
        );

        // Update
        let updated = service.update_binding_rule(
            &rule.id,
            BindingRuleRequest {
                description: Some("Updated description".into()),
                auth_method: "kubernetes".into(),
                selector: None,
                bind_type: "role".into(),
                bind_name: "admin".into(),
                bind_vars: None,
            },
        );
        assert!(updated.is_some());
        let updated = updated.unwrap();
        assert_eq!(updated.bind_type, "role");
        assert_eq!(updated.bind_name, "admin");
        assert_eq!(updated.description, "Updated description");
        assert_eq!(updated.modify_index, rule.modify_index + 1);

        // Delete
        assert!(service.delete_binding_rule(&rule.id));
        assert!(
            service.get_binding_rule(&rule.id).is_none(),
            "Rule should be deleted"
        );
    }

    #[test]
    fn test_binding_rule_delete_nonexistent() {
        let service = AclService::new();
        assert!(
            !service.delete_binding_rule("nonexistent-id"),
            "Deleting nonexistent rule should return false"
        );
    }

    #[test]
    fn test_binding_rule_update_nonexistent() {
        let service = AclService::new();
        let result = service.update_binding_rule(
            "nonexistent-id",
            BindingRuleRequest {
                description: None,
                auth_method: "test".into(),
                selector: None,
                bind_type: "service".into(),
                bind_name: "test".into(),
                bind_vars: None,
            },
        );
        assert!(
            result.is_none(),
            "Updating nonexistent rule should return None"
        );
    }

    #[test]
    fn test_binding_rule_multiple_rules() {
        let service = AclService::new();

        let rule1 = service.create_binding_rule(BindingRuleRequest {
            description: None,
            auth_method: "kubernetes".into(),
            selector: None,
            bind_type: "service".into(),
            bind_name: "web".into(),
            bind_vars: None,
        });
        let rule2 = service.create_binding_rule(BindingRuleRequest {
            description: None,
            auth_method: "jwt".into(),
            selector: None,
            bind_type: "role".into(),
            bind_name: "admin".into(),
            bind_vars: None,
        });

        let rules = service.list_binding_rules();
        assert!(rules.len() >= 2, "Should have at least 2 binding rules");
        assert!(rules.iter().any(|r| r.id == rule1.id));
        assert!(rules.iter().any(|r| r.id == rule2.id));

        // Delete one, verify other remains
        service.delete_binding_rule(&rule1.id);
        let rules = service.list_binding_rules();
        assert!(
            !rules.iter().any(|r| r.id == rule1.id),
            "Deleted rule should be gone"
        );
        assert!(
            rules.iter().any(|r| r.id == rule2.id),
            "Other rule should remain"
        );
    }
}
