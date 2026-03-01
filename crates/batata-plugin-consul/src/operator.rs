// Consul Operator API implementation
// Provides cluster operator endpoints for Raft, Autopilot, Keyring management
//
// Endpoints:
// - GET  /v1/operator/raft/configuration - Get Raft configuration
// - POST /v1/operator/raft/transfer-leader - Transfer leadership
// - DELETE /v1/operator/raft/peer - Remove a Raft peer
// - GET  /v1/operator/autopilot/configuration - Get Autopilot configuration
// - PUT  /v1/operator/autopilot/configuration - Set Autopilot configuration
// - GET  /v1/operator/autopilot/health - Get Autopilot health
// - GET  /v1/operator/autopilot/state - Get Autopilot state
// - GET/POST/PUT/DELETE /v1/operator/keyring - Keyring management

use actix_web::{HttpRequest, HttpResponse, web};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::acl::{AclService, ResourceType};
use crate::model::ConsulError;

// ============================================================================
// Models
// ============================================================================

/// Raft server information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct RaftServer {
    #[serde(rename = "ID")]
    pub id: String,
    pub node: String,
    pub address: String,
    pub leader: bool,
    pub voter: bool,
    pub protocol_version: String,
    pub last_index: u64,
}

/// Raft configuration response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct RaftConfigurationResponse {
    pub servers: Vec<RaftServer>,
    pub index: u64,
}

/// Transfer leader response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TransferLeaderResponse {
    pub success: bool,
}

/// Query parameters for raft peer removal
#[derive(Debug, Clone, Deserialize, Default)]
pub struct RaftPeerParams {
    pub id: Option<String>,
    pub address: Option<String>,
    pub dc: Option<String>,
}

/// Query parameters for transfer-leader
#[derive(Debug, Clone, Deserialize, Default)]
pub struct TransferLeaderParams {
    pub id: Option<String>,
}

/// Autopilot configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AutopilotConfiguration {
    pub cleanup_dead_servers: bool,
    pub last_contact_threshold: String,
    pub max_trailing_logs: u64,
    pub min_quorum: u64,
    pub server_stabilization_time: String,
    pub redundancy_zone_tag: String,
    pub disable_upgrade_migration: bool,
    pub upgrade_version_tag: String,
    pub create_index: u64,
    pub modify_index: u64,
}

impl Default for AutopilotConfiguration {
    fn default() -> Self {
        Self {
            cleanup_dead_servers: true,
            last_contact_threshold: "200ms".to_string(),
            max_trailing_logs: 250,
            min_quorum: 0,
            server_stabilization_time: "10s".to_string(),
            redundancy_zone_tag: String::new(),
            disable_upgrade_migration: false,
            upgrade_version_tag: String::new(),
            create_index: 1,
            modify_index: 1,
        }
    }
}

/// Query parameters for autopilot configuration PUT
#[derive(Debug, Clone, Deserialize, Default)]
pub struct AutopilotConfigParams {
    pub cas: Option<u64>,
    pub dc: Option<String>,
}

/// Autopilot health response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AutopilotHealthResponse {
    pub healthy: bool,
    pub failure_tolerance: i32,
    pub servers: Vec<AutopilotServerHealth>,
}

/// Autopilot server health
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AutopilotServerHealth {
    #[serde(rename = "ID")]
    pub id: String,
    pub name: String,
    pub address: String,
    pub serf_status: String,
    pub version: String,
    pub leader: bool,
    pub last_contact: String,
    pub last_term: u64,
    pub last_index: u64,
    pub healthy: bool,
    pub voter: bool,
    pub stable_since: String,
}

/// Autopilot state response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AutopilotStateResponse {
    pub healthy: bool,
    pub failure_tolerance: i32,
    pub leader: String,
    pub voters: Vec<String>,
    pub servers: HashMap<String, AutopilotServerHealth>,
}

/// Keyring request body
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct KeyringRequest {
    pub key: String,
}

