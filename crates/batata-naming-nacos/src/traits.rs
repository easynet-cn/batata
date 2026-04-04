//! Nacos naming service traits
//!
//! These traits define the Nacos-specific service discovery contract.
//! They use Nacos domain models exclusively — no Consul types.

use std::sync::Arc;

use async_trait::async_trait;

use batata_foundation::types::PagedResult;

use crate::error::NacosNamingError;
use crate::model::{
    ClusterConfig, NacosInstance, NacosInstanceQuery, NacosService, NacosServiceQuery,
    ProtectionInfo, ServiceMetadata,
};

/// Core Nacos naming service provider
///
/// This is the primary trait for Nacos service discovery operations.
/// Implementations handle the full Nacos naming lifecycle:
/// namespace → group → service → cluster → instance
#[async_trait]
#[allow(clippy::too_many_arguments)]
pub trait NacosNamingService: Send + Sync {
    // ========================================================================
    // Instance Operations
    // ========================================================================

    /// Register a service instance
    ///
    /// If the instance already exists (same ip#port#cluster), it is updated.
    /// For ephemeral instances, a heartbeat timer is started automatically.
    async fn register_instance(
        &self,
        namespace: &str,
        group: &str,
        service: &str,
        instance: NacosInstance,
    ) -> Result<bool, NacosNamingError>;

    /// Deregister a service instance
    async fn deregister_instance(
        &self,
        namespace: &str,
        group: &str,
        service: &str,
        ip: &str,
        port: i32,
        cluster: &str,
        ephemeral: bool,
    ) -> Result<bool, NacosNamingError>;

    /// Get instances for a service with query filters
    async fn get_instances(
        &self,
        query: &NacosInstanceQuery,
    ) -> Result<Vec<Arc<NacosInstance>>, NacosNamingError>;

    /// Get a service definition with all its instances
    async fn get_service(
        &self,
        namespace: &str,
        group: &str,
        service: &str,
    ) -> Result<Option<NacosService>, NacosNamingError>;

    /// Batch register instances
    async fn batch_register_instances(
        &self,
        namespace: &str,
        group: &str,
        service: &str,
        instances: Vec<NacosInstance>,
    ) -> Result<bool, NacosNamingError>;

    /// Batch deregister instances
    async fn batch_deregister_instances(
        &self,
        namespace: &str,
        group: &str,
        service: &str,
        instances: Vec<NacosInstance>,
    ) -> Result<bool, NacosNamingError>;

    /// Process a heartbeat from a client
    async fn heartbeat(
        &self,
        namespace: &str,
        group: &str,
        service: &str,
        instance: NacosInstance,
    ) -> Result<HeartbeatResponse, NacosNamingError>;

    /// Manually update instance health status
    async fn update_instance_health(
        &self,
        namespace: &str,
        group: &str,
        service: &str,
        ip: &str,
        port: i32,
        cluster: &str,
        healthy: bool,
    ) -> Result<bool, NacosNamingError>;

    // ========================================================================
    // Service Operations
    // ========================================================================

    /// List services with pagination
    async fn list_services(
        &self,
        query: &NacosServiceQuery,
    ) -> Result<PagedResult<String>, NacosNamingError>;

    /// Check if a service exists
    async fn service_exists(
        &self,
        namespace: &str,
        group: &str,
        service: &str,
    ) -> Result<bool, NacosNamingError>;

    /// Get instance count for a service
    async fn get_instance_count(
        &self,
        namespace: &str,
        group: &str,
        service: &str,
    ) -> Result<usize, NacosNamingError>;

    // ========================================================================
    // Metadata Operations
    // ========================================================================

    /// Set service metadata
    async fn set_service_metadata(
        &self,
        namespace: &str,
        group: &str,
        service: &str,
        metadata: ServiceMetadata,
    ) -> Result<(), NacosNamingError>;

    /// Get service metadata
    async fn get_service_metadata(
        &self,
        namespace: &str,
        group: &str,
        service: &str,
    ) -> Result<Option<ServiceMetadata>, NacosNamingError>;

