//! Consul Peering API
//!
//! Provides cluster peering endpoints for cross-datacenter service discovery.

use actix_web::{HttpRequest, HttpResponse, web};
use chrono::Utc;
use dashmap::DashMap;
use rocksdb::DB;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::constants::CF_CONSUL_PEERING;

use crate::acl::{AclService, ResourceType};
use crate::consul_meta::{ConsulResponseMeta, consul_ok};
use crate::index_provider::{ConsulIndexProvider, ConsulTable};
use crate::model::ConsulError;
use crate::model::ConsulErrorBody;
use crate::raft::{ConsulRaftRequest, ConsulRaftWriter};

// ============================================================================
// Models
// ============================================================================

/// Peering state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[derive(Default)]
pub enum PeeringState {
    #[default]
/// The `Undefined` variant.
    Undefined,
/// The `Pending` variant.
    Pending,
/// The `Establishing` variant.
    Establishing,
/// The `Active` variant.
    Active,
/// The `Failing` variant.
    Failing,
/// The `Deleting` variant.
    Deleting,
/// The `Terminated` variant.
    Terminated,
}

impl std::fmt::Display for PeeringState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Undefined => write!(f, "UNDEFINED"),
            Self::Pending => write!(f, "PENDING"),
            Self::Establishing => write!(f, "ESTABLISHING"),
            Self::Active => write!(f, "ACTIVE"),
            Self::Failing => write!(f, "FAILING"),
            Self::Deleting => write!(f, "DELETING"),
            Self::Terminated => write!(f, "TERMINATED"),
        }
    }
}

/// Stream status for a peering
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PeeringStreamStatus {
/// The `imported_services` field.
    pub imported_services: Vec<String>,
/// The `exported_services` field.
    pub exported_services: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `last_heartbeat` field.
    pub last_heartbeat: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `last_receive` field.
    pub last_receive: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `last_send` field.
    pub last_send: Option<String>,
}

/// Remote peer info
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PeeringRemoteInfo {
    #[serde(default)]
/// The `partition` field.
    pub partition: String,
    #[serde(default)]
/// The `datacenter` field.
    pub datacenter: String,
}

/// A peering relationship
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Peering {
    #[serde(rename = "ID")]
/// The `id` field.
    pub id: String,
/// The `name` field.
    pub name: String,
    #[serde(default)]
/// The `partition` field.
    pub partition: String,
/// The `state` field.
    pub state: PeeringState,
    #[serde(rename = "PeerID")]
/// The `peer_id` field.
    pub peer_id: String,
    #[serde(default)]
/// The `peer_server_name` field.
    pub peer_server_name: String,
    #[serde(default)]
/// The `peer_server_addresses` field.
    pub peer_server_addresses: Vec<String>,
    #[serde(default, rename = "PeerCAPems")]
/// The `peer_ca_pems` field.
    pub peer_ca_pems: Vec<String>,
    #[serde(default)]
/// The `meta` field.
    pub meta: std::collections::HashMap<String, String>,
/// The `stream_status` field.
    pub stream_status: PeeringStreamStatus,
/// The `create_index` field.
    pub create_index: u64,
/// The `modify_index` field.
    pub modify_index: u64,
    #[serde(default)]
/// The `remote` field.
    pub remote: PeeringRemoteInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `deleted_at` field.
    pub deleted_at: Option<String>,
}

/// Request to generate a peering token
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PeeringGenerateTokenRequest {
/// The `peer_name` field.
    pub peer_name: String,
    #[serde(default)]
/// The `partition` field.
    pub partition: String,
    #[serde(default)]
/// The `meta` field.
    pub meta: std::collections::HashMap<String, String>,
    #[serde(default)]
/// The `server_external_addresses` field.
    pub server_external_addresses: Vec<String>,
}

/// Response for generate token
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct PeeringGenerateTokenResponse {
/// The `peering_token` field.
    pub peering_token: String,
}

/// Request to establish a peering
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PeeringEstablishRequest {
/// The `peer_name` field.
    pub peer_name: String,
/// The `peering_token` field.
    pub peering_token: String,
    #[serde(default)]
/// The `partition` field.
    pub partition: String,
    #[serde(default)]
/// The `meta` field.
    pub meta: std::collections::HashMap<String, String>,
}

/// Internal peering token structure
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PeeringToken {
    #[serde(rename = "CA")]
    ca: Vec<String>,
    server_addresses: Vec<String>,
    server_name: String,
    #[serde(rename = "PeerID")]
    peer_id: String,
    establishment_secret: String,
    remote: PeeringRemoteInfo,
}

