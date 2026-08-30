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

use rand::Rng as _;

// ============================================================================
// Models
// ============================================================================

/// Raft server information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct RaftServer {
    #[serde(rename = "ID")]
/// The `id` field.
    pub id: String,
/// The `node` field.
    pub node: String,
/// The `address` field.
    pub address: String,
/// The `leader` field.
    pub leader: bool,
/// The `voter` field.
    pub voter: bool,
/// The `protocol_version` field.
    pub protocol_version: String,
/// The `last_index` field.
    pub last_index: u64,
}

/// Raft configuration response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct RaftConfigurationResponse {
/// The `servers` field.
    pub servers: Vec<RaftServer>,
/// The `index` field.
    pub index: u64,
}

/// Transfer leader response.
///
/// Consul's upstream response is `{"Success": bool}`. Batata additionally
/// carries a `Warning` field when the leader transfer could not be completed
/// within the timeout window — the election was triggered but the leader
/// did not change in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TransferLeaderResponse {
/// The `success` field.
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `warning` field.
    pub warning: Option<String>,
}

/// Query parameters for raft peer removal
#[derive(Debug, Clone, Deserialize, Default)]
pub struct RaftPeerParams {
/// The `id` field.
    pub id: Option<String>,
/// The `address` field.
    pub address: Option<String>,
/// The `dc` field.
    pub dc: Option<String>,
}

/// Query parameters for transfer-leader
#[derive(Debug, Clone, Deserialize, Default)]
pub struct TransferLeaderParams {
/// The `id` field.
    pub id: Option<String>,
}

/// Autopilot configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AutopilotConfiguration {
/// The `cleanup_dead_servers` field.
    pub cleanup_dead_servers: bool,
/// The `last_contact_threshold` field.
    pub last_contact_threshold: String,
/// The `max_trailing_logs` field.
    pub max_trailing_logs: u64,
/// The `min_quorum` field.
    pub min_quorum: u64,
/// The `server_stabilization_time` field.
    pub server_stabilization_time: String,
/// The `redundancy_zone_tag` field.
    pub redundancy_zone_tag: String,
/// The `disable_upgrade_migration` field.
    pub disable_upgrade_migration: bool,
/// The `upgrade_version_tag` field.
    pub upgrade_version_tag: String,
/// The `create_index` field.
    pub create_index: u64,
/// The `modify_index` field.
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
/// The `cas` field.
    pub cas: Option<u64>,
/// The `dc` field.
    pub dc: Option<String>,
}

/// Autopilot health response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AutopilotHealthResponse {
/// The `healthy` field.
    pub healthy: bool,
/// The `failure_tolerance` field.
    pub failure_tolerance: i32,
/// The `servers` field.
    pub servers: Vec<AutopilotServerHealth>,
}

/// Autopilot server health
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AutopilotServerHealth {
    #[serde(rename = "ID")]
/// The `id` field.
    pub id: String,
/// The `name` field.
    pub name: String,
/// The `address` field.
    pub address: String,
/// The `serf_status` field.
    pub serf_status: String,
/// The `version` field.
    pub version: String,
/// The `leader` field.
    pub leader: bool,
/// The `last_contact` field.
    pub last_contact: String,
/// The `last_term` field.
    pub last_term: u64,
/// The `last_index` field.
    pub last_index: u64,
/// The `healthy` field.
    pub healthy: bool,
/// The `voter` field.
    pub voter: bool,
/// The `stable_since` field.
    pub stable_since: String,
}

/// Autopilot state response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AutopilotStateResponse {
/// The `healthy` field.
    pub healthy: bool,
/// The `failure_tolerance` field.
    pub failure_tolerance: i32,
/// The `leader` field.
    pub leader: String,
/// The `voters` field.
    pub voters: Vec<String>,
/// The `servers` field.
    pub servers: HashMap<String, AutopilotServerHealth>,
}

/// Keyring request body
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct KeyringRequest {
/// The `key` field.
    pub key: String,
}

/// Keyring response
/// Consul-compatible keyring response describing the state of a WAN or LAN keyring.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct KeyringResponse {
    #[serde(rename = "WAN")]
/// The `wan` field.
    pub wan: bool,
/// The `datacenter` field.
    pub datacenter: String,
