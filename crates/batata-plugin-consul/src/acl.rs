// Consul ACL (Access Control List) implementation
// Provides token-based authentication and authorization for Consul API endpoints
// Supports both in-memory storage and persistent storage via ConfigService

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::LazyLock;

use actix_web::{HttpRequest, HttpResponse, web};
use dashmap::DashMap;
use moka::sync::Cache;
use rocksdb::DB;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{error, info, warn};

use batata_consistency::raft::state_machine::CF_CONSUL_ACL;

// ACL Token header name
pub const X_CONSUL_TOKEN: &str = "X-Consul-Token";
pub const CONSUL_TOKEN_QUERY: &str = "token";

// Cache for validated tokens (5 minute TTL)
static TOKEN_CACHE: LazyLock<Cache<String, AclToken>> = LazyLock::new(|| {
    Cache::builder()
        .time_to_live(Duration::from_secs(300))
        .max_capacity(10_000)
        .build()
});

// In-memory token store for standalone mode (without database)
static MEMORY_TOKENS: LazyLock<DashMap<String, AclToken>> = LazyLock::new(DashMap::new);
static MEMORY_POLICIES: LazyLock<DashMap<String, AclPolicy>> = LazyLock::new(DashMap::new);

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
    pub create_time: String,
    pub modify_time: String,
}

/// Policy link in token
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PolicyLink {
    #[serde(rename = "ID")]
    pub id: String,
    pub name: String,
}

