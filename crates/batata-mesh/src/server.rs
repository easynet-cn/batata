//! xDS Server Implementation
//!
//! This module provides the gRPC server implementation for xDS protocols,
//! including ADS (Aggregated Discovery Service).

use std::sync::Arc;

use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use crate::snapshot::{ResourceSnapshot, SnapshotCache, SubscriptionTracker};
use crate::xds::ResourceType;
use crate::xds::types::Node;

/// xDS server configuration
#[derive(Debug, Clone)]
pub struct XdsServerConfig {
    /// Server identifier
    pub server_id: String,
    /// Maximum concurrent streams per connection
    pub max_concurrent_streams: u32,
    /// Response timeout in milliseconds
    pub response_timeout_ms: u64,
}

impl Default for XdsServerConfig {
    fn default() -> Self {
        Self {
            server_id: "batata-xds-server".to_string(),
            max_concurrent_streams: 1000,
            response_timeout_ms: 5000,
        }
    }
}

/// xDS Server
///
/// Manages xDS resource distribution to Envoy/Istio clients.
pub struct XdsServer {
    /// Server configuration
    config: XdsServerConfig,
    /// Resource snapshot cache
    snapshot_cache: Arc<SnapshotCache>,
    /// Subscription tracker
    subscription_tracker: Arc<SubscriptionTracker>,
    /// Active streams (node_id -> sender)
    active_streams: dashmap::DashMap<String, StreamSender>,
}

type StreamSender = mpsc::Sender<Result<DiscoveryResponse, tonic::Status>>;

/// Discovery request (simplified)
#[derive(Debug, Clone)]
pub struct DiscoveryRequest {
    /// Version info from last response (empty on first request)
    pub version_info: String,
    /// Node identifier
    pub node: Option<Node>,
    /// Requested resource names (empty = all)
    pub resource_names: Vec<String>,
    /// Resource type URL
    pub type_url: String,
    /// Response nonce for ACK/NACK
    pub response_nonce: String,
    /// Error detail if this is a NACK
    pub error_detail: Option<String>,
}

/// Discovery response (simplified)
#[derive(Debug, Clone)]
pub struct DiscoveryResponse {
    /// Version info for these resources
    pub version_info: String,
    /// Resources (serialized as Any)
    pub resources: Vec<ResourceData>,
    /// Resource type URL
    pub type_url: String,
    /// Nonce for ACK tracking
    pub nonce: String,
    /// Control plane identifier
    pub control_plane_id: String,
}

/// Resource data wrapper
#[derive(Debug, Clone)]
pub struct ResourceData {
    /// Resource name
    pub name: String,
    /// Resource type
    pub resource_type: ResourceType,
    /// Serialized resource data (JSON for now)
    pub data: Vec<u8>,
}

impl XdsServer {
    /// Create a new xDS server
    pub fn new(config: XdsServerConfig) -> Self {
        info!(
            server_id = %config.server_id,
            "Creating xDS server"
        );
        Self {
            config,
            snapshot_cache: Arc::new(SnapshotCache::new()),
            subscription_tracker: Arc::new(SubscriptionTracker::new()),
            active_streams: dashmap::DashMap::new(),
        }
    }

    /// Get the snapshot cache
    pub fn snapshot_cache(&self) -> Arc<SnapshotCache> {
        self.snapshot_cache.clone()
    }

    /// Get the subscription tracker
    pub fn subscription_tracker(&self) -> Arc<SubscriptionTracker> {
        self.subscription_tracker.clone()
    }

    /// Update the default snapshot
    pub fn update_snapshot(&self, snapshot: ResourceSnapshot) {
        self.snapshot_cache.set_default_snapshot(snapshot);
        // Notify all connected clients
        self.notify_all_clients();
    }

    /// Update snapshot for a specific node
    pub fn update_node_snapshot(&self, node_id: &str, snapshot: ResourceSnapshot) {
        self.snapshot_cache.set_snapshot(node_id, snapshot);
        // Notify this specific client
        self.notify_client(node_id);
    }

