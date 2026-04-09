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
use tracing::{error, info};

use crate::acl_store::AclStore;
use crate::consul_meta::{ConsulResponseMeta, consul_ok};
use crate::index_provider::{ConsulIndexProvider, ConsulTable};
use crate::model::{ConsulDatacenterConfig, ConsulError};
use crate::raft::{ConsulRaftRequest, ConsulRaftWriter};

// ACL Token header name
pub const X_CONSUL_TOKEN: &str = "X-Consul-Token";
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

/// Expanded ACL token with resolved policies and roles
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AclTokenExpanded {
    pub expanded_policies: Vec<AclPolicy>,
    pub expanded_roles: Vec<AclRole>,
    #[serde(flatten)]
    pub token: AclToken,
}

/// ACL Token structure
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AclToken {
    pub accessor_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_id: Option<String>,
    pub description: String,
    pub policies: Vec<PolicyLink>,
    pub roles: Vec<RoleLink>,
    pub local: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiration_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "ExpirationTTL")]
    pub expiration_ttl: Option<u64>,
    pub create_time: String,
    pub modify_time: String,
}

/// Policy link in token
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PolicyLink {
    #[serde(rename = "ID", default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
}

/// Role link in token
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct RoleLink {
    #[serde(rename = "ID", default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
}

/// ACL Policy structure
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AclPolicy {
    #[serde(rename = "ID")]
    pub id: String,
    pub name: String,
    pub description: String,
    pub rules: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub datacenters: Option<Vec<String>>,
    pub create_time: String,
    pub modify_time: String,
}

/// ACL Role structure
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AclRole {
    #[serde(rename = "ID")]
    pub id: String,
    pub name: String,
    pub description: String,
    pub policies: Vec<PolicyLink>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_identities: Option<Vec<ServiceIdentity>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_identities: Option<Vec<NodeIdentity>>,
    pub create_time: String,
    pub modify_time: String,
}

/// Service identity for role
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ServiceIdentity {
    pub service_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub datacenters: Option<Vec<String>>,
}

/// Node identity for role
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct NodeIdentity {
    pub node_name: String,
    pub datacenter: String,
}

/// ACL Auth Method structure
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AuthMethod {
    pub name: String,
    #[serde(rename = "Type")]
    pub method_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_token_ttl: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_locality: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<HashMap<String, serde_json::Value>>,
    pub create_index: u64,
    pub modify_index: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

/// Parsed ACL rules for authorization checks
#[derive(Clone, Debug, Default)]
pub struct ParsedRules {
    pub agent_rules: Vec<ResourceRule>,
    pub key_rules: Vec<ResourceRule>,
    pub node_rules: Vec<ResourceRule>,
    pub service_rules: Vec<ResourceRule>,
    pub session_rules: Vec<ResourceRule>,
    pub query_rules: Vec<ResourceRule>,
}

/// Single resource rule
#[derive(Clone, Debug)]
pub struct ResourceRule {
    pub prefix: String,
    pub policy: RulePolicy,
}

/// Rule policy type
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RulePolicy {
    Read,
    Write,
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
    pub fn allows_read(&self) -> bool {
        matches!(self, RulePolicy::Read | RulePolicy::Write)
    }

    pub fn allows_write(&self) -> bool {
        matches!(self, RulePolicy::Write)
    }
}

/// Resource types for authorization
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResourceType {
    Agent,
    Key,
    Node,
    Operator,
    Service,
    Session,
    Query,
}

/// ACL authorization result
#[derive(Clone, Debug)]
pub struct AuthzResult {
    pub allowed: bool,
    pub reason: String,
}

impl AuthzResult {
    pub fn allowed() -> Self {
        Self {
            allowed: true,
            reason: String::new(),
        }
    }

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
            svc.init_bootstrap(None);
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
            ResourceType::Agent | ResourceType::Operator => &all_rules.agent_rules,
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
    pub id: String,
    pub accessor_id: String,
    pub secret_id: String,
    pub description: String,
    pub policies: Vec<PolicyLink>,
    pub local: bool,
    pub create_time: String,
    pub hash: String,
}

// ============================================================================
// ACL Login Request/Response
// ============================================================================

/// Login request with auth method
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LoginRequest {
    pub auth_method: String,
    pub bearer_token: Option<String>,
    pub meta: Option<HashMap<String, String>>,
}

/// Login response with token
#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct LoginResponse {
    pub accessor_id: String,
    pub secret_id: String,
    pub description: String,
    pub policies: Vec<PolicyLink>,
    pub roles: Vec<RoleLink>,
    pub local: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiration_time: Option<String>,
    pub create_time: String,
}

// ============================================================================
// Token Clone Request
// ============================================================================

/// Clone token request
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CloneTokenRequest {
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
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
        None => HttpResponse::NotFound().json(AclError::new("ACL not found")),
    }
}

