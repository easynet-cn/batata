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

use base64::Engine;

use crate::acl::{AclService, ResourceType};
use crate::catalog::ConsulCatalogService;
use crate::consul_meta::{ConsulResponseMeta, consul_ok};
use crate::model::{ConsulError, ConsulErrorBody};

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

/// Transfer leader response.
///
/// Consul's upstream response is `{"Success": bool}`. Batata additionally
/// carries a `Warning` field when the underlying Raft implementation cannot
/// perform a real leadership transfer — see `transfer_leader` handler for
/// the openraft 0.10 upgrade path.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TransferLeaderResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

/// Warning text attached to transfer-leader responses while the underlying
/// Raft implementation (openraft 0.9) lacks a transparent transfer primitive.
/// Tracked in the project memory under `project_consul_plugin_audit`.
///
/// TODO(openraft-0.10): remove this warning once `trigger().transfer_leader()`
/// is available and the handler performs an actual transfer. Clean-up steps:
///   1. Bump `openraft` dependency to 0.10+.
///   2. Call `raft.trigger().transfer_leader(target)` from this handler.
///   3. Drop the `warning` field from `TransferLeaderResponse`.
pub const TRANSFER_LEADER_OPENRAFT09_WARNING: &str =
    "leader transfer acknowledged; actual transfer requires openraft 0.10";

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
// Operator Service (ClusterManager-backed)
// ============================================================================

/// Cluster operator service backed by a `ClusterManager`.
///
/// Surfaces Raft configuration, autopilot, and keyring endpoints derived from
/// the live cluster membership reported by `ClusterManager`. In standalone
/// mode the `ClusterManager` reports a single member, so the same code path
/// works without any parallel in-memory variant.
pub struct ConsulOperatorService {
    pub(crate) member_manager: Arc<dyn batata_common::ClusterManager>,
    autopilot_config: Arc<tokio::sync::RwLock<AutopilotConfiguration>>,
    keyring: Arc<DashMap<String, i32>>,
    primary_key: Arc<tokio::sync::RwLock<Option<String>>>,
    index: Arc<AtomicU64>,
    pub(crate) datacenter: String,
}

impl ConsulOperatorService {
    pub fn new(member_manager: Arc<dyn batata_common::ClusterManager>) -> Self {
        Self::with_datacenter(member_manager, "dc1".to_string())
    }

    pub fn with_datacenter(
        member_manager: Arc<dyn batata_common::ClusterManager>,
        datacenter: String,
    ) -> Self {
        // Seed a default gossip key so `keyring_list` never returns empty.
        // Matches Consul which always reports at least the primary gossip
        // encryption key (or a placeholder in dev mode).
        let keyring = Arc::new(DashMap::new());
        let member_count = member_manager.member_count() as i32;
        let default_key =
            base64::engine::general_purpose::STANDARD.encode(uuid::Uuid::new_v4().as_bytes());
        keyring.insert(default_key.clone(), member_count.max(1));

        Self {
            member_manager,
            autopilot_config: Arc::new(tokio::sync::RwLock::new(AutopilotConfiguration::default())),
            keyring,
            primary_key: Arc::new(tokio::sync::RwLock::new(Some(default_key))),
            index: Arc::new(AtomicU64::new(1)),
            datacenter,
        }
    }