/// The `segment` field.
    pub segment: String,
    /// The partition name (a Consul Enterprise feature; empty string in OSS).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub partition: String,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `messages` field.
    pub messages: Option<HashMap<String, String>>,
    /// `key` -> number of nodes holding that key.
    pub keys: HashMap<String, i32>,
    /// Current primary key -> number of nodes.
    pub primary_keys: HashMap<String, i32>,
/// The `num_nodes` field.
    pub num_nodes: i32,
}

/// Keyring response container.
/// Consul's JSON field name is "Responses" (PascalCase).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct KeyringResponses {
/// The `responses` field.
    pub responses: Vec<KeyringResponse>,
}

/// Query parameters for keyring operations.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct KeyringParams {
    /// `relay-factor`: 0-5, controls how many nodes forward the request via gossip relay.
    #[serde(rename = "relay-factor")]
    pub relay_factor: Option<u8>,
    /// `local-only`: only supported by GET/list operations; returns only the local node's keyring info.
    #[serde(rename = "local-only")]
    pub local_only: Option<String>,
}

/// Query parameters for operator endpoints
#[derive(Debug, Clone, Deserialize, Default)]
pub struct OperatorQueryParams {
/// The `dc` field.
    pub dc: Option<String>,
/// The `stale` field.
    pub stale: Option<String>,
}

/// Validates the key format: must be 32 bytes of base64-encoded data (an AES-256 key).
/// Consul encrypts gossip traffic with AES-256-GCM using a 32-byte key, transmitted base64-encoded.
fn validate_key_format(key: &str) -> Result<Vec<u8>, String> {
    // Decode base64.
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(key)
        .map_err(|e| format!("Invalid base64 key: {}", e))?;
    // Verify the length is 32 bytes (AES-256).
    if decoded.len() != 32 {
        return Err(format!(
            "Key must be 32 bytes (AES-256), got {} bytes",
            decoded.len()
        ));
    }
    Ok(decoded)
}

/// Parses the `local_only` query parameter into a boolean.
/// Consul treats "true"/"1"/"yes" etc. as true and anything else as false.
fn parse_local_only(value: &Option<String>) -> bool {
    match value {
        Some(v) => matches!(v.to_lowercase().as_str(), "true" | "1" | "yes" | "on"),
        None => false,
    }
}

/// Validates the `relay_factor` range (0-5); returns an error message if out of range.
fn validate_relay_factor(relay_factor: Option<u8>) -> Result<(), String> {
    if let Some(rf) = relay_factor {
        if rf > 5 {
            return Err(format!(
                "relay-factor must be between 0 and 5, got {}",
                rf
            ));
        }
    }
    Ok(())
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
    pub(crate) raft_node: Option<Arc<batata_consistency::RaftNode>>,
    autopilot_config: Arc<tokio::sync::RwLock<AutopilotConfiguration>>,
    keyring: Arc<DashMap<String, i32>>,
    primary_key: Arc<tokio::sync::RwLock<Option<String>>>,
    index: Arc<AtomicU64>,
    pub(crate) datacenter: String,
}

impl ConsulOperatorService {
/// The `new` associated function.
    pub fn new(member_manager: Arc<dyn batata_common::ClusterManager>) -> Self {
        Self::with_datacenter(member_manager, "dc1".to_string())
    }

/// The `with_datacenter` associated function.
    pub fn with_datacenter(
        member_manager: Arc<dyn batata_common::ClusterManager>,
        datacenter: String,
    ) -> Self {
        // Seed a default gossip key so `keyring_list` never returns empty.
        // Matches Consul which always reports at least the primary gossip
        // encryption key (or a placeholder in dev mode).
        // Use 32 random bytes as the AES-256 key (44 chars after base64 encoding).
        let keyring = Arc::new(DashMap::new());
        let member_count = member_manager.member_count() as i32;
        let mut default_key_bytes = [0u8; 32];
        rand::rng().fill_bytes(&mut default_key_bytes);
        let default_key =
            base64::engine::general_purpose::STANDARD.encode(default_key_bytes);
        keyring.insert(default_key.clone(), member_count.max(1));

        Self {
            member_manager,
            raft_node: None,
            autopilot_config: Arc::new(tokio::sync::RwLock::new(AutopilotConfiguration::default())),
            keyring,
            primary_key: Arc::new(tokio::sync::RwLock::new(Some(default_key))),
            index: Arc::new(AtomicU64::new(1)),
            datacenter,
        }
    }

