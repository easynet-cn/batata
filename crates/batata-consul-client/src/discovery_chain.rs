//! Discovery Chain API.
//!
//! Maps to Consul Go SDK's `api/discovery_chain.go`. Used by Envoy proxies
//! to resolve service-to-service mesh topology at the L7 layer.
//!
//! Endpoint: `GET /v1/discovery-chain/{service}` (or POST when options set).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::client::ConsulClient;
use crate::error::Result;
use crate::model::{QueryMeta, QueryOptions};

/// Options passed when resolving a discovery chain.
/// Wire-compatible with Consul's `DiscoveryChainOptions`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DiscoveryChainOptions {
    /// EvaluateInDatacenter overrides the target datacenter for compilation.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub evaluate_in_datacenter: String,
    /// EvaluateInNamespace overrides the namespace.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub evaluate_in_namespace: String,
    /// EvaluateInPartition overrides the admin partition.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub evaluate_in_partition: String,
    /// Override mesh-gateway mode.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub override_mesh_gateway: String,
    /// Override protocol for all resolvers in the chain.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub override_protocol: String,
    /// Override connect-timeout for all resolvers.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub override_connect_timeout: String,
}

impl DiscoveryChainOptions {
    /// Matches Go `requiresPOST()` — POST is needed when any override is set.
    pub fn requires_post(&self) -> bool {
        !self.evaluate_in_datacenter.is_empty()
            || !self.evaluate_in_namespace.is_empty()
            || !self.evaluate_in_partition.is_empty()
            || !self.override_mesh_gateway.is_empty()
            || !self.override_protocol.is_empty()
            || !self.override_connect_timeout.is_empty()
    }
}

/// Top-level response from the discovery-chain endpoint.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DiscoveryChainResponse {
    /// The compiled discovery chain.
    pub chain: CompiledDiscoveryChain,
}

/// The compiled chain for a service.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CompiledDiscoveryChain {
    /// The service name the chain was compiled for.
    pub service_name: String,
    /// The namespace of the service.
    pub namespace: Option<String>,
    /// The admin partition of the service.
    pub partition: Option<String>,
    /// The datacenter the chain was compiled for.
    pub datacenter: String,
    /// Whether the chain is the default chain.
    pub default: Option<bool>,
    /// Whether a custom node was used.
    pub custom_node: Option<bool>,
    /// The protocol used by the chain.
    pub protocol: String,
    /// Metadata attached to the service.
    pub service_meta: Option<HashMap<String, String>>,
    /// The name of the starting graph node.
    pub start_node: Option<String>,
    /// The graph nodes keyed by name.
    pub nodes: Option<HashMap<String, DiscoveryGraphNode>>,
    /// The chain targets keyed by name.
    pub targets: Option<HashMap<String, DiscoveryTarget>>,
}

/// A node in the graph (resolver / splitter / router).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DiscoveryGraphNode {
    #[serde(rename = "Type")]
    /// The node type: resolver, splitter or router.
    pub node_type: String,
    /// The name of the node.
    pub name: String,
    /// Routes originating from this node.
    pub routes: Option<Vec<DiscoveryRoute>>,
    /// Splits originating from this node.
    pub splits: Option<Vec<DiscoverySplit>>,
    /// The resolver attached to this node.
    pub resolver: Option<DiscoveryResolver>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
/// Represents a `DiscoveryRoute`.
pub struct DiscoveryRoute {
    /// The raw route definition.
    pub definition: Option<serde_json::Value>,
    /// The name of the next node to visit.
    pub next_node: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
/// Represents a `DiscoverySplit`.
pub struct DiscoverySplit {
    /// The traffic weight of this split.
    pub weight: f32,
    /// The name of the next node to visit.
    pub next_node: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
/// Represents a `DiscoveryResolver`.
pub struct DiscoveryResolver {
    /// The connect timeout as a duration string.
    pub connect_timeout: Option<String>,
    /// The request timeout as a duration string.
    pub request_timeout: Option<String>,
    /// The name of the resolver target.
    pub target: String,
    /// Whether this is the default resolver.
    pub default: Option<bool>,
    /// The failover configuration.
    pub failover: Option<DiscoveryFailover>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
/// Represents a `DiscoveryFailover`.
pub struct DiscoveryFailover {
    /// Ordered list of failover targets.
    pub targets: Option<Vec<String>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
/// Represents a `DiscoveryTarget`.
pub struct DiscoveryTarget {
    #[serde(rename = "ID", default)]
    /// The target ID.
    pub id: String,
    /// The service the target points to.
    pub service: String,
    /// The service subset name.
    pub service_subset: Option<String>,
    /// The namespace of the target.
    pub namespace: Option<String>,
    /// The admin partition of the target.
    pub partition: Option<String>,
    /// The datacenter of the target.
    pub datacenter: Option<String>,
    /// The mesh gateway configuration.
    pub mesh_gateway: Option<serde_json::Value>,
    /// The subset configuration.
    pub subset: Option<serde_json::Value>,
    /// The connect timeout as a duration string.
    pub connect_timeout: Option<String>,
    /// The SNI name used for TLS.
    pub sni: Option<String>,
    /// The display name of the target.
    pub name: Option<String>,
    /// Whether the target is disabled.
    pub disabled: Option<bool>,
}

impl ConsulClient {
    /// Get the compiled discovery chain for a service.
    ///
    /// If `options` has any override set, POSTs the options as the request
    /// body (matching Go SDK `requiresPOST` path).
    pub async fn discovery_chain_get(
        &self,
        service: &str,
        options: Option<&DiscoveryChainOptions>,
        q: &QueryOptions,
    ) -> Result<(DiscoveryChainResponse, QueryMeta)> {
        let path = format!("/v1/discovery-chain/{}", service);
        match options {
            Some(opts) if opts.requires_post() => {
                use crate::model::WriteOptions;
                let wo = WriteOptions {
                    datacenter: q.datacenter.clone(),
                    token: q.token.clone(),
                    ..Default::default()
                };
                let (resp, _wm): (DiscoveryChainResponse, _) =
                    self.put(&path, Some(opts), &wo, &[]).await?;
                Ok((resp, QueryMeta::default()))
            }
            _ => self.get(&path, q).await,
        }
    }
}