    /// Update protection threshold
    async fn update_protect_threshold(
        &self,
        namespace: &str,
        group: &str,
        service: &str,
        threshold: f32,
    ) -> Result<(), NacosNamingError>;

    /// Get protection info for a service
    async fn get_protection_info(
        &self,
        namespace: &str,
        group: &str,
        service: &str,
    ) -> Result<ProtectionInfo, NacosNamingError>;

    // ========================================================================
    // Cluster Operations
    // ========================================================================

    /// Set cluster configuration
    async fn set_cluster_config(
        &self,
        namespace: &str,
        group: &str,
        service: &str,
        config: ClusterConfig,
    ) -> Result<(), NacosNamingError>;

    /// Get cluster configuration
    async fn get_cluster_config(
        &self,
        namespace: &str,
        group: &str,
        service: &str,
        cluster: &str,
    ) -> Result<Option<ClusterConfig>, NacosNamingError>;

    // ========================================================================
    // Subscription Operations
    // ========================================================================

    /// Subscribe to service changes
    async fn subscribe(
        &self,
        subscriber_id: &str,
        namespace: &str,
        group: &str,
        service: &str,
        clusters: &str,
    ) -> Result<(), NacosNamingError>;

    /// Unsubscribe from service changes
    async fn unsubscribe(
        &self,
        subscriber_id: &str,
        namespace: &str,
        group: &str,
        service: &str,
        clusters: &str,
    ) -> Result<(), NacosNamingError>;

    /// Get subscribers for a service
    async fn get_subscribers(
        &self,
        namespace: &str,
        group: &str,
        service: &str,
    ) -> Result<Vec<String>, NacosNamingError>;

    // ========================================================================
    // Connection Lifecycle
    // ========================================================================

    /// Deregister all instances belonging to a connection (on disconnect)
    async fn deregister_all_by_connection(
        &self,
        connection_id: &str,
    ) -> Result<(), NacosNamingError>;
}

/// Heartbeat response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatResponse {
    /// Whether light-beat mode is enabled
    pub light_beat_enabled: bool,
    /// Recommended heartbeat interval in milliseconds
    pub client_beat_interval: i64,
}

impl Default for HeartbeatResponse {
    fn default() -> Self {
        Self {
            light_beat_enabled: true,
            client_beat_interval: 5000,
        }
    }
}

use serde::{Deserialize, Serialize};

/// Nacos health checker — executes active health checks
///
/// Implementations: TcpChecker, HttpChecker, MysqlChecker
#[async_trait]
pub trait NacosHealthChecker: Send + Sync {
    /// Health check type this checker handles
    fn check_type(&self) -> crate::model::HealthCheckType;

    /// Execute a health check against an instance
    async fn check(&self, instance: &NacosInstance, config: &ClusterConfig) -> HealthCheckResult;
}

/// Result of a health check execution
#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    pub healthy: bool,
    pub message: Option<String>,
    pub latency_ms: u64,
}

/// Nacos naming event listener
///
/// Receives notifications when service instances change.
/// Used by gRPC push, long-polling, and inter-node sync.
#[async_trait]
pub trait NacosNamingEventListener: Send + Sync {
    /// Called when instances of a service change
    async fn on_instances_changed(
        &self,
        namespace: &str,
        group: &str,
        service: &str,
        instances: &[Arc<NacosInstance>],
    );
}

/// Distro data handler for ephemeral instance replication (AP protocol)
#[async_trait]
pub trait NacosDistroHandler: Send + Sync {
    /// Data type this handler manages
    fn data_type(&self) -> &str;

    /// Get all keys this node is responsible for
    async fn get_all_keys(&self) -> Result<Vec<String>, NacosNamingError>;

    /// Get data for a specific key
    async fn get_data(&self, key: &str) -> Result<Option<bytes::Bytes>, NacosNamingError>;

    /// Process sync data from a peer
    async fn process_sync_data(&self, data: &[u8]) -> Result<bool, NacosNamingError>;

    /// Process verification data from a peer
    async fn process_verify_data(&self, data: &[u8]) -> Result<bool, NacosNamingError>;

    /// Get a full snapshot for initial sync
    async fn get_snapshot(&self) -> Result<bytes::Bytes, NacosNamingError>;
}