/// Query parameters for peering endpoints
#[derive(Debug, Deserialize)]
pub struct PeeringQueryParams {
/// The `partition` field.
    pub partition: Option<String>,
}

// ============================================================================
// Service (In-Memory)
// ============================================================================

/// In-memory peering service
pub struct ConsulPeeringService {
    /// Peerings by name
    peerings: Arc<DashMap<String, Peering>>,
    /// Index counter
    index: std::sync::atomic::AtomicU64,
    /// Datacenter name
    datacenter: String,
    /// Consul compatibility HTTP port (default 8500)
    consul_port: u16,
    /// Optional RocksDB persistence
    rocks_db: Option<Arc<DB>>,
    /// Optional Raft writer for cluster-mode replication
    raft_node: Option<Arc<ConsulRaftWriter>>,
}

impl ConsulPeeringService {
/// The `new` associated function.
    pub fn new() -> Self {
        Self::with_datacenter("dc1".to_string())
    }

/// The `with_datacenter` associated function.
    pub fn with_datacenter(datacenter: String) -> Self {
        Self {
            peerings: Arc::new(DashMap::new()),
            index: std::sync::atomic::AtomicU64::new(1),
            datacenter,
            consul_port: 8500,
            rocks_db: None,
            raft_node: None,
        }
    }

/// The `with_consul_port` method.
    pub fn with_consul_port(mut self, port: u16) -> Self {
        self.consul_port = port;
        self
    }

/// The `with_rocks` associated function.
    pub fn with_rocks(db: Arc<DB>, datacenter: String, consul_port: u16) -> Self {
        let peerings = Arc::new(DashMap::new());
        let mut max_index = 1u64;

        // Load from RocksDB
        if let Some(cf) = db.cf_handle(CF_CONSUL_PEERING) {
            let iter = db.iterator_cf(cf, rocksdb::IteratorMode::Start);
            let mut count = 0u64;

            for item in iter.flatten() {
                let (key_bytes, value_bytes) = item;
                if let Ok(key) = String::from_utf8(key_bytes.to_vec()) {
                    if let Ok(peering) = serde_json::from_slice::<Peering>(&value_bytes) {
                        if peering.modify_index > max_index {
                            max_index = peering.modify_index;
                        }
                        peerings.insert(key, peering);
                        count += 1;
                    } else {
                        warn!(
                            "Failed to deserialize peering entry: {}",
                            String::from_utf8_lossy(&key_bytes)
                        );
                    }
                }
            }
            info!("Loaded {} peering entries from RocksDB", count);
        }

        Self {
            peerings,
            index: std::sync::atomic::AtomicU64::new(max_index + 1),
            datacenter,
            consul_port,
            rocks_db: Some(db),
            raft_node: None,
        }
    }

    /// Create a peering service with Raft-replicated storage (cluster mode).
    pub fn with_raft(
        db: Arc<DB>,
        raft_node: Arc<ConsulRaftWriter>,
        datacenter: String,
        consul_port: u16,
    ) -> Self {
        let mut svc = Self::with_rocks(db, datacenter, consul_port);
        svc.raft_node = Some(raft_node);
        svc
    }

/// The `generate_token` method.
    pub async fn generate_token(
        &self,
        req: PeeringGenerateTokenRequest,
    ) -> Result<PeeringGenerateTokenResponse, String> {
        if req.peer_name.is_empty() {
            return Err("PeerName is required".to_string());
        }

        let peer_id = uuid::Uuid::new_v4().to_string();
        let secret = uuid::Uuid::new_v4().to_string();
        let index = self.index.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        // Create the peering in PENDING state
        let peering = Peering {
            id: uuid::Uuid::new_v4().to_string(),
            name: req.peer_name.clone(),
            partition: req.partition,
            state: PeeringState::Pending,
            peer_id: peer_id.clone(),
            peer_server_name: String::new(),
            peer_server_addresses: Vec::new(),
            peer_ca_pems: Vec::new(),
            meta: req.meta,
            stream_status: PeeringStreamStatus::default(),
            create_index: index,
            modify_index: index,
            remote: PeeringRemoteInfo::default(),
            deleted_at: None,
        };
        self.peerings.insert(req.peer_name.clone(), peering.clone());
        if let Some(ref raft) = self.raft_node {
            let peering_json = serde_json::to_string(&peering).unwrap_or_default();
            match raft
                .write(ConsulRaftRequest::PeeringWrite {
                    name: req.peer_name.clone(),
                    peering_json,
                })
                .await
            {
                Ok(r) if !r.success => {
                    error!("Raft PeeringWrite rejected: {:?}", r.message);
                }
                Err(e) => {
                    error!("Raft PeeringWrite failed: {}", e);
                }
                _ => {}
            }
        } else {
            self.persist_to_rocks(&req.peer_name, &peering);
        }

        // Generate the token
        let token = PeeringToken {
            ca: Vec::new(),
            server_addresses: if req.server_external_addresses.is_empty() {
                let hostname = hostname::get()
                    .map(|h| h.to_string_lossy().to_string())
                    .unwrap_or_else(|_| "127.0.0.1".to_string());
                vec![format!("{}:{}", hostname, self.consul_port)]
            } else {
                req.server_external_addresses
            },
            server_name: format!("server.{}.consul", self.datacenter),
            peer_id,
            establishment_secret: secret,
            remote: PeeringRemoteInfo {
                partition: "default".to_string(),
                datacenter: self.datacenter.clone(),
            },
        };

        let token_json = serde_json::to_vec(&token).map_err(|e| e.to_string())?;
        let token_b64 =
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &token_json);

