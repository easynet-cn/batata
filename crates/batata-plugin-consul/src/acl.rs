// Consul ACL (Access Control List) implementation
// Provides token-based authentication and authorization for Consul API endpoints
// Supports both in-memory storage and persistent storage via ConfigService

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::LazyLock;

use actix_web::{HttpRequest, HttpResponse, web};
use dashmap::DashMap;
use moka::sync::Cache;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use std::time::Duration;

// Constants for ConfigService mapping
const CONSUL_ACL_NAMESPACE: &str = "public";
const CONSUL_ACL_GROUP: &str = "consul-acl";

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

/// ACL Service for managing tokens and policies
#[derive(Clone)]
pub struct AclService {
    enabled: bool,
    default_policy: RulePolicy,
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
        }
    }

    pub fn disabled() -> Self {
        Self {
            enabled: false,
            default_policy: RulePolicy::Write,
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

        MEMORY_TOKENS.insert(secret_id, token.clone());
        token
    }

    /// Delete a token
    pub fn delete_token(&self, accessor_id: &str) -> bool {
        let mut found = false;
        MEMORY_TOKENS.retain(|_, token| {
            if token.accessor_id == accessor_id {
                found = true;
                false
            } else {
                true
            }
        });
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
        policy
    }

    /// Delete a policy by ID
    pub fn delete_policy(&self, id: &str) -> bool {
        if let Some((_, policy)) = MEMORY_POLICIES.remove(id) {
            MEMORY_POLICIES.remove(&policy.name);
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
        Some(role)
    }

    /// Delete a role
    pub fn delete_role(&self, id: &str) -> bool {
        if let Some((_, role)) = MEMORY_ROLES.remove(id) {
            MEMORY_ROLES.remove(&role.name);
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

        Some(method.clone())
    }

    /// Delete an auth method
    pub fn delete_auth_method(&self, name: &str) -> bool {
        MEMORY_AUTH_METHODS.remove(name).is_some()
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
            ResourceType::Agent => &all_rules.agent_rules,
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
// Persistent ACL Service (Using ConfigService)
// ============================================================================

/// Stored ACL data metadata for persistent storage
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AclTokenMetadata {
    token: AclToken,
    secret_id: String,
}

/// ACL Service with database persistence
/// Uses Batata's ConfigService for storage
#[derive(Clone)]
pub struct AclServicePersistent {
    db: Arc<DatabaseConnection>,
    enabled: bool,
    default_policy: RulePolicy,
    /// In-memory cache for tokens
    token_cache: Arc<DashMap<String, AclToken>>,
    /// In-memory cache for policies
    policy_cache: Arc<DashMap<String, AclPolicy>>,
    /// In-memory cache for roles
    role_cache: Arc<DashMap<String, AclRole>>,
    /// In-memory cache for auth methods
    auth_method_cache: Arc<DashMap<String, AuthMethod>>,
}

impl AclServicePersistent {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self {
            db,
            enabled: true,
            default_policy: RulePolicy::Deny,
            token_cache: Arc::new(DashMap::new()),
            policy_cache: Arc::new(DashMap::new()),
            role_cache: Arc::new(DashMap::new()),
            auth_method_cache: Arc::new(DashMap::new()),
        }
    }

    pub fn disabled(db: Arc<DatabaseConnection>) -> Self {
        Self {
            db,
            enabled: false,
            default_policy: RulePolicy::Write,
            token_cache: Arc::new(DashMap::new()),
            policy_cache: Arc::new(DashMap::new()),
            role_cache: Arc::new(DashMap::new()),
            auth_method_cache: Arc::new(DashMap::new()),
        }
    }

    /// Initialize bootstrap token and global-management policy
    pub async fn init_bootstrap(&self) -> Result<(), String> {
        // Check if bootstrap already exists
        if self.get_policy("global-management").await.is_some() {
            return Ok(());
        }

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
        self.save_policy(&mgmt_policy).await?;

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
        self.save_token(&bootstrap_token, "root").await?;

        Ok(())
    }

    /// Check if ACL is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    // ========== Token Operations ==========

    fn token_data_id(secret_id: &str) -> String {
        format!("token:{}", secret_id.replace('/', ":"))
    }

    async fn save_token(&self, token: &AclToken, secret_id: &str) -> Result<(), String> {
        let metadata = AclTokenMetadata {
            token: token.clone(),
            secret_id: secret_id.to_string(),
        };
        let content =
            serde_json::to_string(&metadata).map_err(|e| format!("Serialization error: {}", e))?;
        let data_id = Self::token_data_id(secret_id);

        batata_config::service::config::create_or_update(
            &self.db,
            &data_id,
            CONSUL_ACL_GROUP,
            CONSUL_ACL_NAMESPACE,
            &content,
            "consul-acl",
            "system",
            "127.0.0.1",
            "",
            &format!("Consul ACL Token: {}", token.accessor_id),
            "",
            "",
            "json",
            "",
            "",
        )
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        // Update cache
        self.token_cache
            .insert(secret_id.to_string(), token.clone());
        Ok(())
    }

    /// Get token by secret_id
    pub async fn get_token(&self, secret_id: &str) -> Option<AclToken> {
        // Check cache first
        if let Some(token) = self.token_cache.get(secret_id) {
            return Some(token.clone());
        }

        // Query from database
        let data_id = Self::token_data_id(secret_id);
        match batata_config::service::config::find_one(
            &self.db,
            &data_id,
            CONSUL_ACL_GROUP,
            CONSUL_ACL_NAMESPACE,
        )
        .await
        {
            Ok(Some(config)) => {
                if let Ok(metadata) = serde_json::from_str::<AclTokenMetadata>(
                    &config.config_info.config_info_base.content,
                ) {
                    self.token_cache
                        .insert(secret_id.to_string(), metadata.token.clone());
                    Some(metadata.token)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Create a new token
    pub async fn create_token(
        &self,
        description: &str,
        policies: Vec<String>,
    ) -> Result<AclToken, String> {
        self.create_token_with_roles(description, policies, vec![], false)
            .await
    }

    /// Create a new token with roles
    pub async fn create_token_with_roles(
        &self,
        description: &str,
        policies: Vec<String>,
        roles: Vec<String>,
        local: bool,
    ) -> Result<AclToken, String> {
        let now = chrono::Utc::now().to_rfc3339();
        let accessor_id = uuid::Uuid::new_v4().to_string();
        let secret_id = uuid::Uuid::new_v4().to_string();

        let mut policy_links = Vec::new();
        for p in &policies {
            if let Some(policy) = self.get_policy(p).await {
                policy_links.push(PolicyLink {
                    id: policy.id.clone(),
                    name: policy.name.clone(),
                });
            }
        }

        let mut role_links = Vec::new();
        for r in &roles {
            if let Some(role) = self.get_role(r).await {
                role_links.push(RoleLink {
                    id: role.id.clone(),
                    name: role.name.clone(),
                });
            }
        }

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

        self.save_token(&token, &secret_id).await?;
        Ok(token)
    }

    /// Delete a token by accessor_id
    pub async fn delete_token(&self, accessor_id: &str) -> bool {
        // Find and delete token
        let tokens = self.list_tokens().await;
        for token in tokens {
            if token.accessor_id == accessor_id
                && let Some(secret) = &token.secret_id
            {
                let data_id = Self::token_data_id(secret);
                self.token_cache.remove(secret);
                let _ = batata_config::service::config::delete(
                    &self.db,
                    &data_id,
                    CONSUL_ACL_GROUP,
                    CONSUL_ACL_NAMESPACE,
                    "",
                    "127.0.0.1",
                    "system",
                )
                .await;
                return true;
            }
        }
        false
    }

    /// List all tokens
    pub async fn list_tokens(&self) -> Vec<AclToken> {
        match batata_config::service::config::search_page(
            &self.db,
            1,
            1000,
            CONSUL_ACL_NAMESPACE,
            "token:*",
            CONSUL_ACL_GROUP,
            "",
            vec![],
            vec![],
            "",
        )
        .await
        {
            Ok(page) => {
                let mut tokens = Vec::new();
                for info in page.page_items {
                    if let Ok(Some(config)) = batata_config::service::config::find_one(
                        &self.db,
                        &info.data_id,
                        CONSUL_ACL_GROUP,
                        CONSUL_ACL_NAMESPACE,
                    )
                    .await
                        && let Ok(metadata) = serde_json::from_str::<AclTokenMetadata>(
                            &config.config_info.config_info_base.content,
                        )
                    {
                        let mut token = metadata.token;
                        token.secret_id = None; // Don't expose secret in list
                        tokens.push(token);
                    }
                }
                tokens
            }
            Err(_) => Vec::new(),
        }
    }

    // ========== Policy Operations ==========

    fn policy_data_id(id: &str) -> String {
        format!("policy:{}", id.replace('/', ":"))
    }

    async fn save_policy(&self, policy: &AclPolicy) -> Result<(), String> {
        let content =
            serde_json::to_string(policy).map_err(|e| format!("Serialization error: {}", e))?;
        let data_id = Self::policy_data_id(&policy.id);

        batata_config::service::config::create_or_update(
            &self.db,
            &data_id,
            CONSUL_ACL_GROUP,
            CONSUL_ACL_NAMESPACE,
            &content,
            "consul-acl",
            "system",
            "127.0.0.1",
            "",
            &format!("Consul ACL Policy: {}", policy.name),
            "",
            "",
            "json",
            "",
            "",
        )
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        // Update cache (by both id and name)
        self.policy_cache.insert(policy.id.clone(), policy.clone());
        self.policy_cache
            .insert(policy.name.clone(), policy.clone());
        Ok(())
    }

    /// Get a policy by ID or name
    pub async fn get_policy(&self, id_or_name: &str) -> Option<AclPolicy> {
        // Check cache first
        if let Some(policy) = self.policy_cache.get(id_or_name) {
            return Some(policy.clone());
        }

        // Try by ID
        let data_id = Self::policy_data_id(id_or_name);
        if let Ok(Some(config)) = batata_config::service::config::find_one(
            &self.db,
            &data_id,
            CONSUL_ACL_GROUP,
            CONSUL_ACL_NAMESPACE,
        )
        .await
            && let Ok(policy) =
                serde_json::from_str::<AclPolicy>(&config.config_info.config_info_base.content)
        {
            self.policy_cache.insert(policy.id.clone(), policy.clone());
            self.policy_cache
                .insert(policy.name.clone(), policy.clone());
            return Some(policy);
        }

        // Search by name
        let policies = self.list_policies().await;
        policies
            .into_iter()
            .find(|p| p.name == id_or_name || p.id == id_or_name)
    }

    /// Create a new policy
    pub async fn create_policy(
        &self,
        name: &str,
        description: &str,
        rules: &str,
    ) -> Result<AclPolicy, String> {
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

        self.save_policy(&policy).await?;
        Ok(policy)
    }

    /// Delete a policy by ID
    pub async fn delete_policy(&self, id: &str) -> bool {
        let data_id = format!("policy:{}", id);
        match batata_config::service::config::delete(
            &self.db,
            &data_id,
            CONSUL_ACL_GROUP,
            CONSUL_ACL_NAMESPACE,
            "",
            "",
            "",
        )
        .await
        {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    /// List all policies
    pub async fn list_policies(&self) -> Vec<AclPolicy> {
        match batata_config::service::config::search_page(
            &self.db,
            1,
            1000,
            CONSUL_ACL_NAMESPACE,
            "policy:*",
            CONSUL_ACL_GROUP,
            "",
            vec![],
            vec![],
            "",
        )
        .await
        {
            Ok(page) => {
                let mut policies = Vec::new();
                for info in page.page_items {
                    if let Ok(Some(config)) = batata_config::service::config::find_one(
                        &self.db,
                        &info.data_id,
                        CONSUL_ACL_GROUP,
                        CONSUL_ACL_NAMESPACE,
                    )
                    .await
                        && let Ok(policy) = serde_json::from_str::<AclPolicy>(
                            &config.config_info.config_info_base.content,
                        )
                    {
                        policies.push(policy);
                    }
                }
                policies
            }
            Err(_) => Vec::new(),
        }
    }

    // ========== Role Operations ==========

    fn role_data_id(id: &str) -> String {
        format!("role:{}", id.replace('/', ":"))
    }

    async fn save_role(&self, role: &AclRole) -> Result<(), String> {
        let content =
            serde_json::to_string(role).map_err(|e| format!("Serialization error: {}", e))?;
        let data_id = Self::role_data_id(&role.id);

        batata_config::service::config::create_or_update(
            &self.db,
            &data_id,
            CONSUL_ACL_GROUP,
            CONSUL_ACL_NAMESPACE,
            &content,
            "consul-acl",
            "system",
            "127.0.0.1",
            "",
            &format!("Consul ACL Role: {}", role.name),
            "",
            "",
            "json",
            "",
            "",
        )
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        // Update cache
        self.role_cache.insert(role.id.clone(), role.clone());
        self.role_cache.insert(role.name.clone(), role.clone());
        Ok(())
    }

    /// Create a new role
    pub async fn create_role(
        &self,
        name: &str,
        description: &str,
        policies: Vec<String>,
    ) -> Result<AclRole, String> {
        let now = chrono::Utc::now().to_rfc3339();
        let role_id = uuid::Uuid::new_v4().to_string();

        let mut policy_links = Vec::new();
        for p in &policies {
            if let Some(policy) = self.get_policy(p).await {
                policy_links.push(PolicyLink {
                    id: policy.id.clone(),
                    name: policy.name.clone(),
                });
            }
        }

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

        self.save_role(&role).await?;
        Ok(role)
    }

    /// Get a role by ID or name
    pub async fn get_role(&self, id_or_name: &str) -> Option<AclRole> {
        // Check cache first
        if let Some(role) = self.role_cache.get(id_or_name) {
            return Some(role.clone());
        }

        // Try by ID
        let data_id = Self::role_data_id(id_or_name);
        if let Ok(Some(config)) = batata_config::service::config::find_one(
            &self.db,
            &data_id,
            CONSUL_ACL_GROUP,
            CONSUL_ACL_NAMESPACE,
        )
        .await
            && let Ok(role) =
                serde_json::from_str::<AclRole>(&config.config_info.config_info_base.content)
        {
            self.role_cache.insert(role.id.clone(), role.clone());
            self.role_cache.insert(role.name.clone(), role.clone());
            return Some(role);
        }

        // Search by name
        let roles = self.list_roles().await;
        roles
            .into_iter()
            .find(|r| r.name == id_or_name || r.id == id_or_name)
    }

    /// Update a role
    pub async fn update_role(
        &self,
        id: &str,
        name: Option<&str>,
        description: Option<&str>,
        policies: Option<Vec<String>>,
    ) -> Option<AclRole> {
        let mut role = self.get_role(id).await?;
        let now = chrono::Utc::now().to_rfc3339();

        // Remove old name from cache if changing
        if let Some(new_name) = name
            && new_name != role.name
        {
            self.role_cache.remove(&role.name);
            role.name = new_name.to_string();
        }

        if let Some(desc) = description {
            role.description = desc.to_string();
        }

        if let Some(policy_names) = policies {
            let mut policy_links = Vec::new();
            for p in &policy_names {
                if let Some(policy) = self.get_policy(p).await {
                    policy_links.push(PolicyLink {
                        id: policy.id.clone(),
                        name: policy.name.clone(),
                    });
                }
            }
            role.policies = policy_links;
        }

        role.modify_time = now;
        self.save_role(&role).await.ok()?;
        Some(role)
    }

    /// Delete a role
    pub async fn delete_role(&self, id: &str) -> bool {
        if let Some(role) = self.get_role(id).await {
            let data_id = Self::role_data_id(&role.id);
            self.role_cache.remove(&role.id);
            self.role_cache.remove(&role.name);
            batata_config::service::config::delete(
                &self.db,
                &data_id,
                CONSUL_ACL_GROUP,
                CONSUL_ACL_NAMESPACE,
                "",
                "127.0.0.1",
                "system",
            )
            .await
            .unwrap_or(false)
        } else {
            false
        }
    }

    /// List all roles
    pub async fn list_roles(&self) -> Vec<AclRole> {
        match batata_config::service::config::search_page(
            &self.db,
            1,
            1000,
            CONSUL_ACL_NAMESPACE,
            "role:*",
            CONSUL_ACL_GROUP,
            "",
            vec![],
            vec![],
            "",
        )
        .await
        {
            Ok(page) => {
                let mut roles = Vec::new();
                for info in page.page_items {
                    if let Ok(Some(config)) = batata_config::service::config::find_one(
                        &self.db,
                        &info.data_id,
                        CONSUL_ACL_GROUP,
                        CONSUL_ACL_NAMESPACE,
                    )
                    .await
                        && let Ok(role) = serde_json::from_str::<AclRole>(
                            &config.config_info.config_info_base.content,
                        )
                    {
                        roles.push(role);
                    }
                }
                roles
            }
            Err(_) => Vec::new(),
        }
    }

    // ========== Auth Method Operations ==========

    fn auth_method_data_id(name: &str) -> String {
        format!("auth-method:{}", name.replace('/', ":"))
    }

    async fn save_auth_method(&self, method: &AuthMethod) -> Result<(), String> {
        let content =
            serde_json::to_string(method).map_err(|e| format!("Serialization error: {}", e))?;
        let data_id = Self::auth_method_data_id(&method.name);

        batata_config::service::config::create_or_update(
            &self.db,
            &data_id,
            CONSUL_ACL_GROUP,
            CONSUL_ACL_NAMESPACE,
            &content,
            "consul-acl",
            "system",
            "127.0.0.1",
            "",
            &format!("Consul ACL Auth Method: {}", method.name),
            "",
            "",
            "json",
            "",
            "",
        )
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        self.auth_method_cache
            .insert(method.name.clone(), method.clone());
        Ok(())
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
    ) -> Result<AuthMethod, String> {
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

        self.save_auth_method(&method).await?;
        Ok(method)
    }

    /// Get an auth method by name
    pub async fn get_auth_method(&self, name: &str) -> Option<AuthMethod> {
        // Check cache first
        if let Some(method) = self.auth_method_cache.get(name) {
            return Some(method.clone());
        }

        // Query from database
        let data_id = Self::auth_method_data_id(name);
        match batata_config::service::config::find_one(
            &self.db,
            &data_id,
            CONSUL_ACL_GROUP,
            CONSUL_ACL_NAMESPACE,
        )
        .await
        {
            Ok(Some(config)) => {
                if let Ok(method) =
                    serde_json::from_str::<AuthMethod>(&config.config_info.config_info_base.content)
                {
                    self.auth_method_cache
                        .insert(name.to_string(), method.clone());
                    Some(method)
                } else {
                    None
                }
            }
            _ => None,
        }
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

        let mut method = self.get_auth_method(name).await?;
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

        self.save_auth_method(&method).await.ok()?;
        Some(method)
    }

    /// Delete an auth method
    pub async fn delete_auth_method(&self, name: &str) -> bool {
        let data_id = Self::auth_method_data_id(name);
        self.auth_method_cache.remove(name);
        batata_config::service::config::delete(
            &self.db,
            &data_id,
            CONSUL_ACL_GROUP,
            CONSUL_ACL_NAMESPACE,
            "",
            "127.0.0.1",
            "system",
        )
        .await
        .unwrap_or(false)
    }

    /// List all auth methods
    pub async fn list_auth_methods(&self) -> Vec<AuthMethod> {
        match batata_config::service::config::search_page(
            &self.db,
            1,
            1000,
            CONSUL_ACL_NAMESPACE,
            "auth-method:*",
            CONSUL_ACL_GROUP,
            "",
            vec![],
            vec![],
            "",
        )
        .await
        {
            Ok(page) => {
                let mut methods = Vec::new();
                for info in page.page_items {
                    if let Ok(Some(config)) = batata_config::service::config::find_one(
                        &self.db,
                        &info.data_id,
                        CONSUL_ACL_GROUP,
                        CONSUL_ACL_NAMESPACE,
                    )
                    .await
                        && let Ok(method) = serde_json::from_str::<AuthMethod>(
                            &config.config_info.config_info_base.content,
                        )
                    {
                        methods.push(method);
                    }
                }
                methods
            }
            Err(_) => Vec::new(),
        }
    }

    // ========== Authorization ==========

    /// Parse policy rules into structured format
    pub fn parse_rules(&self, rules: &str) -> ParsedRules {
        let mut parsed = ParsedRules::default();

        for line in rules.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some((resource_part, policy_part)) = line.split_once('{') {
                let resource_part = resource_part.trim();
                let policy_part = policy_part.trim().trim_end_matches('}').trim();

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
    pub async fn authorize(
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
            if let Some(policy) = self.get_policy(&policy_link.id).await {
                let parsed = self.parse_rules(&policy.rules);
                all_rules.agent_rules.extend(parsed.agent_rules);
                all_rules.key_rules.extend(parsed.key_rules);
                all_rules.node_rules.extend(parsed.node_rules);
                all_rules.service_rules.extend(parsed.service_rules);
                all_rules.session_rules.extend(parsed.session_rules);
                all_rules.query_rules.extend(parsed.query_rules);
            }
        }

        let rules = match resource_type {
            ResourceType::Agent => &all_rules.agent_rules,
            ResourceType::Key => &all_rules.key_rules,
            ResourceType::Node => &all_rules.node_rules,
            ResourceType::Service => &all_rules.service_rules,
            ResourceType::Session => &all_rules.session_rules,
            ResourceType::Query => &all_rules.query_rules,
        };

        let mut best_match: Option<&ResourceRule> = None;
        let mut best_match_len = 0;

        for rule in rules {
            if resource_name.starts_with(&rule.prefix) && rule.prefix.len() >= best_match_len {
                best_match = Some(rule);
                best_match_len = rule.prefix.len();
            }
        }

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
        } else if self.default_policy.allows_write()
            || (!write && self.default_policy.allows_read())
        {
            AuthzResult::allowed()
        } else {
            AuthzResult::denied("Permission denied: no matching ACL rule")
        }
    }

    /// Authorize from request
    pub async fn authorize_request(
        &self,
        req: &HttpRequest,
        resource_type: ResourceType,
        resource_name: &str,
        write: bool,
    ) -> AuthzResult {
        if !self.enabled {
            return AuthzResult::allowed();
        }

        let secret_id = match AclService::extract_token(req) {
            Some(t) => t,
            None => return AuthzResult::denied("ACL token required"),
        };

        let token = match self.get_token(&secret_id).await {
            Some(t) => t,
            None => return AuthzResult::denied("ACL token not found or invalid"),
        };

        self.authorize(&token, resource_type, resource_name, write)
            .await
    }
}

/// ACL error response
#[derive(Serialize)]
struct AclError {
    error: String,
}

impl AclError {
    fn new(msg: &str) -> Self {
        Self {
            error: msg.to_string(),
        }
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
        None => HttpResponse::Ok().json(serde_json::Value::Null),
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
            return HttpResponse::NotFound().json(AclError::new(&format!(
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
    if let Some(token) = acl_service.get_token(&secret_id) {
        if acl_service.delete_token(&token.accessor_id) {
            return HttpResponse::Ok().json(true);
        }
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
// Persistent ACL API Endpoints (Using ConfigService)
// ============================================================================

/// GET /v1/acl/tokens (Persistent)
/// List all tokens with database persistence
pub async fn list_tokens_persistent(acl_service: web::Data<AclServicePersistent>) -> HttpResponse {
    let tokens = acl_service.list_tokens().await;
    HttpResponse::Ok().json(tokens)
}

/// GET /v1/acl/token/{accessor_id} (Persistent)
pub async fn get_token_persistent(
    acl_service: web::Data<AclServicePersistent>,
    path: web::Path<String>,
) -> HttpResponse {
    let accessor_id = path.into_inner();

    // Find token by accessor_id
    let tokens = acl_service.list_tokens().await;
    let token = tokens.into_iter().find(|t| t.accessor_id == accessor_id);

    match token {
        Some(mut t) => {
            // Get full token with secret_id for specific lookup
            if let Some(full_token) = acl_service
                .token_cache
                .iter()
                .find(|entry| entry.value().accessor_id == accessor_id)
            {
                t = full_token.value().clone();
            }
            HttpResponse::Ok().json(t)
        }
        None => HttpResponse::NotFound().json(AclError::new("Token not found")),
    }
}

/// PUT /v1/acl/token (Persistent)
/// Create a new token with database persistence
pub async fn create_token_persistent(
    acl_service: web::Data<AclServicePersistent>,
    body: web::Json<CreateTokenRequest>,
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

    match acl_service
        .create_token_with_roles(
            body.description.as_deref().unwrap_or(""),
            policies,
            roles,
            local,
        )
        .await
    {
        Ok(token) => HttpResponse::Ok().json(token),
        Err(e) => HttpResponse::InternalServerError().json(AclError::new(&e)),
    }
}

/// DELETE /v1/acl/token/{accessor_id} (Persistent)
pub async fn delete_token_persistent(
    acl_service: web::Data<AclServicePersistent>,
    path: web::Path<String>,
) -> HttpResponse {
    let accessor_id = path.into_inner();

    if acl_service.delete_token(&accessor_id).await {
        HttpResponse::Ok().json(true)
    } else {
        HttpResponse::NotFound().json(AclError::new("Token not found"))
    }
}

/// GET /v1/acl/policies (Persistent)
/// List all policies with database persistence
pub async fn list_policies_persistent(
    acl_service: web::Data<AclServicePersistent>,
) -> HttpResponse {
    let policies = acl_service.list_policies().await;
    HttpResponse::Ok().json(policies)
}

/// GET /v1/acl/policy/{id} (Persistent)
pub async fn get_policy_persistent(
    acl_service: web::Data<AclServicePersistent>,
    path: web::Path<String>,
) -> HttpResponse {
    let id = path.into_inner();

    match acl_service.get_policy(&id).await {
        Some(policy) => HttpResponse::Ok().json(policy),
        None => HttpResponse::NotFound().json(AclError::new("Policy not found")),
    }
}

/// PUT /v1/acl/policy (Persistent)
/// Create a new policy with database persistence
pub async fn create_policy_persistent(
    acl_service: web::Data<AclServicePersistent>,
    body: web::Json<CreatePolicyRequest>,
) -> HttpResponse {
    match acl_service
        .create_policy(
            &body.name,
            body.description.as_deref().unwrap_or(""),
            &body.rules,
        )
        .await
    {
        Ok(policy) => HttpResponse::Ok().json(policy),
        Err(e) => HttpResponse::InternalServerError().json(AclError::new(&e)),
    }
}

/// DELETE /v1/acl/policy/{id} (Persistent)
pub async fn delete_policy_persistent(
    acl_service: web::Data<AclServicePersistent>,
    path: web::Path<String>,
) -> HttpResponse {
    let id = path.into_inner();

    if acl_service.delete_policy(&id).await {
        HttpResponse::Ok().json(true)
    } else {
        HttpResponse::NotFound().json(AclError::new("Policy not found"))
    }
}

/// GET /v1/acl/roles (Persistent)
/// List all roles with database persistence
pub async fn list_roles_persistent(acl_service: web::Data<AclServicePersistent>) -> HttpResponse {
    let roles = acl_service.list_roles().await;
    HttpResponse::Ok().json(roles)
}

/// PUT /v1/acl/role (Persistent)
/// Create a new role with database persistence
pub async fn create_role_persistent(
    acl_service: web::Data<AclServicePersistent>,
    body: web::Json<RoleRequest>,
) -> HttpResponse {
    let policies: Vec<String> = body
        .policies
        .as_ref()
        .map(|p| p.iter().map(|pl| pl.name.clone()).collect())
        .unwrap_or_default();

    match acl_service
        .create_role(
            &body.name,
            body.description.as_deref().unwrap_or(""),
            policies,
        )
        .await
    {
        Ok(role) => HttpResponse::Ok().json(role),
        Err(e) => HttpResponse::InternalServerError().json(AclError::new(&e)),
    }
}

/// GET /v1/acl/role/{id} (Persistent)
/// Get a role by ID or name with database persistence
pub async fn get_role_persistent(
    acl_service: web::Data<AclServicePersistent>,
    path: web::Path<String>,
) -> HttpResponse {
    let id = path.into_inner();

    match acl_service.get_role(&id).await {
        Some(role) => HttpResponse::Ok().json(role),
        None => HttpResponse::NotFound().json(AclError::new("Role not found")),
    }
}

/// PUT /v1/acl/role/{id} (Persistent)
/// Update an existing role with database persistence
pub async fn update_role_persistent(
    acl_service: web::Data<AclServicePersistent>,
    path: web::Path<String>,
    body: web::Json<RoleRequest>,
) -> HttpResponse {
    let id = path.into_inner();

    let policies: Option<Vec<String>> = body
        .policies
        .as_ref()
        .map(|p| p.iter().map(|pl| pl.name.clone()).collect());

    match acl_service
        .update_role(&id, Some(&body.name), body.description.as_deref(), policies)
        .await
    {
        Some(role) => HttpResponse::Ok().json(role),
        None => HttpResponse::NotFound().json(AclError::new("Role not found")),
    }
}

/// DELETE /v1/acl/role/{id} (Persistent)
/// Delete a role with database persistence
pub async fn delete_role_persistent(
    acl_service: web::Data<AclServicePersistent>,
    path: web::Path<String>,
) -> HttpResponse {
    let id = path.into_inner();

    if acl_service.delete_role(&id).await {
        HttpResponse::Ok().json(true)
    } else {
        HttpResponse::NotFound().json(AclError::new("Role not found"))
    }
}

/// GET /v1/acl/auth-methods (Persistent)
/// List all auth methods with database persistence
pub async fn list_auth_methods_persistent(
    acl_service: web::Data<AclServicePersistent>,
) -> HttpResponse {
    let methods = acl_service.list_auth_methods().await;
    HttpResponse::Ok().json(methods)
}

/// PUT /v1/acl/auth-method (Persistent)
/// Create a new auth method with database persistence
pub async fn create_auth_method_persistent(
    acl_service: web::Data<AclServicePersistent>,
    body: web::Json<AuthMethodRequest>,
) -> HttpResponse {
    match acl_service
        .create_auth_method(
            &body.name,
            &body.method_type,
            body.display_name.as_deref(),
            body.description.as_deref(),
            body.max_token_ttl.as_deref(),
            body.token_locality.as_deref(),
            body.config.clone(),
        )
        .await
    {
        Ok(method) => HttpResponse::Ok().json(method),
        Err(e) => HttpResponse::InternalServerError().json(AclError::new(&e)),
    }
}

/// GET /v1/acl/auth-method/{name} (Persistent)
/// Get an auth method by name with database persistence
pub async fn get_auth_method_persistent(
    acl_service: web::Data<AclServicePersistent>,
    path: web::Path<String>,
) -> HttpResponse {
    let name = path.into_inner();

    match acl_service.get_auth_method(&name).await {
        Some(method) => HttpResponse::Ok().json(method),
        None => HttpResponse::NotFound().json(AclError::new("Auth method not found")),
    }
}

/// PUT /v1/acl/auth-method/{name} (Persistent)
/// Update an existing auth method with database persistence
pub async fn update_auth_method_persistent(
    acl_service: web::Data<AclServicePersistent>,
    path: web::Path<String>,
    body: web::Json<AuthMethodRequest>,
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
        Some(method) => HttpResponse::Ok().json(method),
        None => HttpResponse::NotFound().json(AclError::new("Auth method not found")),
    }
}

/// DELETE /v1/acl/auth-method/{name} (Persistent)
/// Delete an auth method with database persistence
pub async fn delete_auth_method_persistent(
    acl_service: web::Data<AclServicePersistent>,
    path: web::Path<String>,
) -> HttpResponse {
    let name = path.into_inner();

    if acl_service.delete_auth_method(&name).await {
        HttpResponse::Ok().json(true)
    } else {
        HttpResponse::NotFound().json(AclError::new("Auth method not found"))
    }
}

/// GET /v1/acl/token/self (Persistent)
/// Returns the token associated with the current request
pub async fn get_token_self_persistent(
    acl_service: web::Data<AclServicePersistent>,
    req: HttpRequest,
) -> HttpResponse {
    let secret_id = match AclService::extract_token(&req) {
        Some(t) => t,
        None => return HttpResponse::Forbidden().json(AclError::new("ACL token required")),
    };

    match acl_service.get_token(&secret_id).await {
        Some(token) => HttpResponse::Ok().json(token),
        None => HttpResponse::Forbidden().json(AclError::new("ACL token not found or invalid")),
    }
}

/// PUT /v1/acl/token/{accessor_id}/clone (Persistent)
/// Clone an existing token
pub async fn clone_token_persistent(
    acl_service: web::Data<AclServicePersistent>,
    path: web::Path<String>,
    body: web::Json<CloneTokenRequest>,
) -> HttpResponse {
    let accessor_id = path.into_inner();

    // Find the token to clone
    let tokens = acl_service.list_tokens().await;
    let source_token = tokens.into_iter().find(|t| t.accessor_id == accessor_id);

    match source_token {
        Some(source) => {
            // Get the full token from cache (with secret_id)
            let full_source = acl_service
                .token_cache
                .iter()
                .find(|entry| entry.value().accessor_id == accessor_id)
                .map(|entry| entry.value().clone())
                .unwrap_or(source);

            // Create a new token with the same policies and roles
            let policies: Vec<String> = full_source
                .policies
                .iter()
                .map(|p| p.name.clone())
                .collect();
            let roles: Vec<String> = full_source.roles.iter().map(|r| r.name.clone()).collect();
            let description = body
                .description
                .clone()
                .unwrap_or_else(|| format!("Clone of {}", full_source.description));

            match acl_service
                .create_token_with_roles(&description, policies, roles, full_source.local)
                .await
            {
                Ok(new_token) => HttpResponse::Ok().json(new_token),
                Err(e) => HttpResponse::InternalServerError().json(AclError::new(&e)),
            }
        }
        None => HttpResponse::NotFound().json(AclError::new("Token not found")),
    }
}

/// PUT /v1/acl/bootstrap (Persistent)
/// Bootstrap the ACL system (creates initial management token)
pub async fn acl_bootstrap_persistent(
    acl_service: web::Data<AclServicePersistent>,
) -> HttpResponse {
    // Try to initialize bootstrap
    if let Err(e) = acl_service.init_bootstrap().await {
        return HttpResponse::InternalServerError().json(AclError::new(&e));
    }

    // Get the bootstrap token
    if let Some(token) = acl_service.get_token("root").await {
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

/// POST /v1/acl/login (Persistent)
/// Login with an auth method to get a token
pub async fn acl_login_persistent(
    acl_service: web::Data<AclServicePersistent>,
    body: web::Json<LoginRequest>,
) -> HttpResponse {
    // Check if auth method exists
    let auth_method = match acl_service.get_auth_method(&body.auth_method).await {
        Some(m) => m,
        None => {
            return HttpResponse::NotFound().json(AclError::new(&format!(
                "Auth method '{}' not found",
                body.auth_method
            )));
        }
    };

    // Create a login token
    let now = chrono::Utc::now().to_rfc3339();
    let description = format!("Login token via {}", body.auth_method);

    // Calculate expiration based on max_token_ttl
    let expiration_time = auth_method.max_token_ttl.as_ref().and_then(|ttl| {
        parse_duration(ttl).map(|dur| {
            (chrono::Utc::now() + chrono::Duration::from_std(dur).unwrap_or_default()).to_rfc3339()
        })
    });

    match acl_service.create_token(&description, vec![]).await {
        Ok(token) => {
            let response = LoginResponse {
                accessor_id: token.accessor_id,
                secret_id: token.secret_id.unwrap_or_default(),
                description: token.description,
                policies: token.policies,
                roles: token.roles,
                local: auth_method.token_locality.as_deref() == Some("local"),
                auth_method: Some(body.auth_method.clone()),
                expiration_time,
                create_time: now,
            };
            HttpResponse::Ok().json(response)
        }
        Err(e) => HttpResponse::InternalServerError().json(AclError::new(&e)),
    }
}

/// POST /v1/acl/logout (Persistent)
/// Logout and invalidate the current token
pub async fn acl_logout_persistent(
    acl_service: web::Data<AclServicePersistent>,
    req: HttpRequest,
) -> HttpResponse {
    let secret_id = match AclService::extract_token(&req) {
        Some(t) => t,
        None => return HttpResponse::Forbidden().json(AclError::new("ACL token required")),
    };

    // Don't allow logging out the bootstrap token
    if secret_id == "root" {
        return HttpResponse::Forbidden().json(AclError::new("Cannot logout bootstrap token"));
    }

    // Find and delete the token
    if let Some(token) = acl_service.get_token(&secret_id).await {
        if acl_service.delete_token(&token.accessor_id).await {
            return HttpResponse::Ok().json(true);
        }
    }

    HttpResponse::NotFound().json(AclError::new("Token not found"))
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
}