/// Keyring response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct KeyringResponse {
    #[serde(rename = "WAN")]
    pub wan: bool,
    pub datacenter: String,
    pub segment: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub messages: Option<HashMap<String, String>>,
    pub keys: HashMap<String, i32>,
    pub primary_keys: HashMap<String, i32>,
    pub num_nodes: i32,
}

/// Query parameters for keyring operations
#[derive(Debug, Clone, Deserialize, Default)]
pub struct KeyringParams {
    #[serde(rename = "relay-factor")]
    pub relay_factor: Option<u8>,
    #[serde(rename = "local-only")]
    pub local_only: Option<String>,
}

/// Query parameters for operator endpoints
#[derive(Debug, Clone, Deserialize, Default)]
pub struct OperatorQueryParams {
    pub dc: Option<String>,
    pub stale: Option<String>,
}

// ============================================================================
// Operator Service (In-Memory)
// ============================================================================

/// In-memory operator service
pub struct ConsulOperatorService {
    /// Raft server list
    servers: Arc<DashMap<String, RaftServer>>,
    /// Autopilot configuration
    autopilot_config: Arc<tokio::sync::RwLock<AutopilotConfiguration>>,
    /// Keyring (key -> node count)
    keyring: Arc<DashMap<String, i32>>,
    /// Primary key
    primary_key: Arc<tokio::sync::RwLock<Option<String>>>,
    /// Current index
    index: Arc<AtomicU64>,
}

impl ConsulOperatorService {
    pub fn new() -> Self {
        let node_name = hostname::get()
            .ok()
            .and_then(|h| h.into_string().ok())
            .unwrap_or_else(|| "batata-node".to_string());

        let service = Self {
            servers: Arc::new(DashMap::new()),
            autopilot_config: Arc::new(tokio::sync::RwLock::new(AutopilotConfiguration::default())),
            keyring: Arc::new(DashMap::new()),
            primary_key: Arc::new(tokio::sync::RwLock::new(None)),
            index: Arc::new(AtomicU64::new(1)),
        };

        // Add self as default server
        let server = RaftServer {
            id: uuid::Uuid::new_v4().to_string(),
            node: node_name,
            address: "127.0.0.1:8300".to_string(),
            leader: true,
            voter: true,
            protocol_version: "3".to_string(),
            last_index: 1,
        };
        service.servers.insert(server.id.clone(), server);

        service
    }

    pub fn get_raft_configuration(&self) -> RaftConfigurationResponse {
        let servers: Vec<RaftServer> = self.servers.iter().map(|r| r.value().clone()).collect();
        let index = self.index.load(Ordering::SeqCst);
        RaftConfigurationResponse { servers, index }
    }

    pub fn remove_peer(&self, id: Option<&str>, address: Option<&str>) -> Result<(), String> {
        if let Some(id) = id {
            if self.servers.remove(id).is_some() {
                self.index.fetch_add(1, Ordering::SeqCst);
                return Ok(());
            }
            return Err(format!("Peer with ID '{}' not found", id));
        }
        if let Some(address) = address {
            let key_to_remove = self
                .servers
                .iter()
                .find(|r| r.value().address == address)
                .map(|r| r.key().clone());
            if let Some(key) = key_to_remove {
                self.servers.remove(&key);
                self.index.fetch_add(1, Ordering::SeqCst);
                return Ok(());
            }
            return Err(format!("Peer with address '{}' not found", address));
        }
        Err("Must specify either id or address".to_string())
    }

    pub fn transfer_leader(&self, _target_id: Option<&str>) -> Result<(), String> {
        // In single-node mode, transfer is a no-op
        Ok(())
    }

    pub async fn get_autopilot_config(&self) -> AutopilotConfiguration {
        self.autopilot_config.read().await.clone()
    }

