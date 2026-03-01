//! Consul Connect CA and Intentions API
//!
//! Provides mTLS certificate authority, service intentions,
//! and connection authorization endpoints.

use actix_web::{HttpRequest, HttpResponse, web};
use chrono::Utc;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::acl::{AclService, ResourceType};
use crate::model::ConsulError;

// ============================================================================
// CA Models
// ============================================================================

/// A CA root certificate
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CARoot {
    #[serde(rename = "ID")]
    pub id: String,
    pub name: String,
    pub root_cert: String,
    pub active: bool,
    pub create_index: u64,
    pub modify_index: u64,
}

/// List of CA roots response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CARootList {
    #[serde(rename = "ActiveRootID")]
    pub active_root_id: String,
    pub trust_domain: String,
    pub roots: Vec<CARoot>,
}

/// CA configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CAConfig {
    pub provider: String,
    #[serde(default)]
    pub config: std::collections::HashMap<String, serde_json::Value>,
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub state: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub force_without_cross_signing: bool,
    pub create_index: u64,
    pub modify_index: u64,
}

/// Leaf certificate response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LeafCert {
    pub serial_number: String,
    #[serde(rename = "CertPEM")]
    pub cert_pem: String,
    #[serde(rename = "PrivateKeyPEM")]
    pub private_key_pem: String,
    pub service: String,
    #[serde(rename = "ServiceURI")]
    pub service_uri: String,
    pub valid_after: String,
    pub valid_before: String,
    pub create_index: u64,
    pub modify_index: u64,
}

// ============================================================================
// Intention Models
// ============================================================================

/// Intention action
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IntentionAction {
    Allow,
    Deny,
}

/// HTTP permission match
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct IntentionHTTPPermission {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_exact: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_prefix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_regex: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub methods: Vec<String>,
}

/// Intention permission
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct IntentionPermission {
    pub action: IntentionAction,
    #[serde(rename = "HTTP", skip_serializing_if = "Option::is_none")]
    pub http: Option<IntentionHTTPPermission>,
}

/// A service intention
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Intention {
    #[serde(rename = "ID")]
    pub id: String,
    #[serde(default)]
    pub description: String,
    #[serde(rename = "SourceNS", default)]
    pub source_ns: String,
    pub source_name: String,
    #[serde(rename = "DestinationNS", default)]
    pub destination_ns: String,
    pub destination_name: String,
    pub action: IntentionAction,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub permissions: Vec<IntentionPermission>,
    #[serde(default)]
    pub meta: std::collections::HashMap<String, String>,
    pub precedence: i32,
    pub created_at: String,
    pub updated_at: String,
    pub create_index: u64,
    pub modify_index: u64,
}

/// Request to create/update an intention
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct IntentionRequest {
    #[serde(default)]
    pub description: String,
    #[serde(rename = "SourceNS", default)]
    pub source_ns: String,
    pub source_name: String,
    #[serde(rename = "DestinationNS", default)]
    pub destination_ns: String,
    pub destination_name: String,
    pub action: IntentionAction,
    #[serde(default)]
    pub permissions: Vec<IntentionPermission>,
    #[serde(default)]
    pub meta: std::collections::HashMap<String, String>,
}

/// Intention check response
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct IntentionCheckResponse {
    pub allowed: bool,
}

/// Intention match query
#[derive(Debug, Deserialize)]
pub struct IntentionMatchQuery {
    pub by: String,
    pub name: String,
}

/// Query parameters for exact intention lookup
#[derive(Debug, Deserialize)]
pub struct IntentionExactQuery {
    pub source: Option<String>,
    pub destination: Option<String>,
}

/// Agent authorize request
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AgentAuthorizeRequest {
    pub target: String,
    #[serde(rename = "ClientCertURI")]
    pub client_cert_uri: String,
    #[serde(default)]
    pub client_cert_serial: String,
}

/// Agent authorize response
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct AgentAuthorizeResponse {
    pub authorized: bool,
    pub reason: String,
}

/// Query parameters for intentions
#[derive(Debug, Deserialize)]
pub struct IntentionQueryParams {
    pub filter: Option<String>,
    pub source: Option<String>,
    pub destination: Option<String>,
}

/// Query parameters for CA root
#[derive(Debug, Deserialize)]
pub struct CARootQueryParams {
    pub pem: Option<bool>,
}

