// Naming service provider trait for service discovery operations
//
// This trait abstracts the naming service interface, enabling consumers
// (console, consul plugin, etc.) to depend on the trait rather than the
// concrete implementation.

use std::collections::HashMap;
use std::sync::Arc;

use super::model::{
    ClusterConfig, ClusterStatistics, Instance, ProtectionInfo, Service, ServiceMetadata,
};

/// Naming service provider trait for service discovery operations
///
/// This trait abstracts the naming service interface, enabling consumers
/// (console, consul plugin, etc.) to depend on the trait rather than the
/// concrete implementation.
pub trait NamingServiceProvider: Send + Sync {
    // === Instance operations ===

    /// The `register_instance` method.
    fn register_instance(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instance: Instance,
    ) -> bool;

    /// The `deregister_instance` method.
    fn deregister_instance(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instance: &Instance,
    ) -> bool;

    /// Returns the instances.
    fn get_instances(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster: &str,
        healthy_only: bool,
    ) -> Vec<Instance>;

    /// Zero-copy snapshot of instances: returns `Vec<Arc<Instance>>`.
    ///
    /// Each element is a cheap pointer clone — no `Instance::clone()`, no
    /// `HashMap::clone()` on metadata. Read-only callers (JSON serialization,
    /// filters, protocol sync) should prefer this method, which is 40-50x
    /// faster than `get_instances` for services with 1000+ instances and
    /// typical metadata payloads.
    ///
    /// Default impl calls `get_instances` and wraps in `Arc` — concrete impls
    /// should override to avoid the deep clone.
    fn get_instances_snapshot(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster: &str,
        healthy_only: bool,
    ) -> Vec<Arc<Instance>> {
        self.get_instances(namespace, group_name, service_name, cluster, healthy_only)
            .into_iter()
            .map(Arc::new)
            .collect()
    }

    /// Returns the service.
    fn get_service(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster: &str,
        healthy_only: bool,
    ) -> Service;

    /// Returns the service with protection info.
    fn get_service_with_protection_info(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster: &str,
        healthy_only: bool,
    ) -> (Service, ProtectionInfo);

    /// The `list_services` method.
    fn list_services(
        &self,
        namespace: &str,
        group_name: &str,
        page_no: i32,
        page_size: i32,
    ) -> (i32, Vec<String>);

    /// The `service_exists` method.
    fn service_exists(&self, namespace: &str, group_name: &str, service_name: &str) -> bool;

    /// Returns the all service keys.
    fn get_all_service_keys(&self) -> Vec<String>;

    /// Count of registered services without allocating a key list.
    /// Default impl delegates to `get_all_service_keys().len()` so existing
    /// implementors keep compiling; override for O(1) maps.
    fn service_count(&self) -> usize {
        self.get_all_service_keys().len()
    }

    /// The `batch_register_instances` method.
    fn batch_register_instances(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instances: Vec<Instance>,
    ) -> bool;

    /// The `replace_ephemeral_instances` method.
    fn replace_ephemeral_instances(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instances: Vec<Instance>,
    ) -> bool;

    /// The `merge_remote_instances` method.
    ///
    /// `source` is the address of the node that sent this Distro sync. It scopes
    /// the garbage-collection of stale replicas to that origin (matching Nacos
    /// Distro's per-client reconciliation) so a sync from one node can never
    /// delete instances another node replicated here.
    fn merge_remote_instances(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instances: Vec<Instance>,
        source: &str,
    ) -> bool;

    /// The `batch_deregister_instances` method.
    fn batch_deregister_instances(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instances: Vec<Instance>,
    ) -> bool;

    /// The `heartbeat` method.
    fn heartbeat(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instance: Instance,
    ) -> bool;

    #[allow(clippy::too_many_arguments)]
    /// The `update_instance_health` method.
    fn update_instance_health(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        ip: &str,
        port: i32,
        cluster_name: &str,
        healthy: bool,
    ) -> bool;

    /// Returns the instance count.
    fn get_instance_count(&self) -> (usize, usize);