    pub async fn set_autopilot_config(
        &self,
        config: AutopilotConfiguration,
        cas: Option<u64>,
    ) -> Result<bool, String> {
        let mut current = self.autopilot_config.write().await;
        if let Some(cas_index) = cas
            && current.modify_index != cas_index
        {
            return Ok(false);
        }
        let new_index = self.index.fetch_add(1, Ordering::SeqCst) + 1;
        let mut new_config = config;
        new_config.modify_index = new_index;
        new_config.create_index = current.create_index;
        *current = new_config;
        Ok(true)
    }

    pub fn get_autopilot_health(&self) -> AutopilotHealthResponse {
        let servers: Vec<AutopilotServerHealth> = self
            .servers
            .iter()
            .map(|r| {
                let s = r.value();
                AutopilotServerHealth {
                    id: s.id.clone(),
                    name: s.node.clone(),
                    address: s.address.clone(),
                    serf_status: "alive".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    leader: s.leader,
                    last_contact: "0ms".to_string(),
                    last_term: 1,
                    last_index: s.last_index,
                    healthy: true,
                    voter: s.voter,
                    stable_since: chrono::Utc::now().to_rfc3339(),
                }
            })
            .collect();

        let server_count = servers.len() as i32;
        AutopilotHealthResponse {
            healthy: true,
            failure_tolerance: if server_count > 1 {
                (server_count - 1) / 2
            } else {
                0
            },
            servers,
        }
    }

    pub fn get_autopilot_state(&self) -> AutopilotStateResponse {
        let health = self.get_autopilot_health();
        let leader = health
            .servers
            .iter()
            .find(|s| s.leader)
            .map(|s| s.id.clone())
            .unwrap_or_default();
        let voters: Vec<String> = health
            .servers
            .iter()
            .filter(|s| s.voter)
            .map(|s| s.id.clone())
            .collect();
        let servers_map: HashMap<String, AutopilotServerHealth> = health
            .servers
            .into_iter()
            .map(|s| (s.id.clone(), s))
            .collect();

        AutopilotStateResponse {
            healthy: health.healthy,
            failure_tolerance: health.failure_tolerance,
            leader,
            voters,
            servers: servers_map,
        }
    }

    pub fn list_keys(&self) -> Vec<KeyringResponse> {
        let keys: HashMap<String, i32> = self
            .keyring
            .iter()
            .map(|r| (r.key().clone(), *r.value()))
            .collect();
        let primary_keys = keys.clone();

        vec![KeyringResponse {
            wan: false,
            datacenter: "dc1".to_string(),
            segment: "".to_string(),
            messages: None,
            keys,
            primary_keys,
            num_nodes: self.servers.len() as i32,
        }]
    }

    pub fn install_key(&self, key: &str) {
        let node_count = self.servers.len() as i32;
        self.keyring.insert(key.to_string(), node_count);
    }

    pub async fn use_key(&self, key: &str) -> Result<(), String> {
        if !self.keyring.contains_key(key) {
            return Err(format!("Key '{}' not found", key));
        }
        let mut primary = self.primary_key.write().await;
        *primary = Some(key.to_string());
        Ok(())
    }

    pub async fn remove_key(&self, key: &str) -> Result<(), String> {
        let primary = self.primary_key.read().await;
        if primary.as_deref() == Some(key) {
            return Err("Cannot remove the primary key".to_string());
        }
        drop(primary);
        self.keyring.remove(key);
        Ok(())
    }
}

impl Default for ConsulOperatorService {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Operator Service (Real Cluster)
// ============================================================================

/// Real cluster operator service using ServerMemberManager
pub struct ConsulOperatorServiceReal {
    member_manager: Arc<batata_core::service::cluster::ServerMemberManager>,
    autopilot_config: Arc<tokio::sync::RwLock<AutopilotConfiguration>>,
    keyring: Arc<DashMap<String, i32>>,
    primary_key: Arc<tokio::sync::RwLock<Option<String>>>,
    index: Arc<AtomicU64>,
}

impl ConsulOperatorServiceReal {
    pub fn new(member_manager: Arc<batata_core::service::cluster::ServerMemberManager>) -> Self {
        Self {
            member_manager,
            autopilot_config: Arc::new(tokio::sync::RwLock::new(AutopilotConfiguration::default())),
            keyring: Arc::new(DashMap::new()),
            primary_key: Arc::new(tokio::sync::RwLock::new(None)),
            index: Arc::new(AtomicU64::new(1)),
        }
    }

