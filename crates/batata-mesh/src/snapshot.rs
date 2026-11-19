//! xDS Resource Snapshot Management
//!
//! This module provides a snapshot-based cache for xDS resources,
//! enabling efficient resource management and versioning.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use dashmap::DashMap;
use tracing::{debug, info};

use crate::xds::ResourceType;
use crate::xds::types::{Cluster, ClusterLoadAssignment, Listener, RouteConfiguration, SdsSecret};

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
    /// Secret resources (SDS)
    pub secrets: HashMap<String, SdsSecret>,
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
            secrets: HashMap::new(),
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

    /// Add SDS secrets to this snapshot. Other resource types are untouched.
    pub fn with_secrets(mut self, secrets: Vec<SdsSecret>) -> Self {
        for secret in secrets {
            self.add_secret(secret);
        }
        self
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

    /// Get secret by name
    pub fn get_secret(&self, name: &str) -> Option<&SdsSecret> {
        self.secrets.get(name)
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

    /// Get all secret names
    pub fn secret_names(&self) -> Vec<String> {
        self.secrets.keys().cloned().collect()
    }

    /// Check if snapshot contains a specific resource
    pub fn contains(&self, resource_type: ResourceType, name: &str) -> bool {
        match resource_type {
            ResourceType::Cluster => self.clusters.contains_key(name),
            ResourceType::Endpoint => self.endpoints.contains_key(name),
            ResourceType::Listener => self.listeners.contains_key(name),
            ResourceType::Route => self.routes.contains_key(name),
            ResourceType::Secret => self.secrets.contains_key(name),
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
            ResourceType::Secret => self.secrets.len(),
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

    /// Add a secret to the snapshot
    pub fn add_secret(&mut self, secret: SdsSecret) {
        self.secrets.insert(secret.name.clone(), secret);
    }
}

impl Default for ResourceSnapshot {
    fn default() -> Self {
        Self::new()
    }
}

/// Per-client Delta xDS state for tracking acknowledged resource versions.
///
/// Each client maintains its own view of resource state, enabling
/// efficient incremental updates (only send changed resources).
#[derive(Debug, Clone, Default)]
pub struct DeltaClientState {
    /// Last version known to the client per resource name: resource_name -> version.
    ///
    /// Versions are advanced when the server sends a resource and rolled back
    /// when the matching nonce is NACKed.
    pub acked_versions: HashMap<String, String>,
    /// Resources explicitly subscribed to on this stream.
    pub subscribed_resources: HashSet<String>,
    /// Whether this client currently has a wildcard (`*`) subscription.
    pub wildcard: bool,
    /// Explicitly subscribed names that must be sent on the next response even
    /// when the client already advertises the current version.
    pub force_send: HashSet<String>,
    /// An explicit wildcard subscription must resend every current resource.
    pub force_all: bool,
    /// Whether a real client request has initialized this type's state. Server
    /// pushes must not create or answer a subscription the client never made.
    pub initialized: bool,
    /// Nonce of the most recent response sent to this client for this type.
    pub last_sent_nonce: String,
    /// Versions before the most recent response, used to roll back on NACK.
    pub previous_versions: Option<HashMap<String, String>>,
}

impl DeltaClientState {
    /// Create a new, empty delta client state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that the client acknowledged a resource at a version
    pub fn ack_resource(&mut self, name: &str, version: &str) {
        self.acked_versions
            .insert(name.to_string(), version.to_string());
    }

    /// Add resources to the tracked set.
    ///
    /// Delta xDS treats an empty subscribe list as "no change"; legacy
    /// wildcard is handled explicitly by [`Self::mark_legacy_wildcard`].
    pub fn subscribe(&mut self, names: &[String]) {
        for name in names {
            if name == "*" {
                self.wildcard = true;
                self.force_all = true;
                continue;
            }
            self.subscribed_resources.insert(name.clone());
            // Upstream requires a resend even if the advertised version is
            // current, because the client may have dropped its local copy.
            self.force_send.insert(name.clone());
        }
    }

    /// Remove resources from the tracked set.
    ///
    /// Unsubscribing from `*` only drops the wildcard; specific subscriptions
    /// remain. When a specific name is dropped while the wildcard is still
    /// active, it is forced on the next response so the client can tell
    /// whether the resource is still covered by the wildcard.
    pub fn unsubscribe(&mut self, names: &[String]) {
        for name in names {
            if name == "*" {
                self.wildcard = false;
                self.force_all = false;
                continue;
            }
            self.subscribed_resources.remove(name);
            self.force_send.remove(name);
            if self.wildcard {
                // The client may still cache the resource via the wildcard;
                // keep its version and force one clarifying resend.
                self.force_send.insert(name.clone());
            } else {
                self.acked_versions.remove(name);
            }
        }
    }

    /// Apply the upstream legacy wildcard rule for the first request of a type:
    /// an empty subscribe list and an empty unsubscribe list means `*`.
    pub fn mark_legacy_wildcard(&mut self) {
        self.wildcard = true;
        self.force_all = true;
    }

    /// Seed versions reported by the client on stream (re)connect.
    pub fn seed_versions(&mut self, versions: &HashMap<String, String>) {
        for (name, version) in versions {
            self.acked_versions
                .entry(name.clone())
                .or_insert_with(|| version.clone());
        }
    }

    /// Record a response as sent and snapshot the versions it supersedes.
    pub fn prepare_send(&mut self, nonce: &str, sent_versions: &HashMap<String, String>) {
        self.previous_versions = Some(self.acked_versions.clone());
        for (name, version) in sent_versions {
            self.acked_versions.insert(name.clone(), version.clone());
        }
        self.last_sent_nonce = nonce.to_string();
    }

    /// Accept an ACK for the most recent nonce.
    pub fn record_ack(&mut self, nonce: &str) -> bool {
        if self.last_sent_nonce.is_empty() || self.last_sent_nonce != nonce {
            return false;
        }
        self.previous_versions = None;
        true
    }

    /// Roll back to pre-response versions when the matching response was NACKed.
    pub fn record_nack(&mut self, nonce: &str) -> bool {
        if self.last_sent_nonce.is_empty() || self.last_sent_nonce != nonce {
            return false;
        }
        if let Some(previous) = self.previous_versions.take() {
            self.acked_versions = previous;
        }
        true
    }

    /// Clear one-shot resend markers after a response has been built.
    pub fn clear_forced(&mut self) {
        self.force_send.clear();
        self.force_all = false;
    }

    /// Forget resources reported as removed.
    pub fn forget(&mut self, names: &[String]) {
        for name in names {
            self.acked_versions.remove(name);
            self.subscribed_resources.remove(name);
            self.force_send.remove(name);
        }
    }

    /// Check if a resource needs to be sent to this client.
    /// Returns true if the resource is subscribed and either:
    /// - Client hasn't acked it yet, or
    /// - Client's acked version differs from current version
    pub fn needs_update(&self, name: &str, current_version: &str) -> bool {
        if !self.wildcard && !self.subscribed_resources.contains(name) {
            return false;
        }
        if self.force_all || self.force_send.contains(name) {
            return true;
        }
        match self.acked_versions.get(name) {
            Some(acked) => acked != current_version,
            None => true, // Never acked, needs sending
        }
    }

    /// Get list of resources that were previously acked but are no longer
    /// in the current snapshot (i.e., deleted resources)
    pub fn detect_removals(&self, current_names: &HashSet<String>) -> Vec<String> {
        self.acked_versions
            .keys()
            .filter(|name| {
                !current_names.contains(name.as_str())
                    && (self.wildcard || self.subscribed_resources.contains(name.as_str()))
            })
            .cloned()
            .collect()
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
    /// Delta client states: (node_id, resource_type) -> DeltaClientState.
    ///
    /// Delta versions and subscriptions are walled off per `type_url`, just as
    /// upstream ADS requires.
    delta_states: DashMap<(String, ResourceType), DeltaClientState>,
}

impl SnapshotCache {
    /// Create a new snapshot cache
    pub fn new() -> Self {
        Self {
            snapshots: DashMap::new(),
            default_snapshot: parking_lot::RwLock::new(None),
            delta_states: DashMap::new(),
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

    /// Whether this type already has a delta state on the stream.
    pub fn has_delta_state(&self, node_id: &str, resource_type: ResourceType) -> bool {
        self.delta_states
            .contains_key(&(node_id.to_string(), resource_type))
    }

    /// Whether a real client request has initialized this type's state.
    pub fn is_delta_state_initialized(&self, node_id: &str, resource_type: ResourceType) -> bool {
        self.delta_states
            .get(&(node_id.to_string(), resource_type))
            .map(|state| state.initialized)
            .unwrap_or(false)
    }

    /// Get or create delta state for a node and resource type.
    pub fn get_or_create_delta_state(
        &self,
        node_id: &str,
        resource_type: ResourceType,
    ) -> DeltaClientState {
        self.delta_states
            .entry((node_id.to_string(), resource_type))
            .or_default()
            .clone()
    }

    /// Update delta state for a node and resource type.
    pub fn update_delta_state(
        &self,
        node_id: &str,
        resource_type: ResourceType,
        state: DeltaClientState,
    ) {
        self.delta_states
            .insert((node_id.to_string(), resource_type), state);
    }

    /// Remove delta state for a node and resource type.
    pub fn remove_delta_state(&self, node_id: &str, resource_type: ResourceType) {
        self.delta_states
            .remove(&(node_id.to_string(), resource_type));
    }

    /// Mutate a node/type delta state atomically.
    pub fn with_delta_state<R>(
        &self,
        node_id: &str,
        resource_type: ResourceType,
        f: impl FnOnce(&mut DeltaClientState) -> R,
    ) -> R {
        let mut state = self
            .delta_states
            .entry((node_id.to_string(), resource_type))
            .or_default();
        f(&mut state)
    }

    /// Remove all delta states belonging to a node when its stream closes.
    pub fn remove_delta_states_for_node(&self, node_id: &str) {
        self.delta_states
            .retain(|(stored_node, _), _| stored_node != node_id);
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
    use crate::xds::types::{LbPolicy, SdsSecret, SdsSecretKind};

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
    fn test_snapshot_stores_and_counts_secrets() {
        let leaf = SdsSecret::tls_certificate("server-cert", "CERT", "KEY");
        let ca = SdsSecret::validation_context("root-ca", "CA");

        let mut snapshot = ResourceSnapshot::with_all_resources(vec![], vec![], vec![], vec![])
            .with_secrets(vec![leaf, ca]);
        assert_eq!(snapshot.secrets.len(), 2);
        assert_eq!(snapshot.resource_count(ResourceType::Secret), 2);
        assert!(snapshot.contains(ResourceType::Secret, "server-cert"));
        assert!(snapshot.get_secret("root-ca").is_some());
        assert!(snapshot.get_secret("missing").is_none());
        assert_eq!(snapshot.secret_names().len(), 2);
        assert_eq!(
            snapshot.get_secret("server-cert").unwrap().kind,
            SdsSecretKind::TlsCertificate {
                certificate_chain: "CERT".to_string(),
                private_key: "KEY".to_string(),
            }
        );

        // with_all_resources leaves secrets empty unless explicitly supplied.
        let empty = ResourceSnapshot::with_all_resources(vec![], vec![], vec![], vec![]);
        assert!(empty.secrets.is_empty());
        assert_eq!(empty.resource_count(ResourceType::Secret), 0);

        snapshot.add_secret(SdsSecret::validation_context("new-ca", "CA2"));
        assert_eq!(snapshot.resource_count(ResourceType::Secret), 3);
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

    #[test]
    fn test_delta_client_state_new() {
        let state = DeltaClientState::new();
        assert!(state.acked_versions.is_empty());
        assert!(state.subscribed_resources.is_empty());
        assert!(!state.wildcard);
    }

    #[test]
    fn test_delta_client_state_ack_resource() {
        let mut state = DeltaClientState::new();
        state.ack_resource("cluster-1", "v1");
        state.ack_resource("cluster-2", "v2");

        assert_eq!(
            state.acked_versions.get("cluster-1"),
            Some(&"v1".to_string())
        );
        assert_eq!(
            state.acked_versions.get("cluster-2"),
            Some(&"v2".to_string())
        );

        // Overwrite version
        state.ack_resource("cluster-1", "v3");
        assert_eq!(
            state.acked_versions.get("cluster-1"),
            Some(&"v3".to_string())
        );
    }

    #[test]
    fn test_delta_client_state_subscribe_specific() {
        let mut state = DeltaClientState::new();
        state.subscribe(&["cluster-1".to_string(), "cluster-2".to_string()]);

        assert!(!state.wildcard);
        assert!(state.subscribed_resources.contains("cluster-1"));
        assert!(state.subscribed_resources.contains("cluster-2"));
        assert!(!state.subscribed_resources.contains("cluster-3"));
    }

    #[test]
    fn test_delta_client_state_subscribe_wildcard() {
        let mut state = DeltaClientState::new();
        state.subscribe(&["*".to_string()]);

        assert!(state.wildcard);
        assert!(state.force_all);
    }

    #[test]
    fn test_delta_client_state_empty_subscribe_is_no_change() {
        let mut state = DeltaClientState::new();
        state.subscribe(&[]);

        assert!(!state.wildcard);
        assert!(state.subscribed_resources.is_empty());
    }

    #[test]
    fn test_delta_client_state_unsubscribe() {
        let mut state = DeltaClientState::new();
        state.subscribe(&["cluster-1".to_string(), "cluster-2".to_string()]);
        state.ack_resource("cluster-1", "v1");
        state.ack_resource("cluster-2", "v2");

        state.unsubscribe(&["cluster-1".to_string()]);

        assert!(!state.subscribed_resources.contains("cluster-1"));
        assert!(state.subscribed_resources.contains("cluster-2"));
        // Acked version for unsubscribed resource should also be removed
        assert!(!state.acked_versions.contains_key("cluster-1"));
        assert!(state.acked_versions.contains_key("cluster-2"));
    }

    #[test]
    fn test_delta_client_state_needs_update_specific_subscription() {
        let mut state = DeltaClientState::new();
        state.subscribe(&["cluster-1".to_string(), "cluster-2".to_string()]);
        state.clear_forced();

        // Never acked - needs update
        assert!(state.needs_update("cluster-1", "v1"));

        // Acked same version - no update needed
        state.ack_resource("cluster-1", "v1");
        assert!(!state.needs_update("cluster-1", "v1"));

        // Acked different version - needs update
        assert!(state.needs_update("cluster-1", "v2"));

        // Not subscribed - no update needed
        assert!(!state.needs_update("cluster-3", "v1"));
    }

    #[test]
    fn test_delta_client_state_needs_update_wildcard() {
        let mut state = DeltaClientState::new();
        state.mark_legacy_wildcard();
        state.clear_forced();

        // Wildcard should accept any resource
        assert!(state.needs_update("cluster-1", "v1"));
        assert!(state.needs_update("cluster-99", "v1"));

        state.ack_resource("cluster-1", "v1");
        assert!(!state.needs_update("cluster-1", "v1"));
        assert!(state.needs_update("cluster-1", "v2"));
    }

    #[test]
    fn test_delta_client_state_detect_removals() {
        let mut state = DeltaClientState::new();
        state.subscribe(&[
            "cluster-1".to_string(),
            "cluster-2".to_string(),
            "cluster-3".to_string(),
        ]);
        state.ack_resource("cluster-1", "v1");
        state.ack_resource("cluster-2", "v2");
        state.ack_resource("cluster-3", "v3");

        // cluster-2 was removed from the snapshot
        let current_names: HashSet<String> = vec!["cluster-1".to_string(), "cluster-3".to_string()]
            .into_iter()
            .collect();

        let removals = state.detect_removals(&current_names);
        assert_eq!(removals.len(), 1);
        assert!(removals.contains(&"cluster-2".to_string()));
    }

    #[test]
    fn test_delta_client_state_detect_removals_wildcard() {
        let mut state = DeltaClientState::new();
        state.mark_legacy_wildcard();
        state.ack_resource("cluster-1", "v1");
        state.ack_resource("cluster-2", "v2");

        let current_names: HashSet<String> = vec!["cluster-1".to_string()].into_iter().collect();

        let removals = state.detect_removals(&current_names);
        assert_eq!(removals.len(), 1);
        assert!(removals.contains(&"cluster-2".to_string()));
    }

    #[test]
    fn test_delta_client_state_detect_removals_unsubscribed_ignored() {
        let mut state = DeltaClientState::new();
        state.subscribe(&["cluster-1".to_string()]);
        // Acked cluster-2 but then unsubscribed (shouldn't happen normally,
        // but test that only subscribed resources are reported)
        state
            .acked_versions
            .insert("cluster-2".to_string(), "v2".to_string());

        let current_names: HashSet<String> = vec!["cluster-1".to_string()].into_iter().collect();

        let removals = state.detect_removals(&current_names);
        // cluster-2 is not in subscribed_resources and not wildcard, so should be ignored
        assert!(removals.is_empty());
    }

    #[test]
    fn test_snapshot_cache_delta_state_is_isolated_per_type() {
        let cache = SnapshotCache::new();

        let mut cluster_state = DeltaClientState::new();
        cluster_state.ack_resource("cluster-1", "v1");
        cache.update_delta_state("node-1", ResourceType::Cluster, cluster_state);

        let mut route_state = DeltaClientState::new();
        route_state.ack_resource("route-1", "v2");
        cache.update_delta_state("node-1", ResourceType::Route, route_state);

        let cluster = cache.get_or_create_delta_state("node-1", ResourceType::Cluster);
        assert_eq!(
            cluster.acked_versions.get("cluster-1").map(String::as_str),
            Some("v1")
        );
        assert!(!cluster.acked_versions.contains_key("route-1"));

        let route = cache.get_or_create_delta_state("node-1", ResourceType::Route);
        assert_eq!(
            route.acked_versions.get("route-1").map(String::as_str),
            Some("v2")
        );
        assert!(!route.acked_versions.contains_key("cluster-1"));
    }

    #[test]
    fn test_snapshot_cache_delta_state() {
        let cache = SnapshotCache::new();
        let resource_type = ResourceType::Cluster;

        // Get or create returns default
        assert!(!cache.has_delta_state("node-1", resource_type));
        let state = cache.get_or_create_delta_state("node-1", resource_type);
        assert!(state.acked_versions.is_empty());
        assert!(!state.wildcard);

        // Update state
        let mut updated = DeltaClientState::new();
        updated.subscribe(&["cluster-1".to_string()]);
        updated.ack_resource("cluster-1", "v1");
        cache.update_delta_state("node-1", resource_type, updated);

        // Retrieve updated state
        let state = cache.get_or_create_delta_state("node-1", resource_type);
        assert!(state.subscribed_resources.contains("cluster-1"));
        assert_eq!(
            state.acked_versions.get("cluster-1"),
            Some(&"v1".to_string())
        );

        // States are walled off per type.
        assert!(!cache.has_delta_state("node-1", ResourceType::Route));

        // Remove state
        cache.remove_delta_state("node-1", resource_type);
        let state = cache.get_or_create_delta_state("node-1", resource_type);
        assert!(state.acked_versions.is_empty());

        // All states for a closed stream can be dropped at once.
        cache.update_delta_state("node-1", ResourceType::Cluster, DeltaClientState::new());
        cache.update_delta_state("node-1", ResourceType::Route, DeltaClientState::new());
        cache.remove_delta_states_for_node("node-1");
        assert!(!cache.has_delta_state("node-1", ResourceType::Cluster));
        assert!(!cache.has_delta_state("node-1", ResourceType::Route));
    }
}
