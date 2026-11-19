//! xDS Server Implementation
//!
//! This module provides the gRPC server implementation for xDS protocols,
//! including ADS (Aggregated Discovery Service).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use sha2::{Digest, Sha256};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use crate::snapshot::{DeltaClientState, ResourceSnapshot, SnapshotCache, SubscriptionTracker};
use crate::xds::ResourceType;
use crate::xds::encode::{NativeResource, encode_resource};
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
    /// Active SotW streams (node_id -> sender)
    active_streams: dashmap::DashMap<String, StreamSender>,
    /// Active Delta streams (node_id -> sender). Kept separate so a node using
    /// both protocols cannot overwrite the other protocol's channel.
    delta_streams: dashmap::DashMap<String, DeltaStreamSender>,
}

type StreamSender = mpsc::Sender<Result<DiscoveryResponse, tonic::Status>>;
type DeltaStreamSender = mpsc::Sender<Result<DeltaDiscoveryResponse, tonic::Status>>;

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
    /// Encoded Envoy v3 protobuf payload for `google.protobuf.Any.value`.
    pub data: Vec<u8>,
}

/// Internal Delta ADS request with all upstream delta fields preserved.
#[derive(Debug, Clone)]
pub struct DeltaDiscoveryRequest {
    /// Node identifier from the first wire field.
    pub node: Option<Node>,
    /// Resource type URL.
    pub type_url: String,
    /// Names added to the tracked set.
    pub resource_names_subscribe: Vec<String>,
    /// Names removed from the tracked set.
    pub resource_names_unsubscribe: Vec<String>,
    /// Versions the client already holds, sent on stream reconnect.
    pub initial_resource_versions: HashMap<String, String>,
    /// Nonce of the response being ACKed or NACKed.
    pub response_nonce: String,
    /// Error message when the request is a NACK.
    pub error_detail: Option<String>,
}

/// A changed resource in an internal Delta response.
#[derive(Debug, Clone)]
pub struct DeltaResourceData {
    /// Resource name.
    pub name: String,
    /// Content-based per-resource version.
    pub version: String,
    /// Resource type.
    pub resource_type: ResourceType,
    /// Encoded Envoy v3 protobuf payload.
    pub data: Vec<u8>,
}

