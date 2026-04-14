//! Naming service layer for service discovery operations
//!
//! This module provides in-memory service registry for managing service instances.
//! It is split into sub-modules for maintainability:
//! - `instance`: Instance CRUD, query, heartbeat, health operations
//! - `subscription`: Subscriber, publisher, and connection tracking
//! - `fuzzy_watch`: Fuzzy watch pattern matching
//! - `cluster`: Cluster configuration and statistics
//! - `metadata`: Service metadata operations

mod client_op;
mod cluster;
mod disconnect_listener;
mod fuzzy_watch;
mod instance;
mod metadata;
mod provider;
mod subscription;

pub use disconnect_listener::{
    ConnectionManagerPusher, NamingDisconnectListener, SubscriberPusher,
};

pub use client_op::{
    ClientOperationService, ClientOperationServiceProxy, EphemeralClientOperationService,
    PersistentClientOperationService, deregister_instance_dispatch, register_instance_dispatch,
};

use std::{collections::HashSet, sync::Arc};

use dashmap::DashMap;

use crate::model::Instance;

// Re-export model types from batata-api
pub use batata_api::naming::{ClusterConfig, ClusterStatistics, ProtectionInfo, ServiceMetadata};

// Re-export centralized key builders from batata-common
pub use batata_common::{build_service_key, parse_service_key};

/// Build instance key format: ip#port#clusterName (pre-allocated)
fn build_instance_key(instance: &Instance) -> String {
    build_instance_key_parts(&instance.ip, instance.port, &instance.cluster_name)
}

/// Build instance key from individual parts (pre-allocated)
pub fn build_instance_key_parts(ip: &str, port: i32, cluster_name: &str) -> String {
    // port is typically 1-65535, max 5 digits
    let mut key = String::with_capacity(ip.len() + 7 + cluster_name.len());
    key.push_str(ip);
    key.push('#');
    let _ = std::fmt::Write::write_fmt(&mut key, format_args!("{}", port));
    key.push('#');
    key.push_str(cluster_name);
    key
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
    /// Instances are wrapped in Arc to enable cheap snapshots under DashMap shard locks.
    services: Arc<DashMap<String, DashMap<String, Arc<Instance>>>>,
    /// Key: service_key, Value: service-level metadata
    service_metadata: Arc<DashMap<String, ServiceMetadata>>,
    /// Key: service_key##cluster_name, Value: cluster configuration
    cluster_configs: Arc<DashMap<String, ClusterConfig>>,
    /// Key: connection_id, Value: set of subscribed service keys (HashSet for O(1) lookups)
    subscribers: Arc<DashMap<String, HashSet<String>>>,
    /// Reverse index: service_key -> set of connection_ids subscribed to this service.
    /// Avoids full-table scan in get_subscribers().
    subscriber_index: Arc<DashMap<String, HashSet<String>>>,
    /// Key: connection_id, Value: list of fuzzy watch patterns
    fuzzy_watchers: Arc<DashMap<String, Vec<FuzzyWatchPattern>>>,
    /// Key: connection_id, Value: set of published service keys (for V2 Client API)
    publishers: Arc<DashMap<String, HashSet<String>>>,
    /// Key: connection_id, Value: set of (service_key, instance_key) tuples
    /// Tracks which instances were registered by which gRPC connection,
    /// enabling automatic deregistration when a connection disconnects.
    connection_instances: Arc<DashMap<String, HashSet<(String, String)>>>,
    /// Connections currently being deregistered (prevents concurrent register during cleanup)
    closing_connections: Arc<DashMap<String, ()>>,
    /// Index: namespace@@group prefix -> set of service names.
    /// Enables O(page_size) list_services instead of O(total_services) full scan.
    service_name_index: Arc<DashMap<String, HashSet<String>>>,
}

/// Build cluster config key format: service_key##clusterName (pre-allocated)
fn build_cluster_key(service_key: &str, cluster_name: &str) -> String {
    let mut key = String::with_capacity(service_key.len() + 2 + cluster_name.len());
    key.push_str(service_key);
    key.push_str("##");
    key.push_str(cluster_name);
    key
}