    pub fn get_raft_configuration(&self) -> RaftConfigurationResponse {
        let members = self.member_manager.all_members_extended();
        let servers: Vec<RaftServer> = members
            .iter()
            .enumerate()
            .map(|(i, m)| RaftServer {
                id: m.address.clone(),
                node: m.address.clone(),
                address: m.address.clone(),
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
        use batata_common::MemberState;

        let members = self.member_manager.all_members_extended();
        let current_index = self.index.load(Ordering::SeqCst);
        let mut healthy_voters = 0i32;

        let servers: Vec<AutopilotServerHealth> = members
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let is_healthy = m.state == MemberState::Up;
                let serf_status = match m.state {
                    MemberState::Up => "alive",
                    MemberState::Down => "failed",
                    MemberState::Suspicious => "leaving",
                };
                if is_healthy {
                    healthy_voters += 1;
                }
                let last_contact = "0ms".to_string();
                AutopilotServerHealth {
                    id: m.address.clone(),
                    name: m.address.clone(),
                    address: m.address.clone(),
                    serf_status: serf_status.to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    leader: i == 0,
                    last_contact,
                    last_term: 1,
                    last_index: current_index,
                    healthy: is_healthy,
                    voter: true,
                    stable_since: chrono::Utc::now().to_rfc3339(),
                }
            })
            .collect();

        let total = servers.len() as i32;
        let quorum_needed = total / 2 + 1;
        let failure_tolerance = (healthy_voters - quorum_needed).max(0);
        let all_healthy = healthy_voters == total;

        AutopilotHealthResponse {
            healthy: all_healthy && healthy_voters >= quorum_needed,
            failure_tolerance,
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
        let members = self.member_manager.all_members_extended();
        let num_nodes = members.len() as i32;
        let keys: HashMap<String, i32> = self
            .keyring
            .iter()
            .map(|r| (r.key().clone(), *r.value()))
            .collect();

        // primary_keys only contains the currently active primary key
        let primary_key_guard = self.primary_key.try_read().ok().and_then(|g| g.clone());
        let primary_keys: HashMap<String, i32> = primary_key_guard
            .iter()
            .filter_map(|pk| keys.get(pk).map(|&count| (pk.clone(), count)))
            .collect();

        // Return LAN keyring (WAN keyring is a separate response in real Consul)
        vec![
            KeyringResponse {
                wan: false,
                datacenter: self.datacenter.clone(),
                segment: "".to_string(),
                messages: None,
                keys: keys.clone(),
                primary_keys: primary_keys.clone(),
                num_nodes,
            },
            KeyringResponse {
                wan: true,
                datacenter: self.datacenter.clone(),
                segment: "".to_string(),
                messages: None,
                keys,
                primary_keys,
                num_nodes,
            },
        ]
    }

    pub fn install_key(&self, key: &str) {
        let members = self.member_manager.all_members_extended();
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
    pub connect_service_instances: HashMap<String, i64>,
}

/// Operator usage response
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct OperatorUsageResponse {
    pub usage: HashMap<String, ServiceUsage>,
}

// ============================================================================
// HTTP Handlers
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
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let config = operator_service.get_raft_configuration();
    let meta = ConsulResponseMeta::new(config.index);
    consul_ok(&meta).json(config)
}

/// POST /v1/operator/raft/transfer-leader
///
/// Validates the optional target `id` parameter against the current cluster
/// membership (matches Consul's "target not in peers" error contract).
///
/// NOTE: openraft 0.9 does not expose a transparent leadership transfer
/// primitive (that arrived in 0.10+). For now we perform the validation
/// step and return success; the actual Raft-level transfer becomes a
/// no-op that relies on existing election mechanisms if the current
/// leader steps down for other reasons. Upgrade openraft to 0.10 to
/// implement full semantics.
pub async fn transfer_leader(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorService>,
    query: web::Query<TransferLeaderParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    // Target validation: if a specific node id is requested, it must be a
    // current voting member. Consul Go SDK treats an invalid target as
    // a 5xx with "target not in peers" — we mirror that.
    if let Some(ref target) = query.id {
        if !target.is_empty() {
            let members = operator_service.member_manager.all_members_extended();
            let found = members
                .iter()
                .any(|m| m.address == *target || m.ip == *target);
            if !found {
                return HttpResponse::InternalServerError().consul_error(ConsulError::new(
                    format!(
                        "Leadership transfer target {} is not in the current Raft configuration",
                        target
                    ),
                ));
            }
        }
    }

    // Log the request for operator visibility. Actual openraft-level
    // leader transfer requires 0.10+; this endpoint currently acts as a
    // validator + acknowledger.
    tracing::info!(
        target = %query.id.as_deref().unwrap_or("<any eligible>"),
        "Leader transfer requested (openraft 0.9: no-op acknowledge)"
    );
    HttpResponse::Ok().json(TransferLeaderResponse {
        success: true,
        warning: Some(TRANSFER_LEADER_OPENRAFT09_WARNING.to_string()),
    })
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
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    if query.id.is_some() && query.address.is_some() {
        return HttpResponse::BadRequest().consul_error(ConsulError::new(
            "Must specify either id or address, not both",
        ));
    }
    if query.id.is_none() && query.address.is_none() {
        return HttpResponse::BadRequest()
            .consul_error(ConsulError::new("Must specify either id or address"));
    }

    // Validate the peer actually exists before claiming success — matches
    // Consul behavior (autopilot/Raft returns an error for unknown peers).
    let members = operator_service.member_manager.all_members_extended();
    let peer_found = match (&query.id, &query.address) {
        (Some(id), _) => members.iter().any(|m| m.address == *id || m.ip == *id),
        (_, Some(addr)) => members.iter().any(|m| m.address == *addr || m.ip == *addr),
        _ => false,
    };
    if !peer_found {
        let which = query
            .id
            .as_deref()
            .or(query.address.as_deref())
            .unwrap_or("?");
        return HttpResponse::InternalServerError().consul_error(ConsulError::new(format!(
            "Peer {} not found in the Raft configuration",
            which
        )));
    }

    // Peer removal in Raft membership is a no-op in current implementation.
    HttpResponse::Ok().finish()
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
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
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
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    match operator_service
        .set_autopilot_config(body.into_inner(), query.cas)
        .await
    {
        Ok(success) => HttpResponse::Ok().json(success),
        Err(e) => HttpResponse::InternalServerError().consul_error(ConsulError::new(e)),
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
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
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
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let state = operator_service.get_autopilot_state();
    HttpResponse::Ok().json(state)
}

/// GET /v1/operator/keyring
pub async fn keyring_list(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorService>,
    _query: web::Query<KeyringParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }
    HttpResponse::Ok().json(operator_service.list_keys())
}

/// POST /v1/operator/keyring
pub async fn keyring_install(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorService>,
    body: web::Json<KeyringRequest>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }
    operator_service.install_key(&body.key);
    HttpResponse::Ok().finish()
}