    /// Notify all connected clients of updates
    fn notify_all_clients(&self) {
        let node_ids: Vec<String> = self
            .active_streams
            .iter()
            .map(|e| e.key().clone())
            .collect();
        for node_id in node_ids {
            self.notify_client(&node_id);
        }
    }

    /// Notify a specific client of updates
    fn notify_client(&self, node_id: &str) {
        if let Some(sender) = self.active_streams.get(node_id) {
            // Get snapshot for this node
            if let Some(snapshot) = self.snapshot_cache.get_snapshot(node_id) {
                // Send CDS updates
                if let Err(e) = self.send_cluster_response(node_id, &snapshot, &sender) {
                    warn!(node_id = %node_id, error = %e, "Failed to send CDS update");
                }
                // Send EDS updates
                if let Err(e) = self.send_endpoint_response(node_id, &snapshot, &sender) {
                    warn!(node_id = %node_id, error = %e, "Failed to send EDS update");
                }
            }
        }
    }

    /// Send cluster (CDS) response
    fn send_cluster_response(
        &self,
        node_id: &str,
        snapshot: &ResourceSnapshot,
        sender: &StreamSender,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let resources: Vec<ResourceData> = snapshot
            .clusters
            .iter()
            .map(|(name, cluster)| ResourceData {
                name: name.clone(),
                resource_type: ResourceType::Cluster,
                data: serde_json::to_vec(cluster).unwrap_or_default(),
            })
            .collect();

        if resources.is_empty() {
            return Ok(());
        }

        let response = DiscoveryResponse {
            version_info: snapshot.version.clone(),
            resources,
            type_url: ResourceType::Cluster.type_url().to_string(),
            nonce: uuid::Uuid::new_v4().to_string(),
            control_plane_id: self.config.server_id.clone(),
        };

        sender.try_send(Ok(response))?;
        debug!(node_id = %node_id, "Sent CDS response");
        Ok(())
    }

    /// Send endpoint (EDS) response
    fn send_endpoint_response(
        &self,
        node_id: &str,
        snapshot: &ResourceSnapshot,
        sender: &StreamSender,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let resources: Vec<ResourceData> = snapshot
            .endpoints
            .iter()
            .map(|(name, cla)| ResourceData {
                name: name.clone(),
                resource_type: ResourceType::Endpoint,
                data: serde_json::to_vec(cla).unwrap_or_default(),
            })
            .collect();

        if resources.is_empty() {
            return Ok(());
        }

        let response = DiscoveryResponse {
            version_info: snapshot.version.clone(),
            resources,
            type_url: ResourceType::Endpoint.type_url().to_string(),
            nonce: uuid::Uuid::new_v4().to_string(),
            control_plane_id: self.config.server_id.clone(),
        };

        sender.try_send(Ok(response))?;
        debug!(node_id = %node_id, "Sent EDS response");
        Ok(())
    }

    /// Handle incoming discovery request
    pub async fn handle_request(&self, request: DiscoveryRequest) -> Option<DiscoveryResponse> {
        let node_id = request
            .node
            .as_ref()
            .map(|n| n.id.clone())
            .unwrap_or_else(|| "unknown".to_string());

        let resource_type = ResourceType::from_type_url(&request.type_url)?;

        debug!(
            node_id = %node_id,
            resource_type = %resource_type,
            version = %request.version_info,
            "Handling discovery request"
        );

        // Check if this is a NACK
        if let Some(error) = &request.error_detail {
            warn!(
                node_id = %node_id,
                resource_type = %resource_type,
                error = %error,
                "Received NACK from client"
            );
            // For now, just log and continue
        }

        // Update subscriptions
        self.subscription_tracker.update_subscriptions(
            &node_id,
            resource_type,
            request.resource_names.clone(),
        );

        // Record ACK if version is not empty
        if !request.version_info.is_empty() && !request.response_nonce.is_empty() {
            self.subscription_tracker
                .record_ack(&node_id, resource_type, &request.version_info);
        }

        // Get snapshot
        let snapshot = self.snapshot_cache.get_snapshot(&node_id)?;

        // Check if we have new resources
        let current_version = snapshot.version_for(resource_type);
        if current_version == request.version_info {
            // No update needed
            return None;
        }

        // Build response based on resource type
        self.build_response(&node_id, resource_type, &request, &snapshot)
    }

