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
/// carries a `Warning` field when the leader transfer could not be completed
/// within the timeout window — the election was triggered but the leader
/// did not change in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TransferLeaderResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
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
/// 与Consul兼容的keyring响应，描述一个WAN或LANkeyring的状态
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct KeyringResponse {
    #[serde(rename = "WAN")]
    pub wan: bool,
    pub datacenter: String,
    pub segment: String,
    /// 分区名称（Consul Enterprise特性，OSS为空字符串）
    #[serde(skip_serializing_if = "String::is_empty")]
    pub partition: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub messages: Option<HashMap<String, String>>,
    /// key→持有该key的节点数
    pub keys: HashMap<String, i32>,
    /// 当前primary key→节点数
    pub primary_keys: HashMap<String, i32>,
    pub num_nodes: i32,
}

/// Keyring响应容器
/// Consul的JSON字段名是"Responses"（PascalCase）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct KeyringResponses {
    pub responses: Vec<KeyringResponse>,
}

/// 查询参数 for keyring operations
#[derive(Debug, Clone, Deserialize, Default)]
pub struct KeyringParams {
    /// relay-factor: 0-5，控制通过gossip中继转发请求的节点数
    #[serde(rename = "relay-factor")]
    pub relay_factor: Option<u8>,
    /// local-only: 仅GET/list操作支持，只返回本地节点的keyring信息
    #[serde(rename = "local-only")]
    pub local_only: Option<String>,
}

/// Query parameters for operator endpoints
#[derive(Debug, Clone, Deserialize, Default)]
pub struct OperatorQueryParams {
    pub dc: Option<String>,
    pub stale: Option<String>,
}

/// 验证key格式：必须是base64编码的32字节数据（AES-256密钥）
/// Consul使用AES-256-GCM加密gossip通信，密钥为32字节，base64编码后传输
fn validate_key_format(key: &str) -> Result<Vec<u8>, String> {
    // base64解码
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(key)
        .map_err(|e| format!("Invalid base64 key: {}", e))?;
    // 验证长度为32字节（AES-256）
    if decoded.len() != 32 {
        return Err(format!(
            "Key must be 32 bytes (AES-256), got {} bytes",
            decoded.len()
        ));
    }
    Ok(decoded)
}

/// 解析local_only查询参数为布尔值
/// Consul接受"true"/"1"/"yes"等为true，其他为false
fn parse_local_only(value: &Option<String>) -> bool {
    match value {
        Some(v) => matches!(v.to_lowercase().as_str(), "true" | "1" | "yes" | "on"),
        None => false,
    }
}

