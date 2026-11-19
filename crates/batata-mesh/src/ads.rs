//! Real Envoy ADS (Aggregated Discovery Service) gRPC implementation.
//!
//! This module implements the tonic-generated ADS service against [`XdsServer`].
//! It replaces the earlier hand-written HTTP/2 handler that returned an empty
//! `200` body for every ADS call, which meant no Envoy client could ever obtain
//! configuration from the control plane.

use std::pin::Pin;
use std::sync::Arc;

use futures::Stream;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Streaming};
use tracing::{debug, info, warn};

use crate::server::{
    DiscoveryRequest as InternalRequest, DiscoveryResponse as InternalResponse, XdsServer,
};
use crate::xds::ResourceType;
use crate::xds::generated::discovery::{
    ControlPlane, DeltaDiscoveryRequest, DeltaDiscoveryRequest as DeltaRequest,
    DeltaDiscoveryResponse as DeltaResponse, DiscoveryRequest, DiscoveryResponse, Resource,
    aggregated_discovery_service_server::{
        AggregatedDiscoveryService, AggregatedDiscoveryServiceServer,
    },
};
use crate::xds::types::{Locality as NativeLocality, Node as NativeNode};

/// Stream of SotW discovery responses sent to an Envoy client.
pub type AdsResponseStream = Pin<Box<dyn Stream<Item = Result<DiscoveryResponse, Status>> + Send>>;

/// Stream of delta discovery responses sent to an Envoy client.
pub type AdsDeltaResponseStream = Pin<Box<dyn Stream<Item = Result<DeltaResponse, Status>> + Send>>;

/// ADS service backed by the [`XdsServer`] snapshot cache.
pub struct BatataAdsService {
    xds_server: Arc<XdsServer>,
}

impl BatataAdsService {
    /// Create an ADS service for the given xDS server.
    pub fn new(xds_server: Arc<XdsServer>) -> Self {
        Self { xds_server }
    }

    /// Wrap the service in the tonic-generated server type.
    pub fn into_server(self) -> AggregatedDiscoveryServiceServer<Self> {
        AggregatedDiscoveryServiceServer::new(self)
    }
}

impl Clone for BatataAdsService {
    fn clone(&self) -> Self {
        Self {
            xds_server: self.xds_server.clone(),
        }
    }
}

fn to_internal_node(node: &crate::xds::generated::discovery::Node) -> NativeNode {
    NativeNode {
        id: node.id.clone(),
        cluster: node.cluster.clone(),
        metadata: Default::default(),
        locality: node.locality.as_ref().map(|l| NativeLocality {
            region: l.region.clone(),
            zone: l.zone.clone(),
            sub_zone: l.sub_zone.clone(),
        }),
        user_agent_name: Some(node.user_agent_name.clone()),
        user_agent_version: Some(node.user_agent_version.clone()),
    }
}

/// Convert a wire discovery request into the internal request model.
pub fn to_internal_request(req: &DiscoveryRequest) -> InternalRequest {
    InternalRequest {
        version_info: req.version_info.clone(),
        node: req.node.as_ref().map(to_internal_node),
        resource_names: req.resource_names.clone(),
        type_url: req.type_url.clone(),
        response_nonce: req.response_nonce.clone(),
        error_detail: req.error_detail.as_ref().map(|s| s.message.clone()),
    }
}

/// Convert an internal discovery response into the wire response model.
///
/// `ResourceData.data` already holds encoded Envoy protobuf, so it is placed
/// into `google.protobuf.Any.value` verbatim.
pub fn to_wire_response(resp: InternalResponse) -> DiscoveryResponse {
    DiscoveryResponse {
        version_info: resp.version_info,
        resources: resp
            .resources
            .into_iter()
            .map(|r| ::prost_types::Any {
                type_url: r.resource_type.type_url().to_string(),
                value: r.data,
            })
            .collect(),
        canary: false,
        type_url: resp.type_url,
        nonce: resp.nonce,
        control_plane: Some(ControlPlane {
            identifier: resp.control_plane_id,
        }),
    }
}