// ============================================================================
// Service (In-Memory)
// ============================================================================

/// In-memory Connect CA and Intentions service
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
    index: std::sync::atomic::AtomicU64,
    /// Trust domain
    trust_domain: String,
}

impl ConsulConnectCAService {
    pub fn new() -> Self {
        let root_id = uuid::Uuid::new_v4().to_string();

        let root = CARoot {
            id: root_id.clone(),
            name: "Consul CA Root Cert".to_string(),
            root_cert: Self::placeholder_root_cert(),
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
            index: std::sync::atomic::AtomicU64::new(2),
            trust_domain: "consul".to_string(),
        }
    }

    fn placeholder_root_cert() -> String {
        // Placeholder - in production this would be a real self-signed CA certificate
        "-----BEGIN CERTIFICATE-----\nPlaceholder Consul CA Root Certificate\n-----END CERTIFICATE-----".to_string()
    }

    pub async fn get_roots(&self) -> CARootList {
        let active_id = self.active_root_id.read().await.clone();
        let roots: Vec<CARoot> = self.roots.iter().map(|r| r.value().clone()).collect();
        CARootList {
            active_root_id: active_id,
            trust_domain: self.trust_domain.clone(),
            roots,
        }
    }

    pub async fn get_ca_config(&self) -> CAConfig {
        self.ca_config.read().await.clone()
    }

    pub async fn set_ca_config(&self, mut config: CAConfig) -> Result<(), String> {
        if config.provider.is_empty() {
            return Err("Provider is required".to_string());
        }
        let index = self.index.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        config.modify_index = index;
        *self.ca_config.write().await = config;
        Ok(())
    }

    pub fn get_leaf_cert(&self, service: &str) -> LeafCert {
        let now = Utc::now();
        let valid_before = now + chrono::Duration::hours(72);
        let index = self.index.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        LeafCert {
            serial_number: format!(
                "{:02x}:{:02x}:{:02x}:{:02x}",
                rand_byte(),
                rand_byte(),
                rand_byte(),
                rand_byte()
            ),
            cert_pem: format!(
                "-----BEGIN CERTIFICATE-----\nPlaceholder leaf cert for {}\n-----END CERTIFICATE-----",
                service
            ),
            private_key_pem: "-----BEGIN EC PRIVATE KEY-----\nPlaceholder private key\n-----END EC PRIVATE KEY-----".to_string(),
            service: service.to_string(),
            service_uri: format!(
                "spiffe://{}/ns/default/dc/dc1/svc/{}",
                self.trust_domain, service
            ),
            valid_after: now.to_rfc3339(),
            valid_before: valid_before.to_rfc3339(),
            create_index: index,
            modify_index: index,
        }
    }

    pub fn create_intention(&self, req: IntentionRequest) -> Intention {
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

        self.intentions.insert(id, intention.clone());
        intention
    }

    pub fn get_intention(&self, id: &str) -> Option<Intention> {
        self.intentions.get(id).map(|r| r.value().clone())
    }

    pub fn update_intention(&self, id: &str, req: IntentionRequest) -> Option<Intention> {
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
            Some(entry.clone())
        } else {
            None
        }
    }

    pub fn delete_intention(&self, id: &str) -> bool {
        self.intentions.remove(id).is_some()
    }

    pub fn list_intentions(&self) -> Vec<Intention> {
        let mut intentions: Vec<Intention> =
            self.intentions.iter().map(|r| r.value().clone()).collect();
        // Sort by precedence (highest first)
        intentions.sort_by(|a, b| b.precedence.cmp(&a.precedence));
        intentions
    }

    pub fn check_intention(&self, source: &str, destination: &str) -> bool {
        // Find the highest-precedence matching intention
        let mut best: Option<(i32, IntentionAction)> = None;

        for entry in self.intentions.iter() {
            let i = entry.value();
            let source_match = i.source_name == "*" || i.source_name == source;
            let dest_match = i.destination_name == "*" || i.destination_name == destination;

            if source_match
                && dest_match
                && (best.is_none() || i.precedence > best.as_ref().unwrap().0)
            {
                best = Some((i.precedence, i.action.clone()));
            }
        }

        // Default: allow if no intention matches
        best.map(|(_, action)| action == IntentionAction::Allow)
            .unwrap_or(true)
    }

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
    pub fn delete_intention_exact(&self, source: &str, destination: &str) -> bool {
        let key_to_remove = self
            .intentions
            .iter()
            .find(|r| r.value().source_name == source && r.value().destination_name == destination)
            .map(|r| r.key().clone());
        if let Some(key) = key_to_remove {
            self.intentions.remove(&key);
            true
        } else {
            false
        }
    }

    /// Create or update an intention by exact source and destination name match
    pub fn upsert_intention_exact(&self, req: IntentionRequest) -> Intention {
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
            self.update_intention(&id, req).unwrap()
        } else {
            // Create new
            self.create_intention(req)
        }
    }

    pub fn authorize(&self, target: &str, client_cert_uri: &str) -> AgentAuthorizeResponse {
        // Extract source service from SPIFFE URI
        let source = client_cert_uri.rsplit('/').next().unwrap_or("unknown");

        let allowed = self.check_intention(source, target);
        AgentAuthorizeResponse {
            authorized: allowed,
            reason: if allowed {
                format!(
                    "Intention allows service '{}' to connect to '{}'",
                    source, target
                )
            } else {
                format!(
                    "Intention denies service '{}' from connecting to '{}'",
                    source, target
                )
            },
        }
    }
}

