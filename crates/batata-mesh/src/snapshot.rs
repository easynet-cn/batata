//! xDS Resource Snapshot Management
//!
//! This module provides a snapshot-based cache for xDS resources,
//! enabling efficient resource management and versioning.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use dashmap::DashMap;
use tracing::{debug, info};

use crate::xds::ResourceType;
use crate::xds::types::{Cluster, ClusterLoadAssignment, Listener, RouteConfiguration};

/// Version generator for resource snapshots
static VERSION_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Generate a new unique version string
fn generate_version() -> String {
    let version = VERSION_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("{}", version)
}

/// A snapshot of xDS resources at a point in time
#[derive(Debug, Clone)]
pub struct ResourceSnapshot {
    /// Snapshot version
    pub version: String,
    /// Cluster resources (CDS)
    pub clusters: HashMap<String, Cluster>,
    /// Endpoint resources (EDS)
    pub endpoints: HashMap<String, ClusterLoadAssignment>,
    /// Listener resources (LDS)
    pub listeners: HashMap<String, Listener>,
    /// Route resources (RDS)
    pub routes: HashMap<String, RouteConfiguration>,
    /// Creation timestamp
    pub created_at: i64,
}

impl ResourceSnapshot {
    /// Create a new empty snapshot
    pub fn new() -> Self {
        Self {
            version: generate_version(),
            clusters: HashMap::new(),
            endpoints: HashMap::new(),
            listeners: HashMap::new(),
            routes: HashMap::new(),
            created_at: chrono::Utc::now().timestamp_millis(),
        }
    }

    /// Create a snapshot with clusters and endpoints
    pub fn with_resources(clusters: Vec<Cluster>, endpoints: Vec<ClusterLoadAssignment>) -> Self {
        let mut snapshot = Self::new();

        for cluster in clusters {
            snapshot.clusters.insert(cluster.name.clone(), cluster);
        }

        for cla in endpoints {
            snapshot.endpoints.insert(cla.cluster_name.clone(), cla);
        }

        snapshot
    }

    /// Create a snapshot with all resource types
    pub fn with_all_resources(
        clusters: Vec<Cluster>,
        endpoints: Vec<ClusterLoadAssignment>,
        listeners: Vec<Listener>,
        routes: Vec<RouteConfiguration>,
    ) -> Self {
        let mut snapshot = Self::with_resources(clusters, endpoints);

        for listener in listeners {
            snapshot.listeners.insert(listener.name.clone(), listener);
        }

        for route in routes {
            snapshot.routes.insert(route.name.clone(), route);
        }

        snapshot
    }

    /// Get version for a specific resource type
    pub fn version_for(&self, _resource_type: ResourceType) -> &str {
        &self.version
    }

    /// Get cluster by name
    pub fn get_cluster(&self, name: &str) -> Option<&Cluster> {
        self.clusters.get(name)
    }

    /// Get endpoints by cluster name
    pub fn get_endpoints(&self, cluster_name: &str) -> Option<&ClusterLoadAssignment> {
        self.endpoints.get(cluster_name)
    }

    /// Get listener by name
    pub fn get_listener(&self, name: &str) -> Option<&Listener> {
        self.listeners.get(name)
    }

    /// Get route configuration by name
    pub fn get_route(&self, name: &str) -> Option<&RouteConfiguration> {
        self.routes.get(name)
    }

    /// Get all cluster names
    pub fn cluster_names(&self) -> Vec<String> {
        self.clusters.keys().cloned().collect()
    }

    /// Get all endpoint cluster names
    pub fn endpoint_cluster_names(&self) -> Vec<String> {
        self.endpoints.keys().cloned().collect()
    }

    /// Get all listener names
    pub fn listener_names(&self) -> Vec<String> {
        self.listeners.keys().cloned().collect()
    }

    /// Get all route configuration names
    pub fn route_names(&self) -> Vec<String> {
        self.routes.keys().cloned().collect()
    }

    /// Check if snapshot contains a specific resource
    pub fn contains(&self, resource_type: ResourceType, name: &str) -> bool {
        match resource_type {
            ResourceType::Cluster => self.clusters.contains_key(name),
            ResourceType::Endpoint => self.endpoints.contains_key(name),
            ResourceType::Listener => self.listeners.contains_key(name),
            ResourceType::Route => self.routes.contains_key(name),
            _ => false,
        }
    }

    /// Get resource count for a type
    pub fn resource_count(&self, resource_type: ResourceType) -> usize {
        match resource_type {
            ResourceType::Cluster => self.clusters.len(),
            ResourceType::Endpoint => self.endpoints.len(),
            ResourceType::Listener => self.listeners.len(),
            ResourceType::Route => self.routes.len(),
            _ => 0,
        }
    }