/// 验证relay_factor范围（0-5），超出返回错误消息
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
        // 使用32字节随机数据作为AES-256密钥（base64编码后44字符）
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

    /// 列出keyring中的所有key
    /// 返回KeyringResponses（包含LAN和WAN两个response，或仅LAN如果local_only=true）
    /// batata没有gossip/serf，WAN是LAN的副本（与Consul API兼容）
    pub fn list_keys(&self, local_only: bool) -> KeyringResponses {
        let num_nodes = self.member_manager.member_count() as i32;
        let keys: HashMap<String, i32> = self
            .keyring
            .iter()
            .map(|r| (r.key().clone(), *r.value()))
            .collect();

        // primary_keys只包含当前激活的primary key
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
            // local_only=true时只返回LAN response
            return KeyringResponses {
                responses: vec![lan_resp],
            };
        }

        // WAN response（batata中WAN是LAN的副本，因为没有gossip/serf）
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

    /// 安装新key到keyring
    /// 验证key格式（base64编码的32字节），如果key已存在则成功但不重复添加
    pub fn install_key(&self, key: &str) -> Result<KeyringResponses, String> {
        // 验证key格式
        validate_key_format(key)?;

        // 如果key已存在，返回成功（幂等操作，与Consul行为一致）
        if self.keyring.contains_key(key) {
            return Ok(self.list_keys(false));
        }

        // 添加到keyring
        let member_count = self.member_manager.member_count() as i32;
        self.keyring.insert(key.to_string(), member_count.max(1));

        Ok(self.list_keys(false))
    }

    /// 切换primary key
    /// 验证key格式，检查key是否在keyring中，然后设为新的primary key
    pub async fn use_key(&self, key: &str) -> Result<KeyringResponses, String> {
        // 验证key格式
        validate_key_format(key)?;

        // 检查key是否在keyring中
        if !self.keyring.contains_key(key) {
            return Err(format!("Key '{}' not found in keyring", key));
        }

        // 设置为新的primary key
        let mut primary = self.primary_key.write().await;
        *primary = Some(key.to_string());
        drop(primary);

        Ok(self.list_keys(false))
    }

    /// 从keyring移除key
    /// 验证key格式，检查key是否在keyring中，检查不是primary key
    pub async fn remove_key(&self, key: &str) -> Result<KeyringResponses, String> {
        // 验证key格式
        validate_key_format(key)?;

        // 检查key是否在keyring中
        if !self.keyring.contains_key(key) {
            return Err(format!("Key '{}' not found in keyring", key));
        }

        // 检查不是primary key（不能移除当前正在使用的primary key）
        let primary = self.primary_key.read().await;
        if primary.as_deref() == Some(key) {
            return Err("Cannot remove primary key".to_string());
        }
        drop(primary);

        // 从keyring移除
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
/// 列出keyring中的所有key，返回KeyringResponses格式
pub async fn keyring_list(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorService>,
    query: web::Query<KeyringParams>,
) -> HttpResponse {
    // Keyring list需要KeyringRead权限（read操作）
    let authz = acl_service.authorize_request(&req, ResourceType::Keyring, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    // 验证relay_factor范围（0-5）
    if let Err(e) = validate_relay_factor(query.relay_factor) {
        return HttpResponse::BadRequest().consul_error(ConsulError::new(e));
    }

    // 解析local_only参数（仅list操作支持）
    let local_only = parse_local_only(&query.local_only);

    let responses = operator_service.list_keys(local_only);
    HttpResponse::Ok().json(responses)
}

/// POST /v1/operator/keyring
/// 安装新key到keyring
pub async fn keyring_install(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorService>,
    query: web::Query<KeyringParams>,
    body: web::Json<KeyringRequest>,
) -> HttpResponse {
    // Keyring install需要KeyringWrite权限（write操作）
    let authz = acl_service.authorize_request(&req, ResourceType::Keyring, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    // 验证relay_factor范围（0-5）
    if let Err(e) = validate_relay_factor(query.relay_factor) {
        return HttpResponse::BadRequest().consul_error(ConsulError::new(e));
    }

    // local_only仅list操作支持，其他操作返回400错误
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
/// 切换primary key
pub async fn keyring_use(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorService>,
    query: web::Query<KeyringParams>,
    body: web::Json<KeyringRequest>,
) -> HttpResponse {
    // Keyring use需要KeyringWrite权限（write操作）
    let authz = acl_service.authorize_request(&req, ResourceType::Keyring, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    // 验证relay_factor范围（0-5）
    if let Err(e) = validate_relay_factor(query.relay_factor) {
        return HttpResponse::BadRequest().consul_error(ConsulError::new(e));
    }

    // local_only仅list操作支持，其他操作返回400错误
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
/// 从keyring移除key
pub async fn keyring_remove(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorService>,
    query: web::Query<KeyringParams>,
    body: web::Json<KeyringRequest>,
) -> HttpResponse {
    // Keyring remove需要KeyringWrite权限（write操作）
    let authz = acl_service.authorize_request(&req, ResourceType::Keyring, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    // 验证relay_factor范围（0-5）
    if let Err(e) = validate_relay_factor(query.relay_factor) {
        return HttpResponse::BadRequest().consul_error(ConsulError::new(e));
    }

    // local_only仅list操作支持，其他操作返回400错误
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

    /// 生成有效的AES-256密钥（base64编码的32字节），用于测试
    fn make_valid_key(seed: u8) -> String {
        base64::engine::general_purpose::STANDARD.encode([seed; 32])
    }

    #[test]
    fn test_keyring_validate_key_format() {
        // 有效key：base64编码的32字节
        let valid_key = make_valid_key(0xAA);
        let result = validate_key_format(&valid_key);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 32);

        // 无效base64
        let result = validate_key_format("not-valid-base64!!!");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid base64 key"));

        // 长度不足（16字节 = AES-128，不被接受）
        let short_key = base64::engine::general_purpose::STANDARD.encode([0u8; 16]);
        let result = validate_key_format(&short_key);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("32 bytes"));

        // 长度超出（64字节）
        let long_key = base64::engine::general_purpose::STANDARD.encode([0u8; 64]);
        let result = validate_key_format(&long_key);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("32 bytes"));
    }

    #[test]
    fn test_keyring_install_invalid_key() {
        let service = test_service();

        // 无效key格式应返回错误
        let result = service.install_key("invalid-key");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid base64 key"));

        // 长度不对的key
        let short_key = base64::engine::general_purpose::STANDARD.encode([0u8; 16]);
        let result = service.install_key(&short_key);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("32 bytes"));
    }

    #[test]
    fn test_keyring_relay_factor_validation() {
        // relay_factor 0-5 有效
        assert!(validate_relay_factor(None).is_ok());
        assert!(validate_relay_factor(Some(0)).is_ok());
        assert!(validate_relay_factor(Some(3)).is_ok());
        assert!(validate_relay_factor(Some(5)).is_ok());

        // relay_factor > 5 无效
        let result = validate_relay_factor(Some(6));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("relay-factor must be between 0 and 5"));
    }

    #[test]
    fn test_keyring_local_only_only_for_list() {
        // local_only 参数解析
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

        // 默认返回LAN和WAN两个response
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

        // local_only=true时只返回LAN response
        let responses = service.list_keys(true);
        assert_eq!(responses.responses.len(), 1);
        assert!(!responses.responses[0].wan);
    }

    #[test]
    fn test_keyring_install_duplicate_key() {
        let service = test_service();
        let key = make_valid_key(0x42);

        // 第一次安装
        let result = service.install_key(&key);
        assert!(result.is_ok());

        // 第二次安装同一个key不应报错（幂等操作）
        let result = service.install_key(&key);
        assert!(result.is_ok());

        // key应该只存在一个
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

        // 获取初始primary key
        let initial = service.list_keys(false);
        let initial_primary = initial.responses[0]
            .primary_keys
            .keys()
            .next()
            .cloned()
            .expect("should have an initial primary key");

        // 步骤1: install新key
        let new_key = make_valid_key(0x99);
        let result = service.install_key(&new_key);
        assert!(result.is_ok(), "install should succeed");

        // 确认新key在keyring中
        let responses = service.list_keys(false);
        assert!(
            responses.responses[0].keys.contains_key(&new_key),
            "new key should be in keyring after install"
        );

        // 步骤2: use新key（切换primary）
        let result = service.use_key(&new_key).await;
        assert!(result.is_ok(), "use should succeed");

        // 确认primary key已切换
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

        // 步骤3: remove旧key
        let result = service.remove_key(&initial_primary).await;
        assert!(result.is_ok(), "remove old key should succeed");

        // 确认旧key已移除
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

        // 移除primary key应返回错误
        let result = service.remove_key(&key).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Cannot remove primary key"));
    }

    #[tokio::test]
    async fn test_keyring_use_nonexistent_fails() {
        let service = test_service();
        let valid_key = make_valid_key(0xEE);

        // 使用不在keyring中的有效key应返回错误
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