fn to_internal_delta_request(req: &DeltaRequest) -> InternalRequest {
    InternalRequest {
        version_info: String::new(),
        node: req.node.as_ref().map(to_internal_node),
        resource_names: req.resource_names_subscribe.clone(),
        type_url: req.type_url.clone(),
        response_nonce: req.response_nonce.clone(),
        error_detail: req.error_detail.as_ref().map(|s| s.message.clone()),
    }
}

fn to_wire_delta_response(resp: InternalResponse, removed: Vec<String>) -> DeltaResponse {
    let version = resp.version_info.clone();
    DeltaResponse {
        system_version_info: version.clone(),
        resources: resp
            .resources
            .into_iter()
            .map(|r| Resource {
                name: r.name,
                version: version.clone(),
                resource: Some(::prost_types::Any {
                    type_url: r.resource_type.type_url().to_string(),
                    value: r.data,
                }),
                ..Default::default()
            })
            .collect(),
        type_url: resp.type_url,
        removed_resources: removed,
        nonce: resp.nonce,
        // No control_plane on DeltaDiscoveryResponse: upstream field 7 is
        // udpa_removed_resources, so a ControlPlane here would be misread by
        // a real Envoy and can NACK the response. control_plane_id is only
        // emitted on the SotW DiscoveryResponse (field 6).
    }
}

#[tonic::async_trait]
impl AggregatedDiscoveryService for BatataAdsService {
    type StreamAggregatedResourcesStream = AdsResponseStream;
    type DeltaAggregatedResourcesStream = AdsDeltaResponseStream;

    async fn stream_aggregated_resources(
        &self,
        request: Request<Streaming<DiscoveryRequest>>,
    ) -> Result<Response<Self::StreamAggregatedResourcesStream>, Status> {
        let mut inbound = request.into_inner();
        let xds_server = self.xds_server.clone();
        let (tx, rx) = mpsc::channel::<Result<DiscoveryResponse, Status>>(64);

        tokio::spawn(async move {
            let mut node_id: Option<String> = None;
            let mut push_task: Option<tokio::task::JoinHandle<()>> = None;

            loop {
                match tokio_stream::StreamExt::next(&mut inbound).await {
                    Some(Ok(req)) => {
                        if node_id.is_none() {
                            let id = req
                                .node
                                .as_ref()
                                .map(|n| n.id.clone())
                                .filter(|s| !s.is_empty())
                                .unwrap_or_else(|| "unknown".to_string());
                            info!(node_id = %id, "ADS stream established");

                            // Subscribe to server-side snapshot pushes so that
                            // config changes reach the proxy without a new
                            // client request.
                            let mut push_rx = xds_server.register_stream(&id);
                            let push_tx = tx.clone();
                            push_task = Some(tokio::spawn(async move {
                                while let Some(item) = push_rx.recv().await {
                                    let out = item.map(to_wire_response);
                                    if push_tx.send(out).await.is_err() {
                                        break;
                                    }
                                }
                            }));
                            node_id = Some(id);
                        }

                        let internal = to_internal_request(&req);
                        debug!(
                            type_url = %req.type_url,
                            version = %req.version_info,
                            "ADS request"
                        );
                        if let Some(resp) = xds_server.handle_request(internal).await
                            && tx.send(Ok(to_wire_response(resp))).await.is_err()
                        {
                            break;
                        }
                    }
                    Some(Err(e)) => {
                        warn!(error = %e, "ADS request stream error");
                        break;
                    }
                    None => break,
                }
            }

            if let Some(id) = node_id {
                xds_server.unregister_stream(&id);
                info!(node_id = %id, "ADS stream closed");
            }
            if let Some(task) = push_task {
                task.abort();
            }
        });

        Ok(Response::new(
            Box::pin(ReceiverStream::new(rx)) as AdsResponseStream
        ))
    }

