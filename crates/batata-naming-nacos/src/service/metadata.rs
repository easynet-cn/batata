//! Service metadata operations

use std::collections::HashMap;

use crate::model::{ClusterConfig, ServiceMetadata};

use super::{NacosNamingServiceImpl, build_cluster_key, build_service_key};

impl NacosNamingServiceImpl {
    /// Set service metadata
    pub fn set_service_metadata(
        &self,
        namespace: &str,
        group: &str,
        service: &str,
        metadata: ServiceMetadata,
    ) {
        let service_key = build_service_key(namespace, group, service);
        self.service_metadata.insert(service_key, metadata);
    }

    /// Get service metadata
    pub fn get_service_metadata(
        &self,
        namespace: &str,
        group: &str,
        service: &str,
    ) -> Option<ServiceMetadata> {
        let service_key = build_service_key(namespace, group, service);
        self.service_metadata.get(&service_key).map(|r| r.clone())
    }

    /// Delete service metadata
    pub fn delete_service_metadata(&self, namespace: &str, group: &str, service: &str) {
        let service_key = build_service_key(namespace, group, service);
        self.service_metadata.remove(&service_key);
    }

    /// Update protection threshold
    pub fn update_protect_threshold(
        &self,
        namespace: &str,
        group: &str,
        service: &str,
        threshold: f32,
    ) {
        let service_key = build_service_key(namespace, group, service);
        self.service_metadata
            .entry(service_key)
            .or_default()
            .protect_threshold = threshold;
    }

    /// Update service selector
    pub fn update_selector(
        &self,
        namespace: &str,
        group: &str,
        service: &str,
        selector_type: &str,
        selector_expression: &str,
    ) {
        let service_key = build_service_key(namespace, group, service);
        let mut entry = self.service_metadata.entry(service_key).or_default();
        entry.selector_type = selector_type.to_string();
        entry.selector_expression = selector_expression.to_string();
    }

    /// Update service metadata map
    pub fn update_metadata_map(
        &self,
        namespace: &str,
        group: &str,
        service: &str,
        metadata: HashMap<String, String>,
    ) {
        let service_key = build_service_key(namespace, group, service);
        self.service_metadata
            .entry(service_key)
            .or_default()
            .metadata = metadata;
    }

    // ========================================================================
    // Cluster Configuration
    // ========================================================================

    /// Set cluster config
    pub fn set_cluster_config(
        &self,
        namespace: &str,
        group: &str,
        service: &str,
        config: ClusterConfig,
    ) {
        let service_key = build_service_key(namespace, group, service);
        let cluster_key = build_cluster_key(&service_key, &config.name);
        self.cluster_configs.insert(cluster_key, config);
    }

    /// Get cluster config
    pub fn get_cluster_config(
        &self,
        namespace: &str,
        group: &str,
        service: &str,
        cluster: &str,
    ) -> Option<ClusterConfig> {
        let service_key = build_service_key(namespace, group, service);
        let cluster_key = build_cluster_key(&service_key, cluster);
        self.cluster_configs.get(&cluster_key).map(|r| r.clone())
    }

    /// Delete cluster config
    pub fn delete_cluster_config(
        &self,
        namespace: &str,
        group: &str,
        service: &str,
        cluster: &str,
    ) {
        let service_key = build_service_key(namespace, group, service);
        let cluster_key = build_cluster_key(&service_key, cluster);
        self.cluster_configs.remove(&cluster_key);
    }
}
