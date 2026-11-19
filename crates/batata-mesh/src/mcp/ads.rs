//! Minimal Istio config-source ADS service on the Nacos-compatible MCP port.
//!
//! Upstream Nacos registers both `ResourceSource` and Envoy
//! `AggregatedDiscoveryService` on the same plaintext gRPC server. Istio's
//! `configSources: xds://...` client opens the ADS stream and requests every
//! Pilot collection GVK. This service mirrors the upstream subset needed for
//! `ServiceEntry`: full-state MCP resources for that GVK, empty full-state
//! responses for the other collections, and no response for MeshConfig.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use tokio::sync::{broadcast, mpsc};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Streaming};
use tracing::{debug, info, warn};

use crate::xds::generated::discovery::aggregated_discovery_service_server::{
    AggregatedDiscoveryService, AggregatedDiscoveryServiceServer,
};
use crate::xds::generated::discovery::{
    DeltaDiscoveryResponse, DiscoveryRequest, DiscoveryResponse,
};

use super::encode::{MESH_CONFIG_GVK_TYPE_URL, SERVICE_ENTRY_GVK_TYPE_URL, encode_ads_resources};
use super::grpc::{BatataMcpService, McpSnapshot};

/// Istio config-source ADS facade over the existing MCP snapshot source.
#[derive(Clone)]
pub struct BatataIstioMcpAdsService {
    mcp: BatataMcpService,
}

impl BatataIstioMcpAdsService {
    /// Create an ADS config source backed by the MCP service cache.
    pub fn new(mcp: BatataMcpService) -> Self {
        Self { mcp }
    }

    /// Wrap this service in the generated tonic server type.
    pub fn into_server(self) -> AggregatedDiscoveryServiceServer<Self> {
        AggregatedDiscoveryServiceServer::new(self)
    }

    fn build_response(&self, type_url: &str, snapshot: &McpSnapshot) -> DiscoveryResponse {
        if type_url == SERVICE_ENTRY_GVK_TYPE_URL {
            return encode_ads_resources(
                &snapshot.entries,
                snapshot.version,
                self.mcp.next_nonce(),
            );
        }

        DiscoveryResponse {
            version_info: snapshot.version.to_string(),
            resources: Vec::new(),
            canary: false,
            type_url: type_url.to_string(),
            nonce: self.mcp.next_nonce(),
            control_plane: None,
        }
    }
}

/// Upstream Nacos ignores MeshConfig on both MCP-over-xDS paths because of an
/// Istio client bug. Returning no response is the compatibility behavior.
fn is_ignored_type(type_url: &str) -> bool {
    type_url == MESH_CONFIG_GVK_TYPE_URL
}

#[tonic::async_trait]
impl AggregatedDiscoveryService for BatataIstioMcpAdsService {
    type StreamAggregatedResourcesStream = ReceiverStream<Result<DiscoveryResponse, Status>>;
    type DeltaAggregatedResourcesStream =
        tokio_stream::Empty<Result<DeltaDiscoveryResponse, Status>>;