    async fn delta_aggregated_resources(
        &self,
        request: Request<Streaming<DeltaDiscoveryRequest>>,
    ) -> Result<Response<Self::DeltaAggregatedResourcesStream>, Status> {
        let mut inbound = request.into_inner();
        let xds_server = self.xds_server.clone();
        let (tx, rx) = mpsc::channel::<Result<DeltaResponse, Status>>(64);

        tokio::spawn(async move {
            loop {
                match tokio_stream::StreamExt::next(&mut inbound).await {
                    Some(Ok(req)) => {
                        let removed = req.resource_names_unsubscribe.clone();
                        let internal = to_internal_delta_request(&req);
                        if let Some(resp) = xds_server.handle_request(internal).await
                            && tx
                                .send(Ok(to_wire_delta_response(resp, removed)))
                                .await
                                .is_err()
                        {
                            break;
                        }
                    }
                    Some(Err(e)) => {
                        warn!(error = %e, "Delta ADS request stream error");
                        break;
                    }
                    None => break,
                }
            }
        });

        Ok(Response::new(
            Box::pin(ReceiverStream::new(rx)) as AdsDeltaResponseStream
        ))
    }
}

/// Build a wire request for the given resource type (test helper / tooling).
pub fn cluster_request(node_id: &str) -> DiscoveryRequest {
    DiscoveryRequest {
        node: Some(crate::xds::generated::discovery::Node {
            id: node_id.to_string(),
            ..Default::default()
        }),
        type_url: ResourceType::Cluster.type_url().to_string(),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_internal_request_maps_wire_fields() {
        let req = DiscoveryRequest {
            version_info: "7".to_string(),
            node: Some(crate::xds::generated::discovery::Node {
                id: "sidecar-1".to_string(),
                cluster: "payments".to_string(),
                user_agent_name: "envoy".to_string(),
                ..Default::default()
            }),
            resource_names: vec!["payments".to_string()],
            type_url: ResourceType::Cluster.type_url().to_string(),
            response_nonce: "n-1".to_string(),
            error_detail: None,
        };

        let internal = to_internal_request(&req);
        assert_eq!(internal.version_info, "7");
        assert_eq!(internal.node.as_ref().unwrap().id, "sidecar-1");
        assert_eq!(internal.resource_names, vec!["payments"]);
        assert_eq!(internal.type_url, ResourceType::Cluster.type_url());
    }

    #[test]
    fn test_to_wire_response_wraps_protobuf_in_any() {
        let resp = InternalResponse {
            version_info: "3".to_string(),
            resources: vec![crate::server::ResourceData {
                name: "payments".to_string(),
                resource_type: ResourceType::Cluster,
                data: vec![0x0a, 0x08],
            }],
            type_url: ResourceType::Cluster.type_url().to_string(),
            nonce: "n-3".to_string(),
            control_plane_id: "batata".to_string(),
        };

        let wire = to_wire_response(resp);
        assert_eq!(wire.version_info, "3");
        assert_eq!(wire.resources.len(), 1);
        assert_eq!(wire.resources[0].type_url, ResourceType::Cluster.type_url());
        assert_eq!(wire.resources[0].value, vec![0x0a, 0x08]);
    }

    #[test]
    fn test_delta_response_does_not_emit_control_plane_on_field_7() {
        // Upstream DeltaDiscoveryResponse field 7 is `udpa_removed_resources`,
        // not control_plane. A real Envoy would decode a field-7 ControlPlane
        // message as ResourceName entries and can NACK the whole response.
        use ::prost::Message;

        let resp = InternalResponse {
            version_info: "9".to_string(),
            resources: vec![],
            type_url: ResourceType::Cluster.type_url().to_string(),
            nonce: "n-9".to_string(),
            control_plane_id: "SENTINEL_CP_ID_MUST_NOT_APPEAR".to_string(),
        };

        let wire = to_wire_delta_response(resp, vec![]);
        assert_eq!(wire.system_version_info, "9");

        let encoded = wire.encode_to_vec();
        // The sentinel (from control_plane_id) must not leak onto the wire.
        let sentinel = b"SENTINEL_CP_ID_MUST_NOT_APPEAR";
        assert!(
            encoded.windows(sentinel.len()).all(|w| w != sentinel),
            "DeltaDiscoveryResponse must not encode a ControlPlane on field 7"
        );
        // Field 7 wire tag for a length-delimited field is 0x3a. With an empty
        // removed_resources list and no control plane, no field-7 key exists.
        assert!(
            !encoded.contains(&0x3a),
            "unexpected field-7 key on DeltaDiscoveryResponse"
        );
    }
}