    /// Create an operator service with a RaftNode handle for leader transfer.
    ///
    /// In cluster mode the `RaftNode` is used by the `transfer_leader` handler
    /// to trigger elections on follower nodes. In standalone mode `raft_node`
    /// is `None` and the handler returns a no-op success.
    pub fn with_datacenter_and_raft(
        member_manager: Arc<dyn batata_common::ClusterManager>,
        datacenter: String,
        raft_node: Option<Arc<batata_consistency::RaftNode>>,
    ) -> Self {
        let mut svc = Self::with_datacenter(member_manager, datacenter);
        svc.raft_node = raft_node;
        svc
    }

/// The `get_raft_configuration` method.
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

/// The `get_autopilot_health` method.
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

/// The `get_autopilot_state` method.
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

/// The `get_autopilot_config` method.
    pub async fn get_autopilot_config(&self) -> AutopilotConfiguration {
        self.autopilot_config.read().await.clone()
    }

/// The `set_autopilot_config` method.
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

    /// Lists all keys in the keyring.
    /// Returns `KeyringResponses` (both LAN and WAN responses, or only LAN if `local_only=true`).
    /// batata has no gossip/serf, so WAN is a copy of LAN (for Consul API compatibility).
    pub fn list_keys(&self, local_only: bool) -> KeyringResponses {
        let num_nodes = self.member_manager.member_count() as i32;
        let keys: HashMap<String, i32> = self
            .keyring
            .iter()
            .map(|r| (r.key().clone(), *r.value()))
            .collect();

        // primary_keys contains only the currently active primary key.
        let primary_key_guard = self.primary_key.try_read().ok().and_then(|g| g.clone());
        let primary_keys: HashMap<String, i32> = primary_key_guard
            .iter()
            .filter_map(|pk| keys.get(pk).map(|&count| (pk.clone(), count)))
            .collect();

        // LAN response
        let lan_resp = KeyringResponse {
            wan: false,
            datacenter: self.datacenter.clone(),
            segment: String::new(),
            partition: String::new(),
            messages: None,
            keys: keys.clone(),
            primary_keys: primary_keys.clone(),
            num_nodes,
        };

        if local_only {
            // When local_only=true, return only the LAN response.
            return KeyringResponses {
                responses: vec![lan_resp],
            };
        }

        // WAN response (in batata WAN is a copy of LAN since there is no gossip/serf).
        let wan_resp = KeyringResponse {
            wan: true,
            datacenter: self.datacenter.clone(),
            segment: String::new(),
            partition: String::new(),
            messages: None,
            keys,
            primary_keys,
            num_nodes,
        };

        KeyringResponses {
            responses: vec![lan_resp, wan_resp],
        }
    }

    /// Installs a new key into the keyring.
    /// Validates the key format (32 bytes base64-encoded); succeeds idempotently if the key already exists.
    pub fn install_key(&self, key: &str) -> Result<KeyringResponses, String> {
        // Validate the key format.
        validate_key_format(key)?;

        // If the key already exists, return success (idempotent, matching Consul behavior).
        if self.keyring.contains_key(key) {
            return Ok(self.list_keys(false));
        }

        // Add to the keyring.
        let member_count = self.member_manager.member_count() as i32;
        self.keyring.insert(key.to_string(), member_count.max(1));

        Ok(self.list_keys(false))
    }

    /// Switches the primary key.
    /// Validates the key format, checks it is in the keyring, then sets it as the new primary key.
    pub async fn use_key(&self, key: &str) -> Result<KeyringResponses, String> {
        // Validate the key format.
        validate_key_format(key)?;

        // Check whether the key is in the keyring.
        if !self.keyring.contains_key(key) {
            return Err(format!("Key '{}' not found in keyring", key));
        }

        // Set as the new primary key.
        let mut primary = self.primary_key.write().await;
        *primary = Some(key.to_string());
        drop(primary);

        Ok(self.list_keys(false))
    }