        Ok(PeeringGenerateTokenResponse {
            peering_token: token_b64,
        })
    }

/// The `establish` method.
    pub async fn establish(&self, req: PeeringEstablishRequest) -> Result<(), String> {
        if req.peer_name.is_empty() {
            return Err("PeerName is required".to_string());
        }
        if req.peering_token.is_empty() {
            return Err("PeeringToken is required".to_string());
        }

        // Decode the token
        let token_bytes = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            &req.peering_token,
        )
        .map_err(|e| format!("Invalid peering token: {}", e))?;

        let token: PeeringToken = serde_json::from_slice(&token_bytes)
            .map_err(|e| format!("Invalid token data: {}", e))?;

        let index = self.index.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let now = Utc::now().to_rfc3339();

        let peering = Peering {
            id: uuid::Uuid::new_v4().to_string(),
            name: req.peer_name.clone(),
            partition: req.partition,
            state: PeeringState::Active,
            peer_id: token.peer_id,
            peer_server_name: token.server_name,
            peer_server_addresses: token.server_addresses,
            peer_ca_pems: token.ca,
            meta: req.meta,
            stream_status: PeeringStreamStatus {
                imported_services: Vec::new(),
                exported_services: Vec::new(),
                last_heartbeat: Some(now.clone()),
                last_receive: Some(now.clone()),
                last_send: Some(now),
            },
            create_index: index,
            modify_index: index,
            remote: token.remote,
            deleted_at: None,
        };

        self.peerings.insert(req.peer_name.clone(), peering.clone());
        if let Some(ref raft) = self.raft_node {
            let peering_json = serde_json::to_string(&peering).unwrap_or_default();
            match raft
                .write(ConsulRaftRequest::PeeringWrite {
                    name: req.peer_name.clone(),
                    peering_json,
                })
                .await
            {
                Ok(r) if !r.success => {
                    error!("Raft PeeringWrite rejected: {:?}", r.message);
                }
                Err(e) => {
                    error!("Raft PeeringWrite failed: {}", e);
                }
                _ => {}
            }
        } else {
            self.persist_to_rocks(&req.peer_name, &peering);
        }
        Ok(())
    }

/// The `get_peering` method.
    pub fn get_peering(&self, name: &str) -> Option<Peering> {
        self.peerings
            .get(name)
            .filter(|p| p.deleted_at.is_none())
            .map(|p| p.value().clone())
    }

/// The `list_peerings` method.
    pub fn list_peerings(&self) -> Vec<Peering> {
        let mut peerings: Vec<Peering> = self
            .peerings
            .iter()
            .filter(|r| r.value().deleted_at.is_none())
            .map(|r| r.value().clone())
            .collect();
        peerings.sort_by(|a, b| a.name.cmp(&b.name));
        peerings
    }

