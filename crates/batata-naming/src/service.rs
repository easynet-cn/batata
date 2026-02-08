//! Naming service layer for service discovery operations
//!
//! This module provides in-memory service registry for managing service instances

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use dashmap::DashMap;
use serde::{Deserialize, Serialize};

use crate::model::{Instance, Service};

/// Service-level metadata stored separately from instances
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ServiceMetadata {
    /// Protection threshold (0.0 to 1.0)
    pub protect_threshold: f32,
    /// Service metadata key-value pairs
    pub metadata: HashMap<String, String>,
    /// Service selector type (e.g., "none", "label")
    pub selector_type: String,
    /// Service selector expression
    pub selector_expression: String,
}

/// Protection threshold information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProtectionInfo {
    /// Configured protection threshold (0.0 to 1.0)
    pub threshold: f32,
    /// Total number of instances
    pub total_instances: usize,
    /// Number of healthy instances
    pub healthy_instances: usize,
    /// Current healthy ratio (healthy_instances / total_instances)
    pub healthy_ratio: f32,
    /// Whether protection threshold was triggered
    pub triggered: bool,
}

impl ProtectionInfo {
    /// Check if the service is degraded (protection triggered)
    pub fn is_degraded(&self) -> bool {
        self.triggered
    }

    /// Get the number of unhealthy instances
    pub fn unhealthy_instances(&self) -> usize {
        self.total_instances.saturating_sub(self.healthy_instances)
    }
}

/// Cluster-level configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClusterConfig {
    /// Cluster name
    pub name: String,
    /// Health checker type (e.g., "TCP", "HTTP", "NONE")
    pub health_check_type: String,
    /// Health check port
    pub check_port: i32,
    /// Use instance port for health check
    pub use_instance_port: bool,
    /// Cluster metadata
    pub metadata: HashMap<String, String>,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            name: "DEFAULT".to_string(),
            health_check_type: "TCP".to_string(),
            check_port: 80,
            use_instance_port: true,
            metadata: HashMap::new(),
        }
    }
}

/// Build service key format: namespace@@groupName@@serviceName
fn build_service_key(namespace: &str, group_name: &str, service_name: &str) -> String {
    format!("{}@@{}@@{}", namespace, group_name, service_name)
}

/// Build instance key format: ip#port#clusterName
fn build_instance_key(instance: &Instance) -> String {
    format!(
        "{}#{}#{}",
        instance.ip, instance.port, instance.cluster_name
    )
}

/// Fuzzy watch pattern for service discovery
#[derive(Clone, Debug)]
pub struct FuzzyWatchPattern {
    pub namespace: String,
    pub group_pattern: String,
    pub service_pattern: String,
    pub watch_type: String,
}

impl FuzzyWatchPattern {
    /// Check if a service key matches this pattern
    pub fn matches(&self, namespace: &str, group_name: &str, service_name: &str) -> bool {
        if self.namespace != namespace {
            return false;
        }

        // Convert glob pattern to regex
        let group_matches = Self::glob_match(&self.group_pattern, group_name);
        let service_matches = Self::glob_match(&self.service_pattern, service_name);

        group_matches && service_matches
    }

    /// Simple glob pattern matching (* matches any sequence)
    fn glob_match(pattern: &str, text: &str) -> bool {
        if pattern.is_empty() || pattern == "*" {
            return true;
        }

        // Use cached regex matching from batata-common
        batata_common::glob_matches(pattern, text)
    }
}

/// In-memory service registry for managing services and instances
#[derive(Clone)]
pub struct NamingService {
    /// Key: service_key (namespace@@group@@service), Value: map of instances
    services: Arc<DashMap<String, DashMap<String, Instance>>>,
    /// Key: service_key, Value: service-level metadata
    service_metadata: Arc<DashMap<String, ServiceMetadata>>,
    /// Key: service_key##cluster_name, Value: cluster configuration
    cluster_configs: Arc<DashMap<String, ClusterConfig>>,
    /// Key: connection_id, Value: set of subscribed service keys (HashSet for O(1) lookups)
    subscribers: Arc<DashMap<String, HashSet<String>>>,
    /// Key: connection_id, Value: list of fuzzy watch patterns
    fuzzy_watchers: Arc<DashMap<String, Vec<FuzzyWatchPattern>>>,
    /// Key: connection_id, Value: set of published service keys (for V2 Client API)
    publishers: Arc<DashMap<String, HashSet<String>>>,
}

/// Build cluster config key format: service_key##clusterName
fn build_cluster_key(service_key: &str, cluster_name: &str) -> String {
    format!("{}##{}", service_key, cluster_name)
}

impl NamingService {
    pub fn new() -> Self {
        Self {
            services: Arc::new(DashMap::new()),
            service_metadata: Arc::new(DashMap::new()),
            cluster_configs: Arc::new(DashMap::new()),
            subscribers: Arc::new(DashMap::new()),
            fuzzy_watchers: Arc::new(DashMap::new()),
            publishers: Arc::new(DashMap::new()),
        }
    }

    /// Register a service instance
    pub fn register_instance(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        mut instance: Instance,
    ) -> bool {
        let service_key = build_service_key(namespace, group_name, service_name);

        // Normalize defaults BEFORE building instance key for consistent storage
        if instance.weight < 0.0 {
            instance.weight = 1.0;
        }
        if instance.cluster_name.is_empty() {
            instance.cluster_name = "DEFAULT".to_string();
        }
        instance.service_name = service_name.to_string();

        let instance_key = build_instance_key(&instance);

        // Get or create service entry
        let instances = self.services.entry(service_key).or_default();
        instances.insert(instance_key, instance);
        true
    }

    /// Deregister a service instance
    pub fn deregister_instance(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instance: &Instance,
    ) -> bool {
        let service_key = build_service_key(namespace, group_name, service_name);

        // If cluster_name is empty, try "DEFAULT" (the default cluster name)
        let instance_key = if instance.cluster_name.is_empty() {
            format!("{}#{}#DEFAULT", instance.ip, instance.port)
        } else {
            build_instance_key(instance)
        };

        if let Some(instances) = self.services.get(&service_key) {
            instances.remove(&instance_key);
            return true;
        }
        false
    }

