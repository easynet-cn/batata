// Cluster management and coordination
// Manages cluster membership, health checks, and inter-node communication

use std::{collections::HashSet, sync::Arc};

use dashmap::DashMap;
use serde_json::Value;
use tokio::sync::RwLock;
use tracing::info;

use batata_api::model::{Member, MemberBuilder, NodeState};
use batata_common::local_ip;

use crate::model::Configuration;

use super::{
    cluster_client::{ClusterClientConfig, ClusterClientManager},
    distro::{DistroConfig, DistroProtocol},
    health_check::{HealthCheckConfig, MemberHealthChecker},
    member_event::{
        LoggingMemberChangeListener, MemberChangeEvent, MemberChangeEventPublisher,
        MemberChangeListener,
    },
    member_lookup::{MemberLookup, create_member_lookup},
};

/// Server member manager configuration
#[derive(Clone, Debug)]
pub struct ServerMemberManagerConfig {
    /// Health check configuration
    pub health_check: HealthCheckConfig,
    /// Cluster client configuration
    pub cluster_client: ClusterClientConfig,
    /// Distro protocol configuration
    pub distro: DistroConfig,
    /// Event queue size
    pub event_queue_size: usize,
    /// Whether to enable health checks
    pub health_check_enabled: bool,
    /// Whether to enable distro protocol
    pub distro_enabled: bool,
}

impl Default for ServerMemberManagerConfig {
    fn default() -> Self {
        Self {
            health_check: HealthCheckConfig::default(),
            cluster_client: ClusterClientConfig::default(),
            distro: DistroConfig::default(),
            event_queue_size: 1024,
            health_check_enabled: true,
            distro_enabled: true,
        }
    }
}

/// Server member manager
/// Central component for cluster management
#[derive(Clone)]
pub struct ServerMemberManager {
    port: u16,
    local_address: String,
    self_member: Arc<Member>,
    server_list: Arc<DashMap<String, Member>>,
    is_standalone: bool,
    config: Configuration,
    manager_config: ServerMemberManagerConfig,
    member_lookup: Arc<RwLock<Option<Box<dyn MemberLookup>>>>,
    health_checker: Arc<RwLock<Option<MemberHealthChecker>>>,
    event_publisher: Arc<MemberChangeEventPublisher>,
    client_manager: Arc<RwLock<Option<Arc<ClusterClientManager>>>>,
    distro_protocol: Arc<RwLock<Option<Arc<DistroProtocol>>>>,
    running: Arc<RwLock<bool>>,
}

impl std::fmt::Debug for ServerMemberManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerMemberManager")
            .field("port", &self.port)
            .field("local_address", &self.local_address)
            .field("is_standalone", &self.is_standalone)
            .field("member_count", &self.server_list.len())
            .finish()
    }
}

impl ServerMemberManager {
    pub fn new(config: &Configuration) -> Self {
        Self::with_config(config, ServerMemberManagerConfig::default())
    }