/// Role link in token
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct RoleLink {
    #[serde(rename = "ID")]
    pub id: String,
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

// In-memory role store
static MEMORY_ROLES: LazyLock<DashMap<String, AclRole>> = LazyLock::new(DashMap::new);

// In-memory auth method store
static MEMORY_AUTH_METHODS: LazyLock<DashMap<String, AuthMethod>> = LazyLock::new(DashMap::new);

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
/// When `rocks_db` is `Some`, writes are persisted to RocksDB (write-through cache).
#[derive(Clone)]
pub struct AclService {
    enabled: bool,
    default_policy: RulePolicy,
    /// Optional RocksDB handle for persistence
    rocks_db: Option<Arc<DB>>,
}

impl Default for AclService {
    fn default() -> Self {
        Self::new()
    }
}

impl AclService {
    pub fn new() -> Self {
        // Initialize with bootstrap token and global-management policy
        Self::init_bootstrap();

        Self {
            enabled: true,
            default_policy: RulePolicy::Deny,
            rocks_db: None,
        }
    }

    pub fn disabled() -> Self {
        Self {
            enabled: false,
            default_policy: RulePolicy::Write,
            rocks_db: None,
        }
    }

    /// Create an enabled ACL service with RocksDB persistence.
    /// Loads tokens/policies/roles/auth_methods from RocksDB into the static DashMaps.
    pub fn with_rocks(db: Arc<DB>) -> Self {
        // Load from RocksDB first, before init_bootstrap
        let mut loaded_bootstrap = false;
        if let Some(cf) = db.cf_handle(CF_CONSUL_ACL) {
            let iter = db.iterator_cf(cf, rocksdb::IteratorMode::Start);
            let mut token_count = 0u64;
            let mut policy_count = 0u64;
            let mut role_count = 0u64;
            let mut auth_method_count = 0u64;

            for item in iter.flatten() {
                let (key_bytes, value_bytes) = item;
                if let Ok(key) = String::from_utf8(key_bytes.to_vec()) {
                    if let Some(secret_id) = key.strip_prefix("token::") {
                        if let Ok(token) = serde_json::from_slice::<AclToken>(&value_bytes) {
                            if secret_id == "root" {
                                loaded_bootstrap = true;
                            }
                            MEMORY_TOKENS.insert(secret_id.to_string(), token);
                            token_count += 1;
                        } else {
                            warn!("Failed to deserialize ACL token: {}", key);
                        }
                    } else if let Some(id) = key.strip_prefix("policy::") {
                        if let Ok(policy) = serde_json::from_slice::<AclPolicy>(&value_bytes) {
                            MEMORY_POLICIES.insert(id.to_string(), policy.clone());
                            MEMORY_POLICIES.insert(policy.name.clone(), policy);
                            policy_count += 1;
                        } else {
                            warn!("Failed to deserialize ACL policy: {}", key);
                        }
                    } else if let Some(id) = key.strip_prefix("role::") {
                        if let Ok(role) = serde_json::from_slice::<AclRole>(&value_bytes) {
                            MEMORY_ROLES.insert(id.to_string(), role.clone());
                            MEMORY_ROLES.insert(role.name.clone(), role);
                            role_count += 1;
                        } else {
                            warn!("Failed to deserialize ACL role: {}", key);
                        }
                    } else if let Some(name) = key.strip_prefix("auth_method::") {
                        if let Ok(method) = serde_json::from_slice::<AuthMethod>(&value_bytes) {
                            MEMORY_AUTH_METHODS.insert(name.to_string(), method);
                            auth_method_count += 1;
                        } else {
                            warn!("Failed to deserialize ACL auth method: {}", key);
                        }
                    }
                }
            }
            info!(
                "Loaded ACL data from RocksDB: {} tokens, {} policies, {} roles, {} auth methods",
                token_count, policy_count, role_count, auth_method_count
            );
        }

        // Only create bootstrap token/policy if not loaded from RocksDB
        if !loaded_bootstrap {
            Self::init_bootstrap();
        }

        let svc = Self {
            enabled: true,
            default_policy: RulePolicy::Deny,
            rocks_db: Some(db),
        };

        // Persist the bootstrap data that init_bootstrap() may have created
        if !loaded_bootstrap {
            svc.persist_all_current();
        }

        svc
    }

    /// Persist all current in-memory data to RocksDB (used after init_bootstrap)
    fn persist_all_current(&self) {
        for entry in MEMORY_TOKENS.iter() {
            self.persist_acl("token", entry.key(), entry.value());
        }
        // Policies are stored by ID (not name duplicate)
        let mut seen_ids = std::collections::HashSet::new();
        for entry in MEMORY_POLICIES.iter() {
            if seen_ids.insert(entry.value().id.clone()) {
                self.persist_acl("policy", &entry.value().id, entry.value());
            }
        }
        let mut seen_ids = std::collections::HashSet::new();
        for entry in MEMORY_ROLES.iter() {
            if seen_ids.insert(entry.value().id.clone()) {
                self.persist_acl("role", &entry.value().id, entry.value());
            }
        }
        for entry in MEMORY_AUTH_METHODS.iter() {
            self.persist_acl("auth_method", entry.key(), entry.value());
        }
    }

    /// Persist an ACL entity to RocksDB
    fn persist_acl<T: Serialize>(&self, prefix: &str, id: &str, value: &T) {
        if let Some(ref db) = self.rocks_db
            && let Some(cf) = db.cf_handle(CF_CONSUL_ACL)
        {
            let key = format!("{}::{}", prefix, id);
            match serde_json::to_vec(value) {
                Ok(bytes) => {
                    if let Err(e) = db.put_cf(cf, key.as_bytes(), &bytes) {
                        error!("Failed to persist ACL {} '{}': {}", prefix, id, e);
                    }
                }
                Err(e) => {
                    error!("Failed to serialize ACL {} '{}': {}", prefix, id, e);
                }
            }
        }
    }

    /// Delete an ACL entity from RocksDB
    fn delete_acl(&self, prefix: &str, id: &str) {
        if let Some(ref db) = self.rocks_db
            && let Some(cf) = db.cf_handle(CF_CONSUL_ACL)
        {
            let key = format!("{}::{}", prefix, id);
            if let Err(e) = db.delete_cf(cf, key.as_bytes()) {
                error!(
                    "Failed to delete ACL {} '{}' from RocksDB: {}",
                    prefix, id, e
                );
            }
        }
    }

    fn init_bootstrap() {
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
        MEMORY_POLICIES.insert(mgmt_policy.id.clone(), mgmt_policy.clone());
        MEMORY_POLICIES.insert(mgmt_policy.name.clone(), mgmt_policy);

        // Create bootstrap token
        let bootstrap_token = AclToken {
            accessor_id: "00000000-0000-0000-0000-000000000001".to_string(),
            secret_id: Some("root".to_string()),
            description: "Bootstrap Token (Management)".to_string(),
            policies: vec![PolicyLink {
                id: "00000000-0000-0000-0000-000000000001".to_string(),
                name: "global-management".to_string(),
            }],
            roles: vec![],
            local: false,
            expiration_time: None,
            create_time: chrono::Utc::now().to_rfc3339(),
            modify_time: chrono::Utc::now().to_rfc3339(),
        };
        MEMORY_TOKENS.insert("root".to_string(), bootstrap_token);
    }

    /// Check if ACL is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Extract token from request
    pub fn extract_token(req: &HttpRequest) -> Option<String> {
        // First check header
        if let Some(token) = req.headers().get(X_CONSUL_TOKEN)
            && let Ok(token_str) = token.to_str()
        {
            return Some(token_str.to_string());
        }

        // Then check query parameter
        let query = web::Query::<HashMap<String, String>>::from_query(req.query_string()).ok()?;
        query.get(CONSUL_TOKEN_QUERY).cloned()
    }

    /// Validate and get token
    pub fn get_token(&self, secret_id: &str) -> Option<AclToken> {
        // Check cache first
        if let Some(token) = TOKEN_CACHE.get(secret_id) {
            return Some(token);
        }

        // Check memory store
        if let Some(token) = MEMORY_TOKENS.get(secret_id) {
            let token = token.clone();
            TOKEN_CACHE.insert(secret_id.to_string(), token.clone());
            return Some(token);
        }

        None
    }

    /// Create a new token
    pub fn create_token(
        &self,
        description: &str,
        policies: Vec<String>,
        roles: Vec<String>,
        local: bool,
    ) -> AclToken {
        let now = chrono::Utc::now().to_rfc3339();
        let accessor_id = uuid::Uuid::new_v4().to_string();
        let secret_id = uuid::Uuid::new_v4().to_string();

        let policy_links: Vec<PolicyLink> = policies
            .iter()
            .filter_map(|p| {
                MEMORY_POLICIES.get(p).map(|policy| PolicyLink {
                    id: policy.id.clone(),
                    name: policy.name.clone(),
                })
            })
            .collect();

        let role_links: Vec<RoleLink> = roles
            .iter()
            .filter_map(|r| {
                MEMORY_ROLES.get(r).map(|role| RoleLink {
                    id: role.id.clone(),
                    name: role.name.clone(),
                })
            })
            .collect();

        let token = AclToken {
            accessor_id,
            secret_id: Some(secret_id.clone()),
            description: description.to_string(),
            policies: policy_links,
            roles: role_links,
            local,
            expiration_time: None,
            create_time: now.clone(),
            modify_time: now,
        };

        MEMORY_TOKENS.insert(secret_id.clone(), token.clone());
        self.persist_acl("token", &secret_id, &token);
        token
    }

    /// Delete a token
    pub fn delete_token(&self, accessor_id: &str) -> bool {
        let mut found = false;
        let mut removed_secret_ids = Vec::new();
        MEMORY_TOKENS.retain(|secret_id, token| {
            if token.accessor_id == accessor_id {
                found = true;
                removed_secret_ids.push(secret_id.clone());
                false
            } else {
                true
            }
        });
        for secret_id in &removed_secret_ids {
            self.delete_acl("token", secret_id);
        }
        found
    }

    /// List all tokens
    pub fn list_tokens(&self) -> Vec<AclToken> {
        MEMORY_TOKENS
            .iter()
            .map(|entry| {
                let mut token = entry.value().clone();
                token.secret_id = None; // Don't expose secret_id in list
                token
            })
            .collect()
    }

    /// Create a new policy
    pub fn create_policy(&self, name: &str, description: &str, rules: &str) -> AclPolicy {
        let now = chrono::Utc::now().to_rfc3339();
        let policy_id = uuid::Uuid::new_v4().to_string();

        let policy = AclPolicy {
            id: policy_id,
            name: name.to_string(),
            description: description.to_string(),
            rules: rules.to_string(),
            datacenters: None,
            create_time: now.clone(),
            modify_time: now,
        };

        MEMORY_POLICIES.insert(policy.id.clone(), policy.clone());
        MEMORY_POLICIES.insert(policy.name.clone(), policy.clone());
        self.persist_acl("policy", &policy.id, &policy);
        policy
    }

    /// Delete a policy by ID
    pub fn delete_policy(&self, id: &str) -> bool {
        if let Some((_, policy)) = MEMORY_POLICIES.remove(id) {
            MEMORY_POLICIES.remove(&policy.name);
            self.delete_acl("policy", id);
            true
        } else {
            false
        }
    }

    /// Get a policy by ID or name
    pub fn get_policy(&self, id_or_name: &str) -> Option<AclPolicy> {
        MEMORY_POLICIES.get(id_or_name).map(|p| p.clone())
    }

    /// List all policies
    pub fn list_policies(&self) -> Vec<AclPolicy> {
        let mut seen = std::collections::HashSet::new();
        MEMORY_POLICIES
            .iter()
            .filter_map(|entry| {
                let policy = entry.value();
                if seen.insert(policy.id.clone()) {
                    Some(policy.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Create a new role
    pub fn create_role(&self, name: &str, description: &str, policies: Vec<String>) -> AclRole {
        let now = chrono::Utc::now().to_rfc3339();
        let role_id = uuid::Uuid::new_v4().to_string();

        let policy_links: Vec<PolicyLink> = policies
            .iter()
            .filter_map(|p| {
                MEMORY_POLICIES.get(p).map(|policy| PolicyLink {
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

        MEMORY_ROLES.insert(role.id.clone(), role.clone());
        MEMORY_ROLES.insert(role.name.clone(), role.clone());
        self.persist_acl("role", &role.id, &role);
        role
    }

    /// Get a role by ID or name
    pub fn get_role(&self, id_or_name: &str) -> Option<AclRole> {
        MEMORY_ROLES.get(id_or_name).map(|r| r.clone())
    }

    /// Update a role
    pub fn update_role(
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
            MEMORY_ROLES.remove(&role.name);
            role.name = new_name.to_string();
        }

        if let Some(desc) = description {
            role.description = desc.to_string();
        }

        if let Some(policy_names) = policies {
            let policy_links: Vec<PolicyLink> = policy_names
                .iter()
                .filter_map(|p| {
                    MEMORY_POLICIES.get(p).map(|policy| PolicyLink {
                        id: policy.id.clone(),
                        name: policy.name.clone(),
                    })
                })
                .collect();
            role.policies = policy_links;
        }

        role.modify_time = now;

        MEMORY_ROLES.insert(role.id.clone(), role.clone());
        MEMORY_ROLES.insert(role.name.clone(), role.clone());
        self.persist_acl("role", &role.id, &role);
        Some(role)
    }

    /// Delete a role
    pub fn delete_role(&self, id: &str) -> bool {
        if let Some((_, role)) = MEMORY_ROLES.remove(id) {
            MEMORY_ROLES.remove(&role.name);
            self.delete_acl("role", id);
            true
        } else {
            false
        }
    }

    /// List all roles
    pub fn list_roles(&self) -> Vec<AclRole> {
        let mut seen = std::collections::HashSet::new();
        MEMORY_ROLES
            .iter()
            .filter_map(|entry| {
                let role = entry.value();
                if seen.insert(role.id.clone()) {
                    Some(role.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Create a new auth method
    #[allow(clippy::too_many_arguments)]
    pub fn create_auth_method(
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

        MEMORY_AUTH_METHODS.insert(name.to_string(), method.clone());
        self.persist_acl("auth_method", name, &method);
        method
    }

    /// Get an auth method by name
    pub fn get_auth_method(&self, name: &str) -> Option<AuthMethod> {
        MEMORY_AUTH_METHODS.get(name).map(|m| m.clone())
    }

    /// Update an auth method
    #[allow(clippy::too_many_arguments)]
    pub fn update_auth_method(
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

        let mut method = MEMORY_AUTH_METHODS.get_mut(name)?;
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

        let updated = method.clone();
        drop(method);
        self.persist_acl("auth_method", name, &updated);
        Some(updated)
    }

    /// Delete an auth method
    pub fn delete_auth_method(&self, name: &str) -> bool {
        let removed = MEMORY_AUTH_METHODS.remove(name).is_some();
        if removed {
            self.delete_acl("auth_method", name);
        }
        removed
    }

    /// List all auth methods
    pub fn list_auth_methods(&self) -> Vec<AuthMethod> {
        MEMORY_AUTH_METHODS
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
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

        // Get all rules from token's policies
        let mut all_rules = ParsedRules::default();
        for policy_link in &token.policies {
            if let Some(policy) = self.get_policy(&policy_link.id) {
                let parsed = self.parse_rules(&policy.rules);
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
pub async fn list_tokens(acl_service: web::Data<AclService>) -> HttpResponse {
    let tokens = acl_service.list_tokens();
    HttpResponse::Ok().json(tokens)
}

/// GET /v1/acl/token/{accessor_id}
pub async fn get_token(
    _acl_service: web::Data<AclService>,
    path: web::Path<String>,
) -> HttpResponse {
    let accessor_id = path.into_inner();

    // Find token by accessor_id
    let token = MEMORY_TOKENS.iter().find_map(|entry| {
        if entry.value().accessor_id == accessor_id {
            Some(entry.value().clone())
        } else {
            None
        }
    });

    match token {
        Some(t) => HttpResponse::Ok().json(t),
        None => HttpResponse::NotFound().json(serde_json::Value::Null),
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
}

/// PUT /v1/acl/token
/// Create a new token
pub async fn create_token(
    acl_service: web::Data<AclService>,
    body: web::Json<CreateTokenRequest>,
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

    let token = acl_service.create_token(
        body.description.as_deref().unwrap_or(""),
        policies,
        roles,
        local,
    );

    HttpResponse::Ok().json(token)
}

/// DELETE /v1/acl/token/{accessor_id}
pub async fn delete_token(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
) -> HttpResponse {
    let accessor_id = path.into_inner();

    if acl_service.delete_token(&accessor_id) {
        HttpResponse::Ok().json(true)
    } else {
        HttpResponse::NotFound().json(AclError::new("Token not found"))
    }
}

/// GET /v1/acl/token/self
/// Returns the token associated with the current request
pub async fn get_token_self(acl_service: web::Data<AclService>, req: HttpRequest) -> HttpResponse {
    let secret_id = match AclService::extract_token(&req) {
        Some(t) => t,
        None => return HttpResponse::Forbidden().json(AclError::new("ACL token required")),
    };

    match acl_service.get_token(&secret_id) {
        Some(token) => HttpResponse::Ok().json(token),
        None => HttpResponse::Forbidden().json(AclError::new("ACL token not found or invalid")),
    }
}

/// PUT /v1/acl/token/{accessor_id}/clone
/// Clone an existing token
pub async fn clone_token(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    body: web::Json<CloneTokenRequest>,
) -> HttpResponse {
    let accessor_id = path.into_inner();

    // Find the token to clone
    let source_token = MEMORY_TOKENS.iter().find_map(|entry| {
        if entry.value().accessor_id == accessor_id {
            Some(entry.value().clone())
        } else {
            None
        }
    });

    match source_token {
        Some(source) => {
            // Create a new token with the same policies
            let policies: Vec<String> = source.policies.iter().map(|p| p.name.clone()).collect();
            let description = body
                .description
                .clone()
                .unwrap_or_else(|| format!("Clone of {}", source.description));
            let roles: Vec<String> = source.roles.iter().map(|r| r.name.clone()).collect();
            let new_token = acl_service.create_token(&description, policies, roles, source.local);
            HttpResponse::Ok().json(new_token)
        }
        None => HttpResponse::NotFound().json(AclError::new("Token not found")),
    }
}

/// PUT /v1/acl/bootstrap
/// Bootstrap the ACL system (creates initial management token)
pub async fn acl_bootstrap(_acl_service: web::Data<AclService>) -> HttpResponse {
    // Check if already bootstrapped
    if let Some(existing) = MEMORY_TOKENS.get("root") {
        // If bootstrap token exists, return it (for development/testing)
        // In production, this should return an error if already bootstrapped
        let token = existing.clone();
        let response = BootstrapResponse {
            id: token.accessor_id.clone(),
            accessor_id: token.accessor_id,
            secret_id: token.secret_id.unwrap_or_default(),
            description: token.description,
            policies: token.policies,
            local: token.local,
            create_time: token.create_time,
            hash: "bootstrap".to_string(),
        };
        return HttpResponse::Ok().json(response);
    }

    // Re-initialize bootstrap (this will create the token)
    AclService::init_bootstrap();

    if let Some(token) = MEMORY_TOKENS.get("root") {
        let token = token.clone();
        let response = BootstrapResponse {
            id: token.accessor_id.clone(),
            accessor_id: token.accessor_id,
            secret_id: token.secret_id.unwrap_or_default(),
            description: token.description,
            policies: token.policies,
            local: token.local,
            create_time: token.create_time,
            hash: "bootstrap".to_string(),
        };
        HttpResponse::Ok().json(response)
    } else {
        HttpResponse::InternalServerError().json(AclError::new("Failed to bootstrap ACL"))
    }
}

/// POST /v1/acl/login
/// Login with an auth method to get a token
pub async fn acl_login(
    acl_service: web::Data<AclService>,
    body: web::Json<LoginRequest>,
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
        create_time: now.clone(),
        modify_time: now.clone(),
    };

    MEMORY_TOKENS.insert(secret_id.clone(), token);

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

    HttpResponse::Ok().json(response)
}

/// POST /v1/acl/logout
/// Logout and invalidate the current token
pub async fn acl_logout(acl_service: web::Data<AclService>, req: HttpRequest) -> HttpResponse {
    let secret_id = match AclService::extract_token(&req) {
        Some(t) => t,
        None => return HttpResponse::Forbidden().json(AclError::new("ACL token required")),
    };

    // Don't allow logging out the bootstrap token
    if secret_id == "root" {
        return HttpResponse::Forbidden().json(AclError::new("Cannot logout bootstrap token"));
    }

    // Find and delete the token
    if let Some(token) = acl_service.get_token(&secret_id)
        && acl_service.delete_token(&token.accessor_id)
    {
        return HttpResponse::Ok().json(true);
    }

    // Also try direct removal from memory
    if MEMORY_TOKENS.remove(&secret_id).is_some() {
        HttpResponse::Ok().json(true)
    } else {
        HttpResponse::NotFound().json(AclError::new("Token not found"))
    }
}

/// Helper function to parse duration strings like "1h", "30m", "24h"
fn parse_duration(s: &str) -> Option<std::time::Duration> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    let (num_str, unit) = if s.ends_with('h') || s.ends_with('H') {
        (&s[..s.len() - 1], 'h')
    } else if s.ends_with('m') || s.ends_with('M') {
        (&s[..s.len() - 1], 'm')
    } else if s.ends_with('s') || s.ends_with('S') {
        (&s[..s.len() - 1], 's')
    } else {
        // Default to seconds
        (s, 's')
    };

    let num: u64 = num_str.parse().ok()?;
    let secs = match unit {
        'h' => num * 3600,
        'm' => num * 60,
        's' => num,
        _ => num,
    };

    Some(std::time::Duration::from_secs(secs))
}

/// GET /v1/acl/policies
/// List all policies
pub async fn list_policies(acl_service: web::Data<AclService>) -> HttpResponse {
    let policies = acl_service.list_policies();
    HttpResponse::Ok().json(policies)
}

/// GET /v1/acl/policy/{id}
pub async fn get_policy(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
) -> HttpResponse {
    let id = path.into_inner();

    match acl_service.get_policy(&id) {
        Some(policy) => HttpResponse::Ok().json(policy),
        None => HttpResponse::NotFound().json(AclError::new("Policy not found")),
    }
}

/// DELETE /v1/acl/policy/{id}
pub async fn delete_policy(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
) -> HttpResponse {
    let id = path.into_inner();

    if acl_service.delete_policy(&id) {
        HttpResponse::Ok().json(true)
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
}

/// PUT /v1/acl/policy
/// Create a new policy
pub async fn create_policy(
    acl_service: web::Data<AclService>,
    body: web::Json<CreatePolicyRequest>,
) -> HttpResponse {
    let policy = acl_service.create_policy(
        &body.name,
        body.description.as_deref().unwrap_or(""),
        &body.rules,
    );

    HttpResponse::Ok().json(policy)
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
pub async fn list_roles(acl_service: web::Data<AclService>) -> HttpResponse {
    let roles = acl_service.list_roles();
    HttpResponse::Ok().json(roles)
}

/// PUT /v1/acl/role
/// Create a new role
pub async fn create_role(
    acl_service: web::Data<AclService>,
    body: web::Json<RoleRequest>,
) -> HttpResponse {
    let policies: Vec<String> = body
        .policies
        .as_ref()
        .map(|p| p.iter().map(|pl| pl.name.clone()).collect())
        .unwrap_or_default();

    let role = acl_service.create_role(
        &body.name,
        body.description.as_deref().unwrap_or(""),
        policies,
    );

    HttpResponse::Ok().json(role)
}

/// GET /v1/acl/role/{id}
/// Get a role by ID or name
pub async fn get_role(acl_service: web::Data<AclService>, path: web::Path<String>) -> HttpResponse {
    let id = path.into_inner();

    match acl_service.get_role(&id) {
        Some(role) => HttpResponse::Ok().json(role),
        None => HttpResponse::NotFound().json(AclError::new("Role not found")),
    }
}

/// PUT /v1/acl/role/{id}
/// Update an existing role
pub async fn update_role(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    body: web::Json<RoleRequest>,
) -> HttpResponse {
    let id = path.into_inner();

    let policies: Option<Vec<String>> = body
        .policies
        .as_ref()
        .map(|p| p.iter().map(|pl| pl.name.clone()).collect());

    match acl_service.update_role(&id, Some(&body.name), body.description.as_deref(), policies) {
        Some(role) => HttpResponse::Ok().json(role),
        None => HttpResponse::NotFound().json(AclError::new("Role not found")),
    }
}

/// DELETE /v1/acl/role/{id}
/// Delete a role
pub async fn delete_role(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
) -> HttpResponse {
    let id = path.into_inner();

    if acl_service.delete_role(&id) {
        HttpResponse::Ok().json(true)
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
pub async fn list_auth_methods(acl_service: web::Data<AclService>) -> HttpResponse {
    let methods = acl_service.list_auth_methods();
    HttpResponse::Ok().json(methods)
}

/// PUT /v1/acl/auth-method
/// Create a new auth method
pub async fn create_auth_method(
    acl_service: web::Data<AclService>,
    body: web::Json<AuthMethodRequest>,
) -> HttpResponse {
    let method = acl_service.create_auth_method(
        &body.name,
        &body.method_type,
        body.display_name.as_deref(),
        body.description.as_deref(),
        body.max_token_ttl.as_deref(),
        body.token_locality.as_deref(),
        body.config.clone(),
    );

    HttpResponse::Ok().json(method)
}

/// GET /v1/acl/auth-method/{name}
/// Get an auth method by name
pub async fn get_auth_method(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
) -> HttpResponse {
    let name = path.into_inner();

    match acl_service.get_auth_method(&name) {
        Some(method) => HttpResponse::Ok().json(method),
        None => HttpResponse::NotFound().json(AclError::new("Auth method not found")),
    }
}

/// PUT /v1/acl/auth-method/{name}
/// Update an existing auth method
pub async fn update_auth_method(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    body: web::Json<AuthMethodRequest>,
) -> HttpResponse {
    let name = path.into_inner();

    match acl_service.update_auth_method(
        &name,
        Some(&body.method_type),
        body.display_name.as_deref(),
        body.description.as_deref(),
        body.max_token_ttl.as_deref(),
        body.token_locality.as_deref(),
        body.config.clone(),
    ) {
        Some(method) => HttpResponse::Ok().json(method),
        None => HttpResponse::NotFound().json(AclError::new("Auth method not found")),
    }
}

/// DELETE /v1/acl/auth-method/{name}
/// Delete an auth method
pub async fn delete_auth_method(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
) -> HttpResponse {
    let name = path.into_inner();

    if acl_service.delete_auth_method(&name) {
        HttpResponse::Ok().json(true)
    } else {
        HttpResponse::NotFound().json(AclError::new("Auth method not found"))
    }
}

// ============================================================================
// In-memory stores for binding rules and templated policies
// ============================================================================
static MEMORY_BINDING_RULES: LazyLock<DashMap<String, BindingRule>> = LazyLock::new(DashMap::new);

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
        // Find token by accessor_id
        let secret_key = {
            let mut found_key = None;
            for entry in MEMORY_TOKENS.iter() {
                if entry.value().accessor_id == accessor_id {
                    found_key = Some(entry.key().clone());
                    break;
                }
            }
            found_key?
        };

        let mut token = MEMORY_TOKENS.get(&secret_key)?.value().clone();

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

        MEMORY_TOKENS.insert(secret_key, token.clone());
        Some(token)
    }

    /// Update an existing policy
    pub fn update_policy(&self, id: &str, update: PolicyUpdateRequest) -> Option<AclPolicy> {
        let now = chrono::Utc::now().to_rfc3339();
        let mut policy = MEMORY_POLICIES.get(id)?.value().clone();

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
            MEMORY_POLICIES.remove(&old_name);
            MEMORY_POLICIES.insert(policy.name.clone(), policy.clone());
        }
        MEMORY_POLICIES.insert(policy.id.clone(), policy.clone());
        Some(policy)
    }

    /// Create a binding rule
    pub fn create_binding_rule(&self, req: BindingRuleRequest) -> BindingRule {
        let id = uuid::Uuid::new_v4().to_string();
        let rule = BindingRule {
            id: id.clone(),
            description: req.description.unwrap_or_default(),
            auth_method: req.auth_method,
            selector: req.selector,
            bind_type: req.bind_type,
            bind_name: req.bind_name,
            bind_vars: req.bind_vars,
            create_index: 1,
            modify_index: 1,
        };
        MEMORY_BINDING_RULES.insert(id, rule.clone());
        rule
    }

    /// Get a binding rule by ID
    pub fn get_binding_rule(&self, id: &str) -> Option<BindingRule> {
        MEMORY_BINDING_RULES.get(id).map(|r| r.value().clone())
    }

    /// Update a binding rule
    pub fn update_binding_rule(&self, id: &str, req: BindingRuleRequest) -> Option<BindingRule> {
        let mut rule = MEMORY_BINDING_RULES.get(id)?.value().clone();
        if let Some(desc) = req.description {
            rule.description = desc;
        }
        rule.auth_method = req.auth_method;
        rule.selector = req.selector;
        rule.bind_type = req.bind_type;
        rule.bind_name = req.bind_name;
        rule.bind_vars = req.bind_vars;
        rule.modify_index += 1;
        MEMORY_BINDING_RULES.insert(id.to_string(), rule.clone());
        Some(rule)
    }

    /// Delete a binding rule
    pub fn delete_binding_rule(&self, id: &str) -> bool {
        MEMORY_BINDING_RULES.remove(id).is_some()
    }

    /// List binding rules
    pub fn list_binding_rules(&self) -> Vec<BindingRule> {
        MEMORY_BINDING_RULES
            .iter()
            .map(|r| r.value().clone())
            .collect()
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
) -> HttpResponse {
    let accessor_id = path.into_inner();
    match acl_service.update_token(&accessor_id, body.into_inner()) {
        Some(token) => HttpResponse::Ok().json(token),
        None => HttpResponse::NotFound().json(AclError::new("Token not found")),
    }
}

/// PUT /v1/acl/policy/{id} - Update policy
pub async fn update_policy(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    body: web::Json<PolicyUpdateRequest>,
) -> HttpResponse {
    let id = path.into_inner();
    match acl_service.update_policy(&id, body.into_inner()) {
        Some(policy) => HttpResponse::Ok().json(policy),
        None => HttpResponse::NotFound().json(AclError::new("Policy not found")),
    }
}

/// GET /v1/acl/policy/name/{name} - Get policy by name
pub async fn get_policy_by_name(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
) -> HttpResponse {
    let name = path.into_inner();
    match acl_service.get_policy(&name) {
        Some(policy) => HttpResponse::Ok().json(policy),
        None => HttpResponse::NotFound().json(AclError::new("Policy not found")),
    }
}

/// GET /v1/acl/role/name/{name} - Get role by name
pub async fn get_role_by_name(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
) -> HttpResponse {
    let name = path.into_inner();
    match acl_service.get_role(&name) {
        Some(role) => HttpResponse::Ok().json(role),
        None => HttpResponse::NotFound().json(AclError::new("Role not found")),
    }
}

/// GET /v1/acl/replication - ACL replication status
pub async fn acl_replication() -> HttpResponse {
    HttpResponse::Ok().json(AclReplicationStatus {
        enabled: false,
        running: false,
        source_datacenter: "dc1".to_string(),
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
pub async fn list_binding_rules(acl_service: web::Data<AclService>) -> HttpResponse {
    HttpResponse::Ok().json(acl_service.list_binding_rules())
}

/// PUT /v1/acl/binding-rule - Create binding rule
pub async fn create_binding_rule(
    acl_service: web::Data<AclService>,
    body: web::Json<BindingRuleRequest>,
) -> HttpResponse {
    let rule = acl_service.create_binding_rule(body.into_inner());
    HttpResponse::Ok().json(rule)
}

/// GET /v1/acl/binding-rule/{id} - Get binding rule
pub async fn get_binding_rule(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
) -> HttpResponse {
    let id = path.into_inner();
    match acl_service.get_binding_rule(&id) {
        Some(rule) => HttpResponse::Ok().json(rule),
        None => HttpResponse::NotFound().json(AclError::new("Binding rule not found")),
    }
}

/// PUT /v1/acl/binding-rule/{id} - Update binding rule
pub async fn update_binding_rule(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    body: web::Json<BindingRuleRequest>,
) -> HttpResponse {
    let id = path.into_inner();
    match acl_service.update_binding_rule(&id, body.into_inner()) {
        Some(rule) => HttpResponse::Ok().json(rule),
        None => HttpResponse::NotFound().json(AclError::new("Binding rule not found")),
    }
}

/// DELETE /v1/acl/binding-rule/{id} - Delete binding rule
pub async fn delete_binding_rule(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
) -> HttpResponse {
    let id = path.into_inner();
    if acl_service.delete_binding_rule(&id) {
        HttpResponse::Ok().json(true)
    } else {
        HttpResponse::NotFound().json(AclError::new("Binding rule not found"))
    }
}

/// GET /v1/acl/templated-policies - List templated policies
pub async fn list_templated_policies() -> HttpResponse {
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
    HttpResponse::Ok().json(policies)
}

/// GET /v1/acl/templated-policy/name/{name} - Get templated policy by name
pub async fn get_templated_policy(path: web::Path<String>) -> HttpResponse {
    let name = path.into_inner();
    match name.as_str() {
        "builtin/service" => HttpResponse::Ok().json(TemplatedPolicy {
            template_name: "builtin/service".to_string(),
            schema: Some(r#"{"type":"object","properties":{"Name":{"type":"string"}}}"#.to_string()),
            template: Some(r#"service "{{.Name}}" { policy = "write" } service "{{.Name}}-sidecar-proxy" { policy = "write" }"#.to_string()),
            description: Some("Gives the token or role permissions for a service and its sidecar proxy".to_string()),
        }),
        "builtin/node" => HttpResponse::Ok().json(TemplatedPolicy {
            template_name: "builtin/node".to_string(),
            schema: Some(r#"{"type":"object","properties":{"Name":{"type":"string"}}}"#.to_string()),
            template: Some(r#"node "{{.Name}}" { policy = "write" }"#.to_string()),
            description: Some("Gives the token or role permissions for a node".to_string()),
        }),
        "builtin/dns" => HttpResponse::Ok().json(TemplatedPolicy {
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

    HttpResponse::Ok().json(serde_json::json!({ "Rules": rules }))
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

    HttpResponse::Ok().json(responses)
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
        let token = service.get_token("root");
        assert!(token.is_some());
        let token = token.unwrap();
        assert_eq!(token.description, "Bootstrap Token (Management)");
        assert!(!token.policies.is_empty());
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
        let token = service.get_token("root").unwrap();

        // Bootstrap token should have full access
        let result = service.authorize(&token, ResourceType::Service, "any-service", true);
        assert!(result.allowed);

        let result = service.authorize(&token, ResourceType::Key, "any/key", true);
        assert!(result.allowed);
    }

    #[test]
    fn test_create_token() {
        let service = AclService::new();
        let token = service.create_token(
            "Test token",
            vec!["global-management".to_string()],
            vec![],
            false,
        );

        assert!(!token.accessor_id.is_empty());
        assert!(token.secret_id.is_some());
        assert_eq!(token.description, "Test token");
        assert!(!token.policies.is_empty());
    }

    #[test]
    fn test_create_policy() {
        let service = AclService::new();
        let policy = service.create_policy(
            "test-policy",
            "A test policy",
            r#"service_prefix "test-" { policy = "write" }"#,
        );

        assert!(!policy.id.is_empty());
        assert_eq!(policy.name, "test-policy");

        // Verify we can retrieve it
        let retrieved = service.get_policy("test-policy");
        assert!(retrieved.is_some());
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

    #[test]
    fn test_delete_token() {
        let service = AclService::new();
        let token = service.create_token(
            "to-delete",
            vec!["global-management".to_string()],
            vec![],
            false,
        );
        assert!(service.delete_token(&token.accessor_id));
        // Deleting again should return false
        assert!(!service.delete_token(&token.accessor_id));
    }

    #[test]
    fn test_list_tokens_hides_secret() {
        let service = AclService::new();
        let tokens = service.list_tokens();
        // All tokens in list should have secret_id = None
        for t in &tokens {
            assert!(t.secret_id.is_none());
        }
    }

    #[test]
    fn test_create_and_delete_policy() {
        let service = AclService::new();
        let policy = service.create_policy(
            "acl-test-del-policy",
            "For deletion test",
            r#"key_prefix "" { policy = "read" }"#,
        );

        assert!(service.get_policy(&policy.id).is_some());
        assert!(service.delete_policy(&policy.id));
        assert!(service.get_policy(&policy.id).is_none());
    }

    #[test]
    fn test_delete_nonexistent_policy() {
        let service = AclService::new();
        assert!(!service.delete_policy("nonexistent-policy-id"));
    }

    #[test]
    fn test_role_crud() {
        let service = AclService::new();

        // Create policy first (roles link to policies)
        let policy = service.create_policy(
            "acl-role-test-policy",
            "Test",
            r#"service_prefix "" { policy = "read" }"#,
        );

        // Create role
        let role = service.create_role("acl-test-role", "Test role", vec![policy.name.clone()]);
        assert_eq!(role.name, "acl-test-role");
        assert!(!role.policies.is_empty());

        // Get by ID
        let fetched = service.get_role(&role.id);
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().name, "acl-test-role");

        // Get by name
        let by_name = service.get_role("acl-test-role");
        assert!(by_name.is_some());

        // Update
        let updated = service.update_role(
            &role.id,
            Some("acl-renamed-role"),
            Some("Updated desc"),
            None,
        );
        assert!(updated.is_some());
        assert_eq!(updated.unwrap().name, "acl-renamed-role");

        // Delete
        assert!(service.delete_role(&role.id));
        assert!(service.get_role(&role.id).is_none());
    }

    #[test]
    fn test_delete_nonexistent_role() {
        let service = AclService::new();
        assert!(!service.delete_role("nonexistent-role-id"));
    }

    #[test]
    fn test_auth_method_crud() {
        let service = AclService::new();

        let method = service.create_auth_method(
            "acl-test-kubernetes",
            "kubernetes",
            Some("K8s Auth"),
            Some("Kubernetes auth method"),
            Some("5m"),
            Some("local"),
            None,
        );
        assert_eq!(method.name, "acl-test-kubernetes");
        assert_eq!(method.method_type, "kubernetes");

        // Get
        let fetched = service.get_auth_method("acl-test-kubernetes");
        assert!(fetched.is_some());

        // Get nonexistent
        assert!(service.get_auth_method("nonexistent-method").is_none());
    }

    #[test]
    fn test_authorize_with_custom_policy() {
        let service = AclService::new();

        // Create a read-only policy for services
        let policy = service.create_policy(
            "acl-test-readonly-svc",
            "Read only",
            r#"service_prefix "" { policy = "read" }"#,
        );

        // Create token with this policy
        let token =
            service.create_token("readonly-token", vec![policy.name.clone()], vec![], false);

        // Read should be allowed
        let result = service.authorize(&token, ResourceType::Service, "web", false);
        assert!(result.allowed);

        // Write should be denied
        let result = service.authorize(&token, ResourceType::Service, "web", true);
        assert!(!result.allowed);
    }

    #[test]
    fn test_authorize_deny_policy() {
        let service = AclService::new();

        let policy = service.create_policy(
            "acl-test-deny-policy",
            "Deny",
            r#"key_prefix "secret/" { policy = "deny" }"#,
        );

        let token = service.create_token("deny-token", vec![policy.name.clone()], vec![], false);

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
        let token = service.get_token("root").unwrap();

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
}
