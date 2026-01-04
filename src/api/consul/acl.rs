// Consul ACL (Access Control List) implementation
// Provides token-based authentication and authorization for Consul API endpoints

use std::collections::HashMap;
use std::sync::LazyLock;

use actix_web::{HttpRequest, HttpResponse, web};
use dashmap::DashMap;
use moka::sync::Cache;
use serde::{Deserialize, Serialize};
use std::time::Duration;

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
#[derive(Clone, Debug, Serialize, Deserialize)]
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

impl Default for AclToken {
    fn default() -> Self {
        Self {
            accessor_id: String::new(),
            secret_id: None,
            description: String::new(),
            policies: Vec::new(),
            roles: Vec::new(),
            local: false,
            expiration_time: None,
            create_time: String::new(),
            modify_time: String::new(),
        }
    }
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

impl RulePolicy {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "read" => RulePolicy::Read,
            "write" => RulePolicy::Write,
            "deny" => RulePolicy::Deny,
            _ => RulePolicy::Deny,
        }
    }

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
        if let Some(token) = req.headers().get(X_CONSUL_TOKEN) {
            if let Ok(token_str) = token.to_str() {
                return Some(token_str.to_string());
            }
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
    pub fn create_token(&self, description: &str, policies: Vec<String>) -> AclToken {
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

        let token = AclToken {
            accessor_id,
            secret_id: Some(secret_id.clone()),
            description: description.to_string(),
            policies: policy_links,
            roles: vec![],
            local: false,
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
                let (resource_type, prefix) = if let Some(rest) = resource_part.strip_prefix("agent_prefix") {
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
                    let policy_str = policy_str.trim().trim_start_matches('=').trim().trim_matches('"');
                    RulePolicy::from_str(policy_str)
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
        None => HttpResponse::NotFound().json(AclError::new("Token not found")),
    }
}

/// Token creation request
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CreateTokenRequest {
    pub description: Option<String>,
    pub policies: Option<Vec<PolicyLink>>,
}

/// PUT /v1/acl/token
/// Create a new token
pub async fn create_token(
    acl_service: web::Data<AclService>,
    body: web::Json<CreateTokenRequest>,
) -> HttpResponse {
    let policies: Vec<String> = body
        .policies
        .as_ref()
        .map(|p| p.iter().map(|pl| pl.name.clone()).collect())
        .unwrap_or_default();

    let token = acl_service.create_token(
        body.description.as_deref().unwrap_or(""),
        policies,
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
        let token = service.create_token("Test token", vec!["global-management".to_string()]);

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