    /// Build discovery response
    fn build_response(
        &self,
        node_id: &str,
        resource_type: ResourceType,
        request: &DiscoveryRequest,
        snapshot: &ResourceSnapshot,
    ) -> Option<DiscoveryResponse> {
        let resources = match resource_type {
            ResourceType::Cluster => self.get_cluster_resources(node_id, request, snapshot),
            ResourceType::Endpoint => self.get_endpoint_resources(node_id, request, snapshot),
            ResourceType::Listener => self.get_listener_resources(node_id, request, snapshot),
            ResourceType::Route => self.get_route_resources(node_id, request, snapshot),
            _ => {
                warn!(resource_type = %resource_type, "Unsupported resource type");
                return None;
            }
        };

        Some(DiscoveryResponse {
            version_info: snapshot.version.clone(),
            resources,
            type_url: resource_type.type_url().to_string(),
            nonce: uuid::Uuid::new_v4().to_string(),
            control_plane_id: self.config.server_id.clone(),
        })
    }

    /// Get cluster resources for response
    fn get_cluster_resources(
        &self,
        _node_id: &str,
        request: &DiscoveryRequest,
        snapshot: &ResourceSnapshot,
    ) -> Vec<ResourceData> {
        let names: Vec<&str> = if request.resource_names.is_empty() {
            snapshot.clusters.keys().map(|s| s.as_str()).collect()
        } else {
            request.resource_names.iter().map(|s| s.as_str()).collect()
        };

        names
            .iter()
            .filter_map(|name| {
                snapshot.clusters.get(*name).map(|cluster| ResourceData {
                    name: name.to_string(),
                    resource_type: ResourceType::Cluster,
                    data: serde_json::to_vec(cluster).unwrap_or_default(),
                })
            })
            .collect()
    }

    /// Get endpoint resources for response
    fn get_endpoint_resources(
        &self,
        _node_id: &str,
        request: &DiscoveryRequest,
        snapshot: &ResourceSnapshot,
    ) -> Vec<ResourceData> {
        let names: Vec<&str> = if request.resource_names.is_empty() {
            snapshot.endpoints.keys().map(|s| s.as_str()).collect()
        } else {
            request.resource_names.iter().map(|s| s.as_str()).collect()
        };

        names
            .iter()
            .filter_map(|name| {
                snapshot.endpoints.get(*name).map(|cla| ResourceData {
                    name: name.to_string(),
                    resource_type: ResourceType::Endpoint,
                    data: serde_json::to_vec(cla).unwrap_or_default(),
                })
            })
            .collect()
    }

    /// Get listener resources for response
    fn get_listener_resources(
        &self,
        _node_id: &str,
        request: &DiscoveryRequest,
        snapshot: &ResourceSnapshot,
    ) -> Vec<ResourceData> {
        let names: Vec<&str> = if request.resource_names.is_empty() {
            snapshot.listeners.keys().map(|s| s.as_str()).collect()
        } else {
            request.resource_names.iter().map(|s| s.as_str()).collect()
        };

        names
            .iter()
            .filter_map(|name| {
                snapshot.listeners.get(*name).map(|listener| ResourceData {
                    name: name.to_string(),
                    resource_type: ResourceType::Listener,
                    data: serde_json::to_vec(listener).unwrap_or_default(),
                })
            })
            .collect()
    }