impl Default for ConsulConnectCAService {
    fn default() -> Self {
        Self::new()
    }
}

/// Generate a pseudo-random byte for serial numbers
fn rand_byte() -> u8 {
    // Use a simple method since we don't need cryptographic randomness for placeholders
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    (now & 0xFF) as u8
}

// ============================================================================
// HTTP Handlers (In-Memory)
// ============================================================================

/// GET /v1/connect/ca/roots - List CA root certificates
pub async fn get_ca_roots(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    _query: web::Query<CARootQueryParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    HttpResponse::Ok().json(ca_service.get_roots().await)
}

/// GET /v1/connect/ca/configuration - Get CA configuration
pub async fn get_ca_configuration(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    HttpResponse::Ok().json(ca_service.get_ca_config().await)
}

/// PUT /v1/connect/ca/configuration - Set CA configuration
pub async fn set_ca_configuration(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    body: web::Json<CAConfig>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    match ca_service.set_ca_config(body.into_inner()).await {
        Ok(()) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::BadRequest().json(ConsulError::new(e)),
    }
}

/// GET /v1/agent/connect/ca/leaf/{service} - Get leaf certificate
pub async fn get_leaf_cert(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    path: web::Path<String>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let service = path.into_inner();
    HttpResponse::Ok().json(ca_service.get_leaf_cert(&service))
}

/// GET /v1/connect/intentions - List intentions
pub async fn list_intentions(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    _query: web::Query<IntentionQueryParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    HttpResponse::Ok().json(ca_service.list_intentions())
}

/// POST /v1/connect/intentions - Create intention
pub async fn create_intention(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    body: web::Json<IntentionRequest>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let intention = ca_service.create_intention(body.into_inner());
    HttpResponse::Ok().json(serde_json::json!({ "ID": intention.id }))
}

/// GET /v1/connect/intentions/{id} - Read intention
pub async fn get_intention(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    path: web::Path<String>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let id = path.into_inner();
    match ca_service.get_intention(&id) {
        Some(intention) => HttpResponse::Ok().json(intention),
        None => HttpResponse::NotFound().json(ConsulError::new("Intention not found")),
    }
}

/// PUT /v1/connect/intentions/{id} - Update intention
pub async fn update_intention(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    path: web::Path<String>,
    body: web::Json<IntentionRequest>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let id = path.into_inner();
    match ca_service.update_intention(&id, body.into_inner()) {
        Some(_) => HttpResponse::Ok().json(serde_json::json!({ "ID": id })),
        None => HttpResponse::NotFound().json(ConsulError::new("Intention not found")),
    }
}

/// DELETE /v1/connect/intentions/{id} - Delete intention
pub async fn delete_intention(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    path: web::Path<String>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let id = path.into_inner();
    if ca_service.delete_intention(&id) {
        HttpResponse::Ok().json(true)
    } else {
        HttpResponse::NotFound().json(ConsulError::new("Intention not found"))
    }
}

