//! SDK trait contracts for the maintainer client.
//!
//! Defines composable trait hierarchy matching Nacos Java's interface pattern:
//! - `CoreMaintainerService` - server state, cluster, namespace, plugin management
//! - `ConfigMaintainerService` - configuration CRUD, history, listeners
//! - `NamingMaintainerService` - service/instance/subscriber management

use std::collections::HashMap;

use crate::model::{
    ClusterHealthResponse, ConfigBasicInfo, ConfigDetailInfo, ConfigHistoryBasicInfo,
    ConfigHistoryDetailInfo, ConnectionInfo, IdGeneratorInfo, Instance,
    InstanceMetadataBatchResult, Member, MetricsInfo, Namespace, Page, PluginAvailability,
    PluginDetail, PluginInfo, SelfMemberResponse, ServerLoaderMetrics, ServiceDetailInfo,
    ServiceView, SubscriberInfo,
};

// ============================================================================
// CoreMaintainerService
// ============================================================================

/// Core server administration operations.
///
/// Matches Nacos Java `CoreMaintainerService`.
#[async_trait::async_trait]
pub trait CoreMaintainerService: Send + Sync {
    /// Get server state map.
    async fn server_state(&self) -> anyhow::Result<HashMap<String, Option<String>>>;

    /// Check server liveness.
    async fn liveness(&self) -> anyhow::Result<bool>;

    /// Check server readiness.
    async fn readiness(&self) -> anyhow::Result<bool>;

    /// List cluster member nodes, optionally filtered by address and state.
    async fn list_cluster_nodes(
        &self,
        address: Option<&str>,
        state: Option<&str>,
    ) -> anyhow::Result<Vec<Member>>;

    /// Get the current node's self information.
    async fn cluster_self(&self) -> anyhow::Result<SelfMemberResponse>;

    /// Get cluster health summary.
    async fn cluster_health(&self) -> anyhow::Result<ClusterHealthResponse>;

    /// Update cluster lookup mode ("file" or "address-server").
    async fn update_lookup_mode(&self, mode: &str) -> anyhow::Result<bool>;

    /// Execute Raft operations.
    async fn raft_ops(
        &self,
        command: &str,
        value: &str,
        group_id: &str,
    ) -> anyhow::Result<String>;

    /// Get ID generator status.
    async fn get_id_generators(&self) -> anyhow::Result<HashMap<String, IdGeneratorInfo>>;

    /// Update core module log level.
    async fn update_log_level(&self, log_name: &str, log_level: &str) -> anyhow::Result<bool>;

    // --- Namespace management ---

    /// List all namespaces.
    async fn get_namespace_list(&self) -> anyhow::Result<Vec<Namespace>>;

    /// Get a namespace by ID.
    async fn get_namespace(&self, namespace_id: &str) -> anyhow::Result<Namespace>;

    /// Create a namespace.
    async fn create_namespace(
        &self,
        namespace_id: &str,
        namespace_name: &str,
        namespace_desc: &str,
    ) -> anyhow::Result<bool>;

    /// Update a namespace.
    async fn update_namespace(
        &self,
        namespace_id: &str,
        namespace_name: &str,
        namespace_desc: &str,
    ) -> anyhow::Result<bool>;

    /// Delete a namespace.
    async fn delete_namespace(&self, namespace_id: &str) -> anyhow::Result<bool>;

    /// Check if a namespace ID exists.
    async fn check_namespace_id_exist(&self, namespace_id: &str) -> anyhow::Result<bool>;

    // --- Client connection management ---

    /// Get all currently connected SDK clients.
    async fn get_current_clients(&self) -> anyhow::Result<HashMap<String, ConnectionInfo>>;

    /// Reload SDK connection count.
    async fn reload_connection_count(
        &self,
        count: i32,
        redirect_addr: Option<&str>,
    ) -> anyhow::Result<bool>;

    /// Smart reload cluster connections.
    async fn smart_reload_cluster(
        &self,
        loader_factor: Option<f64>,
    ) -> anyhow::Result<bool>;

    /// Get cluster loader metrics.
    async fn get_cluster_loader_metrics(&self) -> anyhow::Result<ServerLoaderMetrics>;

    // --- Plugin management ---

    /// List all plugins, optionally filtered by type.
    async fn list_plugins(
        &self,
        plugin_type: Option<&str>,
    ) -> anyhow::Result<Vec<PluginInfo>>;

    /// Get detailed information for a specific plugin.
    async fn get_plugin_detail(
        &self,
        plugin_type: &str,
        plugin_name: &str,
    ) -> anyhow::Result<PluginDetail>;

    /// Enable or disable a plugin.
    async fn update_plugin_status(
        &self,
        plugin_type: &str,
        plugin_name: &str,
        enabled: bool,
        local_only: bool,
    ) -> anyhow::Result<bool>;

    /// Update plugin configuration.
    async fn update_plugin_config(
        &self,
        plugin_type: &str,
        plugin_name: &str,
        config: &HashMap<String, String>,
        local_only: bool,
    ) -> anyhow::Result<bool>;

    /// Check plugin availability across cluster nodes.
    async fn get_plugin_availability(
        &self,
        plugin_type: &str,
        plugin_name: &str,
    ) -> anyhow::Result<PluginAvailability>;
}