    /// Removes a key from the keyring.
    /// Validates the key format, checks it is in the keyring, and checks it is not the primary key.
    pub async fn remove_key(&self, key: &str) -> Result<KeyringResponses, String> {
        // Validate the key format.
        validate_key_format(key)?;

        // Check whether the key is in the keyring.
        if !self.keyring.contains_key(key) {
            return Err(format!("Key '{}' not found in keyring", key));
        }

        // Check it is not the primary key (cannot remove the currently used primary key).
        let primary = self.primary_key.read().await;
        if primary.as_deref() == Some(key) {
            return Err("Cannot remove primary key".to_string());
        }
        drop(primary);

        // Remove from the keyring.
        self.keyring.remove(key);

        Ok(self.list_keys(false))
    }
}

// ============================================================================
// Usage / Utilization Models
// ============================================================================

/// Service usage statistics
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct ServiceUsage {
/// The `nodes` field.
    pub nodes: i64,
/// The `services` field.
    pub services: i64,
/// The `service_instances` field.
    pub service_instances: i64,
/// The `connect_service_instances` field.
    pub connect_service_instances: HashMap<String, i64>,
}

/// Operator usage response
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct OperatorUsageResponse {
/// The `usage` field.
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
/// Transfers Raft cluster leadership to another node. If a target node ID
/// is specified, leadership is transferred to that node; otherwise, the
/// leader picks the most suitable follower.
///
/// Implementation: Since openraft 0.9 does not have a native
/// `transfer_leader()` API, we implement leader transfer by:
/// 1. The leader sends a `TriggerElection` gRPC request to the target follower
/// 2. The target follower calls `raft.trigger().elect()` to start a new election
/// 3. The leader waits for the leader to change (with timeout)
/// 4. Returns success if the leader changed
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

    // Get the RaftNode handle
    let raft_node = match &operator_service.raft_node {
        Some(node) => node.clone(),
        None => {
            // Standalone mode — no transfer possible
            return HttpResponse::Ok().json(TransferLeaderResponse {
                success: true,
                warning: Some("Standalone mode: no transfer needed".to_string()),
            });
        }
    };

    // Only the leader can transfer leadership
    if !raft_node.is_leader() {
        return HttpResponse::BadRequest().consul_error(ConsulError::new(
            "Leadership transfer must be requested on the leader node",
        ));
    }

    let current_leader = raft_node.leader_id();

    // Get all cluster members
    let members = operator_service.member_manager.all_members_extended();

    // Determine the target node for transfer
    let target_addr = if let Some(ref target_id) = query.id {
        if !target_id.is_empty() {
            // Find the target node by address or IP
            let found = members
                .iter()
                .find(|m| m.address == *target_id || m.ip == *target_id);
            match found {
                Some(m) => Some(m.address.clone()),
                None => {
                    return HttpResponse::InternalServerError().consul_error(ConsulError::new(
                        format!(
                            "Leadership transfer target {} is not in the current Raft configuration",
                            target_id
                        ),
                    ));
                }
            }
        } else {
            // Empty target — pick the first non-leader voter
            pick_transfer_target(&members, &raft_node)
        }
    } else {
        // No target specified — pick the first non-leader voter
        pick_transfer_target(&members, &raft_node)
    };

    let Some(target_addr) = target_addr else {
        return HttpResponse::InternalServerError().consul_error(ConsulError::new(
            "No suitable follower found for leadership transfer",
        ));
    };

    // Send TriggerElection gRPC to the target node
    tracing::info!(
        target = %target_addr,
        "Initiating leader transfer to {}",
        target_addr
    );

    match raft_node.trigger_remote_election(&target_addr).await {
        Ok(_) => {
            // Wait for leader to change (5 second timeout)
            match raft_node
                .wait_for_leader_change(
                    current_leader.unwrap_or(0),
                    std::time::Duration::from_secs(5),
                )
                .await
            {
                Ok(_) => {
                    tracing::info!("Leader transfer successful");
                    HttpResponse::Ok().json(TransferLeaderResponse {
                        success: true,
                        warning: None,
                    })
                }
                Err(e) => {
                    tracing::warn!("Leader transfer timed out: {}", e);
                    HttpResponse::Ok().json(TransferLeaderResponse {
                        success: false,
                        warning: Some(format!("Leader transfer timed out: {}", e)),
                    })
                }
            }
        }
        Err(e) => {
            tracing::error!("Leader transfer failed: {}", e);
            HttpResponse::InternalServerError().consul_error(ConsulError::new(format!(
                "Failed to transfer leadership: {}",
                e
            )))
        }
    }
}