/// PUT /v1/operator/keyring
pub async fn keyring_use(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorService>,
    body: web::Json<KeyringRequest>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }
    match operator_service.use_key(&body.key).await {
        Ok(()) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::BadRequest().consul_error(ConsulError::new(e)),
    }
}

/// DELETE /v1/operator/keyring
pub async fn keyring_remove(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorService>,
    body: web::Json<KeyringRequest>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }
    match operator_service.remove_key(&body.key).await {
        Ok(()) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::BadRequest().consul_error(ConsulError::new(e)),
    }
}

/// GET /v1/operator/usage - Get cluster usage statistics
pub async fn get_operator_usage(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorService>,
    catalog_service: web::Data<ConsulCatalogService>,
    _query: web::Query<OperatorQueryParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let members = operator_service.member_manager.all_members_extended();

    // Get real service counts from catalog
    let services = catalog_service.get_services("public");
    let service_count = services.len() as i64;
    let instance_count: i64 = services
        .values()
        .map(|tags| std::cmp::max(tags.len(), 1) as i64)
        .sum();

    let mut usage = HashMap::new();
    usage.insert(
        operator_service.datacenter.clone(),
        ServiceUsage {
            nodes: members.len() as i64,
            services: service_count,
            service_instances: instance_count,
            connect_service_instances: HashMap::new(),
        },
    );

    HttpResponse::Ok().json(OperatorUsageResponse { usage })
}

/// Error body returned for Enterprise-only operator endpoints.
///
/// Consul OSS surfaces the utilization endpoint with HTTP 501 and this exact
/// body so SDK callers can do `strings.Contains(err.Error(), "Enterprise")`.
pub const UTILIZATION_ENTERPRISE_BODY: &str =
    "utilization endpoint is only available in Consul Enterprise";

/// GET /v1/operator/utilization - Enterprise-only endpoint.
///
/// Matches Consul OSS behavior: returns 501 Not Implemented with an explicit
/// "Enterprise" body. The Consul Go SDK's `Operator().Utilization()` call will
/// propagate the body so callers can branch on the Enterprise marker.
pub async fn get_operator_utilization(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    _query: web::Query<OperatorQueryParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    HttpResponse::NotImplemented()
        .content_type("text/plain; charset=utf-8")
        .body(UTILIZATION_ENTERPRISE_BODY)
}