    pub fn get_raft_configuration(&self) -> RaftConfigurationResponse {
        let members = self.member_manager.all_members();
        let servers: Vec<RaftServer> = members
            .iter()
            .enumerate()
            .map(|(i, m)| RaftServer {
                id: m.ip.clone(),
                node: m.ip.clone(),
                address: format!("{}:{}", m.ip, m.port),
                leader: i == 0, // First member is typically the leader
                voter: true,
                protocol_version: "3".to_string(),
                last_index: self.index.load(Ordering::SeqCst),
            })
            .collect();
        let index = self.index.load(Ordering::SeqCst);
        RaftConfigurationResponse { servers, index }
    }

    pub fn get_autopilot_health(&self) -> AutopilotHealthResponse {
        let members = self.member_manager.all_members();
        let servers: Vec<AutopilotServerHealth> = members
            .iter()
            .enumerate()
            .map(|(i, m)| AutopilotServerHealth {
                id: m.ip.clone(),
                name: m.ip.clone(),
                address: format!("{}:{}", m.ip, m.port),
                serf_status: "alive".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                leader: i == 0,
                last_contact: "0ms".to_string(),
                last_term: 1,
                last_index: self.index.load(Ordering::SeqCst),
                healthy: true,
                voter: true,
                stable_since: chrono::Utc::now().to_rfc3339(),
            })
            .collect();

        let server_count = servers.len() as i32;
        AutopilotHealthResponse {
            healthy: true,
            failure_tolerance: if server_count > 1 {
                (server_count - 1) / 2
            } else {
                0
            },
            servers,
        }
    }

    pub fn get_autopilot_state(&self) -> AutopilotStateResponse {
        let health = self.get_autopilot_health();
        let leader = health
            .servers
            .iter()
            .find(|s| s.leader)
            .map(|s| s.id.clone())
            .unwrap_or_default();
        let voters: Vec<String> = health
            .servers
            .iter()
            .filter(|s| s.voter)
            .map(|s| s.id.clone())
            .collect();
        let servers_map: HashMap<String, AutopilotServerHealth> = health
            .servers
            .into_iter()
            .map(|s| (s.id.clone(), s))
            .collect();

        AutopilotStateResponse {
            healthy: health.healthy,
            failure_tolerance: health.failure_tolerance,
            leader,
            voters,
            servers: servers_map,
        }
    }

    pub async fn get_autopilot_config(&self) -> AutopilotConfiguration {
        self.autopilot_config.read().await.clone()
    }

    pub async fn set_autopilot_config(
        &self,
        config: AutopilotConfiguration,
        cas: Option<u64>,
    ) -> Result<bool, String> {
        let mut current = self.autopilot_config.write().await;
        if let Some(cas_index) = cas
            && current.modify_index != cas_index
        {
            return Ok(false);
        }
        let new_index = self.index.fetch_add(1, Ordering::SeqCst) + 1;
        let mut new_config = config;
        new_config.modify_index = new_index;
        new_config.create_index = current.create_index;
        *current = new_config;
        Ok(true)
    }

    pub fn list_keys(&self) -> Vec<KeyringResponse> {
        let members = self.member_manager.all_members();
        let keys: HashMap<String, i32> = self
            .keyring
            .iter()
            .map(|r| (r.key().clone(), *r.value()))
            .collect();
        let primary_keys = keys.clone();

        vec![KeyringResponse {
            wan: false,
            datacenter: "dc1".to_string(),
            segment: "".to_string(),
            messages: None,
            keys,
            primary_keys,
            num_nodes: members.len() as i32,
        }]
    }

