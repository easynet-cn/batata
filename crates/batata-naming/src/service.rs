//! Naming service layer for service discovery operations
//!
//! This module provides in-memory service registry for managing service instances

use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use dashmap::DashMap;

use crate::model::{Instance, Service};

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

/// In-memory service registry for managing services and instances
#[derive(Clone)]
pub struct NamingService {
    /// Key: service_key (namespace@@group@@service), Value: map of instances
    services: Arc<DashMap<String, DashMap<String, Instance>>>,
    /// Key: connection_id, Value: list of subscribed service keys
    subscribers: Arc<DashMap<String, Vec<String>>>,
}

impl NamingService {
    pub fn new() -> Self {
        Self {
            services: Arc::new(DashMap::new()),
            subscribers: Arc::new(DashMap::new()),
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
        let instance_key = build_instance_key(&instance);

        // Set default values
        if instance.weight <= 0.0 {
            instance.weight = 1.0;
        }
        instance.service_name = service_name.to_string();

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
        let instance_key = build_instance_key(instance);

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
                    // Filter by cluster if specified
                    let cluster_match =
                        cluster.is_empty() || inst.cluster_name == cluster || cluster == "*";
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
    pub fn get_service(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster: &str,
        healthy_only: bool,
    ) -> Service {
        let instances =
            self.get_instances(namespace, group_name, service_name, cluster, healthy_only);
        let all_instances = self.get_instances(namespace, group_name, service_name, cluster, false);

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
            all_ips: !all_instances.is_empty(),
            reach_protection_threshold: false,
        }
    }

    /// List all services in a namespace
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

        let service_names: Vec<String> = self
            .services
            .iter()
            .filter(|entry| entry.key().starts_with(&prefix))
            .map(|entry| {
                let parts: Vec<&str> = entry.key().split("@@").collect();
                if parts.len() >= 3 {
                    parts[2].to_string()
                } else {
                    entry.key().clone()
                }
            })
            .collect();

        let total = service_names.len() as i32;

        // Paginate
        let start = ((page_no - 1) * page_size) as usize;
        let end = (page_no * page_size) as usize;

        let paginated: Vec<String> = service_names
            .into_iter()
            .skip(start)
            .take(end - start)
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
            .push(service_key);
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
            subs.retain(|s| s != &service_key);
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
    }

    /// Batch register instances
    pub fn batch_register_instances(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instances: Vec<Instance>,
    ) -> bool {
        for instance in instances {
            self.register_instance(namespace, group_name, service_name, instance);
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

    /// Get instance count for a service
    pub fn get_instance_count(&self, namespace: &str, group_name: &str, service_name: &str) -> usize {
        let service_key = build_service_key(namespace, group_name, service_name);

        self.services
            .get(&service_key)
            .map(|instances| instances.len())
            .unwrap_or(0)
    }

    /// Get healthy instance count for a service
    pub fn get_healthy_instance_count(&self, namespace: &str, group_name: &str, service_name: &str) -> usize {
        let service_key = build_service_key(namespace, group_name, service_name);

        self.services
            .get(&service_key)
            .map(|instances| {
                instances.iter().filter(|e| e.value().healthy).count()
            })
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

        let result = naming.heartbeat("public", "DEFAULT_GROUP", "test-service", "127.0.0.1", 8080, "DEFAULT");
        assert!(result);

        let instances = naming.get_instances("public", "DEFAULT_GROUP", "test-service", "", true);
        assert_eq!(instances.len(), 1);
    }

    #[test]
    fn test_instance_count() {
        let naming = NamingService::new();

        naming.register_instance("public", "DEFAULT_GROUP", "test-service", create_test_instance("127.0.0.1", 8080));
        naming.register_instance("public", "DEFAULT_GROUP", "test-service", create_test_instance("127.0.0.2", 8081));

        assert_eq!(naming.get_instance_count("public", "DEFAULT_GROUP", "test-service"), 2);
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
        assert_eq!(naming.get_instance_count("public", "DEFAULT_GROUP", "non-existent"), 0);
    }

    #[test]
    fn test_get_healthy_instance_count() {
        let naming = NamingService::new();

        let healthy = create_test_instance("127.0.0.1", 8080);
        let mut unhealthy = create_test_instance("127.0.0.2", 8081);
        unhealthy.healthy = false;

        naming.register_instance("public", "DEFAULT_GROUP", "test-service", healthy);
        naming.register_instance("public", "DEFAULT_GROUP", "test-service", unhealthy);

        assert_eq!(naming.get_healthy_instance_count("public", "DEFAULT_GROUP", "test-service"), 1);
    }

    #[test]
    fn test_get_healthy_instance_count_non_existent() {
        let naming = NamingService::new();
        assert_eq!(naming.get_healthy_instance_count("public", "DEFAULT_GROUP", "non-existent"), 0);
    }

    #[test]
    fn test_register_instance_zero_weight() {
        let naming = NamingService::new();
        let mut instance = create_test_instance("127.0.0.1", 8080);
        instance.weight = 0.0;

        naming.register_instance("public", "DEFAULT_GROUP", "test-service", instance);

        let instances = naming.get_instances("public", "DEFAULT_GROUP", "test-service", "", false);
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].weight, 1.0); // Should be normalized to 1.0
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

        let result = naming.deregister_instance("public", "DEFAULT_GROUP", "non-existent", &instance);
        assert!(!result);
    }