#[cfg(test)]
mod tests {
    use super::*;
    use batata_common::{ClusterHealthSummary, ClusterManager, ExtendedMemberInfo, MemberState};

    /// Minimal in-test ClusterManager stub. Reports a single healthy member,
    /// matching `ServerMemberManager::new()` in standalone mode.
    struct TestClusterManager {
        address: String,
    }

    impl TestClusterManager {
        fn new() -> Self {
            Self {
                address: "127.0.0.1:8848".to_string(),
            }
        }
        fn member(&self) -> ExtendedMemberInfo {
            ExtendedMemberInfo {
                ip: "127.0.0.1".to_string(),
                port: 8848,
                address: self.address.clone(),
                state: MemberState::Up,
                extend_info: std::collections::BTreeMap::new(),
            }
        }
    }

    impl ClusterManager for TestClusterManager {
        fn is_standalone(&self) -> bool {
            true
        }
        fn is_leader(&self) -> bool {
            true
        }
        fn is_cluster_healthy(&self) -> bool {
            true
        }
        fn leader_address(&self) -> Option<String> {
            Some(self.address.clone())
        }
        fn local_address(&self) -> &str {
            &self.address
        }
        fn member_count(&self) -> usize {
            1
        }
        fn all_members_extended(&self) -> Vec<ExtendedMemberInfo> {
            vec![self.member()]
        }
        fn healthy_members_extended(&self) -> Vec<ExtendedMemberInfo> {
            vec![self.member()]
        }
        fn get_member(&self, address: &str) -> Option<ExtendedMemberInfo> {
            (address == self.address).then(|| self.member())
        }
        fn get_self_member(&self) -> ExtendedMemberInfo {
            self.member()
        }
        fn health_summary(&self) -> ClusterHealthSummary {
            ClusterHealthSummary {
                total: 1,
                up: 1,
                ..Default::default()
            }
        }
        fn refresh_self(&self) {}
        fn is_self(&self, address: &str) -> bool {
            address == self.address
        }
        fn update_member_state(&self, _address: &str, _state: &str) -> Result<String, String> {
            Ok("UP".to_string())
        }
    }

    fn test_service() -> ConsulOperatorService {
        let cm: Arc<dyn ClusterManager> = Arc::new(TestClusterManager::new());
        ConsulOperatorService::new(cm)
    }

    #[test]
    fn test_raft_configuration_default() {
        let service = test_service();
        let config = service.get_raft_configuration();
        assert_eq!(config.servers.len(), 1);
        let server = &config.servers[0];
        assert!(server.leader);
        assert!(server.voter);
        assert!(!server.id.is_empty());
        assert!(!server.node.is_empty());
        assert!(!server.address.is_empty());
        assert!(
            server.address.contains(':'),
            "Address should be host:port format"
        );
    }

    #[tokio::test]
    async fn test_autopilot_config_cas() {
        let service = test_service();
        let config = service.get_autopilot_config().await;
        assert!(config.cleanup_dead_servers);
        assert!(!config.last_contact_threshold.is_empty());
        assert!(config.create_index > 0);

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
        let service = test_service();
        let health = service.get_autopilot_health();
        assert!(health.healthy);
        assert_eq!(health.servers.len(), 1);
        let server = &health.servers[0];
        assert!(server.healthy);
        assert!(!server.id.is_empty());
        assert!(!server.name.is_empty());
        assert!(health.failure_tolerance >= 0);
    }

    #[test]
    fn test_keyring_operations() {
        let service = test_service();

        service.install_key("test-key-1");
        let keys = service.list_keys();
        assert!(!keys.is_empty());
        assert!(keys[0].keys.contains_key("test-key-1"));
        // Value is the use-count (should be >= 0)
        assert!(*keys[0].keys.get("test-key-1").unwrap() >= 0);
    }

    #[tokio::test]
    async fn test_keyring_remove_primary_fails() {
        let service = test_service();
        service.install_key("primary-key");
        service.use_key("primary-key").await.unwrap();
        assert!(service.remove_key("primary-key").await.is_err());
    }