/// The `delete_peering` method.
    pub async fn delete_peering(&self, name: &str) -> bool {
        if let Some(mut peering) = self.peerings.get_mut(name) {
            peering.state = PeeringState::Deleting;
            peering.deleted_at = Some(Utc::now().to_rfc3339());
            peering.modify_index = self.index.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let updated = peering.clone();
            drop(peering);
            if let Some(ref raft) = self.raft_node {
                let peering_json = serde_json::to_string(&updated).unwrap_or_default();
                match raft
                    .write(ConsulRaftRequest::PeeringWrite {
                        name: name.to_string(),
                        peering_json,
                    })
                    .await
                {
                    Ok(r) if !r.success => {
                        error!("Raft PeeringWrite rejected: {:?}", r.message);
                    }
                    Err(e) => {
                        error!("Raft PeeringWrite failed: {}", e);
                    }
                    _ => {}
                }
            } else {
                self.persist_to_rocks(name, &updated);
            }
            true
        } else {
            false
        }
    }

    /// Persist a peering entry to RocksDB
    fn persist_to_rocks(&self, name: &str, peering: &Peering) {
        if let Some(ref db) = self.rocks_db
            && let Some(cf) = db.cf_handle(CF_CONSUL_PEERING)
        {
            match serde_json::to_vec(peering) {
                Ok(bytes) => {
                    if let Err(e) = db.put_cf(cf, name.as_bytes(), &bytes) {
                        error!("Failed to persist peering '{}': {}", name, e);
                    }
                }
                Err(e) => {
                    error!("Failed to serialize peering '{}': {}", name, e);
                }
            }
        }
    }

    /// Delete a peering entry from RocksDB
    #[allow(dead_code)]
    fn delete_from_rocks(&self, name: &str) {
        if let Some(ref db) = self.rocks_db
            && let Some(cf) = db.cf_handle(CF_CONSUL_PEERING)
            && let Err(e) = db.delete_cf(cf, name.as_bytes())
        {
            error!("Failed to delete peering '{}': {}", name, e);
        }
    }
}

impl Default for ConsulPeeringService {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// HTTP Handlers (In-Memory)
// ============================================================================

/// POST /v1/peering/token - Generate a peering token
pub async fn generate_peering_token(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    peering_service: web::Data<ConsulPeeringService>,
    body: web::Json<PeeringGenerateTokenRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    match peering_service.generate_token(body.into_inner()).await {
        Ok(resp) => {
            let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Peering));
            consul_ok(&meta).json(resp)
        }
        Err(e) => HttpResponse::BadRequest().consul_error(ConsulError::new(e)),
    }
}

/// POST /v1/peering/establish - Establish a peering
pub async fn establish_peering(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    peering_service: web::Data<ConsulPeeringService>,
    body: web::Json<PeeringEstablishRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    match peering_service.establish(body.into_inner()).await {
        Ok(()) => {
            let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Peering));
            consul_ok(&meta).json(serde_json::json!({}))
        }
        Err(e) => HttpResponse::BadRequest().consul_error(ConsulError::new(e)),
    }
}

/// GET /v1/peering/{name} - Read a peering
pub async fn get_peering(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    peering_service: web::Data<ConsulPeeringService>,
    path: web::Path<String>,
    _query: web::Query<PeeringQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let name = path.into_inner();
    if name.is_empty() {
        return HttpResponse::BadRequest()
            .consul_error(ConsulError::new("Peering name is required"));
    }

    match peering_service.get_peering(&name) {
        Some(peering) => {
            let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Peering));
            consul_ok(&meta).json(peering)
        }
        None => HttpResponse::NotFound()
            .consul_error(ConsulError::new(format!("Peering '{}' not found", name))),
    }
}

/// DELETE /v1/peering/{name} - Delete a peering
pub async fn delete_peering(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    peering_service: web::Data<ConsulPeeringService>,
    path: web::Path<String>,
    _query: web::Query<PeeringQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let name = path.into_inner();
    if name.is_empty() {
        return HttpResponse::BadRequest()
            .consul_error(ConsulError::new("Peering name is required"));
    }

    if peering_service.delete_peering(&name).await {
        let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Peering));
        consul_ok(&meta).finish()
    } else {
        HttpResponse::NotFound()
            .consul_error(ConsulError::new(format!("Peering '{}' not found", name)))
    }
}

/// GET /v1/peerings - List all peerings
pub async fn list_peerings(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    peering_service: web::Data<ConsulPeeringService>,
    _query: web::Query<PeeringQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Peering));
    consul_ok(&meta).json(peering_service.list_peerings())
}

// ============================================================================
// HTTP Handlers (Persistent)
// ============================================================================