/// GET /v1/connect/intentions/check - Check intention access
pub async fn check_intention(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    query: web::Query<IntentionQueryParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let source = query.source.as_deref().unwrap_or("*");
    let destination = query.destination.as_deref().unwrap_or("*");
    let allowed = ca_service.check_intention(source, destination);
    HttpResponse::Ok().json(IntentionCheckResponse { allowed })
}

/// GET /v1/connect/intentions/match - Match intentions for service
pub async fn match_intentions(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    query: web::Query<IntentionMatchQuery>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let matched = ca_service.match_intentions(&query.by, &query.name);
    let mut result = std::collections::HashMap::new();
    result.insert(query.name.clone(), matched);
    HttpResponse::Ok().json(result)
}

/// POST /v1/agent/connect/authorize - Authorize a connection
pub async fn connect_authorize(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    body: web::Json<AgentAuthorizeRequest>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let auth_req = body.into_inner();
    let response = ca_service.authorize(&auth_req.target, &auth_req.client_cert_uri);
    HttpResponse::Ok().json(response)
}

/// GET /v1/connect/intentions/exact - Get intention by exact source/destination
pub async fn get_intention_exact(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    query: web::Query<IntentionExactQuery>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let source = query.source.as_deref().unwrap_or("*");
    let destination = query.destination.as_deref().unwrap_or("*");
    match ca_service.get_intention_exact(source, destination) {
        Some(intention) => HttpResponse::Ok().json(intention),
        None => HttpResponse::NotFound().json(ConsulError::new("Intention not found")),
    }
}

/// PUT /v1/connect/intentions/exact - Upsert intention by exact source/destination
pub async fn upsert_intention_exact(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    body: web::Json<IntentionRequest>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let intention = ca_service.upsert_intention_exact(body.into_inner());
    HttpResponse::Ok().json(serde_json::json!({ "Created": true, "ID": intention.id }))
}

/// DELETE /v1/connect/intentions/exact - Delete intention by exact source/destination
pub async fn delete_intention_exact(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    query: web::Query<IntentionExactQuery>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let source = query.source.as_deref().unwrap_or("*");
    let destination = query.destination.as_deref().unwrap_or("*");
    if ca_service.delete_intention_exact(source, destination) {
        HttpResponse::Ok().json(true)
    } else {
        HttpResponse::NotFound().json(ConsulError::new("Intention not found"))
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
    _query: web::Query<CARootQueryParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    HttpResponse::Ok().json(ca_service.get_roots().await)
}

/// GET /v1/connect/ca/configuration (persistent)
pub async fn get_ca_configuration_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    HttpResponse::Ok().json(ca_service.get_ca_config().await)
}

/// PUT /v1/connect/ca/configuration (persistent)
pub async fn set_ca_configuration_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    body: web::Json<CAConfig>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    match ca_service.set_ca_config(body.into_inner()).await {
        Ok(()) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::BadRequest().json(ConsulError::new(e)),
    }
}

/// GET /v1/agent/connect/ca/leaf/{service} (persistent)
pub async fn get_leaf_cert_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    path: web::Path<String>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let service = path.into_inner();
    HttpResponse::Ok().json(ca_service.get_leaf_cert(&service))
}

/// GET /v1/connect/intentions (persistent)
pub async fn list_intentions_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    _query: web::Query<IntentionQueryParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    HttpResponse::Ok().json(ca_service.list_intentions())
}

/// POST /v1/connect/intentions (persistent)
pub async fn create_intention_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    body: web::Json<IntentionRequest>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let intention = ca_service.create_intention(body.into_inner());
    HttpResponse::Ok().json(serde_json::json!({ "ID": intention.id }))
}

/// GET /v1/connect/intentions/{id} (persistent)
pub async fn get_intention_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    path: web::Path<String>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let id = path.into_inner();
    match ca_service.get_intention(&id) {
        Some(intention) => HttpResponse::Ok().json(intention),
        None => HttpResponse::NotFound().json(ConsulError::new("Intention not found")),
    }
}

/// PUT /v1/connect/intentions/{id} (persistent)
pub async fn update_intention_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    path: web::Path<String>,
    body: web::Json<IntentionRequest>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let id = path.into_inner();
    match ca_service.update_intention(&id, body.into_inner()) {
        Some(_) => HttpResponse::Ok().json(serde_json::json!({ "ID": id })),
        None => HttpResponse::NotFound().json(ConsulError::new("Intention not found")),
    }
}