/// Token creation request
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CreateTokenRequest {
    pub description: Option<String>,
    pub policies: Option<Vec<PolicyLink>>,
    pub roles: Option<Vec<RoleLink>>,
    pub local: Option<bool>,
    #[serde(default, rename = "ExpirationTTL")]
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
        HttpResponse::NotFound().json(AclError::new("Token not found"))
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
        None => return HttpResponse::Forbidden().json(AclError::new("ACL token required")),
    };

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));
    match acl_service.get_token(&secret_id) {
        Some(token) => consul_ok(&meta).json(token),
        None => HttpResponse::Forbidden().json(AclError::new("ACL token not found or invalid")),
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
        None => HttpResponse::NotFound().json(AclError::new("Token not found")),
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
        HttpResponse::InternalServerError().json(AclError::new("Failed to bootstrap ACL"))
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
            return HttpResponse::NotFound().json(AclError::new(format!(
                "Auth method '{}' not found",
                body.auth_method
            )));
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
        None => return HttpResponse::Forbidden().json(AclError::new("ACL token required")),
    };

    // Don't allow logging out the bootstrap token
    if let Some(token) = acl_service.get_token(&secret_id)
        && token.accessor_id == BOOTSTRAP_ACCESSOR_ID
    {
        return HttpResponse::Forbidden().json(AclError::new("Cannot logout bootstrap token"));
    }

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));

    // Find and delete the token
    if let Some(token) = acl_service.get_token(&secret_id)
        && acl_service.delete_token(&token.accessor_id).await
    {
        consul_ok(&meta).json(true)
    } else {
        HttpResponse::NotFound().json(AclError::new("Token not found"))
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
        None => HttpResponse::NotFound().json(AclError::new("Policy not found")),
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
        HttpResponse::NotFound().json(AclError::new("Policy not found"))
    }
}

/// Policy creation request
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CreatePolicyRequest {
    pub name: String,
    pub description: Option<String>,
    pub rules: String,
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
        Err(e) => HttpResponse::InternalServerError().json(ConsulError::new(&e)),
    }
}

// ============================================================================
// ACL Role API Endpoints
// ============================================================================

/// Role creation/update request
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct RoleRequest {
    pub name: String,
    pub description: Option<String>,
    pub policies: Option<Vec<PolicyLink>>,
    pub service_identities: Option<Vec<ServiceIdentity>>,
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
        None => HttpResponse::NotFound().json(AclError::new("Role not found")),
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
        None => HttpResponse::NotFound().json(AclError::new("Role not found")),
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
        HttpResponse::NotFound().json(AclError::new("Role not found"))
    }
}

// ============================================================================
// ACL Auth Method API Endpoints
// ============================================================================

/// Auth method creation/update request
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AuthMethodRequest {
    pub name: String,
    #[serde(rename = "Type")]
    pub method_type: String,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub max_token_ttl: Option<String>,
    pub token_locality: Option<String>,
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
        None => HttpResponse::NotFound().json(AclError::new("Auth method not found")),
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
        None => HttpResponse::NotFound().json(AclError::new("Auth method not found")),
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
        HttpResponse::NotFound().json(AclError::new("Auth method not found"))
    }
}

/// ACL Binding Rule structure
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct BindingRule {
    #[serde(rename = "ID")]
    pub id: String,
    pub description: String,
    pub auth_method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
    pub bind_type: String,
    pub bind_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bind_vars: Option<HashMap<String, String>>,
    pub create_index: u64,
    pub modify_index: u64,
}

/// Binding Rule create/update request
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct BindingRuleRequest {
    #[serde(default)]
    pub description: Option<String>,
    pub auth_method: String,
    #[serde(default)]
    pub selector: Option<String>,
    pub bind_type: String,
    pub bind_name: String,
    #[serde(default)]
    pub bind_vars: Option<HashMap<String, String>>,
}

/// ACL Replication status
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AclReplicationStatus {
    pub enabled: bool,
    pub running: bool,
    pub source_datacenter: String,
    pub replication_type: String,
    pub replicated_index: u64,
    pub replicated_role_index: u64,
    pub replicated_token_index: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_success: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error_message: Option<String>,
}

/// Templated policy definition
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TemplatedPolicy {
    pub template_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Templated policy preview request
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TemplatedPolicyPreviewRequest {
    pub name: String,
}

/// Token update request
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TokenUpdateRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accessor_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policies: Option<Vec<PolicyLink>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roles: Option<Vec<RoleLink>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local: Option<bool>,
}