    async fn stream_aggregated_resources(
        &self,
        request: Request<Streaming<DiscoveryRequest>>,
    ) -> Result<Response<Self::StreamAggregatedResourcesStream>, Status> {
        let mut inbound = request.into_inner();
        let mut updates = self.mcp.subscribe();
        let (tx, rx) = mpsc::channel(16);
        let service = self.clone();
        let mut watched: HashSet<String> = HashSet::new();
        let mut last_nonce: HashMap<String, String> = HashMap::new();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    incoming = inbound.message() => {
                        let request = match incoming {
                            Ok(Some(request)) => request,
                            Ok(None) => break,
                            Err(error) => {
                                warn!(%error, "Istio MCP ADS stream receive failed");
                                break;
                            }
                        };

                        let type_url = request.type_url;
                        if is_ignored_type(&type_url) {
                            debug!(type_url = %type_url, "ignoring MCP ADS config type");
                            continue;
                        }

                        if let Some(detail) = request.error_detail.as_ref()
                            && detail.code != 0
                        {
                            warn!(
                                type_url = %type_url,
                                code = detail.code,
                                message = %detail.message,
                                nonce = %request.response_nonce,
                                "Istio MCP ADS sink NACK"
                            );
                            continue;
                        }

                        if let Some(nonce) = last_nonce.get(&type_url)
                            && !request.response_nonce.is_empty()
                        {
                            if request.response_nonce == *nonce {
                                debug!(type_url = %type_url, nonce = %nonce, "Istio MCP ADS ACK");
                            } else {
                                warn!(
                                    type_url = %type_url,
                                    got = %request.response_nonce,
                                    expected = %nonce,
                                    "Istio MCP ADS nonce mismatch"
                                );
                            }
                            continue;
                        }

                        watched.insert(type_url.clone());
                        let snapshot = service.mcp.current_snapshot();
                        let response = service.build_response(&type_url, &snapshot);
                        last_nonce.insert(type_url, response.nonce.clone());
                        if tx.send(Ok(response)).await.is_err() {
                            break;
                        }
                    }
                    update = updates.recv() => {
                        use broadcast::error::RecvError;

                        let snapshot = match update {
                            Ok(snapshot) => snapshot,
                            Err(RecvError::Lagged(skipped)) => {
                                warn!(skipped, "Istio MCP ADS sink lagged; resending current state");
                                Arc::new(service.mcp.current_snapshot())
                            }
                            Err(RecvError::Closed) => break,
                        };

                        let mut failed = false;
                        for type_url in watched.iter() {
                            let response = service.build_response(type_url, &snapshot);
                            last_nonce.insert(type_url.clone(), response.nonce.clone());
                            if tx.send(Ok(response)).await.is_err() {
                                failed = true;
                                break;
                            }
                        }
                        if failed {
                            break;
                        }
                    }
                }
            }
        });

        info!("Istio MCP ADS config stream established");
        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn delta_aggregated_resources(
        &self,
        _request: Request<Streaming<crate::xds::generated::discovery::DeltaDiscoveryRequest>>,
    ) -> Result<Response<Self::DeltaAggregatedResourcesStream>, Status> {
        Err(Status::unimplemented("MCP-over-xDS delta is not supported"))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use prost::Message;

    use crate::mcp::types::{Location, Protocol, Resolution, ServiceEntry, ServiceEntrySpec};
    use crate::xds::generated::mcp;

    use super::*;

    fn sample_entry(name: &str) -> ServiceEntry {
        ServiceEntry {
            metadata: super::super::types::ResourceMetadata {
                name: name.to_string(),
                namespace: "public".to_string(),
                resource_version: "7".to_string(),
                ..Default::default()
            },
            spec: ServiceEntrySpec {
                hosts: vec![format!("{name}nacos")],
                ports: vec![super::super::types::Port {
                    number: 0,
                    name: "http".to_string(),
                    protocol: Protocol::Http,
                    target_port: None,
                }],
                resolution: Resolution::Static,
                location: Location::MeshInternal,
                endpoints: Vec::new(),
                export_to: Vec::new(),
                ..Default::default()
            },
        }
    }

    #[test]
    fn mesh_config_type_is_ignored() {
        assert!(is_ignored_type("core/v1alpha1/MeshConfig"));
        assert!(!is_ignored_type(
            "networking.istio.io/v1alpha3/ServiceEntry"
        ));
    }

    #[test]
    fn service_entry_response_uses_upstream_gvk_and_mcp_resource_wrapper() {
        let service = BatataIstioMcpAdsService::new(BatataMcpService::new(Arc::new(
            crate::mcp::McpServer::new(Default::default()),
        )));
        let snapshot = McpSnapshot {
            version: 9,
            entries: vec![sample_entry("zeta"), sample_entry("alpha")],
            nonce: "snapshot-nonce".to_string(),
        };

        let response =
            service.build_response("networking.istio.io/v1alpha3/ServiceEntry", &snapshot);

        assert_eq!(response.version_info, "9");
        assert_eq!(
            response.type_url,
            "networking.istio.io/v1alpha3/ServiceEntry"
        );
        assert_eq!(response.resources.len(), 2);
        let first = &response.resources[0];
        assert_eq!(
            first.type_url,
            "type.googleapis.com/istio.mcp.v1alpha1.Resource"
        );
        let resource = mcp::Resource::decode(first.value.as_slice()).expect("decode MCP resource");
        assert_eq!(resource.metadata.expect("metadata").name, "public/alpha");
    }

    #[test]
    fn unknown_type_gets_empty_full_state_response() {
        let service = BatataIstioMcpAdsService::new(BatataMcpService::new(Arc::new(
            crate::mcp::McpServer::new(Default::default()),
        )));
        let snapshot = McpSnapshot {
            version: 3,
            entries: vec![sample_entry("web")],
            nonce: "snapshot-nonce".to_string(),
        };

        let response =
            service.build_response("networking.istio.io/v1alpha3/VirtualService", &snapshot);

        assert_eq!(response.version_info, "3");
        assert_eq!(
            response.type_url,
            "networking.istio.io/v1alpha3/VirtualService"
        );
        assert!(response.resources.is_empty());
        assert!(!response.nonce.is_empty());
    }

    #[test]
    fn ads_nonce_is_not_the_resource_source_snapshot_nonce() {
        let service = BatataIstioMcpAdsService::new(BatataMcpService::new(Arc::new(
            crate::mcp::McpServer::new(Default::default()),
        )));
        let snapshot = McpSnapshot {
            version: 1,
            entries: Vec::new(),
            nonce: "resource-source-nonce".to_string(),
        };

        let response =
            service.build_response("networking.istio.io/v1alpha3/ServiceEntry", &snapshot);

        assert_ne!(response.nonce, "resource-source-nonce");
        assert_eq!(response.nonce.len(), 32);
    }

    #[test]
    fn empty_entries_still_emit_full_state_signal() {
        let service = BatataIstioMcpAdsService::new(BatataMcpService::new(Arc::new(
            crate::mcp::McpServer::new(Default::default()),
        )));
        let snapshot = McpSnapshot {
            version: 11,
            entries: Vec::new(),
            nonce: "snapshot-nonce".to_string(),
        };

        let response =
            service.build_response("networking.istio.io/v1alpha3/ServiceEntry", &snapshot);

        assert_eq!(response.version_info, "11");
        assert!(response.resources.is_empty());
        assert_eq!(
            response.type_url,
            "networking.istio.io/v1alpha3/ServiceEntry"
        );
    }

    #[test]
    fn map_helpers_are_deterministic() {
        let mut map = HashMap::new();
        map.insert("a".to_string(), "1".to_string());
        assert_eq!(map.get("a").map(String::as_str), Some("1"));
    }
}