    #[tokio::test]
    async fn test_keyring_use_nonexistent_fails() {
        let service = test_service();
        assert!(service.use_key("nonexistent").await.is_err());
    }

    #[test]
    fn test_autopilot_state() {
        let service = test_service();
        let state = service.get_autopilot_state();
        assert!(state.healthy);
        assert_eq!(state.servers.len(), 1);
        assert!(!state.leader.is_empty());
        assert!(!state.voters.is_empty());
        assert!(state.failure_tolerance >= 0);
        let server = state.servers.values().next().unwrap();
        assert!(server.healthy);
        assert!(!server.id.is_empty());
    }

    #[tokio::test]
    async fn test_autopilot_config_update_without_cas() {
        let service = test_service();
        let mut config = service.get_autopilot_config().await;

        config.cleanup_dead_servers = false;
        config.last_contact_threshold = "500ms".to_string();

        let result = service.set_autopilot_config(config.clone(), None).await;
        assert!(result.unwrap());

        let updated = service.get_autopilot_config().await;
        assert!(!updated.cleanup_dead_servers);
        assert_eq!(updated.last_contact_threshold, "500ms");
    }

    #[test]
    fn test_keyring_install_multiple() {
        let service = test_service();

        service.install_key("key-alpha");
        service.install_key("key-beta");
        service.install_key("key-gamma");

        let keys = service.list_keys();
        assert!(keys[0].keys.len() >= 3);
        assert!(keys[0].keys.contains_key("key-alpha"));
        assert!(keys[0].keys.contains_key("key-beta"));
        assert!(keys[0].keys.contains_key("key-gamma"));
    }

    #[tokio::test]
    async fn test_keyring_use_and_remove() {
        let service = test_service();

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

    #[actix_web::test]
    async fn test_transfer_leader_response_includes_openraft_warning() {
        use actix_web::{App, test};

        let cm: Arc<dyn ClusterManager> = Arc::new(TestClusterManager::new());
        let operator_service = web::Data::new(ConsulOperatorService::new(cm));
        let acl_service = web::Data::new(AclService::disabled());

        let app = test::init_service(
            App::new()
                .app_data(operator_service.clone())
                .app_data(acl_service.clone())
                .route(
                    "/v1/operator/raft/transfer-leader",
                    web::post().to(transfer_leader),
                ),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/v1/operator/raft/transfer-leader")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());

        let body: TransferLeaderResponse = test::read_body_json(resp).await;
        assert!(body.success, "transfer-leader should report success");
        let warning = body
            .warning
            .as_deref()
            .expect("warning field must be present while on openraft 0.9");
        assert_eq!(warning, TRANSFER_LEADER_OPENRAFT09_WARNING);
        assert!(
            warning.contains("openraft 0.10"),
            "warning must reference the openraft-0.10 upgrade path",
        );
    }

    #[actix_web::test]
    async fn test_operator_utilization_returns_501_enterprise_body() {
        use actix_web::{App, test};

        let acl_service = web::Data::new(AclService::disabled());

        let app = test::init_service(App::new().app_data(acl_service.clone()).route(
            "/v1/operator/utilization",
            web::put().to(get_operator_utilization),
        ))
        .await;

        let req = test::TestRequest::put()
            .uri("/v1/operator/utilization")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(
            resp.status().as_u16(),
            501,
            "utilization must return HTTP 501 Not Implemented on OSS"
        );

        let body_bytes = test::read_body(resp).await;
        let body_str = std::str::from_utf8(&body_bytes).expect("utf-8 body");
        assert_eq!(body_str, UTILIZATION_ENTERPRISE_BODY);
        assert!(
            body_str.contains("Enterprise"),
            "body must contain the Enterprise marker for SDK branching",
        );
    }

    #[test]
    fn test_raft_configuration_has_valid_data() {
        let service = test_service();
        let config = service.get_raft_configuration();

        let server = &config.servers[0];
        assert!(!server.id.is_empty());
        assert!(!server.node.is_empty());
        assert!(!server.address.is_empty());
        assert!(server.address.contains(':')); // host:port format
    }
}