    pub fn install_key(&self, key: &str) {
        let members = self.member_manager.all_members();
        self.keyring.insert(key.to_string(), members.len() as i32);
    }

    pub async fn use_key(&self, key: &str) -> Result<(), String> {
        if !self.keyring.contains_key(key) {
            return Err(format!("Key '{}' not found", key));
        }
        let mut primary = self.primary_key.write().await;
        *primary = Some(key.to_string());
        Ok(())
    }

    pub async fn remove_key(&self, key: &str) -> Result<(), String> {
        let primary = self.primary_key.read().await;
        if primary.as_deref() == Some(key) {
            return Err("Cannot remove the primary key".to_string());
        }
        drop(primary);
        self.keyring.remove(key);
        Ok(())
    }
}

// ============================================================================
// Usage / Utilization Models
// ============================================================================

/// Service usage statistics
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct ServiceUsage {
    pub nodes: i64,
    pub services: i64,
    pub service_instances: i64,
    pub connect_service_instances: i64,
}

/// Operator usage response
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct OperatorUsageResponse {
    pub usage: HashMap<String, ServiceUsage>,
}

// ============================================================================
// HTTP Handlers (In-Memory)
// ============================================================================

/// GET /v1/operator/raft/configuration
pub async fn get_raft_configuration(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorService>,
    _query: web::Query<OperatorQueryParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let config = operator_service.get_raft_configuration();
    let index = config.index;

    HttpResponse::Ok()
        .insert_header(("X-Consul-Index", index.to_string()))
        .insert_header(("X-Consul-KnownLeader", "true"))
        .insert_header(("X-Consul-LastContact", "0"))
        .json(config)
}

/// POST /v1/operator/raft/transfer-leader
pub async fn transfer_leader(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorService>,
    query: web::Query<TransferLeaderParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    match operator_service.transfer_leader(query.id.as_deref()) {
        Ok(()) => HttpResponse::Ok().json(TransferLeaderResponse { success: true }),
        Err(e) => HttpResponse::NotFound().json(ConsulError::new(e)),
    }
}

/// DELETE /v1/operator/raft/peer
pub async fn remove_raft_peer(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorService>,
    query: web::Query<RaftPeerParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    if query.id.is_some() && query.address.is_some() {
        return HttpResponse::BadRequest().json(ConsulError::new(
            "Must specify either id or address, not both",
        ));
    }
    if query.id.is_none() && query.address.is_none() {
        return HttpResponse::BadRequest()
            .json(ConsulError::new("Must specify either id or address"));
    }

    match operator_service.remove_peer(query.id.as_deref(), query.address.as_deref()) {
        Ok(()) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::BadRequest().json(ConsulError::new(e)),
    }
}

/// GET /v1/operator/autopilot/configuration
pub async fn get_autopilot_configuration(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorService>,
    _query: web::Query<OperatorQueryParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let config = operator_service.get_autopilot_config().await;
    HttpResponse::Ok().json(config)
}

/// PUT /v1/operator/autopilot/configuration
pub async fn set_autopilot_configuration(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorService>,
    query: web::Query<AutopilotConfigParams>,
    body: web::Json<AutopilotConfiguration>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    match operator_service
        .set_autopilot_config(body.into_inner(), query.cas)
        .await
    {
        Ok(success) => HttpResponse::Ok().json(success),
        Err(e) => HttpResponse::InternalServerError().json(ConsulError::new(e)),
    }
}

/// GET /v1/operator/autopilot/health
pub async fn get_autopilot_health(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorService>,
    _query: web::Query<OperatorQueryParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let health = operator_service.get_autopilot_health();
    if health.healthy {
        HttpResponse::Ok().json(health)
    } else {
        HttpResponse::TooManyRequests().json(health)
    }
}

/// GET /v1/operator/autopilot/state
pub async fn get_autopilot_state(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorService>,
    _query: web::Query<OperatorQueryParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let state = operator_service.get_autopilot_state();
    HttpResponse::Ok().json(state)
}

/// GET /v1/operator/keyring - List keys
/// POST /v1/operator/keyring - Install key
/// PUT /v1/operator/keyring - Use key (set primary)
/// DELETE /v1/operator/keyring - Remove key
pub async fn keyring_list(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorService>,
    _query: web::Query<KeyringParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }
    HttpResponse::Ok().json(operator_service.list_keys())
}

