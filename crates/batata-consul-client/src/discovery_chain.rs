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
    pub chain: CompiledDiscoveryChain,
}

/// The compiled chain for a service.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CompiledDiscoveryChain {
    pub service_name: String,
    pub namespace: Option<String>,
    pub partition: Option<String>,
    pub datacenter: String,
    pub default: Option<bool>,
    pub custom_node: Option<bool>,
    pub protocol: String,
    pub service_meta: Option<HashMap<String, String>>,
    pub start_node: Option<String>,
    pub nodes: Option<HashMap<String, DiscoveryGraphNode>>,
    pub targets: Option<HashMap<String, DiscoveryTarget>>,
}

/// A node in the graph (resolver / splitter / router).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DiscoveryGraphNode {
    #[serde(rename = "Type")]
    pub node_type: String,
    pub name: String,
    pub routes: Option<Vec<DiscoveryRoute>>,
    pub splits: Option<Vec<DiscoverySplit>>,
    pub resolver: Option<DiscoveryResolver>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DiscoveryRoute {
    pub definition: Option<serde_json::Value>,
    pub next_node: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DiscoverySplit {
    pub weight: f32,
    pub next_node: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DiscoveryResolver {
    pub connect_timeout: Option<String>,
    pub request_timeout: Option<String>,
    pub target: String,
    pub default: Option<bool>,
    pub failover: Option<DiscoveryFailover>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DiscoveryFailover {
    pub targets: Option<Vec<String>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DiscoveryTarget {
    #[serde(rename = "ID", default)]
    pub id: String,
    pub service: String,
    pub service_subset: Option<String>,
    pub namespace: Option<String>,
    pub partition: Option<String>,
    pub datacenter: Option<String>,
    pub mesh_gateway: Option<serde_json::Value>,
    pub subset: Option<serde_json::Value>,
    pub connect_timeout: Option<String>,
    pub sni: Option<String>,
    pub name: Option<String>,
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
