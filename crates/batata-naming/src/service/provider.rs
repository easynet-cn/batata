//! NamingServiceProvider trait implementation for NamingService
//!
//! Delegates all trait methods to the existing NamingService methods,
//! adapting signatures where necessary.

use std::collections::HashMap;

use batata_api::naming::{
    ClusterConfig, ClusterStatistics, Instance, NamingServiceProvider, ProtectionInfo, Service,
    ServiceMetadata,
};

use super::NamingService;

impl NamingServiceProvider for NamingService {
    // === Instance operations ===

    fn register_instance(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instance: Instance,
    ) -> bool {
        self.register_instance(namespace, group_name, service_name, instance)
    }

    fn deregister_instance(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instance: &Instance,
    ) -> bool {
        self.deregister_instance(namespace, group_name, service_name, instance)
    }

    fn get_instances(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster: &str,
        healthy_only: bool,
    ) -> Vec<Instance> {
        self.get_instances(namespace, group_name, service_name, cluster, healthy_only)
    }

    fn get_service(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster: &str,
        healthy_only: bool,
    ) -> Service {
        self.get_service(namespace, group_name, service_name, cluster, healthy_only)
    }

    fn get_service_with_protection_info(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster: &str,
        healthy_only: bool,
    ) -> (Service, ProtectionInfo) {
        self.get_service_with_protection_info(
            namespace,
            group_name,
            service_name,
            cluster,
            healthy_only,
        )
    }

    fn list_services(
        &self,
        namespace: &str,
        group_name: &str,
        page_no: i32,
        page_size: i32,
    ) -> (i32, Vec<String>) {
        self.list_services(namespace, group_name, page_no, page_size)
    }

    fn service_exists(&self, namespace: &str, group_name: &str, service_name: &str) -> bool {
        self.service_exists(namespace, group_name, service_name)
    }

    fn get_all_service_keys(&self) -> Vec<String> {
        self.get_all_service_keys()
    }

    fn batch_register_instances(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instances: Vec<Instance>,
    ) -> bool {
        self.batch_register_instances(namespace, group_name, service_name, instances)
    }

    fn replace_ephemeral_instances(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instances: Vec<Instance>,
    ) -> bool {
        self.replace_ephemeral_instances(namespace, group_name, service_name, instances)
    }

    fn merge_remote_instances(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instances: Vec<Instance>,
    ) -> bool {
        self.merge_remote_instances(namespace, group_name, service_name, instances);
        true
    }

    fn batch_deregister_instances(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instances: Vec<Instance>,
    ) -> bool {
        self.batch_deregister_instances(namespace, group_name, service_name, &instances)
    }

    fn heartbeat(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instance: Instance,
    ) -> bool {
        self.heartbeat(
            namespace,
            group_name,
            service_name,
            &instance.ip,
            instance.port,
            &instance.cluster_name,
        )
    }

    fn update_instance_health(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        ip: &str,
        port: i32,
        cluster_name: &str,
        healthy: bool,
    ) -> bool {
        self.update_instance_health(
            namespace,
            group_name,
            service_name,
            ip,
            port,
            cluster_name,
            healthy,
        )
    }

    fn get_instance_count(&self) -> (usize, usize) {
        // Aggregate across all services: (total_instances, total_services)
        let mut total_instances = 0usize;
        let total_services = self.get_all_service_keys().len();
        for key in self.get_all_service_keys() {
            if let Some((ns, group, svc)) = super::parse_service_key(&key) {
                total_instances += NamingService::get_instance_count(self, ns, group, svc);
            }
        }
        (total_instances, total_services)
    }

    fn get_healthy_instance_count(&self) -> (usize, usize) {
        // Aggregate across all services: (healthy_instances, total_instances)
        let mut total_healthy = 0usize;
        let mut total_instances = 0usize;
        for key in self.get_all_service_keys() {
            if let Some((ns, group, svc)) = super::parse_service_key(&key) {
                total_healthy += NamingService::get_healthy_instance_count(self, ns, group, svc);
                total_instances += NamingService::get_instance_count(self, ns, group, svc);
            }
        }
        (total_healthy, total_instances)
    }

    // === Metadata operations ===

    fn set_service_metadata(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        metadata: ServiceMetadata,
    ) {
        self.set_service_metadata(namespace, group_name, service_name, metadata);
    }

    fn get_service_metadata(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    ) -> Option<ServiceMetadata> {
        self.get_service_metadata(namespace, group_name, service_name)
    }