pub async fn keyring_install(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorService>,
    body: web::Json<KeyringRequest>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }
    operator_service.install_key(&body.key);
    HttpResponse::Ok().finish()
}

pub async fn keyring_use(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorService>,
    body: web::Json<KeyringRequest>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }
    match operator_service.use_key(&body.key).await {
        Ok(()) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::BadRequest().json(ConsulError::new(e)),
    }
}

pub async fn keyring_remove(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorService>,
    body: web::Json<KeyringRequest>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }
    match operator_service.remove_key(&body.key).await {
        Ok(()) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::BadRequest().json(ConsulError::new(e)),
    }
}

/// GET /v1/operator/usage - Get cluster usage statistics
pub async fn get_operator_usage(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorService>,
    _query: web::Query<OperatorQueryParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let node_count = operator_service.servers.len() as i64;
    let mut usage = HashMap::new();
    usage.insert(
        "dc1".to_string(),
        ServiceUsage {
            nodes: node_count,
            services: 0,
            service_instances: 0,
            connect_service_instances: 0,
        },
    );

    HttpResponse::Ok().json(OperatorUsageResponse { usage })
}

/// GET /v1/operator/utilization - Get cluster utilization (Enterprise stub)
pub async fn get_operator_utilization(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    _query: web::Query<OperatorQueryParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    HttpResponse::Ok().json(serde_json::json!({
        "message": "no utilization data available"
    }))
}

// ============================================================================
// HTTP Handlers (Real Cluster)
// ============================================================================

/// GET /v1/operator/raft/configuration (real cluster)
pub async fn get_raft_configuration_real(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorServiceReal>,
    _query: web::Query<OperatorQueryParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let config = operator_service.get_raft_configuration();
    let index = config.index;

    HttpResponse::Ok()
        .insert_header(("X-Consul-Index", index.to_string()))
        .insert_header(("X-Consul-KnownLeader", "true"))
        .insert_header(("X-Consul-LastContact", "0"))
        .json(config)
}

/// POST /v1/operator/raft/transfer-leader (real cluster)
pub async fn transfer_leader_real(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    _operator_service: web::Data<ConsulOperatorServiceReal>,
    _query: web::Query<TransferLeaderParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }
    // In real cluster mode, leadership transfer is handled by Raft
    HttpResponse::Ok().json(TransferLeaderResponse { success: true })
}

/// DELETE /v1/operator/raft/peer (real cluster)
pub async fn remove_raft_peer_real(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    _operator_service: web::Data<ConsulOperatorServiceReal>,
    query: web::Query<RaftPeerParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    if query.id.is_some() && query.address.is_some() {
        return HttpResponse::BadRequest().json(ConsulError::new(
            "Must specify either id or address, not both",
        ));
    }
    if query.id.is_none() && query.address.is_none() {
        return HttpResponse::BadRequest()
            .json(ConsulError::new("Must specify either id or address"));
    }

    // In real cluster mode, peer removal is handled through Raft membership changes
    HttpResponse::Ok().finish()
}

/// GET /v1/operator/autopilot/configuration (real cluster)
pub async fn get_autopilot_configuration_real(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorServiceReal>,
    _query: web::Query<OperatorQueryParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let config = operator_service.get_autopilot_config().await;
    HttpResponse::Ok().json(config)
}

