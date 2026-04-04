//! Implementation of `NamingServiceProvider` (from batata-api) for `NacosNamingServiceImpl`
//!
//! This adapter enables the new isolated implementation to be used by
//! existing handlers that depend on the legacy trait.

use std::collections::HashMap;

use batata_api::naming::{
    ClusterConfig as ApiClusterConfig, ClusterStatistics, Instance as ApiInstance,
    NamingServiceProvider, ProtectionInfo as ApiProtectionInfo, RegisterSource,
    Service as ApiService, ServiceMetadata as ApiServiceMetadata,
};

use crate::model::HealthCheckType;
use crate::service::NacosNamingServiceImpl;

use super::convert;

#[allow(clippy::too_many_arguments)]
impl NamingServiceProvider for NacosNamingServiceImpl {
    fn register_instance(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instance: ApiInstance,
    ) -> bool {
        self.register_instance(
            namespace,
            group_name,
            service_name,
            convert::from_api_instance(instance),
        )
    }

    fn deregister_instance(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instance: &ApiInstance,
    ) -> bool {
        self.deregister_instance(
            namespace,
            group_name,
            service_name,
            &instance.ip,
            instance.port,
            &instance.cluster_name,
            instance.ephemeral,
        )
    }

    fn get_instances(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster: &str,
        healthy_only: bool,
    ) -> Vec<ApiInstance> {
        self.get_instances(namespace, group_name, service_name, cluster, healthy_only)
            .into_iter()
            .map(|inst| convert::to_api_instance(&inst, RegisterSource::Batata))
            .collect()
    }

    fn get_service(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster: &str,
        healthy_only: bool,
    ) -> ApiService {
        let svc = self.get_service(namespace, group_name, service_name, cluster, healthy_only);
        convert::to_api_service(&svc, RegisterSource::Batata)
    }

    fn get_service_with_protection_info(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster: &str,
        healthy_only: bool,
    ) -> (ApiService, ApiProtectionInfo) {
        let (svc, info) = self.get_service_with_protection_info(
            namespace,
            group_name,
            service_name,
            cluster,
            healthy_only,
        );
        (
            convert::to_api_service(&svc, RegisterSource::Batata),
            convert::to_api_protection_info(&info),
        )
    }

    fn list_services(
        &self,
        namespace: &str,
        group_name: &str,
        page_no: i32,
        page_size: i32,
    ) -> (i32, Vec<String>) {
        let (total, names) =
            self.list_services(namespace, group_name, page_no as u32, page_size as u32);
        (total as i32, names)
    }

    fn service_exists(&self, namespace: &str, group_name: &str, service_name: &str) -> bool {
        self.service_exists(namespace, group_name, service_name)
    }

    fn get_all_service_keys(&self) -> Vec<String> {
        self.get_all_service_keys()
    }

    fn get_service_revision(&self, service_key: &str) -> i64 {
        self.get_revision(service_key) as i64
    }

    fn batch_register_instances(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instances: Vec<ApiInstance>,
    ) -> bool {
        let converted: Vec<_> = instances
            .into_iter()
            .map(convert::from_api_instance)
            .collect();
        self.batch_register_instances(namespace, group_name, service_name, converted)
    }

    fn replace_ephemeral_instances(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instances: Vec<ApiInstance>,
    ) -> bool {
        let converted: Vec<_> = instances
            .into_iter()
            .map(convert::from_api_instance)
            .collect();
        self.replace_ephemeral_instances(namespace, group_name, service_name, converted)
    }

    fn merge_remote_instances(
        &self,
        _namespace: &str,
        _group_name: &str,
        _service_name: &str,
        _instances: Vec<ApiInstance>,
    ) -> bool {
        // TODO: Implement merge_remote_instances for Distro protocol
        true
    }

    fn batch_deregister_instances(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instances: Vec<ApiInstance>,
    ) -> bool {
        let converted: Vec<_> = instances
            .into_iter()
            .map(convert::from_api_instance)
            .collect();
        self.batch_deregister_instances(namespace, group_name, service_name, converted)
    }

    fn heartbeat(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instance: ApiInstance,
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
        // TODO: implement global count
        (0, 0)
    }

    fn get_healthy_instance_count(&self) -> (usize, usize) {
        // TODO: implement healthy count
        (0, 0)
    }

    // === Metadata ===

    fn set_service_metadata(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        metadata: ApiServiceMetadata,
    ) {
        self.set_service_metadata(
            namespace,
            group_name,
            service_name,
            convert::from_api_service_metadata(metadata),
        );
    }