    pub fn with_config(config: &Configuration, manager_config: ServerMemberManagerConfig) -> Self {
        let ip = local_ip();
        let port = config.server_main_port();
        let local_address = format!("{}:{}", ip, port);
        let is_standalone = config.is_standalone();

        let server_list = Arc::new(DashMap::new());
        let member = MemberBuilder::new(ip.clone(), port).build();

        // Set extended member info (acquire write lock once for all inserts)
        {
            let mut extend_info = member
                .extend_info
                .write()
                .unwrap_or_else(|e| e.into_inner());
            extend_info.insert(
                Member::RAFT_PORT.to_string(),
                Value::from(member.calculate_raft_port()),
            );
            extend_info.insert(Member::READY_TO_UPGRADE.to_string(), Value::from(true));
            extend_info.insert(Member::VERSION.to_string(), Value::from(config.version()));
            extend_info.insert(Member::SUPPORT_GRAY_MODEL.to_string(), Value::from(true));
            extend_info.insert(
                Member::LAST_REFRESH_TIME.to_string(),
                Value::from(chrono::Utc::now().timestamp_millis()),
            );
        }

        // Add self to server list
        server_list.insert(local_address.clone(), member.clone());

        // Create event publisher
        let event_publisher = Arc::new(MemberChangeEventPublisher::new(
            manager_config.event_queue_size,
        ));

        Self {
            port,
            local_address,
            self_member: Arc::new(member),
            server_list,
            is_standalone,
            config: config.clone(),
            manager_config,
            member_lookup: Arc::new(RwLock::new(None)),
            health_checker: Arc::new(RwLock::new(None)),
            event_publisher,
            client_manager: Arc::new(RwLock::new(None)),
            distro_protocol: Arc::new(RwLock::new(None)),
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Start the server member manager
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut running = self.running.write().await;
        if *running {
            return Ok(());
        }

        info!(
            "Starting ServerMemberManager, standalone: {}",
            self.is_standalone
        );

        // Start event publisher
        self.event_publisher.start().await;

        // Register logging listener
        self.event_publisher
            .register_listener(Arc::new(LoggingMemberChangeListener))
            .await;

        if !self.is_standalone {
            // Initialize and start member lookup
            let lookup = create_member_lookup(&self.config);
            lookup.start().await?;

            // Load discovered members
            let members = lookup.get_members();
            self.update_members_from_lookup(members).await;

            let mut lookup_guard = self.member_lookup.write().await;
            *lookup_guard = Some(lookup);

            // Initialize cluster client manager (wrap in Arc for sharing)
            let client_manager = Arc::new(ClusterClientManager::new(
                self.local_address.clone(),
                self.manager_config.cluster_client.clone(),
            ));

            {
                let mut cm_guard = self.client_manager.write().await;
                *cm_guard = Some(client_manager.clone());
            }

            // Start health checker if enabled
            if self.manager_config.health_check_enabled {
                let health_checker = MemberHealthChecker::new(
                    self.server_list.clone(),
                    self.local_address.clone(),
                    self.manager_config.health_check.clone(),
                );
                health_checker.start().await;

                let mut hc_guard = self.health_checker.write().await;
                *hc_guard = Some(health_checker);
            }

            // Start distro protocol if enabled
            if self.manager_config.distro_enabled {
                let dp_guard = self.distro_protocol.read().await;
                if let Some(ref distro) = *dp_guard {
                    // Use externally set distro protocol (shared with gRPC handlers)
                    distro.start().await;
                } else {
                    drop(dp_guard);
                    // Create a new distro protocol if none was set externally
                    let distro = Arc::new(DistroProtocol::new(
                        self.local_address.clone(),
                        self.manager_config.distro.clone(),
                        client_manager.clone(),
                        self.server_list.clone(),
                    ));
                    distro.start().await;

                    let mut dp_guard = self.distro_protocol.write().await;
                    *dp_guard = Some(distro);
                }
            }

            info!(
                "Cluster mode started with {} members",
                self.server_list.len()
            );
        } else {
            info!("Standalone mode - skipping cluster initialization");
        }

        *running = true;
        Ok(())
    }

    /// Stop the server member manager
    pub async fn stop(&self) {
        let mut running = self.running.write().await;
        if !*running {
            return;
        }

        info!("Stopping ServerMemberManager");

        // Stop member lookup
        if let Some(lookup) = self.member_lookup.read().await.as_ref() {
            lookup.stop().await;
        }

        // Stop health checker (synchronous - uses atomic bool)
        if let Some(hc) = self.health_checker.read().await.as_ref() {
            hc.stop();
        }

        // Stop distro protocol
        if let Some(dp) = self.distro_protocol.read().await.as_ref() {
            dp.stop().await;
        }

        // Stop event publisher
        self.event_publisher.stop().await;

        *running = false;
    }

    /// Update members from lookup results
    async fn update_members_from_lookup(&self, members: Vec<Member>) {
        let current_addresses: HashSet<String> =
            self.server_list.iter().map(|e| e.key().clone()).collect();

        let new_addresses: HashSet<String> = members.iter().map(|m| m.address.clone()).collect();

        // Add new members
        for member in members {
            if !current_addresses.contains(&member.address) {
                info!("Adding new cluster member: {}", member.address);
                self.server_list
                    .insert(member.address.clone(), member.clone());

                // Publish join event
                self.event_publisher
                    .publish(MemberChangeEvent::member_join(member))
                    .await;
            }
        }

        // Remove members that are no longer in the list (except self)
        for addr in current_addresses {
            if addr != self.local_address
                && !new_addresses.contains(&addr)
                && let Some((_, member)) = self.server_list.remove(&addr)
            {
                info!("Removing cluster member: {}", addr);

                // Publish leave event
                self.event_publisher
                    .publish(MemberChangeEvent::member_leave(member))
                    .await;
            }
        }
    }

    /// Get the shared server list map (for wiring to DistroProtocol)
    pub fn server_list(&self) -> Arc<DashMap<String, Member>> {
        self.server_list.clone()
    }

    /// Set the distro protocol externally (shared with gRPC handlers)
    ///
    /// Must be called before `start()`. The protocol will be started
    /// during `start()` instead of creating a new one.
    pub async fn set_distro_protocol(&self, protocol: Arc<DistroProtocol>) {
        let mut dp_guard = self.distro_protocol.write().await;
        *dp_guard = Some(protocol);
    }

    /// Get all cluster members
    pub fn all_members(&self) -> Vec<Member> {
        self.server_list.iter().map(|e| e.value().clone()).collect()
    }

    /// Get all healthy members
    pub fn healthy_members(&self) -> Vec<Member> {
        self.server_list
            .iter()
            .filter(|e| matches!(e.value().state, NodeState::Up))
            .map(|e| e.value().clone())
            .collect()
    }

    /// Get member by address
    pub fn get_member(&self, address: &str) -> Option<Member> {
        self.server_list.get(address).map(|e| e.value().clone())
    }

    /// Get local member
    pub fn get_self(&self) -> &Member {
        &self.self_member
    }

    /// Get local address
    pub fn local_address(&self) -> &str {
        &self.local_address
    }

    /// Get member count
    pub fn member_count(&self) -> usize {
        self.server_list.len()
    }

    /// Check if running in standalone mode
    pub fn is_standalone(&self) -> bool {
        self.is_standalone
    }

    /// Check if a member is the local node
    pub fn is_self(&self, address: &str) -> bool {
        address == self.local_address
    }

    /// Update member state
    pub async fn update_member_state(&self, address: &str, state: NodeState) {
        if let Some(mut member) = self.server_list.get_mut(address) {
            let previous_state = member.state;
            member.state = state;

            // Publish state change event
            self.event_publisher
                .publish(MemberChangeEvent::member_state_change(
                    member.clone(),
                    previous_state,
                ))
                .await;
        }
    }

    /// Register a member change listener
    pub async fn register_listener(&self, listener: Arc<dyn MemberChangeListener>) {
        self.event_publisher.register_listener(listener).await;
    }

    /// Subscribe to member change events
    pub fn subscribe_events(&self) -> tokio::sync::broadcast::Receiver<MemberChangeEvent> {
        self.event_publisher.subscribe()
    }

    /// Get event publisher
    pub fn event_publisher(&self) -> &Arc<MemberChangeEventPublisher> {
        &self.event_publisher
    }

    /// Check if cluster is healthy (majority of nodes are up)
    pub fn is_cluster_healthy(&self) -> bool {
        if self.is_standalone {
            return true;
        }

        let total = self.server_list.len();
        let healthy = self
            .server_list
            .iter()
            .filter(|e| matches!(e.value().state, NodeState::Up))
            .count();

        // More than half of the nodes should be healthy
        healthy > total / 2
    }

    /// Check if this node is the leader
    ///
    /// In standalone mode, this always returns true.
    /// In cluster mode, the leader is determined by:
    /// - The first healthy node (by sorted address) is considered the leader
    /// - This provides a simple deterministic leader election without Raft overhead
    ///
    /// Note: For CP (consistent) operations, use the Raft consensus layer instead.
    /// This method is suitable for AP (available) operations and cluster status reporting.
    pub fn is_leader(&self) -> bool {
        if self.is_standalone {
            return true;
        }

        // Get all healthy members and sort by address
        let mut healthy_addresses: Vec<String> = self
            .server_list
            .iter()
            .filter(|e| matches!(e.value().state, NodeState::Up))
            .map(|e| e.key().clone())
            .collect();

        healthy_addresses.sort();

        // The first healthy node (by sorted address) is the leader
        healthy_addresses
            .first()
            .map(|addr| addr == &self.local_address)
            .unwrap_or(false)
    }

    /// Get the current leader address
    ///
    /// Returns the address of the current leader node.
    /// In standalone mode, returns the local address.
    /// In cluster mode, returns the first healthy node by sorted address.
    pub fn leader_address(&self) -> Option<String> {
        if self.is_standalone {
            return Some(self.local_address.clone());
        }

        // Get all healthy members and sort by address
        let mut healthy_addresses: Vec<String> = self
            .server_list
            .iter()
            .filter(|e| matches!(e.value().state, NodeState::Up))
            .map(|e| e.key().clone())
            .collect();

        healthy_addresses.sort();
        healthy_addresses.into_iter().next()
    }

    /// Get cluster health summary
    pub fn health_summary(&self) -> ClusterHealthSummary {
        let mut summary = ClusterHealthSummary {
            total: 0,
            up: 0,
            down: 0,
            suspicious: 0,
            starting: 0,
            isolation: 0,
        };

        for member in self.server_list.iter() {
            summary.total += 1;
            match member.value().state {
                NodeState::Up => summary.up += 1,
                NodeState::Down => summary.down += 1,
                NodeState::Suspicious => summary.suspicious += 1,
                NodeState::Starting => summary.starting += 1,
                NodeState::Isolation => summary.isolation += 1,
            }
        }

        summary
    }

    /// Refresh local member's last refresh time
    pub fn refresh_self(&self) {
        if let Some(member) = self.server_list.get_mut(&self.local_address)
            && let Ok(mut extend_info) = member.extend_info.write()
        {
            extend_info.insert(
                Member::LAST_REFRESH_TIME.to_string(),
                Value::from(chrono::Utc::now().timestamp_millis()),
            );
        }
    }
}

/// Cluster health summary
#[derive(Clone, Debug, Default)]
pub struct ClusterHealthSummary {
    pub total: usize,
    pub up: usize,
    pub down: usize,
    pub suspicious: usize,
    pub starting: usize,
    pub isolation: usize,
}

impl ClusterHealthSummary {
    pub fn is_healthy(&self) -> bool {
        self.up > self.total / 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> Configuration {
        let config = config::Config::builder()
            .set_default("nacos.standalone", true)
            .unwrap()
            .set_default("nacos.server.main.port", 8848)
            .unwrap()
            .set_default("nacos.version", "1.0.0")
            .unwrap()
            .build()
            .unwrap();
        Configuration::from_config(config)
    }

    #[test]
    fn test_cluster_health_summary() {
        let summary = ClusterHealthSummary {
            total: 5,
            up: 3,
            down: 1,
            suspicious: 1,
            starting: 0,
            isolation: 0,
        };

        assert!(summary.is_healthy());

        let unhealthy = ClusterHealthSummary {
            total: 5,
            up: 2,
            down: 3,
            suspicious: 0,
            starting: 0,
            isolation: 0,
        };

        assert!(!unhealthy.is_healthy());
    }

    #[test]
    fn test_is_leader_standalone() {
        // In standalone mode, node is always the leader
        let config = test_config();
        let manager = ServerMemberManager::new(&config);

        // Standalone mode should always return true
        assert!(manager.is_standalone());
        assert!(manager.is_leader());
    }

    #[test]
    fn test_leader_address_standalone() {
        let config = test_config();
        let manager = ServerMemberManager::new(&config);

        // Standalone mode should return local address
        let leader = manager.leader_address();
        assert!(leader.is_some());
        assert_eq!(leader.unwrap(), manager.local_address());
    }
}