    #[test]
    fn test_deregister_non_existent_instance() {
        let naming = NamingService::new();
        let instance1 = create_test_instance("127.0.0.1", 8080);
        let instance2 = create_test_instance("127.0.0.2", 9090);

        naming.register_instance("public", "DEFAULT_GROUP", "test-service", instance1);

        // Try to deregister instance that was never registered
        let result = naming.deregister_instance("public", "DEFAULT_GROUP", "test-service", &instance2);
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

        let cluster_a = naming.get_instances("public", "DEFAULT_GROUP", "test-service", "CLUSTER_A", false);
        assert_eq!(cluster_a.len(), 1);
        assert_eq!(cluster_a[0].cluster_name, "CLUSTER_A");

        let cluster_b = naming.get_instances("public", "DEFAULT_GROUP", "test-service", "CLUSTER_B", false);
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
            naming.register_instance("public", "DEFAULT_GROUP", &format!("service-{}", i), instance);
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

        let result = naming.heartbeat("public", "DEFAULT_GROUP", "non-existent", "127.0.0.1", 8080, "DEFAULT");
        assert!(!result);
    }

    #[test]
    fn test_heartbeat_non_existent_instance() {
        let naming = NamingService::new();
        let instance = create_test_instance("127.0.0.1", 8080);

        naming.register_instance("public", "DEFAULT_GROUP", "test-service", instance);

        // Heartbeat for different IP
        let result = naming.heartbeat("public", "DEFAULT_GROUP", "test-service", "192.168.1.1", 8080, "DEFAULT");
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

        let result = naming.batch_register_instances("public", "DEFAULT_GROUP", "test-service", instances);
        assert!(result);

        assert_eq!(naming.get_instance_count("public", "DEFAULT_GROUP", "test-service"), 3);
    }

    #[test]
    fn test_batch_register_instances_empty() {
        let naming = NamingService::new();

        let result = naming.batch_register_instances("public", "DEFAULT_GROUP", "test-service", vec![]);
        assert!(result);

        assert_eq!(naming.get_instance_count("public", "DEFAULT_GROUP", "test-service"), 0);
    }

    #[test]
    fn test_batch_deregister_instances() {
        let naming = NamingService::new();

        let instances = vec![
            create_test_instance("127.0.0.1", 8080),
            create_test_instance("127.0.0.2", 8081),
            create_test_instance("127.0.0.3", 8082),
        ];

        naming.batch_register_instances("public", "DEFAULT_GROUP", "test-service", instances.clone());

        let to_remove = &instances[0..2];
        naming.batch_deregister_instances("public", "DEFAULT_GROUP", "test-service", to_remove);

        assert_eq!(naming.get_instance_count("public", "DEFAULT_GROUP", "test-service"), 1);
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

        let healthy_service = naming.get_service("public", "DEFAULT_GROUP", "test-service", "", true);
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
        naming.register_instance("ns-with-dash", "group.with.dots", "service:with:colons", instance);

        let instances = naming.get_instances("ns-with-dash", "group.with.dots", "service:with:colons", "", false);
        assert_eq!(instances.len(), 1);
    }

    #[test]
    fn test_naming_service_default() {
        let naming = NamingService::default();
        assert_eq!(naming.get_instance_count("public", "DEFAULT_GROUP", "any"), 0);
    }

    #[test]
    fn test_register_same_instance_twice() {
        let naming = NamingService::new();
        let instance = create_test_instance("127.0.0.1", 8080);

        naming.register_instance("public", "DEFAULT_GROUP", "test-service", instance.clone());
        naming.register_instance("public", "DEFAULT_GROUP", "test-service", instance);

        // Should overwrite, not duplicate
        assert_eq!(naming.get_instance_count("public", "DEFAULT_GROUP", "test-service"), 1);
    }
}