    fn get_service_metadata(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    ) -> Option<ApiServiceMetadata> {
        self.get_service_metadata(namespace, group_name, service_name)
            .map(|m| convert::to_api_service_metadata(&m, RegisterSource::Batata))
    }

    fn update_service_protect_threshold(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        protect_threshold: f32,
    ) {
        self.update_protect_threshold(namespace, group_name, service_name, protect_threshold);
    }

    fn update_service_selector(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        selector_type: &str,
        selector_expression: &str,
    ) {
        self.update_selector(
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
        self.update_metadata_map(namespace, group_name, service_name, metadata);
    }

    fn delete_service_metadata(&self, namespace: &str, group_name: &str, service_name: &str) {
        self.delete_service_metadata(namespace, group_name, service_name);
    }

    // === Subscription ===

    fn subscribe(
        &self,
        connection_id: &str,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    ) {
        self.subscribe(connection_id, namespace, group_name, service_name, "");
    }

    fn unsubscribe(
        &self,
        connection_id: &str,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    ) {
        self.unsubscribe(connection_id, namespace, group_name, service_name, "");
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

    fn get_publishers(
        &self,
        _namespace: &str,
        _group_name: &str,
        _service_name: &str,
    ) -> Vec<String> {
        // TODO: implement per-service publisher query
        Vec::new()
    }

    fn get_subscribed_services(&self, _connection_id: &str) -> Vec<String> {
        // TODO: implement subscribed services query
        Vec::new()
    }

    fn get_all_publisher_ids(&self) -> Vec<String> {
        // TODO: implement
        Vec::new()
    }

    fn get_all_subscriber_ids(&self) -> Vec<String> {
        // TODO: implement
        Vec::new()
    }

    // === Connection tracking ===

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

    // === Cluster ===

    fn set_cluster_config(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        _cluster_name: &str,
        config: ApiClusterConfig,
    ) {
        self.set_cluster_config(
            namespace,
            group_name,
            service_name,
            convert::from_api_cluster_config(config),
        );
    }

    fn get_cluster_config(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
    ) -> Option<ApiClusterConfig> {
        self.get_cluster_config(namespace, group_name, service_name, cluster_name)
            .map(|c| convert::to_api_cluster_config(&c))
    }

    fn get_all_cluster_configs(
        &self,
        _namespace: &str,
        _group_name: &str,
        _service_name: &str,
    ) -> Vec<ApiClusterConfig> {
        // TODO: implement
        Vec::new()
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
        let config = crate::model::ClusterConfig {
            name: cluster_name.to_string(),
            health_check_type: match health_check_type {
                "TCP" | "tcp" => HealthCheckType::Tcp,
                "HTTP" | "http" => HealthCheckType::Http,
                "MYSQL" | "mysql" => HealthCheckType::Mysql,
                _ => HealthCheckType::None,
            },
            health_check_port: check_port as u16,
            use_instance_port,
            metadata: HashMap::new(),
        };
        self.set_cluster_config(namespace, group_name, service_name, config);
    }

    fn update_cluster_metadata(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
        metadata: HashMap<String, String>,
    ) {
        if let Some(mut config) =
            self.get_cluster_config(namespace, group_name, service_name, cluster_name)
        {
            config.metadata = metadata;
            self.set_cluster_config(namespace, group_name, service_name, config);
        }
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
        let config = crate::model::ClusterConfig {
            name: cluster_name.to_string(),
            health_check_type: match health_check_type {
                "TCP" | "tcp" => HealthCheckType::Tcp,
                "HTTP" | "http" => HealthCheckType::Http,
                "MYSQL" | "mysql" => HealthCheckType::Mysql,
                _ => HealthCheckType::None,
            },
            health_check_port: check_port as u16,
            use_instance_port,
            metadata,
        };
        self.set_cluster_config(namespace, group_name, service_name, config);
        Ok(())
    }

    fn get_cluster_statistics(
        &self,
        _namespace: &str,
        _group_name: &str,
        _service_name: &str,
    ) -> Vec<ClusterStatistics> {
        // TODO: implement
        Vec::new()
    }

    fn get_single_cluster_statistics(
        &self,
        _namespace: &str,
        _group_name: &str,
        _service_name: &str,
        _cluster_name: &str,
    ) -> Option<ClusterStatistics> {
        // TODO: implement
        None
    }
}

// Implement ConnectionCleanupHandler for seamless gRPC integration
impl batata_core::handler::rpc::ConnectionCleanupHandler for NacosNamingServiceImpl {
    fn deregister_all_by_connection(&self, connection_id: &str) -> Vec<String> {
        self.deregister_all_by_connection(connection_id)
    }

    fn remove_subscriber(&self, connection_id: &str) {
        self.remove_subscriber(connection_id);
    }
}