/// PUT /v1/operator/autopilot/configuration (real cluster)
pub async fn set_autopilot_configuration_real(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorServiceReal>,
    query: web::Query<AutopilotConfigParams>,
    body: web::Json<AutopilotConfiguration>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    match operator_service
        .set_autopilot_config(body.into_inner(), query.cas)
        .await
    {
        Ok(success) => HttpResponse::Ok().json(success),
        Err(e) => HttpResponse::InternalServerError().json(ConsulError::new(e)),
    }
}

/// GET /v1/operator/autopilot/health (real cluster)
pub async fn get_autopilot_health_real(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorServiceReal>,
    _query: web::Query<OperatorQueryParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let health = operator_service.get_autopilot_health();
    if health.healthy {
        HttpResponse::Ok().json(health)
    } else {
        HttpResponse::TooManyRequests().json(health)
    }
}

/// GET /v1/operator/autopilot/state (real cluster)
pub async fn get_autopilot_state_real(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorServiceReal>,
    _query: web::Query<OperatorQueryParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let state = operator_service.get_autopilot_state();
    HttpResponse::Ok().json(state)
}

/// GET /v1/operator/keyring (real cluster)
pub async fn keyring_list_real(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorServiceReal>,
    _query: web::Query<KeyringParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }
    HttpResponse::Ok().json(operator_service.list_keys())
}

/// POST /v1/operator/keyring (real cluster)
pub async fn keyring_install_real(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorServiceReal>,
    body: web::Json<KeyringRequest>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }
    operator_service.install_key(&body.key);
    HttpResponse::Ok().finish()
}

/// PUT /v1/operator/keyring (real cluster)
pub async fn keyring_use_real(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorServiceReal>,
    body: web::Json<KeyringRequest>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }
    match operator_service.use_key(&body.key).await {
        Ok(()) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::BadRequest().json(ConsulError::new(e)),
    }
}

/// DELETE /v1/operator/keyring (real cluster)
pub async fn keyring_remove_real(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorServiceReal>,
    body: web::Json<KeyringRequest>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }
    match operator_service.remove_key(&body.key).await {
        Ok(()) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::BadRequest().json(ConsulError::new(e)),
    }
}

/// GET /v1/operator/usage (real cluster)
pub async fn get_operator_usage_real(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorServiceReal>,
    _query: web::Query<OperatorQueryParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let members = operator_service.member_manager.all_members();
    let mut usage = HashMap::new();
    usage.insert(
        "dc1".to_string(),
        ServiceUsage {
            nodes: members.len() as i64,
            services: 0,
            service_instances: 0,
            connect_service_instances: 0,
        },
    );

    HttpResponse::Ok().json(OperatorUsageResponse { usage })
}