/// POST /v1/peering/token (persistent)
pub async fn generate_peering_token_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    peering_service: web::Data<ConsulPeeringService>,
    body: web::Json<PeeringGenerateTokenRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    match peering_service.generate_token(body.into_inner()).await {
        Ok(resp) => {
            let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Peering));
            consul_ok(&meta).json(resp)
        }
        Err(e) => HttpResponse::BadRequest().consul_error(ConsulError::new(e)),
    }
}

/// POST /v1/peering/establish (persistent)
pub async fn establish_peering_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    peering_service: web::Data<ConsulPeeringService>,
    body: web::Json<PeeringEstablishRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    match peering_service.establish(body.into_inner()).await {
        Ok(()) => {
            let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Peering));
            consul_ok(&meta).json(serde_json::json!({}))
        }
        Err(e) => HttpResponse::BadRequest().consul_error(ConsulError::new(e)),
    }
}

/// GET /v1/peering/{name} (persistent)
pub async fn get_peering_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    peering_service: web::Data<ConsulPeeringService>,
    path: web::Path<String>,
    _query: web::Query<PeeringQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let name = path.into_inner();
    if name.is_empty() {
        return HttpResponse::BadRequest()
            .consul_error(ConsulError::new("Peering name is required"));
    }

    match peering_service.get_peering(&name) {
        Some(peering) => {
            let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Peering));
            consul_ok(&meta).json(peering)
        }
        None => HttpResponse::NotFound()
            .consul_error(ConsulError::new(format!("Peering '{}' not found", name))),
    }
}

/// DELETE /v1/peering/{name} (persistent)
pub async fn delete_peering_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    peering_service: web::Data<ConsulPeeringService>,
    path: web::Path<String>,
    _query: web::Query<PeeringQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let name = path.into_inner();
    if name.is_empty() {
        return HttpResponse::BadRequest()
            .consul_error(ConsulError::new("Peering name is required"));
    }

    if peering_service.delete_peering(&name).await {
        let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Peering));
        consul_ok(&meta).finish()
    } else {
        HttpResponse::NotFound()
            .consul_error(ConsulError::new(format!("Peering '{}' not found", name)))
    }
}

