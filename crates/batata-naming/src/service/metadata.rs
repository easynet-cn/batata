//! Service metadata operations

use std::collections::HashMap;

use super::{NamingService, ServiceMetadata, build_service_key};

impl NamingService {
    /// Create or update service metadata
    pub fn set_service_metadata(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        metadata: ServiceMetadata,
    ) {
        let service_key = build_service_key(namespace, group_name, service_name);
        self.service_metadata.insert(service_key, metadata);
    }

    /// Get service metadata
    pub fn get_service_metadata(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    ) -> Option<ServiceMetadata> {
        let service_key = build_service_key(namespace, group_name, service_name);
        self.service_metadata.get(&service_key).map(|r| r.clone())
    }

    /// Update service protection threshold
    pub fn update_service_protect_threshold(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        protect_threshold: f32,
    ) {
        let service_key = build_service_key(namespace, group_name, service_name);
        self.service_metadata
            .entry(service_key)
            .or_default()
            .protect_threshold = protect_threshold;
    }

    /// Update service selector
    pub fn update_service_selector(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        selector_type: &str,
        selector_expression: &str,
    ) {
        let service_key = build_service_key(namespace, group_name, service_name);
        let mut entry = self.service_metadata.entry(service_key).or_default();
        entry.selector_type = selector_type.to_string();
        entry.selector_expression = selector_expression.to_string();
    }

    /// Update service metadata key-value pairs
    pub fn update_service_metadata_map(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        metadata: HashMap<String, String>,
    ) {
        let service_key = build_service_key(namespace, group_name, service_name);
        self.service_metadata
            .entry(service_key)
            .or_default()
            .metadata = metadata;
    }

    /// Delete service metadata
    pub fn delete_service_metadata(&self, namespace: &str, group_name: &str, service_name: &str) {
        let service_key = build_service_key(namespace, group_name, service_name);
        self.service_metadata.remove(&service_key);
    }
}