/// DELETE /v1/connect/intentions/{id} (persistent)
pub async fn delete_intention_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    path: web::Path<String>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let id = path.into_inner();
    if ca_service.delete_intention(&id) {
        HttpResponse::Ok().json(true)
    } else {
        HttpResponse::NotFound().json(ConsulError::new("Intention not found"))
    }
}

/// GET /v1/connect/intentions/check (persistent)
pub async fn check_intention_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    query: web::Query<IntentionQueryParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let source = query.source.as_deref().unwrap_or("*");
    let destination = query.destination.as_deref().unwrap_or("*");
    let allowed = ca_service.check_intention(source, destination);
    HttpResponse::Ok().json(IntentionCheckResponse { allowed })
}

/// GET /v1/connect/intentions/match (persistent)
pub async fn match_intentions_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    query: web::Query<IntentionMatchQuery>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let matched = ca_service.match_intentions(&query.by, &query.name);
    let mut result = std::collections::HashMap::new();
    result.insert(query.name.clone(), matched);
    HttpResponse::Ok().json(result)
}

/// POST /v1/agent/connect/authorize (persistent)
pub async fn connect_authorize_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    body: web::Json<AgentAuthorizeRequest>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let auth_req = body.into_inner();
    let response = ca_service.authorize(&auth_req.target, &auth_req.client_cert_uri);
    HttpResponse::Ok().json(response)
}

/// GET /v1/connect/intentions/exact (persistent)
pub async fn get_intention_exact_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    query: web::Query<IntentionExactQuery>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let source = query.source.as_deref().unwrap_or("*");
    let destination = query.destination.as_deref().unwrap_or("*");
    match ca_service.get_intention_exact(source, destination) {
        Some(intention) => HttpResponse::Ok().json(intention),
        None => HttpResponse::NotFound().json(ConsulError::new("Intention not found")),
    }
}

/// PUT /v1/connect/intentions/exact (persistent)
pub async fn upsert_intention_exact_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    body: web::Json<IntentionRequest>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let intention = ca_service.upsert_intention_exact(body.into_inner());
    HttpResponse::Ok().json(serde_json::json!({ "Created": true, "ID": intention.id }))
}