    /// Add a cluster to the snapshot
    pub fn add_cluster(&mut self, cluster: Cluster) {
        self.clusters.insert(cluster.name.clone(), cluster);
    }

    /// Add endpoints to the snapshot
    pub fn add_endpoints(&mut self, cla: ClusterLoadAssignment) {
        self.endpoints.insert(cla.cluster_name.clone(), cla);
    }

    /// Add a listener to the snapshot
    pub fn add_listener(&mut self, listener: Listener) {
        self.listeners.insert(listener.name.clone(), listener);
    }

    /// Add a route configuration to the snapshot
    pub fn add_route(&mut self, route: RouteConfiguration) {
        self.routes.insert(route.name.clone(), route);
    }
}

impl Default for ResourceSnapshot {
    fn default() -> Self {
        Self::new()
    }
}

/// Snapshot cache for xDS resources
///
/// Maintains snapshots per node/client and supports efficient lookups.
pub struct SnapshotCache {
    /// Snapshots keyed by node ID
    snapshots: DashMap<String, Arc<ResourceSnapshot>>,
    /// Default snapshot for nodes without specific configuration
    default_snapshot: parking_lot::RwLock<Option<Arc<ResourceSnapshot>>>,
}

impl SnapshotCache {
    /// Create a new snapshot cache
    pub fn new() -> Self {
        Self {
            snapshots: DashMap::new(),
            default_snapshot: parking_lot::RwLock::new(None),
        }
    }

    /// Set the default snapshot
    pub fn set_default_snapshot(&self, snapshot: ResourceSnapshot) {
        let mut default = self.default_snapshot.write();
        info!(
            version = %snapshot.version,
            clusters = snapshot.clusters.len(),
            endpoints = snapshot.endpoints.len(),
            "Setting default xDS snapshot"
        );
        *default = Some(Arc::new(snapshot));
    }

    /// Set a snapshot for a specific node
    pub fn set_snapshot(&self, node_id: &str, snapshot: ResourceSnapshot) {
        debug!(
            node_id = %node_id,
            version = %snapshot.version,
            "Setting xDS snapshot for node"
        );
        self.snapshots
            .insert(node_id.to_string(), Arc::new(snapshot));
    }

    /// Get snapshot for a node (falls back to default)
    pub fn get_snapshot(&self, node_id: &str) -> Option<Arc<ResourceSnapshot>> {
        // Try node-specific snapshot first
        if let Some(snapshot) = self.snapshots.get(node_id) {
            return Some(snapshot.clone());
        }

        // Fall back to default
        self.default_snapshot.read().clone()
    }

    /// Remove snapshot for a node
    pub fn remove_snapshot(&self, node_id: &str) {
        self.snapshots.remove(node_id);
    }

    /// Clear all snapshots
    pub fn clear(&self) {
        self.snapshots.clear();
        *self.default_snapshot.write() = None;
    }

    /// Get all node IDs with snapshots
    pub fn node_ids(&self) -> Vec<String> {
        self.snapshots.iter().map(|e| e.key().clone()).collect()
    }

    /// Check if cache has a snapshot for a node
    pub fn has_snapshot(&self, node_id: &str) -> bool {
        self.snapshots.contains_key(node_id)
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        let default = self.default_snapshot.read();
        CacheStats {
            node_count: self.snapshots.len(),
            has_default: default.is_some(),
            default_version: default.as_ref().map(|s| s.version.clone()),
        }
    }
}

impl Default for SnapshotCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Number of nodes with snapshots
    pub node_count: usize,
    /// Whether a default snapshot is set
    pub has_default: bool,
    /// Version of the default snapshot
    pub default_version: Option<String>,
}

/// Subscription tracker for xDS clients
pub struct SubscriptionTracker {
    /// Active subscriptions: node_id -> (resource_type -> resource_names)
    subscriptions: DashMap<String, HashMap<ResourceType, Vec<String>>>,
    /// Last ACKed versions: node_id -> (resource_type -> version)
    acked_versions: DashMap<String, HashMap<ResourceType, String>>,
}

impl SubscriptionTracker {
    /// Create a new subscription tracker
    pub fn new() -> Self {
        Self {
            subscriptions: DashMap::new(),
            acked_versions: DashMap::new(),
        }
    }