/// GET /v1/peerings (persistent)
pub async fn list_peerings_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    peering_service: web::Data<ConsulPeeringService>,
    _query: web::Query<PeeringQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Peering));
    consul_ok(&meta).json(peering_service.list_peerings())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_generate_token() {
        let service = ConsulPeeringService::new();
        let result = service
            .generate_token(PeeringGenerateTokenRequest {
                peer_name: "cluster-02".to_string(),
                partition: String::new(),
                meta: Default::default(),
                server_external_addresses: vec![],
            })
            .await;
        assert!(result.is_ok());
        let resp = result.unwrap();
        assert!(!resp.peering_token.is_empty());

        // Peering should be in PENDING state
        let peering = service.get_peering("cluster-02").unwrap();
        assert_eq!(peering.state, PeeringState::Pending);
    }

    #[tokio::test]
    async fn test_generate_token_empty_name() {
        let service = ConsulPeeringService::new();
        let result = service
            .generate_token(PeeringGenerateTokenRequest {
                peer_name: String::new(),
                partition: String::new(),
                meta: Default::default(),
                server_external_addresses: vec![],
            })
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_establish_peering() {
        let service = ConsulPeeringService::new();

        // First generate a token
        let token_resp = service
            .generate_token(PeeringGenerateTokenRequest {
                peer_name: "cluster-02".to_string(),
                partition: String::new(),
                meta: Default::default(),
                server_external_addresses: vec![],
            })
            .await
            .unwrap();

        // Create another service and establish
        let service2 = ConsulPeeringService::new();
        let result = service2
            .establish(PeeringEstablishRequest {
                peer_name: "cluster-01".to_string(),
                peering_token: token_resp.peering_token,
                partition: String::new(),
                meta: Default::default(),
            })
            .await;
        assert!(result.is_ok());

        let peering = service2.get_peering("cluster-01").unwrap();
        assert_eq!(peering.state, PeeringState::Active);
    }

    #[tokio::test]
    async fn test_list_peerings() {
        let service = ConsulPeeringService::new();
        service
            .generate_token(PeeringGenerateTokenRequest {
                peer_name: "peer-b".to_string(),
                partition: String::new(),
                meta: Default::default(),
                server_external_addresses: vec![],
            })
            .await
            .unwrap();
        service
            .generate_token(PeeringGenerateTokenRequest {
                peer_name: "peer-a".to_string(),
                partition: String::new(),
                meta: Default::default(),
                server_external_addresses: vec![],
            })
            .await
            .unwrap();

        let peerings = service.list_peerings();
        assert_eq!(peerings.len(), 2);
        // Should be sorted by name
        assert_eq!(peerings[0].name, "peer-a");
        assert_eq!(peerings[1].name, "peer-b");
    }

    #[tokio::test]
    async fn test_delete_peering() {
        let service = ConsulPeeringService::new();
        service
            .generate_token(PeeringGenerateTokenRequest {
                peer_name: "to-delete".to_string(),
                partition: String::new(),
                meta: Default::default(),
                server_external_addresses: vec![],
            })
            .await
            .unwrap();

        assert!(service.delete_peering("to-delete").await);
        // Should not appear in list after deletion
        assert!(service.get_peering("to-delete").is_none());
        assert!(service.list_peerings().is_empty());
    }

    #[tokio::test]
    async fn test_delete_nonexistent() {
        let service = ConsulPeeringService::new();
        assert!(!service.delete_peering("nonexistent").await);
    }

    #[test]
    fn test_get_nonexistent_peering() {
        let service = ConsulPeeringService::new();
        assert!(service.get_peering("nonexistent").is_none());
    }

    #[tokio::test]
    async fn test_generate_token_creates_pending_peering() {
        let service = ConsulPeeringService::new();
        service
            .generate_token(PeeringGenerateTokenRequest {
                peer_name: "pending-peer".to_string(),
                partition: String::new(),
                meta: Default::default(),
                server_external_addresses: vec![],
            })
            .await
            .unwrap();

        let peering = service.get_peering("pending-peer").unwrap();
        assert_eq!(peering.state, PeeringState::Pending);
        assert_eq!(peering.name, "pending-peer");
        assert!(!peering.id.is_empty());
    }

    #[tokio::test]
    async fn test_generate_token_duplicate_name_overwrites() {
        let service = ConsulPeeringService::new();

        service
            .generate_token(PeeringGenerateTokenRequest {
                peer_name: "dup-peer".to_string(),
                partition: String::new(),
                meta: Default::default(),
                server_external_addresses: vec![],
            })
            .await
            .unwrap();

        // Second token for same name overwrites (insert into DashMap)
        let result = service
            .generate_token(PeeringGenerateTokenRequest {
                peer_name: "dup-peer".to_string(),
                partition: String::new(),
                meta: Default::default(),
                server_external_addresses: vec![],
            })
            .await;
        assert!(result.is_ok());

        // Should still only be one peering
        assert_eq!(
            service
                .list_peerings()
                .iter()
                .filter(|p| p.name == "dup-peer")
                .count(),
            1
        );
    }

    #[tokio::test]
    async fn test_establish_with_invalid_token() {
        let service = ConsulPeeringService::new();

        let result = service
            .establish(PeeringEstablishRequest {
                peer_name: "bad-peer".to_string(),
                peering_token: "not-valid-base64!@#$".to_string(),
                partition: String::new(),
                meta: Default::default(),
            })
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_peering_with_meta() {
        let service = ConsulPeeringService::new();
        let mut meta = std::collections::HashMap::new();
        meta.insert("env".to_string(), "production".to_string());

        service
            .generate_token(PeeringGenerateTokenRequest {
                peer_name: "meta-peer".to_string(),
                partition: String::new(),
                meta,
                server_external_addresses: vec![],
            })
            .await
            .unwrap();

        let peering = service.get_peering("meta-peer").unwrap();
        assert_eq!(peering.meta.get("env").unwrap(), "production");
    }

    #[tokio::test]
    async fn test_generate_token_with_external_addresses() {
        let service = ConsulPeeringService::new();

        let resp = service
            .generate_token(PeeringGenerateTokenRequest {
                peer_name: "addr-peer".to_string(),
                partition: String::new(),
                meta: Default::default(),
                server_external_addresses: vec![
                    "10.0.1.1:8502".to_string(),
                    "10.0.1.2:8502".to_string(),
                ],
            })
            .await
            .unwrap();

        // Token should contain the addresses (base64 encoded)
        assert!(!resp.peering_token.is_empty());

        // The peering itself stores addresses in the token, not in peer_server_addresses
        // peer_server_addresses is populated when establishing from the remote side
        let peering = service.get_peering("addr-peer").unwrap();
        assert_eq!(peering.state, PeeringState::Pending);
    }
}