/// Policy update request
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PolicyUpdateRequest {
    #[serde(rename = "ID", skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub rules: String,
    #[serde(skip_serializing_if = "Option::is_none")]
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
        None => HttpResponse::NotFound().json(AclError::new("Token not found")),
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
        None => HttpResponse::NotFound().json(AclError::new("Policy not found")),
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
        None => HttpResponse::NotFound().json(AclError::new("Policy not found")),
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
        None => HttpResponse::NotFound().json(AclError::new("Role not found")),
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
        None => HttpResponse::NotFound().json(AclError::new("Binding rule not found")),
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
        None => HttpResponse::NotFound().json(AclError::new("Binding rule not found")),
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
        HttpResponse::NotFound().json(AclError::new("Binding rule not found"))
    }
}

/// GET /v1/acl/templated-policies - List templated policies
pub async fn list_templated_policies(
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let mut policies = HashMap::new();
    policies.insert(
        "builtin/service".to_string(),
        TemplatedPolicy {
            template_name: "builtin/service".to_string(),
            schema: Some(
                r#"{"type":"object","properties":{"Name":{"type":"string"}}}"#.to_string(),
            ),
            template: None,
            description: Some(
                "Gives the token or role permissions for a service and its sidecar proxy"
                    .to_string(),
            ),
        },
    );
    policies.insert(
        "builtin/node".to_string(),
        TemplatedPolicy {
            template_name: "builtin/node".to_string(),
            schema: Some(
                r#"{"type":"object","properties":{"Name":{"type":"string"}}}"#.to_string(),
            ),
            template: None,
            description: Some("Gives the token or role permissions for a node".to_string()),
        },
    );
    policies.insert(
        "builtin/dns".to_string(),
        TemplatedPolicy {
            template_name: "builtin/dns".to_string(),
            schema: None,
            template: None,
            description: Some(
                "Gives the token or role permissions for the DNS catalog".to_string(),
            ),
        },
    );
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));
    consul_ok(&meta).json(policies)
}

/// GET /v1/acl/templated-policy/name/{name} - Get templated policy by name
pub async fn get_templated_policy(
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let name = path.into_inner();
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));
    match name.as_str() {
        "builtin/service" => consul_ok(&meta).json(TemplatedPolicy {
            template_name: "builtin/service".to_string(),
            schema: Some(r#"{"type":"object","properties":{"Name":{"type":"string"}}}"#.to_string()),
            template: Some(r#"service "{{.Name}}" { policy = "write" } service "{{.Name}}-sidecar-proxy" { policy = "write" }"#.to_string()),
            description: Some("Gives the token or role permissions for a service and its sidecar proxy".to_string()),
        }),
        "builtin/node" => consul_ok(&meta).json(TemplatedPolicy {
            template_name: "builtin/node".to_string(),
            schema: Some(r#"{"type":"object","properties":{"Name":{"type":"string"}}}"#.to_string()),
            template: Some(r#"node "{{.Name}}" { policy = "write" }"#.to_string()),
            description: Some("Gives the token or role permissions for a node".to_string()),
        }),
        "builtin/dns" => consul_ok(&meta).json(TemplatedPolicy {
            template_name: "builtin/dns".to_string(),
            schema: None,
            template: Some(r#"node_prefix "" { policy = "read" } service_prefix "" { policy = "read" }"#.to_string()),
            description: Some("Gives the token or role permissions for the DNS catalog".to_string()),
        }),
        _ => HttpResponse::BadRequest().json(AclError::new(format!("Unknown templated policy: {}", name))),
    }
}

/// POST /v1/acl/templated-policy/preview/{name} - Preview rendered template
pub async fn preview_templated_policy(
    path: web::Path<String>,
    body: web::Json<TemplatedPolicyPreviewRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let template_name = path.into_inner();
    let var_name = &body.name;

    let rules = match template_name.as_str() {
        "builtin/service" => format!(
            r#"service "{}" {{ policy = "write" }} service "{}-sidecar-proxy" {{ policy = "write" }}"#,
            var_name, var_name
        ),
        "builtin/node" => format!(r#"node "{}" {{ policy = "write" }}"#, var_name),
        "builtin/dns" => {
            r#"node_prefix "" { policy = "read" } service_prefix "" { policy = "read" }"#
                .to_string()
        }
        _ => {
            return HttpResponse::BadRequest().json(AclError::new(format!(
                "Unknown templated policy: {}",
                template_name
            )));
        }
    };

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ACL));
    consul_ok(&meta).json(serde_json::json!({ "Rules": rules }))
}

// ============================================================================
// ACL Authorize Endpoint
// ============================================================================

/// ACL authorization check request
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AclAuthorizationCheck {
    pub resource: String,
    #[serde(default)]
    pub segment: Option<String>,
    pub access: String,
}

/// ACL authorization response
#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct AclAuthorizationResponse {
    pub allow: bool,
    pub resource: String,
    pub access: String,
    #[serde(skip_serializing_if = "String::is_empty")]
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
