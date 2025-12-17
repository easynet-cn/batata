// Naming service layer for service discovery operations
// This module provides in-memory service registry for managing service instances

use std::{
    collections::HashMap,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use dashmap::DashMap;

use crate::api::naming::model::{Instance, Service, ServiceInfo};

// Service key format: namespace@@groupName@@serviceName
fn build_service_key(namespace: &str, group_name: &str, service_name: &str) -> String {
    format!("{}@@{}@@{}", namespace, group_name, service_name)
}

// Instance key format: ip#port#clusterName
fn build_instance_key(instance: &Instance) -> String {
    format!("{}#{}#{}", instance.ip, instance.port, instance.cluster_name)
}

// In-memory service registry for managing services and instances
#[derive(Clone)]
pub struct NamingService {
    // Key: service_key (namespace@@group@@service), Value: map of instances
    services: Arc<DashMap<String, DashMap<String, Instance>>>,
    // Key: connection_id, Value: list of subscribed service keys
    subscribers: Arc<DashMap<String, Vec<String>>>,
}

impl NamingService {
    pub fn new() -> Self {
        Self {
            services: Arc::new(DashMap::new()),
            subscribers: Arc::new(DashMap::new()),
        }
    }

    // Register a service instance
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
        let instances = self
            .services
            .entry(service_key)
            .or_insert_with(DashMap::new);

        instances.insert(instance_key, instance);
        true
    }

    // Deregister a service instance
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

    // Get all instances for a service
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

    // Get service info with instances
    pub fn get_service(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster: &str,
        healthy_only: bool,
    ) -> Service {
        let instances = self.get_instances(namespace, group_name, service_name, cluster, healthy_only);
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

    // List all services in a namespace
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

    // Subscribe to a service
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
            .or_insert_with(Vec::new)
            .push(service_key);
    }

    // Unsubscribe from a service
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

    // Get subscribers for a service
    pub fn get_subscribers(&self, namespace: &str, group_name: &str, service_name: &str) -> Vec<String> {
        let service_key = build_service_key(namespace, group_name, service_name);

        self.subscribers
            .iter()
            .filter(|entry| entry.value().contains(&service_key))
            .map(|entry| entry.key().clone())
            .collect()
    }

    // Clean up subscriber when connection is closed
    pub fn remove_subscriber(&self, connection_id: &str) {
        self.subscribers.remove(connection_id);
    }

    // Batch register instances
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

    // Batch deregister instances
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
}

impl Default for NamingService {
    fn default() -> Self {
        Self::new()
    }
}