/// Internal Delta ADS response carrying only changed and removed resources.
#[derive(Debug, Clone)]
pub struct DeltaDiscoveryResponse {
    /// Snapshot version, used only for debugging on the wire.
    pub system_version_info: String,
    /// Added or changed resources.
    pub resources: Vec<DeltaResourceData>,
    /// Names deleted from the snapshot.
    pub removed_resources: Vec<String>,
    /// Resource type URL.
    pub type_url: String,
    /// Nonce used to correlate the client ACK/NACK.
    pub nonce: String,
    /// Accepted for SotW parity but deliberately never emitted on a delta wire
    /// response: upstream field 7 is `udpa_removed_resources`.
    pub control_plane_id: String,
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
            delta_streams: dashmap::DashMap::new(),
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
        let mut node_ids: HashSet<String> = self
            .active_streams
            .iter()
            .map(|e| e.key().clone())
            .chain(self.delta_streams.iter().map(|e| e.key().clone()))
            .collect();
        let mut node_ids: Vec<String> = node_ids.drain().collect();
        node_ids.sort();
        for node_id in node_ids {
            self.notify_client(&node_id);
        }
    }

    /// Notify a specific client of updates
    fn notify_client(&self, node_id: &str) {
        // Get snapshot for this node
        let Some(snapshot) = self.snapshot_cache.get_snapshot(node_id) else {
            return;
        };

        if let Some(sender) = self.active_streams.get(node_id) {
            // Send SDS updates first so the certificates referenced by
            // CDS/LDS are already present when those resources arrive.
            if let Err(e) = self.send_secret_response(node_id, &snapshot, &sender) {
                warn!(node_id = %node_id, error = %e, "Failed to send SDS update");
            }
            // Send CDS updates
            if let Err(e) = self.send_cluster_response(node_id, &snapshot, &sender) {
                warn!(node_id = %node_id, error = %e, "Failed to send CDS update");
            }
            // Send EDS updates
            if let Err(e) = self.send_endpoint_response(node_id, &snapshot, &sender) {
                warn!(node_id = %node_id, error = %e, "Failed to send EDS update");
            }
            // Send LDS updates (listeners with encodable filters only)
            if let Err(e) = self.send_listener_response(node_id, &snapshot, &sender) {
                warn!(node_id = %node_id, error = %e, "Failed to send LDS update");
            }
            // Send RDS updates
            if let Err(e) = self.send_route_response(node_id, &snapshot, &sender) {
                warn!(node_id = %node_id, error = %e, "Failed to send RDS update");
            }
        }

        self.notify_delta_client(node_id, &snapshot);
    }

    /// Push only changed resources to a connected Delta ADS stream.
    fn notify_delta_client(&self, node_id: &str, snapshot: &ResourceSnapshot) {
        let Some(sender) = self.delta_streams.get(node_id) else {
            return;
        };

        // SDS first, matching the SotW push order so referenced secrets exist
        // before listeners and clusters that use them are applied.
        for resource_type in [
            ResourceType::Secret,
            ResourceType::Cluster,
            ResourceType::Endpoint,
            ResourceType::Listener,
            ResourceType::Route,
        ] {
            // A snapshot update must never create a subscription for a type
            // the client has not asked for yet.
            if !self
                .snapshot_cache
                .is_delta_state_initialized(node_id, resource_type)
            {
                continue;
            }

            let request = DeltaDiscoveryRequest {
                node: Some(Node {
                    id: node_id.to_string(),
                    ..Default::default()
                }),
                type_url: resource_type.type_url().to_string(),
                resource_names_subscribe: Vec::new(),
                resource_names_unsubscribe: Vec::new(),
                initial_resource_versions: HashMap::new(),
                response_nonce: String::new(),
                error_detail: None,
            };
            if let Some(response) = self.compute_delta_response(node_id, &request, snapshot) {
                if let Err(e) = sender.try_send(Ok(response)) {
                    warn!(node_id = %node_id, error = %e, "Failed to send Delta push");
                    break;
                }
                debug!(node_id = %node_id, resource_type = %resource_type, "Sent Delta push");
            }
        }
    }

    /// Send secret (SDS) response.
    ///
    /// Pushed proactively on snapshot updates so that Envoy already has the
    /// leaf certificate and validation context referenced by CDS/LDS.
    fn send_secret_response(
        &self,
        node_id: &str,
        snapshot: &ResourceSnapshot,
        sender: &StreamSender,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let resources: Vec<ResourceData> = snapshot
            .secrets
            .iter()
            .filter_map(|(name, secret)| {
                encode_resource(ResourceType::Secret, NativeResource::Secret(secret)).map(|data| {
                    ResourceData {
                        name: name.clone(),
                        resource_type: ResourceType::Secret,
                        data,
                    }
                })
            })
            .collect();

        if resources.is_empty() {
            return Ok(());
        }

        let response = DiscoveryResponse {
            version_info: snapshot.version.clone(),
            resources,
            type_url: ResourceType::Secret.type_url().to_string(),
            nonce: uuid::Uuid::new_v4().to_string(),
            control_plane_id: self.config.server_id.clone(),
        };

        sender.try_send(Ok(response))?;
        debug!(node_id = %node_id, "Sent SDS response");
        Ok(())
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
            .filter_map(|(name, cluster)| {
                encode_resource(ResourceType::Cluster, NativeResource::Cluster(cluster)).map(
                    |data| ResourceData {
                        name: name.clone(),
                        resource_type: ResourceType::Cluster,
                        data,
                    },
                )
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
            .filter_map(|(name, cla)| {
                encode_resource(ResourceType::Endpoint, NativeResource::Endpoints(cla)).map(
                    |data| ResourceData {
                        name: name.clone(),
                        resource_type: ResourceType::Endpoint,
                        data,
                    },
                )
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

    /// Send listener (LDS) response.
    ///
    /// Listeners whose filters cannot be encoded are skipped by
    /// [`encode_resource`], so a partially representable snapshot never emits
    /// an invalid `Any` payload.
    fn send_listener_response(
        &self,
        node_id: &str,
        snapshot: &ResourceSnapshot,
        sender: &StreamSender,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let resources: Vec<ResourceData> = snapshot
            .listeners
            .iter()
            .filter_map(|(name, listener)| {
                encode_resource(ResourceType::Listener, NativeResource::Listener(listener)).map(
                    |data| ResourceData {
                        name: name.clone(),
                        resource_type: ResourceType::Listener,
                        data,
                    },
                )
            })
            .collect();

        if resources.is_empty() {
            return Ok(());
        }

        let response = DiscoveryResponse {
            version_info: snapshot.version.clone(),
            resources,
            type_url: ResourceType::Listener.type_url().to_string(),
            nonce: uuid::Uuid::new_v4().to_string(),
            control_plane_id: self.config.server_id.clone(),
        };

        sender.try_send(Ok(response))?;
        debug!(node_id = %node_id, "Sent LDS response");
        Ok(())
    }

    /// Send route (RDS) response.
    fn send_route_response(
        &self,
        node_id: &str,
        snapshot: &ResourceSnapshot,
        sender: &StreamSender,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let resources: Vec<ResourceData> = snapshot
            .routes
            .iter()
            .filter_map(|(name, route)| {
                encode_resource(ResourceType::Route, NativeResource::Route(route)).map(|data| {
                    ResourceData {
                        name: name.clone(),
                        resource_type: ResourceType::Route,
                        data,
                    }
                })
            })
            .collect();

        if resources.is_empty() {
            return Ok(());
        }

        let response = DiscoveryResponse {
            version_info: snapshot.version.clone(),
            resources,
            type_url: ResourceType::Route.type_url().to_string(),
            nonce: uuid::Uuid::new_v4().to_string(),
            control_plane_id: self.config.server_id.clone(),
        };

        sender.try_send(Ok(response))?;
        debug!(node_id = %node_id, "Sent RDS response");
        Ok(())
    }

    /// Compute a content-addressed version for one encoded xDS resource.
    fn resource_version(data: &[u8]) -> String {
        let digest = Sha256::digest(data);
        let mut version = String::with_capacity(7 + digest.len() * 2);
        version.push_str("sha256:");
        for byte in digest {
            version.push_str(&format!("{byte:02x}"));
        }
        version
    }

    /// Encode every resource of one type in the snapshot for Delta diffing.
    fn collect_delta_resources(
        resource_type: ResourceType,
        snapshot: &ResourceSnapshot,
    ) -> Vec<ResourceData> {
        let mut resources: Vec<ResourceData> = match resource_type {
            ResourceType::Cluster => snapshot
                .clusters
                .iter()
                .filter_map(|(name, cluster)| {
                    encode_resource(ResourceType::Cluster, NativeResource::Cluster(cluster)).map(
                        |data| ResourceData {
                            name: name.clone(),
                            resource_type,
                            data,
                        },
                    )
                })
                .collect(),
            ResourceType::Endpoint => snapshot
                .endpoints
                .iter()
                .filter_map(|(name, endpoints)| {
                    encode_resource(ResourceType::Endpoint, NativeResource::Endpoints(endpoints))
                        .map(|data| ResourceData {
                            name: name.clone(),
                            resource_type,
                            data,
                        })
                })
                .collect(),
            ResourceType::Listener => snapshot
                .listeners
                .iter()
                .filter_map(|(name, listener)| {
                    encode_resource(ResourceType::Listener, NativeResource::Listener(listener)).map(
                        |data| ResourceData {
                            name: name.clone(),
                            resource_type,
                            data,
                        },
                    )
                })
                .collect(),
            ResourceType::Route => snapshot
                .routes
                .iter()
                .filter_map(|(name, route)| {
                    encode_resource(ResourceType::Route, NativeResource::Route(route)).map(|data| {
                        ResourceData {
                            name: name.clone(),
                            resource_type,
                            data,
                        }
                    })
                })
                .collect(),
            ResourceType::Secret => snapshot
                .secrets
                .iter()
                .filter_map(|(name, secret)| {
                    encode_resource(ResourceType::Secret, NativeResource::Secret(secret)).map(
                        |data| ResourceData {
                            name: name.clone(),
                            resource_type,
                            data,
                        },
                    )
                })
                .collect(),
            ResourceType::Runtime => Vec::new(),
        };
        resources.sort_by(|a, b| a.name.cmp(&b.name));
        resources
    }

    /// Build an incremental Delta response from a client request or snapshot push.
    fn compute_delta_response(
        &self,
        node_id: &str,
        request: &DeltaDiscoveryRequest,
        snapshot: &ResourceSnapshot,
    ) -> Option<DeltaDiscoveryResponse> {
        let resource_type = ResourceType::from_type_url(&request.type_url)?;
        if resource_type == ResourceType::Runtime {
            warn!(resource_type = %resource_type, "Unsupported Delta resource type");
            return None;
        }

        let is_new_state = !self.snapshot_cache.has_delta_state(node_id, resource_type);
        let resources = Self::collect_delta_resources(resource_type, snapshot);
        let current_names: HashSet<String> = resources
            .iter()
            .map(|resource| resource.name.clone())
            .collect();

        self.snapshot_cache.with_delta_state(
            node_id,
            resource_type,
            |state: &mut DeltaClientState| {
                if is_new_state {
                    state.initialized = true;
                    state.seed_versions(&request.initial_resource_versions);
                    // Upstream legacy rule: the first request for a type with
                    // no explicit subscribe/unsubscribe names means `*`.
                    if request.resource_names_subscribe.is_empty()
                        && request.resource_names_unsubscribe.is_empty()
                    {
                        state.mark_legacy_wildcard();
                    }
                }

                if !request.response_nonce.is_empty() {
                    match &request.error_detail {
                        Some(error) => {
                            if !state.record_nack(&request.response_nonce) {
                                warn!(node_id = %node_id, nonce = %request.response_nonce, "Delta NACK for unknown nonce");
                            } else {
                                warn!(node_id = %node_id, error = %error, "Received Delta NACK from client");
                            }
                        }
                        None => {
                            if !state.record_ack(&request.response_nonce) {
                                debug!(node_id = %node_id, nonce = %request.response_nonce, "Delta ACK for unknown nonce");
                            }
                        }
                    }
                }

                state.unsubscribe(&request.resource_names_unsubscribe);
                state.subscribe(&request.resource_names_subscribe);

                // `initial_resource_versions` exists to resume a logical
                // session after reconnect. A wildcard still tracks all
                // resources, but known versions must not be forced onto the
                // wire again. Explicit named subscriptions keep their resend
                // guarantee in `force_send`.
                if is_new_state && !request.initial_resource_versions.is_empty() {
                    state.force_all = false;
                }

                let mut changed = Vec::new();
                let mut sent_versions = HashMap::new();
                for resource in &resources {
                    let version = Self::resource_version(&resource.data);
                    if state.needs_update(&resource.name, &version) {
                        sent_versions.insert(resource.name.clone(), version.clone());
                        changed.push(DeltaResourceData {
                            name: resource.name.clone(),
                            version,
                            resource_type,
                            data: resource.data.clone(),
                        });
                    }
                }

                let mut removed = state.detect_removals(&current_names);
                removed.sort();
                state.forget(&removed);
                state.clear_forced();

                if changed.is_empty() && removed.is_empty() {
                    return None;
                }

                let nonce = uuid::Uuid::new_v4().to_string();
                state.prepare_send(&nonce, &sent_versions);

                Some(DeltaDiscoveryResponse {
                    system_version_info: snapshot.version.clone(),
                    resources: changed,
                    removed_resources: removed,
                    type_url: resource_type.type_url().to_string(),
                    nonce,
                    control_plane_id: self.config.server_id.clone(),
                })
            },
        )
    }

    /// Handle an incoming Delta ADS request.
    pub async fn handle_delta_request(
        &self,
        request: DeltaDiscoveryRequest,
    ) -> Option<DeltaDiscoveryResponse> {
        let node_id = request
            .node
            .as_ref()
            .map(|n| n.id.clone())
            .filter(|id| !id.is_empty())
            .unwrap_or_else(|| "unknown".to_string());
        let resource_type = ResourceType::from_type_url(&request.type_url)?;

        debug!(
            node_id = %node_id,
            resource_type = %resource_type,
            subscribe = request.resource_names_subscribe.len(),
            unsubscribe = request.resource_names_unsubscribe.len(),
            "Handling Delta discovery request"
        );

        // A Delta client may subscribe before the first snapshot is published.
        // Keep its subscription state so the later snapshot push can answer it;
        // an empty snapshot produces no response but still initializes state.
        let snapshot = self
            .snapshot_cache
            .get_snapshot(&node_id)
            .unwrap_or_else(|| Arc::new(ResourceSnapshot::new()));
        self.compute_delta_response(&node_id, &request, &snapshot)
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
            ResourceType::Secret => self.get_secret_resources(node_id, request, snapshot),
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
                let cluster = snapshot.clusters.get(*name)?;
                let data =
                    encode_resource(ResourceType::Cluster, NativeResource::Cluster(cluster))?;
                Some(ResourceData {
                    name: name.to_string(),
                    resource_type: ResourceType::Cluster,
                    data,
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
                let cla = snapshot.endpoints.get(*name)?;
                let data = encode_resource(ResourceType::Endpoint, NativeResource::Endpoints(cla))?;
                Some(ResourceData {
                    name: name.to_string(),
                    resource_type: ResourceType::Endpoint,
                    data,
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
                let listener = snapshot.listeners.get(*name)?;
                let data =
                    encode_resource(ResourceType::Listener, NativeResource::Listener(listener))?;
                Some(ResourceData {
                    name: name.to_string(),
                    resource_type: ResourceType::Listener,
                    data,
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
                let route = snapshot.routes.get(*name)?;
                let data = encode_resource(ResourceType::Route, NativeResource::Route(route))?;
                Some(ResourceData {
                    name: name.to_string(),
                    resource_type: ResourceType::Route,
                    data,
                })
            })
            .collect()
    }

    /// Get secret resources for an SDS response.
    ///
    /// Only names the client requested are answered; unrequested or missing
    /// names are skipped rather than guessed.
    fn get_secret_resources(
        &self,
        _node_id: &str,
        request: &DiscoveryRequest,
        snapshot: &ResourceSnapshot,
    ) -> Vec<ResourceData> {
        let names: Vec<&str> = if request.resource_names.is_empty() {
            snapshot.secrets.keys().map(String::as_str).collect()
        } else {
            request.resource_names.iter().map(String::as_str).collect()
        };

        names
            .iter()
            .filter_map(|name| {
                let secret = snapshot.secrets.get(*name)?;
                let data = encode_resource(ResourceType::Secret, NativeResource::Secret(secret))?;
                Some(ResourceData {
                    name: name.to_string(),
                    resource_type: ResourceType::Secret,
                    data,
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

    /// Register a Delta ADS stream for a node.
    pub fn register_delta_stream(
        &self,
        node_id: &str,
    ) -> mpsc::Receiver<Result<DeltaDiscoveryResponse, tonic::Status>> {
        let (tx, rx) = mpsc::channel(100);
        self.delta_streams.insert(node_id.to_string(), tx);
        info!(node_id = %node_id, "Registered Delta xDS stream");
        rx
    }

    /// Unregister a Delta ADS stream and its per-type client state.
    pub fn unregister_delta_stream(&self, node_id: &str) {
        self.delta_streams.remove(node_id);
        self.snapshot_cache.remove_delta_states_for_node(node_id);
        info!(node_id = %node_id, "Unregistered Delta xDS stream");
    }

    /// Get server statistics
    pub fn stats(&self) -> ServerStats {
        let cache_stats = self.snapshot_cache.stats();
        ServerStats {
            active_streams: self.active_streams.len() + self.delta_streams.len(),
            tracked_nodes: self.subscription_tracker.node_ids().len(),
            cache_node_count: cache_stats.node_count,
            has_default_snapshot: cache_stats.has_default,
        }
    }

    /// Return the node IDs of all currently connected xDS clients.
    ///
    /// Used by sync bridges to build per-node (per-proxy) snapshots. Delta-only
    /// clients must be included or they would keep receiving the default
    /// snapshot instead of their proxy-specific one.
    pub fn connected_nodes(&self) -> Vec<String> {
        let mut nodes: HashSet<String> = self
            .active_streams
            .iter()
            .map(|e| e.key().clone())
            .chain(self.delta_streams.iter().map(|e| e.key().clone()))
            .collect();
        let mut nodes: Vec<String> = nodes.drain().collect();
        nodes.sort();
        nodes
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
    use crate::xds::types::{Cluster, ClusterLoadAssignment, LbPolicy, SdsSecret};

    fn create_test_snapshot() -> ResourceSnapshot {
        let clusters = vec![Cluster {
            name: "test-cluster".to_string(),
            lb_policy: LbPolicy::RoundRobin,
            ..Default::default()
        }];
        let endpoints = vec![ClusterLoadAssignment::new("test-cluster")];
        ResourceSnapshot::with_resources(clusters, endpoints)
    }

    fn cluster(name: &str, lb_policy: LbPolicy) -> Cluster {
        Cluster {
            name: name.to_string(),
            lb_policy,
            ..Default::default()
        }
    }

    fn cluster_snapshot(names: &[(&str, LbPolicy)]) -> ResourceSnapshot {
        ResourceSnapshot::with_resources(
            names
                .iter()
                .map(|(name, policy)| cluster(name, *policy))
                .collect(),
            Vec::new(),
        )
    }

    fn delta_request(node_id: &str, resource_type: ResourceType) -> DeltaDiscoveryRequest {
        DeltaDiscoveryRequest {
            node: Some(Node {
                id: node_id.to_string(),
                ..Default::default()
            }),
            type_url: resource_type.type_url().to_string(),
            resource_names_subscribe: Vec::new(),
            resource_names_unsubscribe: Vec::new(),
            initial_resource_versions: HashMap::new(),
            response_nonce: String::new(),
            error_detail: None,
        }
    }

    fn content_version(name: &str, policy: LbPolicy) -> String {
        let resource = cluster(name, policy);
        let data = encode_resource(ResourceType::Cluster, NativeResource::Cluster(&resource))
            .expect("test cluster must encode");
        XdsServer::resource_version(&data)
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

    #[tokio::test]
    async fn test_handle_sds_request_answers_requested_names_and_skips_missing() {
        let server = XdsServer::new(XdsServerConfig::default());
        let snapshot = ResourceSnapshot::with_all_resources(vec![], vec![], vec![], vec![])
            .with_secrets(vec![
                SdsSecret::tls_certificate("server-cert", "CERT", "KEY"),
                SdsSecret::validation_context("root-ca", "CA"),
            ]);
        server.update_snapshot(snapshot);

        let request = DiscoveryRequest {
            version_info: String::new(),
            node: Some(Node {
                id: "sds-node".to_string(),
                ..Default::default()
            }),
            resource_names: vec!["server-cert".to_string(), "does-not-exist".to_string()],
            type_url: ResourceType::Secret.type_url().to_string(),
            response_nonce: String::new(),
            error_detail: None,
        };

        let response = server
            .handle_request(request)
            .await
            .expect("SDS request must get a response");
        assert_eq!(response.type_url, ResourceType::Secret.type_url());
        assert_eq!(response.resources.len(), 1);
        assert_eq!(response.resources[0].name, "server-cert");
        assert_eq!(response.resources[0].resource_type, ResourceType::Secret);
        let secret = crate::xds::encode::decode_secret(&response.resources[0].data).unwrap();
        assert_eq!(secret.name, "server-cert");

        // An empty subscription is a wildcard and returns every secret.
        let wildcard = DiscoveryRequest {
            version_info: String::new(),
            node: Some(Node {
                id: "sds-node".to_string(),
                ..Default::default()
            }),
            resource_names: vec![],
            type_url: ResourceType::Secret.type_url().to_string(),
            response_nonce: String::new(),
            error_detail: None,
        };
        let response = server.handle_request(wildcard).await.unwrap();
        assert_eq!(response.resources.len(), 2);
    }

    #[tokio::test]
    async fn test_delta_first_request_is_incremental_and_second_request_is_empty() {
        let server = XdsServer::new(XdsServerConfig::default());
        server.update_snapshot(cluster_snapshot(&[
            ("alpha", LbPolicy::RoundRobin),
            ("beta", LbPolicy::LeastRequest),
        ]));

        let first = server
            .handle_delta_request(delta_request("delta-node", ResourceType::Cluster))
            .await
            .expect("first delta request must return resources");
        assert_eq!(
            first.system_version_info,
            server.snapshot_cache().stats().default_version.unwrap()
        );
        assert_eq!(first.resources.len(), 2);
        assert!(
            first
                .resources
                .iter()
                .all(|r| r.version.starts_with("sha256:"))
        );
        assert_ne!(first.resources[0].version, first.resources[1].version);
        assert!(first.removed_resources.is_empty());

        let mut ack = delta_request("delta-node", ResourceType::Cluster);
        ack.response_nonce = first.nonce.clone();
        let second = server.handle_delta_request(ack).await;
        assert!(second.is_none(), "unchanged resources must not be resent");
    }

    #[tokio::test]
    async fn test_delta_initial_resource_versions_suppress_known_resources() {
        let server = XdsServer::new(XdsServerConfig::default());
        server.update_snapshot(cluster_snapshot(&[
            ("alpha", LbPolicy::RoundRobin),
            ("beta", LbPolicy::LeastRequest),
        ]));

        let mut request = delta_request("delta-node", ResourceType::Cluster);
        request.initial_resource_versions.insert(
            "alpha".to_string(),
            content_version("alpha", LbPolicy::RoundRobin),
        );
        let response = server
            .handle_delta_request(request)
            .await
            .expect("unknown beta must still be sent");
        assert_eq!(response.resources.len(), 1);
        assert_eq!(response.resources[0].name, "beta");
    }

    #[tokio::test]
    async fn test_delta_identical_content_after_snapshot_version_change_is_not_resent() {
        let server = XdsServer::new(XdsServerConfig::default());
        server.update_snapshot(cluster_snapshot(&[("alpha", LbPolicy::RoundRobin)]));

        let first = server
            .handle_delta_request(delta_request("delta-node", ResourceType::Cluster))
            .await
            .unwrap();
        let mut ack = delta_request("delta-node", ResourceType::Cluster);
        ack.response_nonce = first.nonce;
        assert!(server.handle_delta_request(ack).await.is_none());

        // A new snapshot version with byte-identical resource content must not
        // look like a resource change to a Delta client.
        let mut new_snapshot = ResourceSnapshot::new();
        new_snapshot.add_cluster(cluster("alpha", LbPolicy::RoundRobin));
        assert_ne!(
            new_snapshot.version,
            server.snapshot_cache().stats().default_version.unwrap()
        );
        server.update_snapshot(new_snapshot);

        let response = server
            .handle_delta_request(delta_request("delta-node", ResourceType::Cluster))
            .await;
        assert!(
            response.is_none(),
            "identical encoded bytes must keep the same per-resource version"
        );
    }

    #[tokio::test]
    async fn test_delta_explicit_resubscribe_forces_resend() {
        let server = XdsServer::new(XdsServerConfig::default());
        server.update_snapshot(cluster_snapshot(&[("alpha", LbPolicy::RoundRobin)]));

        let first = server
            .handle_delta_request(delta_request("delta-node", ResourceType::Cluster))
            .await
            .unwrap();
        assert_eq!(first.resources.len(), 1);

        let mut ack = delta_request("delta-node", ResourceType::Cluster);
        ack.response_nonce = first.nonce;
        assert!(server.handle_delta_request(ack).await.is_none());

        let mut resubscribe = delta_request("delta-node", ResourceType::Cluster);
        resubscribe
            .resource_names_subscribe
            .push("alpha".to_string());
        let resent = server
            .handle_delta_request(resubscribe)
            .await
            .expect("explicit subscribe must resend even at the current version");
        assert_eq!(resent.resources.len(), 1);
        assert_eq!(resent.resources[0].name, "alpha");
    }

    #[tokio::test]
    async fn test_delta_unsubscribe_stops_future_updates() {
        let server = XdsServer::new(XdsServerConfig::default());
        server.update_snapshot(cluster_snapshot(&[("alpha", LbPolicy::RoundRobin)]));

        let mut subscribe = delta_request("delta-node", ResourceType::Cluster);
        subscribe.resource_names_subscribe.push("alpha".to_string());
        let first = server.handle_delta_request(subscribe).await.unwrap();
        assert_eq!(first.resources.len(), 1);

        let mut unsubscribe = delta_request("delta-node", ResourceType::Cluster);
        unsubscribe
            .resource_names_unsubscribe
            .push("alpha".to_string());
        assert!(server.handle_delta_request(unsubscribe).await.is_none());

        server.update_snapshot(cluster_snapshot(&[("alpha", LbPolicy::Random)]));
        let after_change = server
            .handle_delta_request(delta_request("delta-node", ResourceType::Cluster))
            .await;
        assert!(
            after_change.is_none(),
            "unsubscribed resources must not produce updates"
        );
    }

    #[tokio::test]
    async fn test_delta_unsubscribe_under_wildcard_resends_resource() {
        let server = XdsServer::new(XdsServerConfig::default());
        server.update_snapshot(cluster_snapshot(&[("alpha", LbPolicy::RoundRobin)]));

        let first = server
            .handle_delta_request(delta_request("delta-node", ResourceType::Cluster))
            .await
            .unwrap();
        assert_eq!(first.resources.len(), 1);

        let mut ack = delta_request("delta-node", ResourceType::Cluster);
        ack.response_nonce = first.nonce;
        assert!(server.handle_delta_request(ack).await.is_none());

        let mut unsubscribe = delta_request("delta-node", ResourceType::Cluster);
        unsubscribe
            .resource_names_unsubscribe
            .push("alpha".to_string());
        let resent = server
            .handle_delta_request(unsubscribe)
            .await
            .expect("wildcard clients must be told whether the resource is still covered");
        assert_eq!(resent.resources.len(), 1);
        assert_eq!(resent.resources[0].name, "alpha");
    }

    #[tokio::test]
    async fn test_delta_snapshot_deletion_is_reported_as_removed() {
        let server = XdsServer::new(XdsServerConfig::default());
        server.update_snapshot(cluster_snapshot(&[
            ("alpha", LbPolicy::RoundRobin),
            ("beta", LbPolicy::LeastRequest),
        ]));

        let first = server
            .handle_delta_request(delta_request("delta-node", ResourceType::Cluster))
            .await
            .unwrap();
        let mut ack = delta_request("delta-node", ResourceType::Cluster);
        ack.response_nonce = first.nonce;
        assert!(server.handle_delta_request(ack).await.is_none());

        server.update_snapshot(cluster_snapshot(&[("alpha", LbPolicy::RoundRobin)]));
        let response = server
            .handle_delta_request(delta_request("delta-node", ResourceType::Cluster))
            .await
            .expect("deletions must produce a delta response");
        assert!(response.resources.is_empty());
        assert_eq!(response.removed_resources, vec!["beta".to_string()]);
    }

    #[tokio::test]
    async fn test_delta_nack_restores_versions_and_resends_changed_resource() {
        let server = XdsServer::new(XdsServerConfig::default());
        server.update_snapshot(cluster_snapshot(&[("alpha", LbPolicy::RoundRobin)]));

        let first = server
            .handle_delta_request(delta_request("delta-node", ResourceType::Cluster))
            .await
            .unwrap();
        let mut ack = delta_request("delta-node", ResourceType::Cluster);
        ack.response_nonce = first.nonce;
        assert!(server.handle_delta_request(ack).await.is_none());

        server.update_snapshot(cluster_snapshot(&[("alpha", LbPolicy::Random)]));
        let changed = server
            .handle_delta_request(delta_request("delta-node", ResourceType::Cluster))
            .await
            .unwrap();
        assert_eq!(changed.resources.len(), 1);
        let changed_version = changed.resources[0].version.clone();

        // The changed version was advanced optimistically at send time. A NACK
        // must restore the previous version so the next request retries it.
        let mut nack = delta_request("delta-node", ResourceType::Cluster);
        nack.response_nonce = changed.nonce.clone();
        nack.error_detail = Some("config rejected".to_string());
        let retried = server
            .handle_delta_request(nack)
            .await
            .expect("NACK must cause a retry");
        assert_eq!(retried.resources.len(), 1);
        assert_eq!(retried.resources[0].version, changed_version);
        assert_ne!(retried.nonce, changed.nonce);
    }

    #[tokio::test]
    async fn test_delta_server_push_sends_only_changed_resources() {
        let server = XdsServer::new(XdsServerConfig::default());
        server.update_snapshot(cluster_snapshot(&[
            ("alpha", LbPolicy::RoundRobin),
            ("beta", LbPolicy::LeastRequest),
        ]));

        let first = server
            .handle_delta_request(delta_request("push-node", ResourceType::Cluster))
            .await
            .unwrap();
        assert_eq!(first.resources.len(), 2);
        let mut ack = delta_request("push-node", ResourceType::Cluster);
        ack.response_nonce = first.nonce;
        assert!(server.handle_delta_request(ack).await.is_none());

        let mut push_rx = server.register_delta_stream("push-node");
        server.update_snapshot(cluster_snapshot(&[
            ("alpha", LbPolicy::Random),
            ("beta", LbPolicy::LeastRequest),
        ]));

        let pushed = tokio::time::timeout(std::time::Duration::from_secs(1), push_rx.recv())
            .await
            .expect("delta push must arrive")
            .expect("delta channel must stay open")
            .expect("delta response must not be an error");
        assert_eq!(pushed.resources.len(), 1);
        assert_eq!(pushed.resources[0].name, "alpha");

        server.unregister_delta_stream("push-node");
        assert!(!server.connected_nodes().contains(&"push-node".to_string()));
    }

    #[tokio::test]
    async fn test_delta_request_before_snapshot_is_answered_by_later_push() {
        let server = XdsServer::new(XdsServerConfig::default());
        let mut push_rx = server.register_delta_stream("early-node");

        let response = server
            .handle_delta_request(delta_request("early-node", ResourceType::Cluster))
            .await;
        assert!(
            response.is_none(),
            "no snapshot means no immediate resources"
        );

        server.update_snapshot(cluster_snapshot(&[("alpha", LbPolicy::RoundRobin)]));

        let pushed = tokio::time::timeout(std::time::Duration::from_secs(1), push_rx.recv())
            .await
            .expect("later snapshot must reach the early subscriber")
            .expect("delta channel must stay open")
            .expect("delta response must not be an error");
        assert_eq!(pushed.resources.len(), 1);
        assert_eq!(pushed.resources[0].name, "alpha");

        server.unregister_delta_stream("early-node");
    }

    #[tokio::test]
    async fn test_delta_push_is_not_sent_before_client_subscribes() {
        let server = XdsServer::new(XdsServerConfig::default());
        let mut push_rx = server.register_delta_stream("push-node");

        server.update_snapshot(cluster_snapshot(&[("alpha", LbPolicy::RoundRobin)]));

        assert!(
            push_rx.try_recv().is_err(),
            "snapshot push must not invent a Delta subscription"
        );
        server.unregister_delta_stream("push-node");
    }

    #[test]
    fn test_connected_nodes_includes_delta_only_clients() {
        let server = XdsServer::new(XdsServerConfig::default());
        let _rx = server.register_delta_stream("delta-only");
        assert_eq!(server.connected_nodes(), vec!["delta-only".to_string()]);
        server.unregister_delta_stream("delta-only");
        assert!(server.connected_nodes().is_empty());
    }
}