/// DELETE /v1/connect/intentions/exact (persistent)
pub async fn delete_intention_exact_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    query: web::Query<IntentionExactQuery>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let source = query.source.as_deref().unwrap_or("*");
    let destination = query.destination.as_deref().unwrap_or("*");
    if ca_service.delete_intention_exact(source, destination) {
        HttpResponse::Ok().json(true)
    } else {
        HttpResponse::NotFound().json(ConsulError::new("Intention not found"))
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
        assert!(roots.roots[0].active);
        assert_eq!(roots.trust_domain, "consul");
    }

    #[tokio::test]
    async fn test_ca_config() {
        let service = ConsulConnectCAService::new();
        let config = service.get_ca_config().await;
        assert_eq!(config.provider, "consul");
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

    #[test]
    fn test_create_intention() {
        let service = ConsulConnectCAService::new();
        let intention = service.create_intention(IntentionRequest {
            description: "Allow web to api".to_string(),
            source_ns: String::new(),
            source_name: "web".to_string(),
            destination_ns: String::new(),
            destination_name: "api".to_string(),
            action: IntentionAction::Allow,
            permissions: vec![],
            meta: Default::default(),
        });
        assert_eq!(intention.source_name, "web");
        assert_eq!(intention.destination_name, "api");
        assert_eq!(intention.action, IntentionAction::Allow);
        assert_eq!(intention.precedence, 4); // specific to specific
    }

    #[test]
    fn test_intention_precedence() {
        let service = ConsulConnectCAService::new();
        // Create a deny-all intention (lowest precedence)
        service.create_intention(IntentionRequest {
            description: String::new(),
            source_ns: String::new(),
            source_name: "*".to_string(),
            destination_ns: String::new(),
            destination_name: "*".to_string(),
            action: IntentionAction::Deny,
            permissions: vec![],
            meta: Default::default(),
        });
        // Create a specific allow
        service.create_intention(IntentionRequest {
            description: String::new(),
            source_ns: String::new(),
            source_name: "web".to_string(),
            destination_ns: String::new(),
            destination_name: "api".to_string(),
            action: IntentionAction::Allow,
            permissions: vec![],
            meta: Default::default(),
        });

        // Specific allow should win over wildcard deny
        assert!(service.check_intention("web", "api"));
        // Unknown services should be denied (wildcard deny)
        assert!(!service.check_intention("unknown", "other"));
    }

    #[test]
    fn test_list_intentions() {
        let service = ConsulConnectCAService::new();
        service.create_intention(IntentionRequest {
            description: String::new(),
            source_ns: String::new(),
            source_name: "a".to_string(),
            destination_ns: String::new(),
            destination_name: "b".to_string(),
            action: IntentionAction::Allow,
            permissions: vec![],
            meta: Default::default(),
        });
        service.create_intention(IntentionRequest {
            description: String::new(),
            source_ns: String::new(),
            source_name: "*".to_string(),
            destination_ns: String::new(),
            destination_name: "*".to_string(),
            action: IntentionAction::Deny,
            permissions: vec![],
            meta: Default::default(),
        });

        let intentions = service.list_intentions();
        assert_eq!(intentions.len(), 2);
        // Higher precedence first
        assert!(intentions[0].precedence >= intentions[1].precedence);
    }

    #[test]
    fn test_delete_intention() {
        let service = ConsulConnectCAService::new();
        let intention = service.create_intention(IntentionRequest {
            description: String::new(),
            source_ns: String::new(),
            source_name: "web".to_string(),
            destination_ns: String::new(),
            destination_name: "api".to_string(),
            action: IntentionAction::Allow,
            permissions: vec![],
            meta: Default::default(),
        });
        assert!(service.delete_intention(&intention.id));
        assert!(service.get_intention(&intention.id).is_none());
    }

    #[test]
    fn test_authorize() {
        let service = ConsulConnectCAService::new();
        service.create_intention(IntentionRequest {
            description: String::new(),
            source_ns: String::new(),
            source_name: "web".to_string(),
            destination_ns: String::new(),
            destination_name: "api".to_string(),
            action: IntentionAction::Allow,
            permissions: vec![],
            meta: Default::default(),
        });

        let result = service.authorize("api", "spiffe://consul/ns/default/dc/dc1/svc/web");
        assert!(result.authorized);
    }

    #[test]
    fn test_match_intentions() {
        let service = ConsulConnectCAService::new();
        service.create_intention(IntentionRequest {
            description: String::new(),
            source_ns: String::new(),
            source_name: "web".to_string(),
            destination_ns: String::new(),
            destination_name: "api".to_string(),
            action: IntentionAction::Allow,
            permissions: vec![],
            meta: Default::default(),
        });

        let matched = service.match_intentions("source", "web");
        assert_eq!(matched.len(), 1);

        let matched = service.match_intentions("destination", "api");
        assert_eq!(matched.len(), 1);
    }

    #[test]
    fn test_delete_nonexistent_intention() {
        let service = ConsulConnectCAService::new();
        assert!(!service.delete_intention("nonexistent-id"));
    }

    #[test]
    fn test_get_nonexistent_intention() {
        let service = ConsulConnectCAService::new();
        assert!(service.get_intention("nonexistent-id").is_none());
    }

    #[test]
    fn test_update_intention() {
        let service = ConsulConnectCAService::new();
        let intention = service.create_intention(IntentionRequest {
            description: "original".to_string(),
            source_ns: String::new(),
            source_name: "web".to_string(),
            destination_ns: String::new(),
            destination_name: "api".to_string(),
            action: IntentionAction::Allow,
            permissions: vec![],
            meta: Default::default(),
        });

        let updated = service.update_intention(
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
        );

        assert!(updated.is_some());
        let u = updated.unwrap();
        assert_eq!(u.description, "updated");
        assert_eq!(u.source_name, "frontend");
        assert_eq!(u.destination_name, "backend");
        assert_eq!(u.action, IntentionAction::Deny);
        assert_eq!(u.id, intention.id); // ID preserved
    }

    #[test]
    fn test_update_nonexistent_intention() {
        let service = ConsulConnectCAService::new();
        let result = service.update_intention(
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
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_authorize_deny() {
        let service = ConsulConnectCAService::new();
        service.create_intention(IntentionRequest {
            description: String::new(),
            source_ns: String::new(),
            source_name: "malicious".to_string(),
            destination_ns: String::new(),
            destination_name: "api".to_string(),
            action: IntentionAction::Deny,
            permissions: vec![],
            meta: Default::default(),
        });

        let result = service.authorize("api", "spiffe://consul/ns/default/dc/dc1/svc/malicious");
        assert!(!result.authorized);
        assert!(result.reason.contains("denies"));
    }

    #[test]
    fn test_authorize_no_intentions_allows() {
        let service = ConsulConnectCAService::new();
        // No intentions configured - default allow
        let result = service.authorize("api", "spiffe://consul/ns/default/dc/dc1/svc/web");
        assert!(result.authorized);
    }

    #[test]
    fn test_intention_precedence_wildcard_vs_specific() {
        let service = ConsulConnectCAService::new();

        // Wildcard deny-all
        service.create_intention(IntentionRequest {
            description: String::new(),
            source_ns: String::new(),
            source_name: "*".to_string(),
            destination_ns: String::new(),
            destination_name: "*".to_string(),
            action: IntentionAction::Deny,
            permissions: vec![],
            meta: Default::default(),
        });

        // Specific allow
        service.create_intention(IntentionRequest {
            description: String::new(),
            source_ns: String::new(),
            source_name: "web".to_string(),
            destination_ns: String::new(),
            destination_name: "api".to_string(),
            action: IntentionAction::Allow,
            permissions: vec![],
            meta: Default::default(),
        });

        // Specific rule (precedence 4) should beat wildcard (precedence 1)
        assert!(service.check_intention("web", "api"));

        // Other services should be denied by wildcard
        assert!(!service.check_intention("unknown", "api"));
    }

    #[test]
    fn test_leaf_cert_fields() {
        let service = ConsulConnectCAService::new();
        let cert = service.get_leaf_cert("my-service");

        assert!(!cert.serial_number.is_empty());
        assert!(cert.cert_pem.contains("CERTIFICATE"));
        assert!(cert.private_key_pem.contains("EC PRIVATE KEY"));
        assert!(cert.service.contains("my-service"));
        assert!(cert.service_uri.contains("my-service"));
        assert!(!cert.valid_after.is_empty());
        assert!(!cert.valid_before.is_empty());
    }

    #[test]
    fn test_ca_config_empty_provider() {
        let service = ConsulConnectCAService::new();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(service.set_ca_config(CAConfig {
            provider: String::new(),
            config: Default::default(),
            state: Default::default(),
            force_without_cross_signing: false,
            create_index: 0,
            modify_index: 0,
        }));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Provider is required"));
    }

    #[test]
    fn test_intention_default_namespace() {
        let service = ConsulConnectCAService::new();
        let intention = service.create_intention(IntentionRequest {
            description: String::new(),
            source_ns: String::new(), // empty → default
            source_name: "web".to_string(),
            destination_ns: String::new(),
            destination_name: "api".to_string(),
            action: IntentionAction::Allow,
            permissions: vec![],
            meta: Default::default(),
        });

        assert_eq!(intention.source_ns, "default");
        assert_eq!(intention.destination_ns, "default");
    }

    #[test]
    fn test_match_intentions_wildcard() {
        let service = ConsulConnectCAService::new();

        service.create_intention(IntentionRequest {
            description: String::new(),
            source_ns: String::new(),
            source_name: "*".to_string(),
            destination_ns: String::new(),
            destination_name: "db".to_string(),
            action: IntentionAction::Deny,
            permissions: vec![],
            meta: Default::default(),
        });

        // Wildcard source matches any source query
        let matched = service.match_intentions("source", "anything");
        assert_eq!(matched.len(), 1);
    }

    #[test]
    fn test_intention_with_meta() {
        let service = ConsulConnectCAService::new();
        let mut meta = std::collections::HashMap::new();
        meta.insert("env".to_string(), "production".to_string());
        meta.insert("team".to_string(), "platform".to_string());

        let intention = service.create_intention(IntentionRequest {
            description: "with metadata".to_string(),
            source_ns: String::new(),
            source_name: "web".to_string(),
            destination_ns: String::new(),
            destination_name: "api".to_string(),
            action: IntentionAction::Allow,
            permissions: vec![],
            meta,
        });

        assert_eq!(intention.meta.len(), 2);
        assert_eq!(intention.meta.get("env").unwrap(), "production");
    }
}