    /// Get all instances for a service
    pub fn get_instances(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster: &str,
        healthy_only: bool,
    ) -> Vec<Instance> {
        let service_key = build_service_key(namespace, group_name, service_name);

        if let Some(instances) = self.services.get(&service_key) {
            instances
                .iter()
                .filter(|entry| {
                    let inst = entry.value();
                    // Filter by cluster if specified (supports comma-separated list)
                    let cluster_match = cluster.is_empty()
                        || cluster == "*"
                        || cluster.split(',').any(|c| c.trim() == inst.cluster_name);
                    // Filter by health if required
                    let health_match = !healthy_only || inst.healthy;
                    cluster_match && health_match
                })
                .map(|entry| entry.value().clone())
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get service info with instances
    ///
    /// This method implements protection threshold logic:
    /// - If healthy_ratio < protect_threshold, return ALL instances (including unhealthy)
    /// - This prevents cascading failures when too many instances become unhealthy
    pub fn get_service(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster: &str,
        healthy_only: bool,
    ) -> Service {
        let service_key = build_service_key(namespace, group_name, service_name);

        // Get protection threshold from service metadata
        let protect_threshold = self
            .service_metadata
            .get(&service_key)
            .map(|m| m.protect_threshold)
            .unwrap_or(0.0);

        let (instances, has_any_instances, reach_protection) = if let Some(all_instances) =
            self.services.get(&service_key)
        {
            let has_any = !all_instances.is_empty();

            // First filter by cluster
            let cluster_filtered: Vec<Instance> = all_instances
                .iter()
                .filter(|entry| {
                    let inst = entry.value();
                    cluster.is_empty()
                        || cluster == "*"
                        || cluster.split(',').any(|c| c.trim() == inst.cluster_name)
                })
                .map(|entry| entry.value().clone())
                .collect();

            let total = cluster_filtered.len();
            let healthy_count = cluster_filtered.iter().filter(|i| i.healthy).count();

            // Check if protection threshold is reached
            let healthy_ratio = if total > 0 {
                healthy_count as f32 / total as f32
            } else {
                1.0 // No instances means 100% healthy ratio
            };

            let protection_triggered = protect_threshold > 0.0 && healthy_ratio < protect_threshold;

            // If protection is triggered, return all instances regardless of health
            // Otherwise, filter by health if requested
            let final_instances = if protection_triggered {
                // Return all instances including unhealthy to prevent total service outage
                cluster_filtered
            } else if healthy_only {
                cluster_filtered.into_iter().filter(|i| i.healthy).collect()
            } else {
                cluster_filtered
            };

            (final_instances, has_any, protection_triggered)
        } else {
            (Vec::new(), false, false)
        };

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        Service {
            name: service_name.to_string(),
            group_name: group_name.to_string(),
            clusters: cluster.to_string(),
            cache_millis: 10000,
            hosts: instances,
            last_ref_time: now,
            checksum: String::new(),
            all_ips: has_any_instances,
            reach_protection_threshold: reach_protection,
        }
    }

    /// Get service info with protection threshold check
    ///
    /// Returns additional information about protection threshold status
    pub fn get_service_with_protection_info(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster: &str,
        healthy_only: bool,
    ) -> (Service, ProtectionInfo) {
        let service_key = build_service_key(namespace, group_name, service_name);

        // Get protection threshold from service metadata
        let protect_threshold = self
            .service_metadata
            .get(&service_key)
            .map(|m| m.protect_threshold)
            .unwrap_or(0.0);

        let mut protection_info = ProtectionInfo {
            threshold: protect_threshold,
            total_instances: 0,
            healthy_instances: 0,
            healthy_ratio: 1.0,
            triggered: false,
        };

        let service = self.get_service(namespace, group_name, service_name, cluster, healthy_only);

        // Calculate protection info by counting directly (avoids collecting references)
        if let Some(all_instances) = self.services.get(&service_key) {
            let total: usize = all_instances
                .iter()
                .filter(|entry| {
                    let inst = entry.value();
                    cluster.is_empty()
                        || cluster == "*"
                        || cluster.split(',').any(|c| c.trim() == inst.cluster_name)
                })
                .count();

            let healthy: usize = all_instances
                .iter()
                .filter(|entry| {
                    let inst = entry.value();
                    let cluster_match = cluster.is_empty()
                        || cluster == "*"
                        || cluster.split(',').any(|c| c.trim() == inst.cluster_name);
                    cluster_match && inst.healthy
                })
                .count();

            protection_info.total_instances = total;
            protection_info.healthy_instances = healthy;
            protection_info.healthy_ratio = if total > 0 {
                healthy as f32 / total as f32
            } else {
                1.0
            };
            protection_info.triggered = service.reach_protection_threshold;
        }

        (service, protection_info)
    }

    /// Get all service keys (for distro protocol sync)
    /// Returns keys in format: namespace@@group@@serviceName
    pub fn get_all_service_keys(&self) -> Vec<String> {
        self.services.iter().map(|e| e.key().clone()).collect()
    }

    /// List all services in a namespace
    /// Uses iterator-based pagination to avoid allocating intermediate Vec for large service counts
    pub fn list_services(
        &self,
        namespace: &str,
        group_name: &str,
        page_no: i32,
        page_size: i32,
    ) -> (i32, Vec<String>) {
        let prefix = if group_name.is_empty() {
            format!("{}@@", namespace)
        } else {
            format!("{}@@{}@@", namespace, group_name)
        };

        // Extract service name from key (third part after "@@")
        fn extract_service_name(key: &str) -> String {
            key.split("@@")
                .nth(2)
                .map(|s| s.to_string())
                .unwrap_or_else(|| key.to_string())
        }

        // Count total matching services (single pass)
        let total = self
            .services
            .iter()
            .filter(|entry| entry.key().starts_with(&prefix))
            .count() as i32;

        // Paginate directly on iterator to avoid intermediate allocation
        let start = ((page_no - 1) * page_size) as usize;
        let paginated: Vec<String> = self
            .services
            .iter()
            .filter(|entry| entry.key().starts_with(&prefix))
            .skip(start)
            .take(page_size as usize)
            .map(|entry| extract_service_name(entry.key()))
            .collect();

        (total, paginated)
    }

    /// Subscribe to a service
    pub fn subscribe(
        &self,
        connection_id: &str,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    ) {
        let service_key = build_service_key(namespace, group_name, service_name);

        self.subscribers
            .entry(connection_id.to_string())
            .or_default()
            .insert(service_key);
    }

    /// Unsubscribe from a service
    pub fn unsubscribe(
        &self,
        connection_id: &str,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    ) {
        let service_key = build_service_key(namespace, group_name, service_name);

        if let Some(mut subs) = self.subscribers.get_mut(connection_id) {
            subs.remove(&service_key);
        }
    }

    /// Get subscribers for a service
    pub fn get_subscribers(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    ) -> Vec<String> {
        let service_key = build_service_key(namespace, group_name, service_name);

        self.subscribers
            .iter()
            .filter(|entry| entry.value().contains(&service_key))
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Clean up subscriber when connection is closed
    pub fn remove_subscriber(&self, connection_id: &str) {
        self.subscribers.remove(connection_id);
        self.fuzzy_watchers.remove(connection_id);
        self.publishers.remove(connection_id);
    }

    // ============== Publisher Tracking (for V2 Client API) ==============

    /// Track that a connection published a service
    pub fn add_publisher(
        &self,
        connection_id: &str,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    ) {
        let service_key = build_service_key(namespace, group_name, service_name);
        self.publishers
            .entry(connection_id.to_string())
            .or_default()
            .insert(service_key);
    }

    /// Remove publisher tracking for a service
    pub fn remove_publisher(
        &self,
        connection_id: &str,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    ) {
        let service_key = build_service_key(namespace, group_name, service_name);
        if let Some(mut pubs) = self.publishers.get_mut(connection_id) {
            pubs.remove(&service_key);
        }
    }

    /// Get all services published by a connection
    pub fn get_published_services(&self, connection_id: &str) -> Vec<String> {
        self.publishers
            .get(connection_id)
            .map(|pubs| pubs.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Get all connections that published a specific service
    pub fn get_publishers(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    ) -> Vec<String> {
        let service_key = build_service_key(namespace, group_name, service_name);
        self.publishers
            .iter()
            .filter(|entry| entry.value().contains(&service_key))
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Get all services subscribed by a connection
    pub fn get_subscribed_services(&self, connection_id: &str) -> Vec<String> {
        self.subscribers
            .get(connection_id)
            .map(|subs| subs.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Get all connection IDs that are publishers
    pub fn get_all_publisher_ids(&self) -> Vec<String> {
        self.publishers.iter().map(|e| e.key().clone()).collect()
    }

    /// Get all connection IDs that are subscribers
    pub fn get_all_subscriber_ids(&self) -> Vec<String> {
        self.subscribers.iter().map(|e| e.key().clone()).collect()
    }

    /// Register a fuzzy watch pattern for a connection
    pub fn register_fuzzy_watch(
        &self,
        connection_id: &str,
        namespace: &str,
        group_pattern: &str,
        service_pattern: &str,
        watch_type: &str,
    ) {
        let pattern = FuzzyWatchPattern {
            namespace: namespace.to_string(),
            group_pattern: group_pattern.to_string(),
            service_pattern: service_pattern.to_string(),
            watch_type: watch_type.to_string(),
        };

        self.fuzzy_watchers
            .entry(connection_id.to_string())
            .or_default()
            .push(pattern);
    }

    /// Unregister fuzzy watch patterns for a connection
    pub fn unregister_fuzzy_watch(&self, connection_id: &str) {
        self.fuzzy_watchers.remove(connection_id);
    }

    /// Get all services matching a fuzzy watch pattern
    pub fn get_services_by_pattern(
        &self,
        namespace: &str,
        group_pattern: &str,
        service_pattern: &str,
    ) -> Vec<String> {
        let pattern = FuzzyWatchPattern {
            namespace: namespace.to_string(),
            group_pattern: group_pattern.to_string(),
            service_pattern: service_pattern.to_string(),
            watch_type: String::new(),
        };

        self.services
            .iter()
            .filter_map(|entry| {
                let key = entry.key();
                let parts: Vec<&str> = key.split("@@").collect();
                if parts.len() == 3 {
                    let (ns, group, service) = (parts[0], parts[1], parts[2]);
                    if pattern.matches(ns, group, service) {
                        return Some(key.clone());
                    }
                }
                None
            })
            .collect()
    }

    /// Get all service keys that a connection is fuzzy-watching
    pub fn get_fuzzy_watched_services(&self, connection_id: &str) -> Vec<String> {
        let Some(patterns) = self.fuzzy_watchers.get(connection_id) else {
            return vec![];
        };

        let mut matched_services = HashSet::new();

        for pattern in patterns.iter() {
            for entry in self.services.iter() {
                let key = entry.key();
                let parts: Vec<&str> = key.split("@@").collect();
                if parts.len() == 3 {
                    let (ns, group, service) = (parts[0], parts[1], parts[2]);
                    if pattern.matches(ns, group, service) {
                        matched_services.insert(key.clone());
                    }
                }
            }
        }

        matched_services.into_iter().collect()
    }

    /// Get connections watching a specific service through fuzzy patterns
    pub fn get_fuzzy_watchers_for_service(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    ) -> Vec<String> {
        self.fuzzy_watchers
            .iter()
            .filter(|entry| {
                entry
                    .value()
                    .iter()
                    .any(|pattern| pattern.matches(namespace, group_name, service_name))
            })
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Batch register instances (reconciliation model)
    ///
    /// Replaces ALL instances for the given service with the provided list.
    /// This follows the Nacos reconciliation model where batch register
    /// represents the full desired state, not an incremental addition.
    pub fn batch_register_instances(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instances: Vec<Instance>,
    ) -> bool {
        let service_key = build_service_key(namespace, group_name, service_name);

        // Clear existing instances for this service, then register the new ones
        let entry = self.services.entry(service_key).or_default();
        entry.clear();

        for instance in instances {
            let mut instance = instance;
            if instance.weight < 0.0 {
                instance.weight = 1.0;
            }
            if instance.cluster_name.is_empty() {
                instance.cluster_name = "DEFAULT".to_string();
            }
            instance.service_name = service_name.to_string();
            let instance_key = build_instance_key(&instance);
            entry.insert(instance_key, instance);
        }
        true
    }

    /// Batch deregister instances
    pub fn batch_deregister_instances(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instances: &[Instance],
    ) -> bool {
        for instance in instances {
            self.deregister_instance(namespace, group_name, service_name, instance);
        }
        true
    }

    /// Update instance heartbeat
    pub fn heartbeat(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        ip: &str,
        port: i32,
        cluster_name: &str,
    ) -> bool {
        let service_key = build_service_key(namespace, group_name, service_name);
        let instance_key = format!("{}#{}#{}", ip, port, cluster_name);

        if let Some(instances) = self.services.get(&service_key)
            && let Some(mut entry) = instances.get_mut(&instance_key)
        {
            entry.healthy = true;
            return true;
        }
        false
    }

    /// Update instance health status (for V2 Health API)
    pub fn update_instance_health(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        ip: &str,
        port: i32,
        cluster_name: &str,
        healthy: bool,
    ) -> bool {
        let service_key = build_service_key(namespace, group_name, service_name);
        let instance_key = format!("{}#{}#{}", ip, port, cluster_name);

        if let Some(instances) = self.services.get(&service_key)
            && let Some(mut entry) = instances.get_mut(&instance_key)
        {
            entry.healthy = healthy;
            return true;
        }
        false
    }

    /// Get instance count for a service
    pub fn get_instance_count(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    ) -> usize {
        let service_key = build_service_key(namespace, group_name, service_name);

        self.services
            .get(&service_key)
            .map(|instances| instances.len())
            .unwrap_or(0)
    }

    /// Get healthy instance count for a service
    pub fn get_healthy_instance_count(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    ) -> usize {
        let service_key = build_service_key(namespace, group_name, service_name);

        self.services
            .get(&service_key)
            .map(|instances| instances.iter().filter(|e| e.value().healthy).count())
            .unwrap_or(0)
    }

    // ============== Service Metadata Operations ==============

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

    // ============== Cluster Config Operations ==============

    /// Set cluster configuration
    pub fn set_cluster_config(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
        config: ClusterConfig,
    ) {
        let service_key = build_service_key(namespace, group_name, service_name);
        let cluster_key = build_cluster_key(&service_key, cluster_name);
        self.cluster_configs.insert(cluster_key, config);
    }

    /// Get cluster configuration
    pub fn get_cluster_config(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
    ) -> Option<ClusterConfig> {
        let service_key = build_service_key(namespace, group_name, service_name);
        let cluster_key = build_cluster_key(&service_key, cluster_name);
        self.cluster_configs.get(&cluster_key).map(|r| r.clone())
    }

    /// Get all cluster configs for a service
    pub fn get_all_cluster_configs(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    ) -> Vec<ClusterConfig> {
        let service_key = build_service_key(namespace, group_name, service_name);
        let prefix = format!("{}##", service_key);

        self.cluster_configs
            .iter()
            .filter(|entry| entry.key().starts_with(&prefix))
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Update cluster health check configuration
    pub fn update_cluster_health_check(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
        health_check_type: &str,
        check_port: i32,
        use_instance_port: bool,
    ) {
        let service_key = build_service_key(namespace, group_name, service_name);
        let cluster_key = build_cluster_key(&service_key, cluster_name);

        let mut entry = self
            .cluster_configs
            .entry(cluster_key)
            .or_insert_with(|| ClusterConfig {
                name: cluster_name.to_string(),
                ..Default::default()
            });
        entry.health_check_type = health_check_type.to_string();
        entry.check_port = check_port;
        entry.use_instance_port = use_instance_port;
    }

    /// Update cluster metadata
    pub fn update_cluster_metadata(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
        metadata: HashMap<String, String>,
    ) {
        let service_key = build_service_key(namespace, group_name, service_name);
        let cluster_key = build_cluster_key(&service_key, cluster_name);

        self.cluster_configs
            .entry(cluster_key)
            .or_insert_with(|| ClusterConfig {
                name: cluster_name.to_string(),
                ..Default::default()
            })
            .metadata = metadata;
    }

    /// Delete cluster configuration
    pub fn delete_cluster_config(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
    ) {
        let service_key = build_service_key(namespace, group_name, service_name);
        let cluster_key = build_cluster_key(&service_key, cluster_name);
        self.cluster_configs.remove(&cluster_key);
    }

    /// Check if a service exists (has metadata or instances)
    pub fn service_exists(&self, namespace: &str, group_name: &str, service_name: &str) -> bool {
        let service_key = build_service_key(namespace, group_name, service_name);
        self.service_metadata.contains_key(&service_key)
            || self
                .services
                .get(&service_key)
                .is_some_and(|s| !s.is_empty())
    }
}

impl Default for NamingService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_instance(ip: &str, port: i32) -> Instance {
        Instance {
            instance_id: format!("{}#{}#DEFAULT", ip, port),
            ip: ip.to_string(),
            port,
            weight: 1.0,
            healthy: true,
            enabled: true,
            ephemeral: true,
            cluster_name: "DEFAULT".to_string(),
            service_name: "test-service".to_string(),
            metadata: std::collections::HashMap::new(),
            instance_heart_beat_interval: 5000,
            instance_heart_beat_time_out: 15000,
            ip_delete_timeout: 30000,
            instance_id_generator: String::new(),
        }
    }

    #[test]
    fn test_build_service_key() {
        let key = build_service_key("public", "DEFAULT_GROUP", "test-service");
        assert_eq!(key, "public@@DEFAULT_GROUP@@test-service");
    }

    #[test]
    fn test_build_instance_key() {
        let instance = create_test_instance("127.0.0.1", 8080);
        let key = build_instance_key(&instance);
        assert_eq!(key, "127.0.0.1#8080#DEFAULT");
    }

    #[test]
    fn test_register_instance() {
        let naming = NamingService::new();
        let instance = create_test_instance("127.0.0.1", 8080);

        let result = naming.register_instance("public", "DEFAULT_GROUP", "test-service", instance);
        assert!(result);

        let instances = naming.get_instances("public", "DEFAULT_GROUP", "test-service", "", false);
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].ip, "127.0.0.1");
    }

    #[test]
    fn test_deregister_instance() {
        let naming = NamingService::new();
        let instance = create_test_instance("127.0.0.1", 8080);

        naming.register_instance("public", "DEFAULT_GROUP", "test-service", instance.clone());

        let result =
            naming.deregister_instance("public", "DEFAULT_GROUP", "test-service", &instance);
        assert!(result);

        let instances = naming.get_instances("public", "DEFAULT_GROUP", "test-service", "", false);
        assert!(instances.is_empty());
    }

    #[test]
    fn test_get_instances_healthy_only() {
        let naming = NamingService::new();

        let healthy_instance = create_test_instance("127.0.0.1", 8080);
        let mut unhealthy_instance = create_test_instance("127.0.0.2", 8081);
        unhealthy_instance.healthy = false;

        naming.register_instance("public", "DEFAULT_GROUP", "test-service", healthy_instance);
        naming.register_instance(
            "public",
            "DEFAULT_GROUP",
            "test-service",
            unhealthy_instance,
        );

        let all_instances =
            naming.get_instances("public", "DEFAULT_GROUP", "test-service", "", false);
        assert_eq!(all_instances.len(), 2);

        let healthy_instances =
            naming.get_instances("public", "DEFAULT_GROUP", "test-service", "", true);
        assert_eq!(healthy_instances.len(), 1);
        assert_eq!(healthy_instances[0].ip, "127.0.0.1");
    }

    #[test]
    fn test_subscribe_and_get_subscribers() {
        let naming = NamingService::new();

        naming.subscribe("conn-1", "public", "DEFAULT_GROUP", "test-service");
        naming.subscribe("conn-2", "public", "DEFAULT_GROUP", "test-service");

        let subscribers = naming.get_subscribers("public", "DEFAULT_GROUP", "test-service");
        assert_eq!(subscribers.len(), 2);
        assert!(subscribers.contains(&"conn-1".to_string()));
        assert!(subscribers.contains(&"conn-2".to_string()));
    }

    #[test]
    fn test_heartbeat() {
        let naming = NamingService::new();
        let mut instance = create_test_instance("127.0.0.1", 8080);
        instance.healthy = false;

        naming.register_instance("public", "DEFAULT_GROUP", "test-service", instance);

        let result = naming.heartbeat(
            "public",
            "DEFAULT_GROUP",
            "test-service",
            "127.0.0.1",
            8080,
            "DEFAULT",
        );
        assert!(result);

        let instances = naming.get_instances("public", "DEFAULT_GROUP", "test-service", "", true);
        assert_eq!(instances.len(), 1);
    }

    #[test]
    fn test_instance_count() {
        let naming = NamingService::new();

        naming.register_instance(
            "public",
            "DEFAULT_GROUP",
            "test-service",
            create_test_instance("127.0.0.1", 8080),
        );
        naming.register_instance(
            "public",
            "DEFAULT_GROUP",
            "test-service",
            create_test_instance("127.0.0.2", 8081),
        );

        assert_eq!(
            naming.get_instance_count("public", "DEFAULT_GROUP", "test-service"),
            2
        );
    }

    // === Boundary and Edge Case Tests ===

    #[test]
    fn test_get_instances_empty_service() {
        let naming = NamingService::new();
        let instances = naming.get_instances("public", "DEFAULT_GROUP", "non-existent", "", false);
        assert!(instances.is_empty());
    }

    #[test]
    fn test_get_instance_count_non_existent() {
        let naming = NamingService::new();
        assert_eq!(
            naming.get_instance_count("public", "DEFAULT_GROUP", "non-existent"),
            0
        );
    }

    #[test]
    fn test_get_healthy_instance_count() {
        let naming = NamingService::new();

        let healthy = create_test_instance("127.0.0.1", 8080);
        let mut unhealthy = create_test_instance("127.0.0.2", 8081);
        unhealthy.healthy = false;

        naming.register_instance("public", "DEFAULT_GROUP", "test-service", healthy);
        naming.register_instance("public", "DEFAULT_GROUP", "test-service", unhealthy);

        assert_eq!(
            naming.get_healthy_instance_count("public", "DEFAULT_GROUP", "test-service"),
            1
        );
    }

    #[test]
    fn test_get_healthy_instance_count_non_existent() {
        let naming = NamingService::new();
        assert_eq!(
            naming.get_healthy_instance_count("public", "DEFAULT_GROUP", "non-existent"),
            0
        );
    }

    #[test]
    fn test_register_instance_zero_weight() {
        let naming = NamingService::new();
        let mut instance = create_test_instance("127.0.0.1", 8080);
        instance.weight = 0.0;

        naming.register_instance("public", "DEFAULT_GROUP", "test-service", instance);

        let instances = naming.get_instances("public", "DEFAULT_GROUP", "test-service", "", false);
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].weight, 0.0); // Zero weight is preserved for zero-weight semantics
    }

    #[test]
    fn test_register_instance_negative_weight() {
        let naming = NamingService::new();
        let mut instance = create_test_instance("127.0.0.1", 8080);
        instance.weight = -5.0;

        naming.register_instance("public", "DEFAULT_GROUP", "test-service", instance);

        let instances = naming.get_instances("public", "DEFAULT_GROUP", "test-service", "", false);
        assert_eq!(instances[0].weight, 1.0); // Should be normalized to 1.0
    }

    #[test]
    fn test_deregister_non_existent_service() {
        let naming = NamingService::new();
        let instance = create_test_instance("127.0.0.1", 8080);

        let result =
            naming.deregister_instance("public", "DEFAULT_GROUP", "non-existent", &instance);
        assert!(!result);
    }

    #[test]
    fn test_deregister_non_existent_instance() {
        let naming = NamingService::new();
        let instance1 = create_test_instance("127.0.0.1", 8080);
        let instance2 = create_test_instance("127.0.0.2", 9090);

        naming.register_instance("public", "DEFAULT_GROUP", "test-service", instance1);

        // Try to deregister instance that was never registered
        let result =
            naming.deregister_instance("public", "DEFAULT_GROUP", "test-service", &instance2);
        // Should still return true because the service exists, even if the instance doesn't
        assert!(result);
    }

    #[test]
    fn test_get_instances_cluster_filter() {
        let naming = NamingService::new();

        let mut instance1 = create_test_instance("127.0.0.1", 8080);
        instance1.cluster_name = "CLUSTER_A".to_string();

        let mut instance2 = create_test_instance("127.0.0.2", 8081);
        instance2.cluster_name = "CLUSTER_B".to_string();

        naming.register_instance("public", "DEFAULT_GROUP", "test-service", instance1);
        naming.register_instance("public", "DEFAULT_GROUP", "test-service", instance2);

        let cluster_a = naming.get_instances(
            "public",
            "DEFAULT_GROUP",
            "test-service",
            "CLUSTER_A",
            false,
        );
        assert_eq!(cluster_a.len(), 1);
        assert_eq!(cluster_a[0].cluster_name, "CLUSTER_A");

        let cluster_b = naming.get_instances(
            "public",
            "DEFAULT_GROUP",
            "test-service",
            "CLUSTER_B",
            false,
        );
        assert_eq!(cluster_b.len(), 1);
        assert_eq!(cluster_b[0].cluster_name, "CLUSTER_B");
    }

    #[test]
    fn test_get_instances_wildcard_cluster() {
        let naming = NamingService::new();

        let mut instance1 = create_test_instance("127.0.0.1", 8080);
        instance1.cluster_name = "CLUSTER_A".to_string();

        let mut instance2 = create_test_instance("127.0.0.2", 8081);
        instance2.cluster_name = "CLUSTER_B".to_string();

        naming.register_instance("public", "DEFAULT_GROUP", "test-service", instance1);
        naming.register_instance("public", "DEFAULT_GROUP", "test-service", instance2);

        // Wildcard should return all clusters
        let all = naming.get_instances("public", "DEFAULT_GROUP", "test-service", "*", false);
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_list_services_pagination() {
        let naming = NamingService::new();

        // Register 5 services
        for i in 0..5 {
            let instance = create_test_instance("127.0.0.1", 8080 + i);
            naming.register_instance(
                "public",
                "DEFAULT_GROUP",
                &format!("service-{}", i),
                instance,
            );
        }

        // Page 1 with size 2
        let (total, services) = naming.list_services("public", "DEFAULT_GROUP", 1, 2);
        assert_eq!(total, 5);
        assert_eq!(services.len(), 2);

        // Page 2 with size 2
        let (total, services) = naming.list_services("public", "DEFAULT_GROUP", 2, 2);
        assert_eq!(total, 5);
        assert_eq!(services.len(), 2);

        // Page 3 with size 2 (last page, only 1 result)
        let (total, services) = naming.list_services("public", "DEFAULT_GROUP", 3, 2);
        assert_eq!(total, 5);
        assert_eq!(services.len(), 1);
    }

    #[test]
    fn test_list_services_page_beyond_total() {
        let naming = NamingService::new();

        let instance = create_test_instance("127.0.0.1", 8080);
        naming.register_instance("public", "DEFAULT_GROUP", "test-service", instance);

        // Request page 100 when only 1 service exists
        let (total, services) = naming.list_services("public", "DEFAULT_GROUP", 100, 10);
        assert_eq!(total, 1);
        assert!(services.is_empty());
    }

    #[test]
    fn test_list_services_empty_namespace() {
        let naming = NamingService::new();

        let (total, services) = naming.list_services("empty-namespace", "DEFAULT_GROUP", 1, 10);
        assert_eq!(total, 0);
        assert!(services.is_empty());
    }

    #[test]
    fn test_list_services_empty_group_filter() {
        let naming = NamingService::new();

        let instance1 = create_test_instance("127.0.0.1", 8080);
        let instance2 = create_test_instance("127.0.0.2", 8081);

        naming.register_instance("public", "GROUP_A", "service-a", instance1);
        naming.register_instance("public", "GROUP_B", "service-b", instance2);

        // Empty group should match all groups in namespace
        let (total, _services) = naming.list_services("public", "", 1, 10);
        assert_eq!(total, 2);
    }

    #[test]
    fn test_unsubscribe() {
        let naming = NamingService::new();

        naming.subscribe("conn-1", "public", "DEFAULT_GROUP", "test-service");
        naming.subscribe("conn-1", "public", "DEFAULT_GROUP", "another-service");

        naming.unsubscribe("conn-1", "public", "DEFAULT_GROUP", "test-service");

        if let Some(subs) = naming.subscribers.get("conn-1") {
            assert_eq!(subs.len(), 1);
            assert!(!subs.contains(&"public@@DEFAULT_GROUP@@test-service".to_string()));
        }
    }

    #[test]
    fn test_unsubscribe_non_existent_connection() {
        let naming = NamingService::new();
        // Should not panic
        naming.unsubscribe("non-existent", "public", "DEFAULT_GROUP", "test-service");
    }

    #[test]
    fn test_remove_subscriber() {
        let naming = NamingService::new();

        naming.subscribe("conn-1", "public", "DEFAULT_GROUP", "test-service");
        naming.subscribe("conn-1", "public", "DEFAULT_GROUP", "another-service");

        naming.remove_subscriber("conn-1");

        assert!(naming.subscribers.get("conn-1").is_none());
    }

    #[test]
    fn test_heartbeat_non_existent_service() {
        let naming = NamingService::new();

        let result = naming.heartbeat(
            "public",
            "DEFAULT_GROUP",
            "non-existent",
            "127.0.0.1",
            8080,
            "DEFAULT",
        );
        assert!(!result);
    }

    #[test]
    fn test_heartbeat_non_existent_instance() {
        let naming = NamingService::new();
        let instance = create_test_instance("127.0.0.1", 8080);

        naming.register_instance("public", "DEFAULT_GROUP", "test-service", instance);

        // Heartbeat for different IP
        let result = naming.heartbeat(
            "public",
            "DEFAULT_GROUP",
            "test-service",
            "192.168.1.1",
            8080,
            "DEFAULT",
        );
        assert!(!result);
    }

    #[test]
    fn test_batch_register_instances() {
        let naming = NamingService::new();

        let instances = vec![
            create_test_instance("127.0.0.1", 8080),
            create_test_instance("127.0.0.2", 8081),
            create_test_instance("127.0.0.3", 8082),
        ];

        let result =
            naming.batch_register_instances("public", "DEFAULT_GROUP", "test-service", instances);
        assert!(result);

        assert_eq!(
            naming.get_instance_count("public", "DEFAULT_GROUP", "test-service"),
            3
        );
    }

    #[test]
    fn test_batch_register_instances_empty() {
        let naming = NamingService::new();

        let result =
            naming.batch_register_instances("public", "DEFAULT_GROUP", "test-service", vec![]);
        assert!(result);

        assert_eq!(
            naming.get_instance_count("public", "DEFAULT_GROUP", "test-service"),
            0
        );
    }

    #[test]
    fn test_batch_deregister_instances() {
        let naming = NamingService::new();

        let instances = vec![
            create_test_instance("127.0.0.1", 8080),
            create_test_instance("127.0.0.2", 8081),
            create_test_instance("127.0.0.3", 8082),
        ];

        naming.batch_register_instances(
            "public",
            "DEFAULT_GROUP",
            "test-service",
            instances.clone(),
        );

        let to_remove = &instances[0..2];
        naming.batch_deregister_instances("public", "DEFAULT_GROUP", "test-service", to_remove);

        assert_eq!(
            naming.get_instance_count("public", "DEFAULT_GROUP", "test-service"),
            1
        );
    }

    #[test]
    fn test_get_service() {
        let naming = NamingService::new();

        let healthy = create_test_instance("127.0.0.1", 8080);
        let mut unhealthy = create_test_instance("127.0.0.2", 8081);
        unhealthy.healthy = false;

        naming.register_instance("public", "DEFAULT_GROUP", "test-service", healthy);
        naming.register_instance("public", "DEFAULT_GROUP", "test-service", unhealthy);

        let service = naming.get_service("public", "DEFAULT_GROUP", "test-service", "", false);
        assert_eq!(service.name, "test-service");
        assert_eq!(service.group_name, "DEFAULT_GROUP");
        assert_eq!(service.hosts.len(), 2);
        assert!(service.all_ips);

        let healthy_service =
            naming.get_service("public", "DEFAULT_GROUP", "test-service", "", true);
        assert_eq!(healthy_service.hosts.len(), 1);
    }

    #[test]
    fn test_get_service_empty() {
        let naming = NamingService::new();

        let service = naming.get_service("public", "DEFAULT_GROUP", "non-existent", "", false);
        assert_eq!(service.name, "non-existent");
        assert!(service.hosts.is_empty());
        assert!(!service.all_ips);
    }

    #[test]
    fn test_special_characters_in_names() {
        let naming = NamingService::new();
        let instance = create_test_instance("127.0.0.1", 8080);

        // Test with special characters that might break key format
        naming.register_instance(
            "ns-with-dash",
            "group.with.dots",
            "service:with:colons",
            instance,
        );

        let instances = naming.get_instances(
            "ns-with-dash",
            "group.with.dots",
            "service:with:colons",
            "",
            false,
        );
        assert_eq!(instances.len(), 1);
    }

    #[test]
    fn test_naming_service_default() {
        let naming = NamingService::default();
        assert_eq!(
            naming.get_instance_count("public", "DEFAULT_GROUP", "any"),
            0
        );
    }

    #[test]
    fn test_register_same_instance_twice() {
        let naming = NamingService::new();
        let instance = create_test_instance("127.0.0.1", 8080);

        naming.register_instance("public", "DEFAULT_GROUP", "test-service", instance.clone());
        naming.register_instance("public", "DEFAULT_GROUP", "test-service", instance);

        // Should overwrite, not duplicate
        assert_eq!(
            naming.get_instance_count("public", "DEFAULT_GROUP", "test-service"),
            1
        );
    }

    // === Fuzzy Watch Tests ===

    #[test]
    fn test_fuzzy_watch_pattern_matches() {
        let pattern = FuzzyWatchPattern {
            namespace: "public".to_string(),
            group_pattern: "DEFAULT_GROUP".to_string(),
            service_pattern: "service-*".to_string(),
            watch_type: "add".to_string(),
        };

        assert!(pattern.matches("public", "DEFAULT_GROUP", "service-a"));
        assert!(pattern.matches("public", "DEFAULT_GROUP", "service-test"));
        assert!(!pattern.matches("public", "DEFAULT_GROUP", "other-service"));
        assert!(!pattern.matches("private", "DEFAULT_GROUP", "service-a"));
    }

    #[test]
    fn test_fuzzy_watch_pattern_wildcard_group() {
        let pattern = FuzzyWatchPattern {
            namespace: "public".to_string(),
            group_pattern: "*".to_string(),
            service_pattern: "*".to_string(),
            watch_type: "add".to_string(),
        };

        assert!(pattern.matches("public", "DEFAULT_GROUP", "any-service"));
        assert!(pattern.matches("public", "CUSTOM_GROUP", "another-service"));
        assert!(!pattern.matches("private", "DEFAULT_GROUP", "any-service"));
    }

    #[test]
    fn test_register_fuzzy_watch() {
        let naming = NamingService::new();

        naming.register_fuzzy_watch("conn-1", "public", "DEFAULT_GROUP", "service-*", "add");

        let services = naming.get_fuzzy_watched_services("conn-1");
        // No services registered yet, so no matches
        assert!(services.is_empty());
    }

    #[test]
    fn test_fuzzy_watch_matches_registered_services() {
        let naming = NamingService::new();

        // Register services first
        let instance1 = create_test_instance("127.0.0.1", 8080);
        let instance2 = create_test_instance("127.0.0.2", 8081);

        naming.register_instance("public", "DEFAULT_GROUP", "service-a", instance1);
        naming.register_instance("public", "DEFAULT_GROUP", "service-b", instance2);
        naming.register_instance(
            "public",
            "DEFAULT_GROUP",
            "other-service",
            create_test_instance("127.0.0.3", 8082),
        );

        // Register fuzzy watch
        naming.register_fuzzy_watch("conn-1", "public", "DEFAULT_GROUP", "service-*", "add");

        let matched = naming.get_fuzzy_watched_services("conn-1");
        assert_eq!(matched.len(), 2);
        assert!(matched.iter().any(|s| s.contains("service-a")));
        assert!(matched.iter().any(|s| s.contains("service-b")));
    }

    #[test]
    fn test_get_fuzzy_watchers_for_service() {
        let naming = NamingService::new();

        naming.register_fuzzy_watch("conn-1", "public", "DEFAULT_GROUP", "service-*", "add");
        naming.register_fuzzy_watch("conn-2", "public", "*", "*", "add");
        naming.register_fuzzy_watch("conn-3", "public", "OTHER_GROUP", "*", "add");

        let watchers =
            naming.get_fuzzy_watchers_for_service("public", "DEFAULT_GROUP", "service-a");
        assert_eq!(watchers.len(), 2);
        assert!(watchers.contains(&"conn-1".to_string()));
        assert!(watchers.contains(&"conn-2".to_string()));
    }

    #[test]
    fn test_unregister_fuzzy_watch() {
        let naming = NamingService::new();

        naming.register_fuzzy_watch("conn-1", "public", "DEFAULT_GROUP", "*", "add");

        // Unregister
        naming.unregister_fuzzy_watch("conn-1");

        // Should have no patterns now
        let matched = naming.get_fuzzy_watched_services("conn-1");
        assert!(matched.is_empty());
    }

    #[test]
    fn test_remove_subscriber_also_removes_fuzzy_watch() {
        let naming = NamingService::new();

        naming.subscribe("conn-1", "public", "DEFAULT_GROUP", "test-service");
        naming.register_fuzzy_watch("conn-1", "public", "*", "*", "add");

        // Remove subscriber should also remove fuzzy watch
        naming.remove_subscriber("conn-1");

        let matched = naming.get_fuzzy_watched_services("conn-1");
        assert!(matched.is_empty());
    }

    #[test]
    fn test_get_services_by_pattern() {
        let naming = NamingService::new();

        let instance1 = create_test_instance("127.0.0.1", 8080);
        let instance2 = create_test_instance("127.0.0.2", 8081);
        let instance3 = create_test_instance("127.0.0.3", 8082);

        naming.register_instance("public", "DEFAULT_GROUP", "app-service", instance1);
        naming.register_instance("public", "DEFAULT_GROUP", "app-config", instance2);
        naming.register_instance("public", "DEFAULT_GROUP", "db-service", instance3);

        let matched = naming.get_services_by_pattern("public", "DEFAULT_GROUP", "app-*");
        assert_eq!(matched.len(), 2);
        assert!(matched.iter().any(|s| s.contains("app-service")));
        assert!(matched.iter().any(|s| s.contains("app-config")));
    }

    // === Protection Threshold Tests ===

    #[test]
    fn test_protection_threshold_not_triggered() {
        let naming = NamingService::new();

        // Register instances - all healthy
        let instance1 = create_test_instance("127.0.0.1", 8080);
        let instance2 = create_test_instance("127.0.0.2", 8081);
        let instance3 = create_test_instance("127.0.0.3", 8082);

        naming.register_instance("public", "DEFAULT_GROUP", "test-service", instance1);
        naming.register_instance("public", "DEFAULT_GROUP", "test-service", instance2);
        naming.register_instance("public", "DEFAULT_GROUP", "test-service", instance3);

        // Set protection threshold to 50%
        naming.update_service_protect_threshold("public", "DEFAULT_GROUP", "test-service", 0.5);

        // Get service - protection should NOT be triggered (100% healthy > 50%)
        let service = naming.get_service("public", "DEFAULT_GROUP", "test-service", "", true);
        assert!(!service.reach_protection_threshold);
        assert_eq!(service.hosts.len(), 3);
    }

    #[test]
    fn test_protection_threshold_triggered() {
        let naming = NamingService::new();

        // Register instances - 1 healthy, 2 unhealthy
        let instance1 = create_test_instance("127.0.0.1", 8080);
        let mut instance2 = create_test_instance("127.0.0.2", 8081);
        instance2.healthy = false;
        let mut instance3 = create_test_instance("127.0.0.3", 8082);
        instance3.healthy = false;

        naming.register_instance("public", "DEFAULT_GROUP", "test-service", instance1);
        naming.register_instance("public", "DEFAULT_GROUP", "test-service", instance2);
        naming.register_instance("public", "DEFAULT_GROUP", "test-service", instance3);

        // Set protection threshold to 50%
        naming.update_service_protect_threshold("public", "DEFAULT_GROUP", "test-service", 0.5);

        // Get service with healthy_only - protection should be triggered
        // Because healthy ratio is 33% < 50% threshold
        let service = naming.get_service("public", "DEFAULT_GROUP", "test-service", "", true);
        assert!(service.reach_protection_threshold);
        // Should return ALL instances (including unhealthy) when protection is triggered
        assert_eq!(service.hosts.len(), 3);
    }

    #[test]
    fn test_protection_threshold_zero_means_disabled() {
        let naming = NamingService::new();

        // Register instances - only 1 healthy
        let instance1 = create_test_instance("127.0.0.1", 8080);
        let mut instance2 = create_test_instance("127.0.0.2", 8081);
        instance2.healthy = false;

        naming.register_instance("public", "DEFAULT_GROUP", "test-service", instance1);
        naming.register_instance("public", "DEFAULT_GROUP", "test-service", instance2);

        // Protection threshold is 0 (default - disabled)
        // Should only return healthy instances
        let service = naming.get_service("public", "DEFAULT_GROUP", "test-service", "", true);
        assert!(!service.reach_protection_threshold);
        assert_eq!(service.hosts.len(), 1);
    }

    #[test]
    fn test_protection_info() {
        let naming = NamingService::new();

        // Register instances
        let instance1 = create_test_instance("127.0.0.1", 8080);
        let mut instance2 = create_test_instance("127.0.0.2", 8081);
        instance2.healthy = false;
        let instance3 = create_test_instance("127.0.0.3", 8082);

        naming.register_instance("public", "DEFAULT_GROUP", "test-service", instance1);
        naming.register_instance("public", "DEFAULT_GROUP", "test-service", instance2);
        naming.register_instance("public", "DEFAULT_GROUP", "test-service", instance3);

        // Set protection threshold
        naming.update_service_protect_threshold("public", "DEFAULT_GROUP", "test-service", 0.8);

        // Get protection info
        let (service, info) = naming.get_service_with_protection_info(
            "public",
            "DEFAULT_GROUP",
            "test-service",
            "",
            true,
        );

        assert_eq!(info.threshold, 0.8);
        assert_eq!(info.total_instances, 3);
        assert_eq!(info.healthy_instances, 2);
        assert!((info.healthy_ratio - 0.6666667).abs() < 0.001);
        assert!(info.triggered); // 66% < 80%
        assert!(info.is_degraded());
        assert_eq!(info.unhealthy_instances(), 1);
        // Service should return all instances due to protection trigger
        assert_eq!(service.hosts.len(), 3);
    }

    #[test]
    fn test_protection_threshold_boundary() {
        let naming = NamingService::new();

        // Register 2 healthy, 2 unhealthy (50% healthy ratio)
        let instance1 = create_test_instance("127.0.0.1", 8080);
        let instance2 = create_test_instance("127.0.0.2", 8081);
        let mut instance3 = create_test_instance("127.0.0.3", 8082);
        instance3.healthy = false;
        let mut instance4 = create_test_instance("127.0.0.4", 8083);
        instance4.healthy = false;

        naming.register_instance("public", "DEFAULT_GROUP", "test-service", instance1);
        naming.register_instance("public", "DEFAULT_GROUP", "test-service", instance2);
        naming.register_instance("public", "DEFAULT_GROUP", "test-service", instance3);
        naming.register_instance("public", "DEFAULT_GROUP", "test-service", instance4);

        // Set threshold to exactly 50%
        naming.update_service_protect_threshold("public", "DEFAULT_GROUP", "test-service", 0.5);

        // healthy ratio (50%) is NOT less than threshold (50%), so protection NOT triggered
        let service = naming.get_service("public", "DEFAULT_GROUP", "test-service", "", true);
        assert!(!service.reach_protection_threshold);
        assert_eq!(service.hosts.len(), 2); // Only healthy instances

        // Set threshold to 51%
        naming.update_service_protect_threshold("public", "DEFAULT_GROUP", "test-service", 0.51);

        // Now healthy ratio (50%) < threshold (51%), protection IS triggered
        let service = naming.get_service("public", "DEFAULT_GROUP", "test-service", "", true);
        assert!(service.reach_protection_threshold);
        assert_eq!(service.hosts.len(), 4); // All instances
    }
}