impl NamingService {
    pub fn new() -> Self {
        Self {
            services: Arc::new(DashMap::new()),
            service_metadata: Arc::new(DashMap::new()),
            cluster_configs: Arc::new(DashMap::new()),
            subscribers: Arc::new(DashMap::new()),
            subscriber_index: Arc::new(DashMap::new()),
            fuzzy_watchers: Arc::new(DashMap::new()),
            publishers: Arc::new(DashMap::new()),
            connection_instances: Arc::new(DashMap::new()),
            closing_connections: Arc::new(DashMap::new()),
            service_name_index: Arc::new(DashMap::new()),
        }
    }

    /// Add a service name to the namespace-group index.
    fn index_service_name(&self, service_key: &str) {
        if let Some((ns, group, svc)) = parse_service_key(service_key) {
            let prefix = format!("{}@@{}", ns, group);
            self.service_name_index
                .entry(prefix)
                .or_default()
                .insert(svc.to_string());
        }
    }

    /// Remove a service name from the namespace-group index.
    /// Only removes if the service has no instances AND no metadata.
    fn maybe_unindex_service_name(&self, service_key: &str) {
        let has_instances = self
            .services
            .get(service_key)
            .map(|s| !s.is_empty())
            .unwrap_or(false);
        let has_metadata = self.service_metadata.contains_key(service_key);
        if !has_instances
            && !has_metadata
            && let Some((ns, group, svc)) = parse_service_key(service_key)
        {
            let prefix = format!("{}@@{}", ns, group);
            if let Some(mut names) = self.service_name_index.get_mut(&prefix) {
                names.remove(svc);
                if names.is_empty() {
                    drop(names);
                    self.service_name_index.remove(&prefix);
                }
            }
        }
    }

    /// Increment the revision counter for a service (for change detection / cache invalidation)
    fn increment_service_revision(&self, service_key: &str) {
        if let Some(mut meta) = self.service_metadata.get_mut(service_key) {
            meta.revision = meta.revision.wrapping_add(1);
        }
    }