    fn update_service_protect_threshold(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        protect_threshold: f32,
    ) {
        self.update_service_protect_threshold(
            namespace,
            group_name,
            service_name,
            protect_threshold,
        );
    }

    fn update_service_selector(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        selector_type: &str,
        selector_expression: &str,
    ) {
        self.update_service_selector(
            namespace,
            group_name,
            service_name,
            selector_type,
            selector_expression,
        );
    }

    fn update_service_metadata_map(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        metadata: HashMap<String, String>,
    ) {
        self.update_service_metadata_map(namespace, group_name, service_name, metadata);
    }

    fn delete_service_metadata(&self, namespace: &str, group_name: &str, service_name: &str) {
        self.delete_service_metadata(namespace, group_name, service_name);
    }

    // === Subscription operations ===

    fn subscribe(
        &self,
        connection_id: &str,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    ) {
        self.subscribe(connection_id, namespace, group_name, service_name);
    }

    fn unsubscribe(
        &self,
        connection_id: &str,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    ) {
        self.unsubscribe(connection_id, namespace, group_name, service_name);
    }

    fn get_subscribers(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    ) -> Vec<String> {
        self.get_subscribers(namespace, group_name, service_name)
    }

    fn remove_subscriber(&self, connection_id: &str) {
        self.remove_subscriber(connection_id);
    }

    fn add_publisher(
        &self,
        connection_id: &str,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    ) {
        self.add_publisher(connection_id, namespace, group_name, service_name);
    }

    fn remove_publisher(
        &self,
        connection_id: &str,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    ) {
        self.remove_publisher(connection_id, namespace, group_name, service_name);
    }

    fn get_published_services(&self, connection_id: &str) -> Vec<String> {
        self.get_published_services(connection_id)
    }

    fn get_publishers(&self, namespace: &str, group_name: &str, service_name: &str) -> Vec<String> {
        self.get_publishers(namespace, group_name, service_name)
    }

    fn get_subscribed_services(&self, connection_id: &str) -> Vec<String> {
        self.get_subscribed_services(connection_id)
    }

    fn get_all_publisher_ids(&self) -> Vec<String> {
        self.get_all_publisher_ids()
    }

    fn get_all_subscriber_ids(&self) -> Vec<String> {
        self.get_all_subscriber_ids()
    }

    // === Connection instance tracking ===

    fn add_connection_instance(&self, connection_id: &str, service_key: &str, instance_key: &str) {
        self.add_connection_instance(connection_id, service_key, instance_key);
    }

    fn remove_connection_instance(
        &self,
        connection_id: &str,
        service_key: &str,
        instance_key: &str,
    ) {
        self.remove_connection_instance(connection_id, service_key, instance_key);
    }

    fn deregister_all_by_connection(&self, connection_id: &str) -> Vec<String> {
        self.deregister_all_by_connection(connection_id)
    }

    // === Cluster operations ===

    fn set_cluster_config(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
        config: ClusterConfig,
    ) {
        self.set_cluster_config(namespace, group_name, service_name, cluster_name, config);
    }

    fn get_cluster_config(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
    ) -> Option<ClusterConfig> {
        self.get_cluster_config(namespace, group_name, service_name, cluster_name)
    }

    fn get_all_cluster_configs(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    ) -> Vec<ClusterConfig> {
        self.get_all_cluster_configs(namespace, group_name, service_name)
    }

    fn update_cluster_health_check(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
        health_check_type: &str,
        check_port: i32,
        use_instance_port: bool,
    ) {
        self.update_cluster_health_check(
            namespace,
            group_name,
            service_name,
            cluster_name,
            health_check_type,
            check_port,
            use_instance_port,
        );
    }

    fn update_cluster_metadata(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
        metadata: HashMap<String, String>,
    ) {
        self.update_cluster_metadata(namespace, group_name, service_name, cluster_name, metadata);
    }

    fn delete_cluster_config(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
    ) {
        self.delete_cluster_config(namespace, group_name, service_name, cluster_name);
    }

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
    ) -> Result<(), String> {
        self.create_cluster_config(
            namespace,
            group_name,
            service_name,
            cluster_name,
            health_check_type,
            check_port,
            use_instance_port,
            metadata,
        )
    }

    fn get_cluster_statistics(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    ) -> Vec<ClusterStatistics> {
        self.get_cluster_statistics(namespace, group_name, service_name)
    }

    fn get_single_cluster_statistics(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
    ) -> Option<ClusterStatistics> {
        self.get_single_cluster_statistics(namespace, group_name, service_name, cluster_name)
    }
}