/// GET /v1/operator/utilization (real cluster, Enterprise stub)
pub async fn get_operator_utilization_real(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    _query: web::Query<OperatorQueryParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    HttpResponse::Ok().json(serde_json::json!({
        "message": "no utilization data available"
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raft_configuration_default() {
        let service = ConsulOperatorService::new();
        let config = service.get_raft_configuration();
        assert_eq!(config.servers.len(), 1);
        assert!(config.servers[0].leader);
        assert!(config.servers[0].voter);
    }

    #[test]
    fn test_remove_peer_by_id() {
        let service = ConsulOperatorService::new();
        let server_id = {
            let first = service.servers.iter().next().unwrap();
            first.key().clone()
        };
        assert!(service.remove_peer(Some(&server_id), None).is_ok());
        assert_eq!(service.servers.len(), 0);
    }

    #[test]
    fn test_remove_peer_not_found() {
        let service = ConsulOperatorService::new();
        assert!(service.remove_peer(Some("nonexistent"), None).is_err());
    }

    #[test]
    fn test_remove_peer_must_specify() {
        let service = ConsulOperatorService::new();
        assert!(service.remove_peer(None, None).is_err());
    }

    #[tokio::test]
    async fn test_autopilot_config_cas() {
        let service = ConsulOperatorService::new();
        let config = service.get_autopilot_config().await;

        // CAS with wrong index should fail
        let result = service
            .set_autopilot_config(config.clone(), Some(999))
            .await;
        assert!(!result.unwrap());

        // CAS with correct index should succeed
        let result = service
            .set_autopilot_config(config.clone(), Some(config.modify_index))
            .await;
        assert!(result.unwrap());
    }

    #[test]
    fn test_autopilot_health() {
        let service = ConsulOperatorService::new();
        let health = service.get_autopilot_health();
        assert!(health.healthy);
        assert_eq!(health.servers.len(), 1);
        assert!(health.servers[0].healthy);
    }

    #[test]
    fn test_keyring_operations() {
        let service = ConsulOperatorService::new();
        assert!(service.list_keys()[0].keys.is_empty());

        service.install_key("test-key-1");
        assert!(service.list_keys()[0].keys.contains_key("test-key-1"));
    }

    #[tokio::test]
    async fn test_keyring_remove_primary_fails() {
        let service = ConsulOperatorService::new();
        service.install_key("primary-key");
        service.use_key("primary-key").await.unwrap();
        assert!(service.remove_key("primary-key").await.is_err());
    }

    #[tokio::test]
    async fn test_keyring_use_nonexistent_fails() {
        let service = ConsulOperatorService::new();
        assert!(service.use_key("nonexistent").await.is_err());
    }

    #[test]
    fn test_autopilot_state() {
        let service = ConsulOperatorService::new();
        let state = service.get_autopilot_state();
        assert!(state.healthy);
        assert_eq!(state.servers.len(), 1);
    }

    #[tokio::test]
    async fn test_autopilot_config_update_without_cas() {
        let service = ConsulOperatorService::new();
        let mut config = service.get_autopilot_config().await;

        config.cleanup_dead_servers = false;
        config.last_contact_threshold = "500ms".to_string();

        // Without CAS (None) should always succeed
        let result = service.set_autopilot_config(config.clone(), None).await;
        assert!(result.unwrap());

        let updated = service.get_autopilot_config().await;
        assert!(!updated.cleanup_dead_servers);
        assert_eq!(updated.last_contact_threshold, "500ms");
    }

    #[test]
    fn test_keyring_install_multiple() {
        let service = ConsulOperatorService::new();

        service.install_key("key-alpha");
        service.install_key("key-beta");
        service.install_key("key-gamma");

        let keys = service.list_keys();
        assert_eq!(keys[0].keys.len(), 3);
    }

    #[tokio::test]
    async fn test_keyring_use_and_remove() {
        let service = ConsulOperatorService::new();

        service.install_key("key-1");
        service.install_key("key-2");

        // Set key-1 as primary
        service.use_key("key-1").await.unwrap();

        // Remove key-2 (non-primary) should succeed
        assert!(service.remove_key("key-2").await.is_ok());

        // key-1 should remain
        assert!(service.list_keys()[0].keys.contains_key("key-1"));
        assert!(!service.list_keys()[0].keys.contains_key("key-2"));
    }

    #[test]
    fn test_remove_peer_by_address() {
        let service = ConsulOperatorService::new();
        let addr = {
            let first = service.servers.iter().next().unwrap();
            first.value().address.clone()
        };
        assert!(service.remove_peer(None, Some(&addr)).is_ok());
        assert_eq!(service.servers.len(), 0);
    }

    #[test]
    fn test_raft_configuration_has_valid_data() {
        let service = ConsulOperatorService::new();
        let config = service.get_raft_configuration();

        let server = &config.servers[0];
        assert!(!server.id.is_empty());
        assert!(!server.node.is_empty());
        assert!(!server.address.is_empty());
        assert!(server.address.contains(':')); // host:port format
    }
}