    /// Get the current revision for a service.
    /// Returns 0 if the service has no metadata entry yet.
    /// Used by Distro verify to detect changes without timestamp drift.
    pub fn get_service_revision(&self, service_key: &str) -> i64 {
        self.service_metadata
            .get(service_key)
            .map(|m| m.revision as i64)
            .unwrap_or(0)
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
    use std::collections::HashMap;

    fn create_test_instance(ip: &str, port: i32) -> Instance {
        Instance {
            instance_id: format!("{}#{}#DEFAULT#test-service", ip, port),
            ip: ip.to_string(),
            port,
            weight: 1.0,
            healthy: true,
            enabled: true,
            ephemeral: true,
            cluster_name: "DEFAULT".to_string(),
            service_name: "test-service".to_string(),
            metadata: HashMap::new(),
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
    fn test_deregister_non_existent_service_is_idempotent() {
        let naming = NamingService::new();
        let instance = create_test_instance("127.0.0.1", 8080);

        // Deregistering from a non-existent service returns true (idempotent)
        // Nacos SDK expects this during cleanup — no error should occur
        let result =
            naming.deregister_instance("public", "DEFAULT_GROUP", "non-existent", &instance);
        assert!(
            result,
            "Deregister non-existent service should be idempotent (return true)"
        );
    }

    #[test]
    fn test_deregister_non_existent_instance_is_idempotent() {
        let naming = NamingService::new();
        let instance1 = create_test_instance("127.0.0.1", 8080);
        let instance2 = create_test_instance("127.0.0.2", 9090);

        naming.register_instance("public", "DEFAULT_GROUP", "test-service", instance1);

        // Deregistering instance that was never registered returns true (idempotent)
        let result =
            naming.deregister_instance("public", "DEFAULT_GROUP", "test-service", &instance2);
        assert!(
            result,
            "Deregister non-existent instance should be idempotent (return true)"
        );

        // Verify original instance is still there
        let instances = naming.get_instances("public", "DEFAULT_GROUP", "test-service", "", false);
        assert_eq!(
            instances.len(),
            1,
            "Original instance should not be affected"
        );
        assert_eq!(instances[0].ip, "127.0.0.1");
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

    // ============== Concurrent Stress Tests ==============
    //
    // These tests exercise concurrent read/write paths to detect deadlocks.
    // They use a timeout-based approach: if a test hangs beyond the timeout,
    // it indicates a deadlock.

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_concurrent_register_and_get_instances() {
        let naming = Arc::new(NamingService::new());
        let barrier = Arc::new(tokio::sync::Barrier::new(8));

        let mut handles = Vec::new();

        // 4 writers: register instances
        for i in 0..4 {
            let naming = naming.clone();
            let barrier = barrier.clone();
            handles.push(tokio::spawn(async move {
                barrier.wait().await;
                for j in 0..50 {
                    let instance = Instance {
                        instance_id: format!("inst-{}-{}#8080#DEFAULT#svc", i, j),
                        ip: format!("10.0.{}.{}", i, j),
                        port: 8080,
                        weight: 1.0,
                        healthy: true,
                        enabled: true,
                        ephemeral: true,
                        cluster_name: "DEFAULT".to_string(),
                        service_name: "stress-svc".to_string(),
                        metadata: HashMap::new(),
                    };
                    naming.register_instance("public", "DEFAULT_GROUP", "stress-svc", instance);
                }
            }));
        }

        // 4 readers: get instances concurrently
        for _ in 0..4 {
            let naming = naming.clone();
            let barrier = barrier.clone();
            handles.push(tokio::spawn(async move {
                barrier.wait().await;
                for _ in 0..50 {
                    let _ = naming.get_instances("public", "DEFAULT_GROUP", "stress-svc", "", true);
                    tokio::task::yield_now().await;
                }
            }));
        }

        let result = tokio::time::timeout(std::time::Duration::from_secs(10), async {
            for h in handles {
                h.await.unwrap();
            }
        })
        .await;

        assert!(
            result.is_ok(),
            "Deadlock detected: concurrent register + get_instances timed out"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_concurrent_register_deregister_list() {
        let naming = Arc::new(NamingService::new());
        let barrier = Arc::new(tokio::sync::Barrier::new(6));

        let mut handles = Vec::new();

        // 2 writers: register
        for i in 0..2 {
            let naming = naming.clone();
            let barrier = barrier.clone();
            handles.push(tokio::spawn(async move {
                barrier.wait().await;
                for j in 0..30 {
                    let svc_name = format!("svc-{}-{}", i, j);
                    let instance = Instance {
                        instance_id: format!("inst#8080#DEFAULT#{}", svc_name),
                        ip: format!("10.1.{}.{}", i, j),
                        port: 8080,
                        weight: 1.0,
                        healthy: true,
                        enabled: true,
                        ephemeral: true,
                        cluster_name: "DEFAULT".to_string(),
                        service_name: svc_name.clone(),
                        metadata: HashMap::new(),
                    };
                    naming.register_instance("public", "DEFAULT_GROUP", &svc_name, instance);
                }
            }));
        }

        // 2 deregisters
        for i in 0..2 {
            let naming = naming.clone();
            let barrier = barrier.clone();
            handles.push(tokio::spawn(async move {
                barrier.wait().await;
                for j in 0..30 {
                    let svc_name = format!("svc-{}-{}", i, j);
                    let instance = Instance {
                        ip: format!("10.1.{}.{}", i, j),
                        port: 8080,
                        cluster_name: "DEFAULT".to_string(),
                        ..Default::default()
                    };
                    naming.deregister_instance("public", "DEFAULT_GROUP", &svc_name, &instance);
                    tokio::task::yield_now().await;
                }
            }));
        }

        // 2 readers: list services
        for _ in 0..2 {
            let naming = naming.clone();
            let barrier = barrier.clone();
            handles.push(tokio::spawn(async move {
                barrier.wait().await;
                for _ in 0..30 {
                    let _ = naming.list_services("public", "DEFAULT_GROUP", 1, 100);
                    tokio::task::yield_now().await;
                }
            }));
        }

        let result = tokio::time::timeout(std::time::Duration::from_secs(10), async {
            for h in handles {
                h.await.unwrap();
            }
        })
        .await;

        assert!(
            result.is_ok(),
            "Deadlock detected: concurrent register + deregister + list_services timed out"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_concurrent_get_service_and_heartbeat() {
        let naming = Arc::new(NamingService::new());

        // Pre-register some instances
        for i in 0..10 {
            let instance = Instance {
                instance_id: format!("hb-inst-{}#8080#DEFAULT#hb-svc", i),
                ip: format!("10.2.0.{}", i),
                port: 8080,
                weight: 1.0,
                healthy: true,
                enabled: true,
                ephemeral: true,
                cluster_name: "DEFAULT".to_string(),
                service_name: "hb-svc".to_string(),
                metadata: HashMap::new(),
            };
            naming.register_instance("public", "DEFAULT_GROUP", "hb-svc", instance);
        }

        let barrier = Arc::new(tokio::sync::Barrier::new(6));
        let mut handles = Vec::new();

        // 2 get_service readers
        for _ in 0..2 {
            let naming = naming.clone();
            let barrier = barrier.clone();
            handles.push(tokio::spawn(async move {
                barrier.wait().await;
                for _ in 0..50 {
                    let _ = naming.get_service("public", "DEFAULT_GROUP", "hb-svc", "", true);
                    tokio::task::yield_now().await;
                }
            }));
        }

        // 2 heartbeat writers
        for _ in 0..2 {
            let naming = naming.clone();
            let barrier = barrier.clone();
            handles.push(tokio::spawn(async move {
                barrier.wait().await;
                for i in 0..50 {
                    let idx = i % 10;
                    naming.heartbeat(
                        "public",
                        "DEFAULT_GROUP",
                        "hb-svc",
                        &format!("10.2.0.{}", idx),
                        8080,
                        "DEFAULT",
                    );
                    tokio::task::yield_now().await;
                }
            }));
        }

        // 2 protection info readers
        for _ in 0..2 {
            let naming = naming.clone();
            let barrier = barrier.clone();
            handles.push(tokio::spawn(async move {
                barrier.wait().await;
                for _ in 0..50 {
                    let _ = naming.get_service_with_protection_info(
                        "public",
                        "DEFAULT_GROUP",
                        "hb-svc",
                        "",
                        true,
                    );
                    tokio::task::yield_now().await;
                }
            }));
        }

        let result = tokio::time::timeout(std::time::Duration::from_secs(10), async {
            for h in handles {
                h.await.unwrap();
            }
        })
        .await;

        assert!(
            result.is_ok(),
            "Deadlock detected: concurrent get_service + heartbeat + protection_info timed out"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_concurrent_connection_lifecycle() {
        let naming = Arc::new(NamingService::new());

        let service_key = "public@@DEFAULT_GROUP@@cl-svc";

        // Pre-register instances via connections
        for c in 0..4 {
            let conn_id = format!("conn-{}", c);
            for i in 0..5 {
                let instance = Instance {
                    instance_id: format!("cl-inst-{}-{}#8080#DEFAULT#cl-svc", c, i),
                    ip: format!("10.3.{}.{}", c, i),
                    port: 8080,
                    weight: 1.0,
                    healthy: true,
                    enabled: true,
                    ephemeral: true,
                    cluster_name: "DEFAULT".to_string(),
                    service_name: "cl-svc".to_string(),
                    metadata: HashMap::new(),
                };
                let instance_key = format!("10.3.{}.{}#8080#DEFAULT", c, i);
                naming.register_instance("public", "DEFAULT_GROUP", "cl-svc", instance);
                naming.add_connection_instance(&conn_id, service_key, &instance_key);
            }
        }

        let barrier = Arc::new(tokio::sync::Barrier::new(6));
        let mut handles = Vec::new();

        // 2 deregister-all-by-connection
        for c in 0..2 {
            let naming = naming.clone();
            let barrier = barrier.clone();
            handles.push(tokio::spawn(async move {
                barrier.wait().await;
                naming.deregister_all_by_connection(&format!("conn-{}", c));
            }));
        }

        // 2 readers: get instances
        for _ in 0..2 {
            let naming = naming.clone();
            let barrier = barrier.clone();
            handles.push(tokio::spawn(async move {
                barrier.wait().await;
                for _ in 0..20 {
                    let _ = naming.get_instances("public", "DEFAULT_GROUP", "cl-svc", "", true);
                    tokio::task::yield_now().await;
                }
            }));
        }

        // 2 new registrations (different connections)
        for c in 4..6 {
            let naming = naming.clone();
            let barrier = barrier.clone();
            let svc_key = service_key.to_string();
            handles.push(tokio::spawn(async move {
                barrier.wait().await;
                for i in 0..5 {
                    let instance = Instance {
                        instance_id: format!("cl-inst-{}-{}#8080#DEFAULT#cl-svc", c, i),
                        ip: format!("10.3.{}.{}", c, i),
                        port: 8080,
                        weight: 1.0,
                        healthy: true,
                        enabled: true,
                        ephemeral: true,
                        cluster_name: "DEFAULT".to_string(),
                        service_name: "cl-svc".to_string(),
                        metadata: HashMap::new(),
                    };
                    let instance_key = format!("10.3.{}.{}#8080#DEFAULT", c, i);
                    naming.register_instance("public", "DEFAULT_GROUP", "cl-svc", instance);
                    naming.add_connection_instance(&format!("conn-{}", c), &svc_key, &instance_key);
                }
            }));
        }

        let result = tokio::time::timeout(std::time::Duration::from_secs(10), async {
            for h in handles {
                h.await.unwrap();
            }
        })
        .await;

        assert!(
            result.is_ok(),
            "Deadlock detected: concurrent connection lifecycle operations timed out"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_concurrent_fuzzy_watch_with_register() {
        let naming = Arc::new(NamingService::new());

        let barrier = Arc::new(tokio::sync::Barrier::new(4));
        let mut handles = Vec::new();

        // 2 writers: register services + fuzzy watches
        for i in 0..2 {
            let naming = naming.clone();
            let barrier = barrier.clone();
            handles.push(tokio::spawn(async move {
                barrier.wait().await;
                for j in 0..10 {
                    let svc_name = format!("fw-svc-{}-{}", i, j);
                    let instance = Instance {
                        instance_id: format!("fw-inst#8080#DEFAULT#{}", svc_name),
                        ip: format!("10.4.{}.{}", i, j),
                        port: 8080,
                        weight: 1.0,
                        healthy: true,
                        enabled: true,
                        ephemeral: true,
                        cluster_name: "DEFAULT".to_string(),
                        service_name: svc_name.clone(),
                        metadata: HashMap::new(),
                    };
                    naming.register_instance("public", "DEFAULT_GROUP", &svc_name, instance);
                    naming.register_fuzzy_watch(
                        &format!("conn-fw-{}", i),
                        "public",
                        "DEFAULT_GROUP",
                        &format!("fw-svc-{}*", i),
                        "SUBSCRIBE_SERVICE",
                    );
                    tokio::task::yield_now().await;
                }
            }));
        }

        // 2 readers: get fuzzy watched services
        for i in 0..2 {
            let naming = naming.clone();
            let barrier = barrier.clone();
            handles.push(tokio::spawn(async move {
                barrier.wait().await;
                for _ in 0..10 {
                    let _ = naming.get_fuzzy_watched_services(&format!("conn-fw-{}", i));
                    tokio::task::yield_now().await;
                }
            }));
        }

        let result = tokio::time::timeout(std::time::Duration::from_secs(15), async {
            for h in handles {
                h.await.unwrap();
            }
        })
        .await;

        assert!(
            result.is_ok(),
            "Deadlock detected: concurrent fuzzy_watch + register timed out"
        );
    }
}