    /// Update subscriptions for a node
    pub fn update_subscriptions(
        &self,
        node_id: &str,
        resource_type: ResourceType,
        resource_names: Vec<String>,
    ) {
        let mut entry = self.subscriptions.entry(node_id.to_string()).or_default();

        if resource_names.is_empty() {
            // Empty list means subscribe to all (wildcard)
            entry.remove(&resource_type);
        } else {
            entry.insert(resource_type, resource_names);
        }

        debug!(
            node_id = %node_id,
            resource_type = %resource_type,
            "Updated subscriptions"
        );
    }

    /// Record ACK for a resource type
    pub fn record_ack(&self, node_id: &str, resource_type: ResourceType, version: &str) {
        let mut entry = self.acked_versions.entry(node_id.to_string()).or_default();

        entry.insert(resource_type, version.to_string());
    }

    /// Get last ACKed version for a node and resource type
    pub fn get_acked_version(&self, node_id: &str, resource_type: ResourceType) -> Option<String> {
        self.acked_versions
            .get(node_id)
            .and_then(|entry| entry.get(&resource_type).cloned())
    }

    /// Check if node is subscribed to a resource
    pub fn is_subscribed(
        &self,
        node_id: &str,
        resource_type: ResourceType,
        resource_name: &str,
    ) -> bool {
        if let Some(subs) = self.subscriptions.get(node_id) {
            if let Some(names) = subs.get(&resource_type) {
                names.iter().any(|n| n == resource_name || n == "*")
            } else {
                // No specific subscription = wildcard
                true
            }
        } else {
            false
        }
    }

    /// Remove tracking for a node
    pub fn remove_node(&self, node_id: &str) {
        self.subscriptions.remove(node_id);
        self.acked_versions.remove(node_id);
    }

    /// Get all tracked node IDs
    pub fn node_ids(&self) -> Vec<String> {
        self.subscriptions.iter().map(|e| e.key().clone()).collect()
    }
}

impl Default for SubscriptionTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xds::types::LbPolicy;

    fn create_test_cluster(name: &str) -> Cluster {
        Cluster {
            name: name.to_string(),
            lb_policy: LbPolicy::RoundRobin,
            ..Default::default()
        }
    }

    fn create_test_cla(name: &str) -> ClusterLoadAssignment {
        ClusterLoadAssignment::new(name)
    }

    #[test]
    fn test_snapshot_creation() {
        let snapshot = ResourceSnapshot::with_resources(
            vec![
                create_test_cluster("cluster-1"),
                create_test_cluster("cluster-2"),
            ],
            vec![create_test_cla("cluster-1"), create_test_cla("cluster-2")],
        );

        assert_eq!(snapshot.clusters.len(), 2);
        assert_eq!(snapshot.endpoints.len(), 2);
        assert!(snapshot.get_cluster("cluster-1").is_some());
        assert!(snapshot.get_endpoints("cluster-1").is_some());
    }

    #[test]
    fn test_snapshot_cache() {
        let cache = SnapshotCache::new();

        // Set default snapshot
        let default_snapshot =
            ResourceSnapshot::with_resources(vec![create_test_cluster("default-cluster")], vec![]);
        cache.set_default_snapshot(default_snapshot);

        // Node without specific snapshot should get default
        let snapshot = cache.get_snapshot("node-1").unwrap();
        assert!(snapshot.get_cluster("default-cluster").is_some());

        // Set node-specific snapshot
        let node_snapshot =
            ResourceSnapshot::with_resources(vec![create_test_cluster("node-cluster")], vec![]);
        cache.set_snapshot("node-1", node_snapshot);

        // Node should now get its specific snapshot
        let snapshot = cache.get_snapshot("node-1").unwrap();
        assert!(snapshot.get_cluster("node-cluster").is_some());
        assert!(snapshot.get_cluster("default-cluster").is_none());
    }

    #[test]
    fn test_subscription_tracker() {
        let tracker = SubscriptionTracker::new();

        // Subscribe to specific resources
        tracker.update_subscriptions(
            "node-1",
            ResourceType::Cluster,
            vec!["cluster-1".to_string(), "cluster-2".to_string()],
        );

        assert!(tracker.is_subscribed("node-1", ResourceType::Cluster, "cluster-1"));
        assert!(tracker.is_subscribed("node-1", ResourceType::Cluster, "cluster-2"));
        assert!(!tracker.is_subscribed("node-1", ResourceType::Cluster, "cluster-3"));

        // Record ACK
        tracker.record_ack("node-1", ResourceType::Cluster, "v1");
        assert_eq!(
            tracker.get_acked_version("node-1", ResourceType::Cluster),
            Some("v1".to_string())
        );
    }

    #[test]
    fn test_version_generation() {
        let v1 = generate_version();
        let v2 = generate_version();
        assert_ne!(v1, v2);
    }
}