/// Pick the best follower node for leader transfer.
/// Prefers the first healthy voter that is not the current leader.
fn pick_transfer_target(
    members: &[batata_common::ExtendedMemberInfo],
    raft_node: &batata_consistency::RaftNode,
) -> Option<String> {
    use batata_common::MemberState;
    let local_addr = raft_node.addr();
    members
        .iter()
        .find(|m| m.address != local_addr && m.state == MemberState::Up)
        .map(|m| m.address.clone())
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
/// Lists all keys in the keyring, returning a `KeyringResponses`.
pub async fn keyring_list(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorService>,
    query: web::Query<KeyringParams>,
) -> HttpResponse {
    // Keyring list requires the KeyringRead permission (read operation).
    let authz = acl_service.authorize_request(&req, ResourceType::Keyring, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    // Validate the relay_factor range (0-5).
    if let Err(e) = validate_relay_factor(query.relay_factor) {
        return HttpResponse::BadRequest().consul_error(ConsulError::new(e));
    }

    // Parse the local_only parameter (only supported by list operations).
    let local_only = parse_local_only(&query.local_only);

    let responses = operator_service.list_keys(local_only);
    HttpResponse::Ok().json(responses)
}

/// POST /v1/operator/keyring
/// Installs a new key into the keyring.
pub async fn keyring_install(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorService>,
    query: web::Query<KeyringParams>,
    body: web::Json<KeyringRequest>,
) -> HttpResponse {
    // Keyring install requires the KeyringWrite permission (write operation).
    let authz = acl_service.authorize_request(&req, ResourceType::Keyring, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    // Validate the relay_factor range (0-5).
    if let Err(e) = validate_relay_factor(query.relay_factor) {
        return HttpResponse::BadRequest().consul_error(ConsulError::new(e));
    }

    // local_only is only supported by list operations; other operations return 400.
    if query.local_only.is_some() {
        return HttpResponse::BadRequest().consul_error(ConsulError::new(
            "local-only parameter is only valid for list (GET) operations",
        ));
    }

    match operator_service.install_key(&body.key) {
        Ok(responses) => HttpResponse::Ok().json(responses),
        Err(e) => HttpResponse::BadRequest().consul_error(ConsulError::new(e)),
    }
}

/// PUT /v1/operator/keyring
/// Switches the primary key.
pub async fn keyring_use(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorService>,
    query: web::Query<KeyringParams>,
    body: web::Json<KeyringRequest>,
) -> HttpResponse {
    // Keyring use requires the KeyringWrite permission (write operation).
    let authz = acl_service.authorize_request(&req, ResourceType::Keyring, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    // Validate the relay_factor range (0-5).
    if let Err(e) = validate_relay_factor(query.relay_factor) {
        return HttpResponse::BadRequest().consul_error(ConsulError::new(e));
    }

    // local_only is only supported by list operations; other operations return 400.
    if query.local_only.is_some() {
        return HttpResponse::BadRequest().consul_error(ConsulError::new(
            "local-only parameter is only valid for list (GET) operations",
        ));
    }

    match operator_service.use_key(&body.key).await {
        Ok(responses) => HttpResponse::Ok().json(responses),
        Err(e) => HttpResponse::BadRequest().consul_error(ConsulError::new(e)),
    }
}

/// DELETE /v1/operator/keyring
/// Removes a key from the keyring.
pub async fn keyring_remove(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorService>,
    query: web::Query<KeyringParams>,
    body: web::Json<KeyringRequest>,
) -> HttpResponse {
    // Keyring remove requires the KeyringWrite permission (write operation).
    let authz = acl_service.authorize_request(&req, ResourceType::Keyring, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    // Validate the relay_factor range (0-5).
    if let Err(e) = validate_relay_factor(query.relay_factor) {
        return HttpResponse::BadRequest().consul_error(ConsulError::new(e));
    }

    // local_only is only supported by list operations; other operations return 400.
    if query.local_only.is_some() {
        return HttpResponse::BadRequest().consul_error(ConsulError::new(
            "local-only parameter is only valid for list (GET) operations",
        ));
    }

    match operator_service.remove_key(&body.key).await {
        Ok(responses) => HttpResponse::Ok().json(responses),
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

    /// Generates a valid AES-256 key (32 bytes base64-encoded), for tests.
    fn make_valid_key(seed: u8) -> String {
        base64::engine::general_purpose::STANDARD.encode([seed; 32])
    }

    #[test]
    fn test_keyring_validate_key_format() {
        // Valid key: 32 bytes base64-encoded.
        let valid_key = make_valid_key(0xAA);
        let result = validate_key_format(&valid_key);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 32);

        // Invalid base64.
        let result = validate_key_format("not-valid-base64!!!");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid base64 key"));

        // Length too short (16 bytes = AES-128, not accepted).
        let short_key = base64::engine::general_purpose::STANDARD.encode([0u8; 16]);
        let result = validate_key_format(&short_key);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("32 bytes"));

        // Length too long (64 bytes).
        let long_key = base64::engine::general_purpose::STANDARD.encode([0u8; 64]);
        let result = validate_key_format(&long_key);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("32 bytes"));
    }

    #[test]
    fn test_keyring_install_invalid_key() {
        let service = test_service();

        // Invalid key format should return an error.
        let result = service.install_key("invalid-key");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid base64 key"));

        // Key with wrong length.
        let short_key = base64::engine::general_purpose::STANDARD.encode([0u8; 16]);
        let result = service.install_key(&short_key);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("32 bytes"));
    }

    #[test]
    fn test_keyring_relay_factor_validation() {
        // relay_factor 0-5 is valid.
        assert!(validate_relay_factor(None).is_ok());
        assert!(validate_relay_factor(Some(0)).is_ok());
        assert!(validate_relay_factor(Some(3)).is_ok());
        assert!(validate_relay_factor(Some(5)).is_ok());

        // relay_factor > 5 is invalid.
        let result = validate_relay_factor(Some(6));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("relay-factor must be between 0 and 5"));
    }

    #[test]
    fn test_keyring_local_only_only_for_list() {
        // Parse the local_only parameter.
        assert!(!parse_local_only(&None));
        assert!(parse_local_only(&Some("true".to_string())));
        assert!(parse_local_only(&Some("1".to_string())));
        assert!(parse_local_only(&Some("yes".to_string())));
        assert!(!parse_local_only(&Some("false".to_string())));
        assert!(!parse_local_only(&Some("".to_string())));
    }

    #[test]
    fn test_keyring_list_returns_keyring_responses() {
        let service = test_service();

        // By default return both LAN and WAN responses.
        let responses = service.list_keys(false);
        assert_eq!(responses.responses.len(), 2);

        // LAN response (WAN=false)
        let lan = &responses.responses[0];
        assert!(!lan.wan);
        assert_eq!(lan.datacenter, "dc1");
        assert_eq!(lan.num_nodes, 1);
        assert!(!lan.keys.is_empty());
        assert!(!lan.primary_keys.is_empty());

        // WAN response (WAN=true)
        let wan = &responses.responses[1];
        assert!(wan.wan);
        assert_eq!(wan.datacenter, "dc1");
        assert_eq!(wan.num_nodes, 1);
        assert!(!wan.keys.is_empty());
        assert!(!wan.primary_keys.is_empty());

        // When local_only=true, return only the LAN response.
        let responses = service.list_keys(true);
        assert_eq!(responses.responses.len(), 1);
        assert!(!responses.responses[0].wan);
    }

    #[test]
    fn test_keyring_install_duplicate_key() {
        let service = test_service();
        let key = make_valid_key(0x42);

        // First install.
        let result = service.install_key(&key);
        assert!(result.is_ok());

        // Installing the same key a second time should not error (idempotent).
        let result = service.install_key(&key);
        assert!(result.is_ok());

        // The key should exist only once.
        let responses = service.list_keys(false);
        let count = responses.responses[0]
            .keys
            .get(&key)
            .copied()
            .unwrap_or(0);
        assert_eq!(count, 1, "duplicate install should not create a second entry");
    }

    #[tokio::test]
    async fn test_keyring_full_rotation() {
        let service = test_service();

        // Get the initial primary key.
        let initial = service.list_keys(false);
        let initial_primary = initial.responses[0]
            .primary_keys
            .keys()
            .next()
            .cloned()
            .expect("should have an initial primary key");

        // Step 1: install a new key.
        let new_key = make_valid_key(0x99);
        let result = service.install_key(&new_key);
        assert!(result.is_ok(), "install should succeed");

        // Confirm the new key is in the keyring.
        let responses = service.list_keys(false);
        assert!(
            responses.responses[0].keys.contains_key(&new_key),
            "new key should be in keyring after install"
        );

        // Step 2: use the new key (switch primary).
        let result = service.use_key(&new_key).await;
        assert!(result.is_ok(), "use should succeed");

        // Confirm the primary key has switched.
        let responses = service.list_keys(false);
        let current_primary = responses.responses[0]
            .primary_keys
            .keys()
            .next()
            .cloned()
            .expect("should have a primary key");
        assert_eq!(
            current_primary, new_key,
            "primary key should be the new key after use"
        );

        // Step 3: remove the old key.
        let result = service.remove_key(&initial_primary).await;
        assert!(result.is_ok(), "remove old key should succeed");

        // Confirm the old key has been removed.
        let responses = service.list_keys(false);
        assert!(
            !responses.responses[0].keys.contains_key(&initial_primary),
            "old key should be removed"
        );
        assert!(
            responses.responses[0].keys.contains_key(&new_key),
            "new key should still be present"
        );
    }

    #[tokio::test]
    async fn test_keyring_remove_primary_fails() {
        let service = test_service();
        let key = make_valid_key(0x77);
        service.install_key(&key).unwrap();
        service.use_key(&key).await.unwrap();

        // Removing the primary key should return an error.
        let result = service.remove_key(&key).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Cannot remove primary key"));
    }

    #[tokio::test]
    async fn test_keyring_use_nonexistent_fails() {
        let service = test_service();
        let valid_key = make_valid_key(0xEE);

        // Using a valid key not in the keyring should return an error.
        let result = service.use_key(&valid_key).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found in keyring"));
    }

    #[test]
    fn test_keyring_install_multiple() {
        let service = test_service();

        let key_a = make_valid_key(0x01);
        let key_b = make_valid_key(0x02);
        let key_c = make_valid_key(0x03);

        service.install_key(&key_a).unwrap();
        service.install_key(&key_b).unwrap();
        service.install_key(&key_c).unwrap();

        let responses = service.list_keys(false);
        assert!(responses.responses[0].keys.len() >= 3);
        assert!(responses.responses[0].keys.contains_key(&key_a));
        assert!(responses.responses[0].keys.contains_key(&key_b));
        assert!(responses.responses[0].keys.contains_key(&key_c));
    }

    #[tokio::test]
    async fn test_keyring_use_and_remove() {
        let service = test_service();

        let key_1 = make_valid_key(0x11);
        let key_2 = make_valid_key(0x22);

        service.install_key(&key_1).unwrap();
        service.install_key(&key_2).unwrap();

        // Set key-1 as primary
        service.use_key(&key_1).await.unwrap();

        // Remove key-2 (non-primary) should succeed
        assert!(service.remove_key(&key_2).await.is_ok());

        // key-1 should remain
        let responses = service.list_keys(false);
        assert!(responses.responses[0].keys.contains_key(&key_1));
        assert!(!responses.responses[0].keys.contains_key(&key_2));
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

    #[actix_web::test]
    async fn test_transfer_leader_standalone_returns_success() {
        use actix_web::{App, test};

        // In standalone mode (no raft_node), transfer-leader returns
        // success with a "Standalone mode" warning — no actual transfer
        // is needed because there is only one node.
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
        assert!(
            body.success,
            "transfer-leader should report success in standalone mode"
        );
        let warning = body
            .warning
            .as_deref()
            .expect("standalone mode should include a warning");
        assert!(
            warning.contains("Standalone mode"),
            "warning should mention standalone mode, got: {}",
            warning
        );
    }

    #[actix_web::test]
    async fn test_transfer_leader_standalone_with_target_returns_success() {
        use actix_web::{App, test};

        // Even with a target ID specified, standalone mode short-circuits
        // to success because there is no RaftNode to perform the transfer.
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
            .uri("/v1/operator/raft/transfer-leader?id=127.0.0.1:8848")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(
            resp.status().is_success(),
            "standalone mode should succeed regardless of target"
        );

        let body: TransferLeaderResponse = test::read_body_json(resp).await;
        assert!(body.success);
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