// ============================================================================
// ConfigMaintainerService
// ============================================================================

/// Configuration administration operations.
///
/// Matches Nacos Java `ConfigMaintainerService`.
#[async_trait::async_trait]
pub trait ConfigMaintainerService: Send + Sync {
    /// Get a configuration by ID.
    async fn get_config(
        &self,
        data_id: &str,
        group: &str,
        namespace_id: &str,
    ) -> anyhow::Result<ConfigDetailInfo>;

    /// Search configurations with pagination.
    async fn search_configs(
        &self,
        namespace_id: &str,
        group: &str,
        data_id: &str,
        page_no: i32,
        page_size: i32,
        search_mode: &str,
    ) -> anyhow::Result<Page<ConfigBasicInfo>>;

    /// Publish (create or update) a configuration.
    async fn publish_config(
        &self,
        namespace_id: &str,
        group: &str,
        data_id: &str,
        content: &str,
        config_type: &str,
        desc: &str,
    ) -> anyhow::Result<bool>;

    /// Delete a configuration.
    async fn delete_config(
        &self,
        namespace_id: &str,
        group: &str,
        data_id: &str,
    ) -> anyhow::Result<bool>;

    /// List configuration history with pagination.
    async fn list_config_history(
        &self,
        namespace_id: &str,
        group: &str,
        data_id: &str,
        page_no: i32,
        page_size: i32,
    ) -> anyhow::Result<Page<ConfigHistoryBasicInfo>>;

    /// Get configuration history entry by nid.
    async fn get_config_history_info(
        &self,
        nid: u64,
        namespace_id: &str,
        group: &str,
        data_id: &str,
    ) -> anyhow::Result<ConfigHistoryDetailInfo>;

    /// Get configuration listeners.
    async fn get_config_listeners(
        &self,
        namespace_id: &str,
        group: &str,
        data_id: &str,
    ) -> anyhow::Result<HashMap<String, String>>;
}

// ============================================================================
// NamingMaintainerService
// ============================================================================

/// Naming (service discovery) administration operations.
///
/// Matches Nacos Java `NamingMaintainerService` which combines
/// `ServiceMaintainerService`, `InstanceMaintainerService`, and
/// `NamingClientMaintainerService`.
#[async_trait::async_trait]
pub trait NamingMaintainerService: Send + Sync {
    // --- Service management ---

    /// List services with pagination.
    async fn list_services(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name_pattern: &str,
        page_no: i32,
        page_size: i32,
    ) -> anyhow::Result<Page<ServiceView>>;

    /// Get service detail.
    async fn get_service_detail(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
    ) -> anyhow::Result<ServiceDetailInfo>;

    /// Create a service.
    async fn create_service(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        protect_threshold: f64,
        ephemeral: bool,
        metadata: Option<&str>,
    ) -> anyhow::Result<bool>;

    /// Update a service.
    async fn update_service(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        protect_threshold: f64,
        metadata: Option<&str>,
    ) -> anyhow::Result<bool>;

    /// Delete a service.
    async fn remove_service(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
    ) -> anyhow::Result<bool>;

    /// List service subscribers with pagination.
    async fn get_subscribers(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        page_no: i32,
        page_size: i32,
    ) -> anyhow::Result<Page<SubscriberInfo>>;

    /// List available service selector types.
    async fn list_selector_types(&self) -> anyhow::Result<Vec<String>>;

    // --- Instance management ---

    /// List instances for a service.
    async fn list_instances(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
        healthy_only: bool,
    ) -> anyhow::Result<Vec<Instance>>;

    /// Get instance detail.
    async fn get_instance_detail(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        ip: &str,
        port: i32,
        cluster_name: &str,
    ) -> anyhow::Result<Instance>;

    /// Register a persistent instance.
    async fn register_instance(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        instance: &Instance,
    ) -> anyhow::Result<bool>;

    /// Deregister an instance.
    async fn deregister_instance(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        ip: &str,
        port: i32,
        cluster_name: &str,
    ) -> anyhow::Result<bool>;

    /// Update an instance.
    async fn update_instance(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        instance: &Instance,
    ) -> anyhow::Result<bool>;

    /// Update persistent instance health status.
    async fn update_instance_health_status(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        ip: &str,
        port: i32,
        healthy: bool,
    ) -> anyhow::Result<bool>;

    /// Batch update instance metadata.
    async fn batch_update_instance_metadata(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        metadata: &str,
        instances: Option<&str>,
    ) -> anyhow::Result<InstanceMetadataBatchResult>;

    /// Batch delete instance metadata keys.
    async fn batch_delete_instance_metadata(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        metadata: &str,
        instances: Option<&str>,
    ) -> anyhow::Result<InstanceMetadataBatchResult>;

    // --- Naming metrics ---

    /// Get naming module metrics.
    async fn get_metrics(&self, only_status: bool) -> anyhow::Result<MetricsInfo>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_core_maintainer_service_is_object_safe() {
        fn _accept(_: &dyn CoreMaintainerService) {}
    }

    #[test]
    fn test_config_maintainer_service_is_object_safe() {
        fn _accept(_: &dyn ConfigMaintainerService) {}
    }

    #[test]
    fn test_naming_maintainer_service_is_object_safe() {
        fn _accept(_: &dyn NamingMaintainerService) {}
    }
}
