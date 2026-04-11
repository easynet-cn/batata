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

    fn register_instance(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instance: Instance,
    ) -> bool;

    fn deregister_instance(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instance: &Instance,
    ) -> bool;

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

    fn get_service(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster: &str,
        healthy_only: bool,
    ) -> Service;

    fn get_service_with_protection_info(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster: &str,
        healthy_only: bool,
    ) -> (Service, ProtectionInfo);

    fn list_services(
        &self,
        namespace: &str,
        group_name: &str,
        page_no: i32,
        page_size: i32,
    ) -> (i32, Vec<String>);

    fn service_exists(&self, namespace: &str, group_name: &str, service_name: &str) -> bool;

    fn get_all_service_keys(&self) -> Vec<String>;

    fn batch_register_instances(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instances: Vec<Instance>,
    ) -> bool;

    fn replace_ephemeral_instances(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instances: Vec<Instance>,
    ) -> bool;

    fn merge_remote_instances(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instances: Vec<Instance>,
    ) -> bool;

    fn batch_deregister_instances(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instances: Vec<Instance>,
    ) -> bool;

    fn heartbeat(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instance: Instance,
    ) -> bool;

    #[allow(clippy::too_many_arguments)]
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

    fn get_instance_count(&self) -> (usize, usize);

    fn get_healthy_instance_count(&self) -> (usize, usize);

    // === Metadata operations ===

    fn set_service_metadata(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        metadata: ServiceMetadata,
    );

    fn get_service_metadata(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    ) -> Option<ServiceMetadata>;

    fn update_service_protect_threshold(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        protect_threshold: f32,
    );

    fn update_service_selector(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        selector_type: &str,
        selector_expression: &str,
    );

    fn update_service_metadata_map(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        metadata: HashMap<String, String>,
    );

    fn delete_service_metadata(&self, namespace: &str, group_name: &str, service_name: &str);

    // === Subscription operations ===

    fn subscribe(&self, connection_id: &str, namespace: &str, group_name: &str, service_name: &str);

    fn unsubscribe(
        &self,
        connection_id: &str,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    );

    fn get_subscribers(&self, namespace: &str, group_name: &str, service_name: &str)
    -> Vec<String>;

    fn remove_subscriber(&self, connection_id: &str);

    fn add_publisher(
        &self,
        connection_id: &str,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    );

    fn remove_publisher(
        &self,
        connection_id: &str,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    );

    fn get_published_services(&self, connection_id: &str) -> Vec<String>;

    fn get_publishers(&self, namespace: &str, group_name: &str, service_name: &str) -> Vec<String>;

    fn get_subscribed_services(&self, connection_id: &str) -> Vec<String>;

    fn get_all_publisher_ids(&self) -> Vec<String>;

    fn get_all_subscriber_ids(&self) -> Vec<String>;

    // === Connection instance tracking ===

    fn add_connection_instance(&self, connection_id: &str, service_key: &str, instance_key: &str);

    fn remove_connection_instance(
        &self,
        connection_id: &str,
        service_key: &str,
        instance_key: &str,
    );

    fn deregister_all_by_connection(&self, connection_id: &str) -> Vec<String>;

    // === Cluster operations ===

    fn set_cluster_config(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
        config: ClusterConfig,
    );

    fn get_cluster_config(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
    ) -> Option<ClusterConfig>;

    fn get_all_cluster_configs(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    ) -> Vec<ClusterConfig>;
    #[allow(clippy::too_many_arguments)]
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

    fn update_cluster_metadata(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
        metadata: HashMap<String, String>,
    );

    fn delete_cluster_config(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
    );

    #[allow(clippy::too_many_arguments)]
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

    fn get_cluster_statistics(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    ) -> Vec<ClusterStatistics>;

    fn get_single_cluster_statistics(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
    ) -> Option<ClusterStatistics>;
}
