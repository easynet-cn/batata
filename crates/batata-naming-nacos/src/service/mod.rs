//! In-memory Nacos naming service implementation
//!
//! DashMap-based concurrent service registry with:
//! - Instance CRUD with protection threshold
//! - Subscription and publisher tracking
//! - Connection-based instance lifecycle
//! - Cluster configuration management

mod instance;
mod metadata;
mod subscription;

use std::{collections::HashSet, sync::Arc};

use dashmap::DashMap;

use crate::model::{ClusterConfig, NacosInstance, ServiceMetadata};

/// Build service key: "namespace@@group@@service"
pub fn build_service_key(namespace: &str, group: &str, service: &str) -> String {
    let mut key = String::with_capacity(namespace.len() + group.len() + service.len() + 4);
    key.push_str(namespace);
    key.push_str("@@");
    key.push_str(group);
    key.push_str("@@");
    key.push_str(service);
    key
}

/// Parse a service key into (namespace, group, service)
pub fn parse_service_key(key: &str) -> Option<(&str, &str, &str)> {
    let mut parts = key.splitn(3, "@@");
    let ns = parts.next()?;
    let group = parts.next()?;
    let service = parts.next()?;
    Some((ns, group, service))
}

/// Build instance key: "ip#port#clusterName"
fn build_instance_key(instance: &NacosInstance) -> String {
    build_instance_key_parts(&instance.ip, instance.port, &instance.cluster_name)
}

/// Build instance key from parts
pub fn build_instance_key_parts(ip: &str, port: i32, cluster: &str) -> String {
    let mut key = String::with_capacity(ip.len() + 7 + cluster.len());
    key.push_str(ip);
    key.push('#');
    let _ = std::fmt::Write::write_fmt(&mut key, format_args!("{}", port));
    key.push('#');
    key.push_str(cluster);
    key
}

/// Build cluster config key: "service_key##clusterName"
fn build_cluster_key(service_key: &str, cluster: &str) -> String {
    format!("{service_key}##{cluster}")
}

/// In-memory Nacos naming service
///
/// Thread-safe concurrent registry using DashMap.
/// Instances are wrapped in Arc for cheap snapshots.
#[derive(Clone)]
pub struct NacosNamingServiceImpl {
    /// Key: service_key (namespace@@group@@service), Value: map of instances
    services: Arc<DashMap<String, DashMap<String, Arc<NacosInstance>>>>,
    /// Key: service_key, Value: service-level metadata
    service_metadata: Arc<DashMap<String, ServiceMetadata>>,
    /// Key: service_key##cluster_name, Value: cluster configuration
    cluster_configs: Arc<DashMap<String, ClusterConfig>>,
    /// Key: connection_id, Value: set of subscribed service keys
    subscribers: Arc<DashMap<String, HashSet<String>>>,
    /// Reverse index: service_key -> set of subscriber connection_ids
    subscriber_index: Arc<DashMap<String, HashSet<String>>>,
    /// Key: connection_id, Value: set of published service keys
    publishers: Arc<DashMap<String, HashSet<String>>>,
    /// Key: connection_id, Value: set of (service_key, instance_key)
    connection_instances: Arc<DashMap<String, HashSet<(String, String)>>>,
    /// Connections currently being deregistered (prevents races)
    closing_connections: Arc<DashMap<String, ()>>,
}

impl NacosNamingServiceImpl {
    pub fn new() -> Self {
        Self {
            services: Arc::new(DashMap::new()),
            service_metadata: Arc::new(DashMap::new()),
            cluster_configs: Arc::new(DashMap::new()),
            subscribers: Arc::new(DashMap::new()),
            subscriber_index: Arc::new(DashMap::new()),
            publishers: Arc::new(DashMap::new()),
            connection_instances: Arc::new(DashMap::new()),
            closing_connections: Arc::new(DashMap::new()),
        }
    }

    /// Increment revision counter for change detection
    fn increment_revision(&self, service_key: &str) {
        if let Some(mut meta) = self.service_metadata.get_mut(service_key) {
            meta.revision = meta.revision.wrapping_add(1);
        }
    }

    /// Get current revision for a service (for Distro verify)
    pub fn get_revision(&self, service_key: &str) -> u64 {
        self.service_metadata
            .get(service_key)
            .map(|m| m.revision)
            .unwrap_or(0)
    }
}

impl Default for NacosNamingServiceImpl {
    fn default() -> Self {
        Self::new()
    }
}