    /// Get route resources for response
    fn get_route_resources(
        &self,
        _node_id: &str,
        request: &DiscoveryRequest,
        snapshot: &ResourceSnapshot,
    ) -> Vec<ResourceData> {
        let names: Vec<&str> = if request.resource_names.is_empty() {
            snapshot.routes.keys().map(|s| s.as_str()).collect()
        } else {
            request.resource_names.iter().map(|s| s.as_str()).collect()
        };

        names
            .iter()
            .filter_map(|name| {
                snapshot.routes.get(*name).map(|route| ResourceData {
                    name: name.to_string(),
                    resource_type: ResourceType::Route,
                    data: serde_json::to_vec(route).unwrap_or_default(),
                })
            })
            .collect()
    }

    /// Register a new stream for a node
    pub fn register_stream(
        &self,
        node_id: &str,
    ) -> mpsc::Receiver<Result<DiscoveryResponse, tonic::Status>> {
        let (tx, rx) = mpsc::channel(100);
        self.active_streams.insert(node_id.to_string(), tx);
        info!(node_id = %node_id, "Registered xDS stream");
        rx
    }

    /// Unregister a stream for a node
    pub fn unregister_stream(&self, node_id: &str) {
        self.active_streams.remove(node_id);
        self.subscription_tracker.remove_node(node_id);
        info!(node_id = %node_id, "Unregistered xDS stream");
    }

    /// Get server statistics
    pub fn stats(&self) -> ServerStats {
        let cache_stats = self.snapshot_cache.stats();
        ServerStats {
            active_streams: self.active_streams.len(),
            tracked_nodes: self.subscription_tracker.node_ids().len(),
            cache_node_count: cache_stats.node_count,
            has_default_snapshot: cache_stats.has_default,
        }
    }
}

/// Server statistics
#[derive(Debug, Clone)]
pub struct ServerStats {
    /// Number of active streams
    pub active_streams: usize,
    /// Number of tracked nodes
    pub tracked_nodes: usize,
    /// Number of nodes with snapshots
    pub cache_node_count: usize,
    /// Whether default snapshot is set
    pub has_default_snapshot: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xds::types::{Cluster, ClusterLoadAssignment, LbPolicy};

    fn create_test_snapshot() -> ResourceSnapshot {
        let clusters = vec![Cluster {
            name: "test-cluster".to_string(),
            lb_policy: LbPolicy::RoundRobin,
            ..Default::default()
        }];
        let endpoints = vec![ClusterLoadAssignment::new("test-cluster")];
        ResourceSnapshot::with_resources(clusters, endpoints)
    }

    #[test]
    fn test_server_creation() {
        let server = XdsServer::new(XdsServerConfig::default());
        let stats = server.stats();
        assert_eq!(stats.active_streams, 0);
        assert!(!stats.has_default_snapshot);
    }

    #[test]
    fn test_update_snapshot() {
        let server = XdsServer::new(XdsServerConfig::default());
        let snapshot = create_test_snapshot();

        server.update_snapshot(snapshot);

        let stats = server.stats();
        assert!(stats.has_default_snapshot);
    }

    #[tokio::test]
    async fn test_handle_request() {
        let server = XdsServer::new(XdsServerConfig::default());
        let snapshot = create_test_snapshot();
        server.update_snapshot(snapshot);

        let request = DiscoveryRequest {
            version_info: String::new(),
            node: Some(Node {
                id: "test-node".to_string(),
                ..Default::default()
            }),
            resource_names: vec![],
            type_url: ResourceType::Cluster.type_url().to_string(),
            response_nonce: String::new(),
            error_detail: None,
        };

        let response = server.handle_request(request).await;
        assert!(response.is_some());

        let response = response.unwrap();
        assert!(!response.version_info.is_empty());
        assert_eq!(response.resources.len(), 1);
    }

    #[test]
    fn test_register_unregister_stream() {
        let server = XdsServer::new(XdsServerConfig::default());

        let _rx = server.register_stream("node-1");
        assert_eq!(server.stats().active_streams, 1);

        server.unregister_stream("node-1");
        assert_eq!(server.stats().active_streams, 0);
    }
}