    /// Returns the healthy instance count.
    fn get_healthy_instance_count(&self) -> (usize, usize);

    // === Metadata operations ===

    /// Sets the service metadata.
    fn set_service_metadata(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        metadata: ServiceMetadata,
    );

    /// Returns the service metadata.
    fn get_service_metadata(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    ) -> Option<ServiceMetadata>;

    /// The `update_service_protect_threshold` method.
    fn update_service_protect_threshold(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        protect_threshold: f32,
    );

    /// The `update_service_selector` method.
    fn update_service_selector(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        selector_type: &str,
        selector_expression: &str,
    );

    /// The `update_service_metadata_map` method.
    fn update_service_metadata_map(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        metadata: HashMap<String, String>,
    );

    /// The `delete_service_metadata` method.
    fn delete_service_metadata(&self, namespace: &str, group_name: &str, service_name: &str);

    // === Subscription operations ===

    /// The `subscribe` method.
    fn subscribe(&self, connection_id: &str, namespace: &str, group_name: &str, service_name: &str);

    /// The `unsubscribe` method.
    fn unsubscribe(
        &self,
        connection_id: &str,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    );

    /// Returns the subscribers.
    fn get_subscribers(&self, namespace: &str, group_name: &str, service_name: &str)
    -> Vec<String>;

    /// The `remove_subscriber` method.
    fn remove_subscriber(&self, connection_id: &str);

    /// The `add_publisher` method.
    fn add_publisher(
        &self,
        connection_id: &str,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    );

    /// The `remove_publisher` method.
    fn remove_publisher(
        &self,
        connection_id: &str,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    );

    /// Returns the published services.
    fn get_published_services(&self, connection_id: &str) -> Vec<String>;

    /// Returns the publishers.
    fn get_publishers(&self, namespace: &str, group_name: &str, service_name: &str) -> Vec<String>;

    /// Returns the subscribed services.
    fn get_subscribed_services(&self, connection_id: &str) -> Vec<String>;

    /// Returns the all publisher ids.
    fn get_all_publisher_ids(&self) -> Vec<String>;

    /// Returns the all subscriber ids.
    fn get_all_subscriber_ids(&self) -> Vec<String>;

    // === Connection instance tracking ===

    /// The `add_connection_instance` method.
    fn add_connection_instance(&self, connection_id: &str, service_key: &str, instance_key: &str);

    /// The `remove_connection_instance` method.
    fn remove_connection_instance(
        &self,
        connection_id: &str,
        service_key: &str,
        instance_key: &str,
    );

    /// The `deregister_all_by_connection` method.
    fn deregister_all_by_connection(&self, connection_id: &str) -> Vec<String>;

    // === Cluster operations ===

    /// Sets the cluster config.
    fn set_cluster_config(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
        config: ClusterConfig,
    );

    /// Returns the cluster config.
    fn get_cluster_config(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
    ) -> Option<ClusterConfig>;

    /// Returns the all cluster configs.
    fn get_all_cluster_configs(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    ) -> Vec<ClusterConfig>;
    #[allow(clippy::too_many_arguments)]
    /// The `update_cluster_health_check` method.
    fn update_cluster_health_check(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
        health_check_type: &str,
        check_port: i32,
        use_instance_port: bool,
    );

    /// The `update_cluster_metadata` method.
    fn update_cluster_metadata(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
        metadata: HashMap<String, String>,
    );

    /// The `delete_cluster_config` method.
    fn delete_cluster_config(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
    );

    #[allow(clippy::too_many_arguments)]
    /// The `create_cluster_config` method.
    fn create_cluster_config(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
        health_check_type: &str,
        check_port: i32,
        use_instance_port: bool,
        metadata: HashMap<String, String>,
    ) -> Result<(), String>;

    /// Returns the cluster statistics.
    fn get_cluster_statistics(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    ) -> Vec<ClusterStatistics>;

    /// Returns the single cluster statistics.
    fn get_single_cluster_statistics(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
    ) -> Option<ClusterStatistics>;
}
