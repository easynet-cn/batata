//! Consul Connect/Service Mesh API
//!
//! Provides discovery chain, exported services, and imported services endpoints.

use actix_web::{HttpRequest, HttpResponse, web};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::acl::{AclService, ResourceType};
use crate::agent::ConsulAgentService;
use crate::config_entry::ConsulConfigEntryService;
use crate::consul_meta::{ConsulResponseMeta, consul_ok};
use crate::index_provider::{ConsulIndexProvider, ConsulTable};
use crate::model::{AgentServiceRegistration, ConsulDatacenterConfig, ConsulError, ConsulErrorBody};

// ============================================================================
// Envoy Bootstrap Models (for /v1/connect/proxy/:service_id)
// ============================================================================

/// Envoy v3 bootstrap configuration returned by `/v1/connect/proxy/:service_id`.
///
/// Provides the sidecar proxy with its node identity, admin endpoint, and
/// xDS (ADS) configuration pointing back at the local Consul agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvoyBootstrapConfig {
/// The `node` field.
    pub node: EnvoyNode,
/// The `admin` field.
    pub admin: EnvoyAdmin,
/// The `dynamic_resources` field.
    pub dynamic_resources: EnvoyDynamicResources,
/// The `static_resources` field.
    pub static_resources: EnvoyStaticResources,
}

/// Envoy node identity presented to the xDS management server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvoyNode {
/// The `id` field — proxy service ID.
    pub id: String,
/// The `cluster` field — datacenter name.
    pub cluster: String,
/// The `metadata` field — Consul-specific node metadata.
    pub metadata: HashMap<String, String>,
}

/// Envoy admin endpoint configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvoyAdmin {
/// The `access_log_path` field.
    pub access_log_path: String,
/// The `address` field.
    pub address: EnvoySocketAddress,
}

/// Socket address used in admin and cluster endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvoySocketAddress {
/// The `socket_address` field.
    pub socket_address: EnvoySocketAddressInner,
}

/// Inner socket address with address and port.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvoySocketAddressInner {
/// The `address` field.
    pub address: String,
/// The `port_value` field.
    pub port_value: u16,
}

/// Dynamic xDS resources (LDS + CDS via aggregated discovery service).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvoyDynamicResources {
/// The `lds_config` field.
    pub lds_config: EnvoyAdsConfig,
/// The `cds_config` field.
    pub cds_config: EnvoyAdsConfig,
/// The `ads_config` field.
    pub ads_config: EnvoyAdsApiConfigSource,
}

/// Minimal ADS config pointing at the aggregated discovery service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvoyAdsConfig {
/// The `ads` field — empty object to enable ADS.
    pub ads: serde_json::Value,
/// The `resource_api_version` field.
    pub resource_api_version: String,
}

/// API config source for the aggregated discovery service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvoyAdsApiConfigSource {
/// The `api_type` field.
    pub api_type: String,
/// The `transport_api_version` field.
    pub transport_api_version: String,
/// The `grpc_services` field.
    pub grpc_services: Vec<EnvoyGrpcService>,
}

/// gRPC service definition pointing at the local agent cluster.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvoyGrpcService {
/// The `envoy_grpc` field.
    pub envoy_grpc: EnvoyGrpcServiceInner,
}

/// Inner gRPC service with cluster name reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvoyGrpcServiceInner {
/// The `cluster_name` field.
    pub cluster_name: String,
}

/// Static resources containing the local_agent xDS cluster.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvoyStaticResources {
/// The `clusters` field.
    pub clusters: Vec<EnvoyCluster>,
}

/// Envoy cluster definition for the local agent xDS server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvoyCluster {
/// The `name` field.
    pub name: String,
/// The `connect_timeout` field.
    pub connect_timeout: String,
/// The `type` field.
    #[serde(rename = "type")]
    pub cluster_type: String,
/// The `typed_extension_protocol_options` field.
    pub typed_extension_protocol_options: serde_json::Value,
/// The `load_assignment` field.
    pub load_assignment: EnvoyLoadAssignment,
}

/// Load assignment for the local agent cluster.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvoyLoadAssignment {
/// The `cluster_name` field.
    pub cluster_name: String,
/// The `endpoints` field.
    pub endpoints: Vec<EnvoyLocalityLbEndpoints>,
}

/// Locality load balancer endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvoyLocalityLbEndpoints {
/// The `lb_endpoints` field.
    pub lb_endpoints: Vec<EnvoyLbEndpoint>,
}

/// A single load balancer endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvoyLbEndpoint {
/// The `endpoint` field.
    pub endpoint: EnvoyEndpoint,
}

/// Endpoint address wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvoyEndpoint {
/// The `address` field.
    pub address: EnvoySocketAddress,
}

// ============================================================================
// Discovery Chain Models
// ============================================================================

/// Graph node types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DiscoveryGraphNodeType {
/// The `Router` variant.
    Router,
/// The `Splitter` variant.
    Splitter,
/// The `Resolver` variant.
    Resolver,
}

/// A route match condition
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DiscoveryRouteMatch {
    #[serde(rename = "HTTP")]
/// The `http` field.
    pub http: Option<DiscoveryHTTPRouteMatch>,
}

/// HTTP route match
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DiscoveryHTTPRouteMatch {
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `path_exact` field.
    pub path_exact: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `path_prefix` field.
    pub path_prefix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `path_regex` field.
    pub path_regex: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
/// The `header` field.
    pub header: Vec<DiscoveryHTTPHeaderMatch>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
/// The `query_param` field.
    pub query_param: Vec<DiscoveryHTTPQueryMatch>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
/// The `methods` field.
    pub methods: Vec<String>,
}

/// HTTP header match
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DiscoveryHTTPHeaderMatch {
/// The `name` field.
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `exact` field.
    pub exact: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `prefix` field.
    pub prefix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `suffix` field.
    pub suffix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `regex` field.
    pub regex: Option<String>,
    #[serde(default)]
/// The `present` field.
    pub present: bool,
    #[serde(default)]
/// The `invert` field.
    pub invert: bool,
}

/// HTTP query parameter match
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DiscoveryHTTPQueryMatch {
/// The `name` field.
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `exact` field.
    pub exact: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `regex` field.
    pub regex: Option<String>,
    #[serde(default)]
/// The `present` field.
    pub present: bool,
}

/// A route definition within a router node
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DiscoveryRoute {
/// The `definition` field.
    pub definition: Option<DiscoveryRouteMatch>,
/// The `next_node` field.
    pub next_node: String,
}

/// A split definition within a splitter node
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DiscoverySplit {
/// The `definition` field.
    pub definition: Option<DiscoverySplitDefinition>,
/// The `weight` field.
    pub weight: f64,
/// The `next_node` field.
    pub next_node: String,
}

/// Split definition details
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DiscoverySplitDefinition {
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `service` field.
    pub service: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `service_subset` field.
    pub service_subset: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `namespace` field.
    pub namespace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `partition` field.
    pub partition: Option<String>,
}

/// Resolver configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DiscoveryResolver {
/// The `default` field.
    pub default: bool,
/// The `connect_timeout` field.
    pub connect_timeout: String,
/// The `target` field.
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
/// The `failover` field.
    pub failover: Option<DiscoveryFailover>,
}

/// Failover configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DiscoveryFailover {
/// The `targets` field.
    pub targets: Vec<String>,
}

/// A node in the discovery graph
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DiscoveryGraphNode {
    #[serde(rename = "Type")]
/// The `node_type` field.
    pub node_type: DiscoveryGraphNodeType,
/// The `name` field.
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
/// The `routes` field.
    pub routes: Vec<DiscoveryRoute>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
/// The `splits` field.
    pub splits: Vec<DiscoverySplit>,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `resolver` field.
    pub resolver: Option<DiscoveryResolver>,
}

/// A target in the discovery chain
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DiscoveryTarget {
    #[serde(rename = "ID")]
/// The `id` field.
    pub id: String,
/// The `service` field.
    pub service: String,
/// The `service_subset` field.
    pub service_subset: String,
/// The `namespace` field.
    pub namespace: String,
/// The `partition` field.
    pub partition: String,
/// The `datacenter` field.
    pub datacenter: String,
    #[serde(rename = "MeshGateway")]
/// The `mesh_gateway` field.
    pub mesh_gateway: MeshGatewayConfig,
/// The `subset` field.
    pub subset: DiscoveryTargetSubset,
/// The `connect_timeout` field.
    pub connect_timeout: String,
    #[serde(rename = "SNI")]
/// The `sni` field.
    pub sni: String,
/// The `name` field.
    pub name: String,
}

/// Mesh gateway configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MeshGatewayConfig {
    #[serde(default)]
/// The `mode` field.
    pub mode: String,
}

/// Subset configuration for a target
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DiscoveryTargetSubset {
    #[serde(default)]
/// The `filter` field.
    pub filter: String,
    #[serde(default)]
/// The `only_passing` field.
    pub only_passing: bool,
}

/// The compiled discovery chain
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CompiledDiscoveryChain {
/// The `service_name` field.
    pub service_name: String,
/// The `namespace` field.
    pub namespace: String,
/// The `datacenter` field.
    pub datacenter: String,
    #[serde(default)]
/// The `customization_hash` field.
    pub customization_hash: String,
/// The `protocol` field.
    pub protocol: String,
/// The `start_node` field.
    pub start_node: String,
/// The `nodes` field.
    pub nodes: HashMap<String, DiscoveryGraphNode>,
/// The `targets` field.
    pub targets: HashMap<String, DiscoveryTarget>,
}

/// Discovery chain API response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DiscoveryChainResponse {
/// The `chain` field.
    pub chain: CompiledDiscoveryChain,
}

// ============================================================================
// Exported/Imported Services Models
// ============================================================================

/// Resolved consumer info
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ResolvedConsumers {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
/// The `peers` field.
    pub peers: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
/// The `partitions` field.
    pub partitions: Vec<String>,
}

/// An exported service entry
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ResolvedExportedService {
/// The `service` field.
    pub service: String,
/// The `consumers` field.
    pub consumers: ResolvedConsumers,
}

/// An imported service entry
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ImportedService {
/// The `service` field.
    pub service: String,
    #[serde(default)]
/// The `source_peer` field.
    pub source_peer: String,
}

// ============================================================================
// Query Parameters
// ============================================================================

/// Query parameters for discovery chain
#[derive(Debug, Deserialize)]
pub struct DiscoveryChainQueryParams {
/// The `dc` field.
    pub dc: Option<String>,
/// The `ns` field.
    pub ns: Option<String>,
/// The `partition` field.
    pub partition: Option<String>,
    #[serde(rename = "compile-dc")]
/// The `compile_dc` field.
    pub compile_dc: Option<String>,
}

/// Request body for POST /v1/discovery-chain/{service} with overrides
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DiscoveryChainOverrides {
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `override_protocol` field.
    pub override_protocol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `override_connect_timeout` field.
    pub override_connect_timeout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
/// The `override_mesh_gateway` field.
    pub override_mesh_gateway: Option<MeshGatewayConfig>,
}

/// Query parameters for exported/imported services
#[derive(Debug, Deserialize)]
pub struct ServiceVisibilityQueryParams {
/// The `partition` field.
    pub partition: Option<String>,
}

// ============================================================================
// Service (In-Memory)
// ============================================================================

/// In-memory connect service for discovery chain and service visibility
#[derive(Clone)]
pub struct ConsulConnectService {
    /// Exported services configuration
    exported_services: Arc<DashMap<String, ResolvedExportedService>>,
    /// Imported services configuration
    imported_services: Arc<DashMap<String, ImportedService>>,
    /// Optional config entry service for building discovery chains from config entries
    config_entry_service: Option<Arc<ConsulConfigEntryService>>,
    /// Optional peering service for merging peering-imported services
    peering_service: Option<Arc<crate::peering::ConsulPeeringService>>,
    /// Datacenter name
    datacenter: String,
}

impl ConsulConnectService {
/// The `new` associated function.
    pub fn new() -> Self {
        Self::with_datacenter("dc1".to_string())
    }

/// The `with_datacenter` associated function.
    pub fn with_datacenter(datacenter: String) -> Self {
        Self {
            exported_services: Arc::new(DashMap::new()),
            imported_services: Arc::new(DashMap::new()),
            config_entry_service: None,
            peering_service: None,
            datacenter,
        }
    }

    /// Set the config entry service for building discovery chains from config entries
    pub fn with_config_entry_service(mut self, service: Arc<ConsulConfigEntryService>) -> Self {
        self.config_entry_service = Some(service);
        self
    }

    /// Set the peering service so imported services are derived from active
    /// peerings as the single source of truth.
    pub fn with_peering_service(
        mut self,
        service: Arc<crate::peering::ConsulPeeringService>,
    ) -> Self {
        self.peering_service = Some(service);
        self
    }

    /// Get the compiled discovery chain for a service.
    /// Builds the chain from config entries (service-router, service-splitter, service-resolver)
    /// if available, otherwise returns a default chain with a single resolver node.
    ///
    /// Compilation handles:
    /// - Service subsets from service-resolver (each subset becomes a DiscoveryTarget)
    /// - DefaultSubset routing (resolver points to the default subset target)
    /// - Failover targets (each becomes a resolver node + DiscoveryTarget)
    /// - Mesh gateway mode (from service-resolver or proxy-defaults)
    /// - SNI generation (local DC vs cross-DC/mesh gateway)
    /// - Connect timeout and protocol merging (service-resolver/service-defaults -> proxy-defaults)
    pub fn get_discovery_chain(&self, service_name: &str) -> DiscoveryChainResponse {
        let default_ns = "default".to_string();
        let default_partition = "default".to_string();
        let trust_domain = "consul";

        // Look up config entries if the config entry service is available
        let router_entry = self
            .config_entry_service
            .as_ref()
            .and_then(|s| s.get_entry("service-router", service_name));
        let splitter_entry = self
            .config_entry_service
            .as_ref()
            .and_then(|s| s.get_entry("service-splitter", service_name));
        let resolver_entry = self
            .config_entry_service
            .as_ref()
            .and_then(|s| s.get_entry("service-resolver", service_name));
        let proxy_defaults_entry = self
            .config_entry_service
            .as_ref()
            .and_then(|s| s.get_entry("proxy-defaults", "global"));
        let service_defaults_entry = self
            .config_entry_service
            .as_ref()
            .and_then(|s| s.get_entry("service-defaults", service_name));

        // Resolve mesh gateway mode: service-resolver > proxy-defaults > "none"
        let mesh_gateway_mode = Self::resolve_mesh_gateway_mode(
            resolver_entry.as_ref(),
            proxy_defaults_entry.as_ref(),
        );

        // Resolve connect timeout: service-resolver > proxy-defaults > "5s"
        let connect_timeout = Self::resolve_connect_timeout(
            resolver_entry.as_ref(),
            proxy_defaults_entry.as_ref(),
        );

        // Resolve protocol: service-defaults > proxy-defaults > "tcp"
        let protocol = Self::resolve_protocol(
            service_defaults_entry.as_ref(),
            proxy_defaults_entry.as_ref(),
        );

        let mut nodes = HashMap::new();
        let mut targets: HashMap<String, DiscoveryTarget> = HashMap::new();

        // ---- Build the default target ----
        let default_target_id = format!(
            "{}.{}.{}.{}",
            service_name, default_ns, default_partition, self.datacenter
        );
        let default_target = Self::build_target(
            &default_target_id,
            service_name,
            "",
            &default_ns,
            &default_partition,
            &self.datacenter,
            &self.datacenter,
            &mesh_gateway_mode,
            &connect_timeout,
            trust_domain,
            DiscoveryTargetSubset::default(),
        );
        targets.insert(default_target_id.clone(), default_target);

        // ---- Determine the resolver's primary target (respect DefaultSubset) ----
        let default_subset = resolver_entry
            .as_ref()
            .and_then(|e| e.extra.get("DefaultSubset"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let primary_target_id = if !default_subset.is_empty() {
            format!(
                "{}.{}.{}.{}.{}",
                service_name, default_subset, default_ns, default_partition, self.datacenter
            )
        } else {
            default_target_id.clone()
        };

        // ---- Build subset targets from service-resolver ----
        if let Some(ref resolver) = resolver_entry
            && let Some(subsets) = resolver.extra.get("Subsets").and_then(|v| v.as_object())
        {
            for (subset_name, subset_val) in subsets {
                let subset_target_id = format!(
                    "{}.{}.{}.{}.{}",
                    service_name, subset_name, default_ns, default_partition, self.datacenter
                );
                let filter = subset_val
                    .get("Filter")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let only_passing = subset_val
                    .get("OnlyPassing")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let subset_target = Self::build_target(
                    &subset_target_id,
                    service_name,
                    subset_name,
                    &default_ns,
                    &default_partition,
                    &self.datacenter,
                    &self.datacenter,
                    &mesh_gateway_mode,
                    &connect_timeout,
                    trust_domain,
                    DiscoveryTargetSubset { filter, only_passing },
                );
                targets.insert(subset_target_id, subset_target);
            }
        }

        // ---- Compile failover targets ----
        let failover_targets: Vec<String> = resolver_entry
            .as_ref()
            .and_then(|e| e.extra.get("Failover"))
            .and_then(|v| v.as_object())
            .and_then(|obj| obj.get("Targets"))
            .and_then(|t| t.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let mut failover_resolver_names = Vec::new();
        for failover_target_str in &failover_targets {
            let (fo_service, fo_ns, fo_partition, fo_dc) =
                Self::parse_failover_target(
                    failover_target_str,
                    &default_ns,
                    &default_partition,
                    &self.datacenter,
                );
            let fo_target_id = format!(
                "{}.{}.{}.{}",
                fo_service, fo_ns, fo_partition, fo_dc
            );
            let fo_resolver_name = format!("resolver:{}", fo_target_id);
            let fo_target = Self::build_target(
                &fo_target_id,
                &fo_service,
                "",
                &fo_ns,
                &fo_partition,
                &fo_dc,
                &self.datacenter,
                &mesh_gateway_mode,
                &connect_timeout,
                trust_domain,
                DiscoveryTargetSubset::default(),
            );
            targets.insert(fo_target_id.clone(), fo_target);

            // Failover resolver nodes do NOT carry their own failover config
            // to avoid infinite recursion loops.
            nodes.insert(
                fo_resolver_name.clone(),
                DiscoveryGraphNode {
                    node_type: DiscoveryGraphNodeType::Resolver,
                    name: fo_resolver_name.clone(),
                    routes: Vec::new(),
                    splits: Vec::new(),
                    resolver: Some(DiscoveryResolver {
                        default: false,
                        connect_timeout: connect_timeout.clone(),
                        target: fo_target_id,
                        failover: None,
                    }),
                },
            );
            failover_resolver_names.push(fo_resolver_name);
        }

        // ---- Build router / splitter nodes ----
        let resolver_key = format!("resolver:{}", default_target_id);
        let mut start_node = resolver_key.clone();

        if let Some(ref router) = router_entry {
            let router_name = format!("router:{}", default_target_id);
            let routes = Self::extract_routes_from_entry(router, service_name, &self.datacenter);
            nodes.insert(
                router_name.clone(),
                DiscoveryGraphNode {
                    node_type: DiscoveryGraphNodeType::Router,
                    name: router_name.clone(),
                    routes,
                    splits: Vec::new(),
                    resolver: None,
                },
            );
            start_node = router_name;
        }

        if let Some(ref splitter) = splitter_entry {
            let splitter_name = format!("splitter:{}", default_target_id);
            let splits = Self::extract_splits_from_entry(splitter, service_name, &self.datacenter);
            nodes.insert(
                splitter_name.clone(),
                DiscoveryGraphNode {
                    node_type: DiscoveryGraphNodeType::Splitter,
                    name: splitter_name.clone(),
                    routes: Vec::new(),
                    splits,
                    resolver: None,
                },
            );
            if router_entry.is_none() {
                start_node = splitter_name;
            }
        }

        // ---- Build the primary resolver node ----
        let resolver_node = DiscoveryGraphNode {
            node_type: DiscoveryGraphNodeType::Resolver,
            name: resolver_key.clone(),
            routes: Vec::new(),
            splits: Vec::new(),
            resolver: Some(DiscoveryResolver {
                default: resolver_entry.is_none(),
                connect_timeout: connect_timeout.clone(),
                target: primary_target_id,
                failover: if failover_resolver_names.is_empty() {
                    None
                } else {
                    Some(DiscoveryFailover {
                        targets: failover_resolver_names,
                    })
                },
            }),
        };
        nodes.insert(resolver_key, resolver_node);

        DiscoveryChainResponse {
            chain: CompiledDiscoveryChain {
                service_name: service_name.to_string(),
                namespace: default_ns.clone(),
                datacenter: self.datacenter.clone(),
                customization_hash: String::new(),
                protocol,
                start_node,
                nodes,
                targets,
            },
        }
    }

    /// Build a DiscoveryTarget with correct SNI based on DC and mesh gateway mode.
    /// `local_dc` is the datacenter of the compiling agent, used to decide
    /// whether the target is cross-DC.
    fn build_target(
        id: &str,
        service: &str,
        service_subset: &str,
        namespace: &str,
        partition: &str,
        datacenter: &str,
        local_dc: &str,
        mesh_gateway_mode: &str,
        connect_timeout: &str,
        trust_domain: &str,
        subset: DiscoveryTargetSubset,
    ) -> DiscoveryTarget {
        // SNI differs for cross-DC / mesh-gateway targets
        let sni = if mesh_gateway_mode == "none" && local_dc == datacenter {
            format!("{}.{}.{}.internal", service, namespace, datacenter)
        } else {
            format!(
                "{}.{}.{}.{}.alt.consul",
                service, namespace, datacenter, trust_domain
            )
        };
        DiscoveryTarget {
            id: id.to_string(),
            service: service.to_string(),
            service_subset: service_subset.to_string(),
            namespace: namespace.to_string(),
            partition: partition.to_string(),
            datacenter: datacenter.to_string(),
            mesh_gateway: MeshGatewayConfig {
                mode: mesh_gateway_mode.to_string(),
            },
            subset,
            connect_timeout: connect_timeout.to_string(),
            sni,
            name: format!("{}.{}.{}", service, namespace, datacenter),
        }
    }

    /// Parse a failover target string into (service, namespace, partition, datacenter).
    /// Supported formats: `service`, `service.namespace`, `service.namespace.datacenter`.
    /// Partition always defaults to "default" (Consul OSS).
    /// When the datacenter segment is omitted, `default_dc` is used.
    fn parse_failover_target(
        target: &str,
        default_ns: &str,
        default_partition: &str,
        default_dc: &str,
    ) -> (String, String, String, String) {
        let parts: Vec<&str> = target.split('.').collect();
        let service = parts.first().copied().unwrap_or(target).to_string();
        let namespace = parts.get(1).copied().unwrap_or(default_ns).to_string();
        let datacenter = parts.get(2).copied().unwrap_or(default_dc).to_string();
        (service, namespace, default_partition.to_string(), datacenter)
    }

    /// Resolve mesh gateway mode: service-resolver > proxy-defaults > "none".
    fn resolve_mesh_gateway_mode(
        resolver_entry: Option<&crate::config_entry::ConfigEntry>,
        proxy_defaults_entry: Option<&crate::config_entry::ConfigEntry>,
    ) -> String {
        if let Some(mode) = resolver_entry
            .and_then(|e| e.extra.get("MeshGateway"))
            .and_then(|m| m.get("Mode"))
            .and_then(|v| v.as_str())
        {
            return mode.to_string();
        }
        if let Some(mode) = proxy_defaults_entry
            .and_then(|e| e.extra.get("MeshGateway"))
            .and_then(|m| m.get("Mode"))
            .and_then(|v| v.as_str())
        {
            return mode.to_string();
        }
        "none".to_string()
    }

    /// Resolve connect timeout: service-resolver > proxy-defaults > "5s".
    fn resolve_connect_timeout(
        resolver_entry: Option<&crate::config_entry::ConfigEntry>,
        proxy_defaults_entry: Option<&crate::config_entry::ConfigEntry>,
    ) -> String {
        if let Some(ct) = resolver_entry
            .and_then(|e| e.extra.get("ConnectTimeout"))
            .and_then(|v| v.as_str())
        {
            return ct.to_string();
        }
        if let Some(ct) = proxy_defaults_entry
            .and_then(|e| e.extra.get("ConnectTimeout"))
            .and_then(|v| v.as_str())
        {
            return ct.to_string();
        }
        "5s".to_string()
    }

    /// Resolve protocol: service-defaults > proxy-defaults > "tcp".
    fn resolve_protocol(
        service_defaults_entry: Option<&crate::config_entry::ConfigEntry>,
        proxy_defaults_entry: Option<&crate::config_entry::ConfigEntry>,
    ) -> String {
        if let Some(p) = service_defaults_entry
            .and_then(|e| e.extra.get("Protocol"))
            .and_then(|v| v.as_str())
        {
            return p.to_string();
        }
        if let Some(p) = proxy_defaults_entry
            .and_then(|e| e.extra.get("Protocol"))
            .and_then(|v| v.as_str())
        {
            return p.to_string();
        }
        "tcp".to_string()
    }

    /// Build an Envoy v3 bootstrap configuration for the given proxy service.
    ///
    /// The returned config gives a sidecar proxy its node identity, admin
    /// endpoint, and an ADS (aggregated xDS) configuration that points Envoy
    /// at the local agent's gRPC port for LDS/CDS.
    pub fn build_envoy_bootstrap(
        &self,
        service_id: &str,
        service_name: &str,
        namespace: &str,
        admin_port: u16,
        grpc_port: u16,
        node_name: &str,
    ) -> EnvoyBootstrapConfig {
        let mut metadata = HashMap::new();
        metadata.insert("CONSUL_DC".to_string(), self.datacenter.clone());
        metadata.insert("CONSUL_NODE_NAME".to_string(), node_name.to_string());
        metadata.insert("CONSUL_SERVICE_NAME".to_string(), service_name.to_string());
        metadata.insert("CONSUL_SERVICE_ID".to_string(), service_id.to_string());
        metadata.insert("CONSUL_NAMESPACE".to_string(), namespace.to_string());
        metadata.insert("CONSUL_PARTITION".to_string(), "default".to_string());
        metadata.insert("CONSUL_PROXY_ID".to_string(), service_id.to_string());
        metadata.insert("CONSUL_PROXY_SERVICE_NAME".to_string(), service_name.to_string());

        EnvoyBootstrapConfig {
            node: EnvoyNode {
                id: service_id.to_string(),
                cluster: self.datacenter.clone(),
                metadata,
            },
            admin: EnvoyAdmin {
                access_log_path: "/dev/null".to_string(),
                address: EnvoySocketAddress {
                    socket_address: EnvoySocketAddressInner {
                        address: "127.0.0.1".to_string(),
                        port_value: admin_port,
                    },
                },
            },
            dynamic_resources: EnvoyDynamicResources {
                lds_config: EnvoyAdsConfig {
                    ads: serde_json::json!({}),
                    resource_api_version: "V3".to_string(),
                },
                cds_config: EnvoyAdsConfig {
                    ads: serde_json::json!({}),
                    resource_api_version: "V3".to_string(),
                },
                ads_config: EnvoyAdsApiConfigSource {
                    api_type: "GRPC".to_string(),
                    transport_api_version: "V3".to_string(),
                    grpc_services: vec![EnvoyGrpcService {
                        envoy_grpc: EnvoyGrpcServiceInner {
                            cluster_name: "local_agent".to_string(),
                        },
                    }],
                },
            },
            static_resources: EnvoyStaticResources {
                clusters: vec![EnvoyCluster {
                    name: "local_agent".to_string(),
                    connect_timeout: "1s".to_string(),
                    cluster_type: "STRICT_DNS".to_string(),
                    typed_extension_protocol_options: serde_json::json!({
                        "envoy.extensions.upstreams.http.v3.HttpProtocolOptions": {
                            "@type": "type.googleapis.com/envoy.extensions.upstreams.http.v3.HttpProtocolOptions",
                            "explicit_http_config": {
                                "http2_protocol_options": {}
                            }
                        }
                    }),
                    load_assignment: EnvoyLoadAssignment {
                        cluster_name: "local_agent".to_string(),
                        endpoints: vec![EnvoyLocalityLbEndpoints {
                            lb_endpoints: vec![EnvoyLbEndpoint {
                                endpoint: EnvoyEndpoint {
                                    address: EnvoySocketAddress {
                                        socket_address: EnvoySocketAddressInner {
                                            address: "127.0.0.1".to_string(),
                                            port_value: grpc_port,
                                        },
                                    },
                                },
                            }],
                        }],
                    },
                }],
            },
        }
    }

    /// Extract route definitions from a service-router config entry
    fn extract_routes_from_entry(
        entry: &crate::config_entry::ConfigEntry,
        service_name: &str,
        datacenter: &str,
    ) -> Vec<DiscoveryRoute> {
        let mut routes = Vec::new();

        if let Some(routes_val) = entry.extra.get("Routes")
            && let Some(routes_arr) = routes_val.as_array()
        {
            for route in routes_arr {
                #[allow(clippy::bind_instead_of_map)]
                let match_def = route.get("Match").and_then(|m| {
                    let http = m.get("HTTP").map(|http| DiscoveryHTTPRouteMatch {
                        path_exact: http
                            .get("PathExact")
                            .and_then(|v| v.as_str().map(|s| s.to_string())),
                        path_prefix: http
                            .get("PathPrefix")
                            .and_then(|v| v.as_str().map(|s| s.to_string())),
                        path_regex: http
                            .get("PathRegex")
                            .and_then(|v| v.as_str().map(|s| s.to_string())),
                        header: Vec::new(),
                        query_param: Vec::new(),
                        methods: http
                            .get("Methods")
                            .and_then(|v| v.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                    .collect()
                            })
                            .unwrap_or_default(),
                    });
                    Some(DiscoveryRouteMatch { http })
                });

                let next_service = route
                    .get("Destination")
                    .and_then(|d| d.get("Service"))
                    .and_then(|v| v.as_str())
                    .unwrap_or(service_name);
                let next_node = format!("resolver:{}.default.default.{}", next_service, datacenter);

                routes.push(DiscoveryRoute {
                    definition: match_def,
                    next_node,
                });
            }
        }

        // Always add a default catch-all route to the resolver
        if routes.is_empty() {
            routes.push(DiscoveryRoute {
                definition: None,
                next_node: format!("resolver:{}.default.default.{}", service_name, datacenter),
            });
        }

        routes
    }

    /// Extract split definitions from a service-splitter config entry
    fn extract_splits_from_entry(
        entry: &crate::config_entry::ConfigEntry,
        service_name: &str,
        datacenter: &str,
    ) -> Vec<DiscoverySplit> {
        let mut splits = Vec::new();

        if let Some(splits_val) = entry.extra.get("Splits")
            && let Some(splits_arr) = splits_val.as_array()
        {
            for split in splits_arr {
                let weight = split
                    .get("Weight")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(100.0);
                let target_service = split
                    .get("Service")
                    .and_then(|v| v.as_str())
                    .unwrap_or(service_name);
                let service_subset = split
                    .get("ServiceSubset")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let next_node =
                    format!("resolver:{}.default.default.{}", target_service, datacenter);

                splits.push(DiscoverySplit {
                    definition: Some(DiscoverySplitDefinition {
                        service: Some(target_service.to_string()),
                        service_subset,
                        namespace: split
                            .get("Namespace")
                            .and_then(|v| v.as_str().map(|s| s.to_string())),
                        partition: split
                            .get("Partition")
                            .and_then(|v| v.as_str().map(|s| s.to_string())),
                    }),
                    weight,
                    next_node,
                });
            }
        }

        splits
    }

    /// Get the compiled discovery chain with optional overrides applied.
    pub fn get_discovery_chain_with_overrides(
        &self,
        service_name: &str,
        overrides: &DiscoveryChainOverrides,
    ) -> DiscoveryChainResponse {
        let mut response = self.get_discovery_chain(service_name);

        // Apply protocol override
        if let Some(ref protocol) = overrides.override_protocol {
            response.chain.protocol = protocol.clone();
        }

        // Apply connect_timeout override to all resolvers and targets
        if let Some(ref timeout) = overrides.override_connect_timeout {
            for node in response.chain.nodes.values_mut() {
                if let Some(ref mut resolver) = node.resolver {
                    resolver.connect_timeout = timeout.clone();
                }
            }
            for target in response.chain.targets.values_mut() {
                target.connect_timeout = timeout.clone();
            }
        }

        // Apply mesh gateway override to all targets
        if let Some(ref mesh_gw) = overrides.override_mesh_gateway {
            for target in response.chain.targets.values_mut() {
                target.mesh_gateway = mesh_gw.clone();
            }
        }

        response
    }

/// The `list_exported_services` method.
    pub fn list_exported_services(&self) -> Vec<ResolvedExportedService> {
        let mut services: Vec<ResolvedExportedService> = self
            .exported_services
            .iter()
            .map(|r| r.value().clone())
            .collect();
        services.sort_by(|a, b| a.service.cmp(&b.service));
        services
    }

/// The `list_imported_services` method.
    pub fn list_imported_services(&self) -> Vec<ImportedService> {
        use std::collections::BTreeSet;

        // Start with services explicitly added via add_imported_service.
        let mut services: Vec<ImportedService> = self
            .imported_services
            .iter()
            .map(|r| r.value().clone())
            .collect();

        // Merge in services imported through active peerings (single source of
        // truth for peering-derived imports). Deduplicate by (service, peer).
        if let Some(ref peering) = self.peering_service {
            let seen: BTreeSet<(String, String)> = services
                .iter()
                .map(|s| (s.service.clone(), s.source_peer.clone()))
                .collect();
            for (svc, peer) in peering.list_all_imported_service_names() {
                if !seen.contains(&(svc.clone(), peer.clone())) {
                    services.push(ImportedService {
                        service: svc,
                        source_peer: peer,
                    });
                }
            }
        }

        services.sort_by(|a, b| a.service.cmp(&b.service));
        services
    }

    /// Add an exported service (used by config entries or peering)
    pub fn add_exported_service(&self, service: ResolvedExportedService) {
        self.exported_services
            .insert(service.service.clone(), service);
    }

    /// Add an imported service (used by peering)
    pub fn add_imported_service(&self, service: ImportedService) {
        self.imported_services.insert(
            format!("{}:{}", service.service, service.source_peer),
            service,
        );
    }
}

impl Default for ConsulConnectService {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// HTTP Handlers (In-Memory)
// ============================================================================

/// GET /v1/discovery-chain/{service} - Read discovery chain
pub async fn get_discovery_chain(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    connect_service: web::Data<ConsulConnectService>,
    path: web::Path<String>,
    _query: web::Query<DiscoveryChainQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let service_name = path.into_inner();
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Catalog));
    consul_ok(&meta).json(connect_service.get_discovery_chain(&service_name))
}

/// Query parameters for the proxy config endpoint
#[derive(Debug, Deserialize, Default)]
pub struct ProxyConfigQueryParams {
/// The `ns` field — namespace (defaults to "default").
    pub ns: Option<String>,
}

/// GET /v1/connect/proxy/{service_id} - Envoy bootstrap config for a proxy service
///
/// Returns the Envoy v3 bootstrap configuration (node identity, admin endpoint,
/// ADS xDS config) for the registered proxy service identified by `service_id`.
pub async fn get_proxy_config(
    req: HttpRequest,
    agent: web::Data<ConsulAgentService>,
    acl_service: web::Data<AclService>,
    connect_service: web::Data<ConsulConnectService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    index_provider: web::Data<ConsulIndexProvider>,
    path: web::Path<String>,
    query: web::Query<ProxyConfigQueryParams>,
) -> HttpResponse {
    let service_id = path.into_inner();
    let namespace = dc_config.resolve_ns(&query.ns);

    // ACL: proxy config carries service identity — require service:read.
    let authz = acl_service.authorize_request(&req, ResourceType::Service, &service_id, false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    // Resolve the service registration by ID.
    let Some(data) = agent.naming_store().get_by_service_id(&namespace, &service_id) else {
        return HttpResponse::NotFound()
            .consul_error(ConsulError::new(format!("Service not found: {}", service_id)));
    };
    let Ok(reg) = serde_json::from_slice::<AgentServiceRegistration>(&data) else {
        return HttpResponse::InternalServerError()
            .consul_error(ConsulError::new("Failed to decode service registration"));
    };
    let service_name = reg.name.clone();

    // Extract the Envoy admin bind port from proxy.config, default 19000.
    let admin_port = reg
        .proxy
        .as_ref()
        .and_then(|p| p.get("Config"))
        .and_then(|c| c.get("envoy_admin_bind_port"))
        .and_then(|v| v.as_u64())
        .map(|p| p as u16)
        .unwrap_or(19000);

    let bootstrap = connect_service.build_envoy_bootstrap(
        &service_id,
        &service_name,
        &namespace,
        admin_port,
        dc_config.grpc_port,
        &dc_config.node_name,
    );

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Catalog));
    consul_ok(&meta).json(bootstrap)
}

/// GET /v1/exported-services - List exported services
pub async fn list_exported_services(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    connect_service: web::Data<ConsulConnectService>,
    _query: web::Query<ServiceVisibilityQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Catalog));
    consul_ok(&meta).json(connect_service.list_exported_services())
}

/// GET /v1/imported-services - List imported services
pub async fn list_imported_services(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    connect_service: web::Data<ConsulConnectService>,
    _query: web::Query<ServiceVisibilityQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Catalog));
    consul_ok(&meta).json(connect_service.list_imported_services())
}

/// POST /v1/discovery-chain/{service} - Read discovery chain with overrides
pub async fn post_discovery_chain(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    connect_service: web::Data<ConsulConnectService>,
    path: web::Path<String>,
    _query: web::Query<DiscoveryChainQueryParams>,
    body: web::Json<DiscoveryChainOverrides>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let service_name = path.into_inner();
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Catalog));
    consul_ok(&meta)
        .json(connect_service.get_discovery_chain_with_overrides(&service_name, &body.into_inner()))
}

// ============================================================================
// HTTP Handlers (Persistent)
// ============================================================================

/// GET /v1/discovery-chain/{service} (persistent)
pub async fn get_discovery_chain_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    connect_service: web::Data<ConsulConnectService>,
    path: web::Path<String>,
    _query: web::Query<DiscoveryChainQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let service_name = path.into_inner();
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Catalog));
    consul_ok(&meta).json(connect_service.get_discovery_chain(&service_name))
}

/// POST /v1/discovery-chain/{service} (persistent)
pub async fn post_discovery_chain_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    connect_service: web::Data<ConsulConnectService>,
    path: web::Path<String>,
    _query: web::Query<DiscoveryChainQueryParams>,
    body: web::Json<DiscoveryChainOverrides>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let service_name = path.into_inner();
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Catalog));
    consul_ok(&meta)
        .json(connect_service.get_discovery_chain_with_overrides(&service_name, &body.into_inner()))
}

/// GET /v1/exported-services (persistent)
pub async fn list_exported_services_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    connect_service: web::Data<ConsulConnectService>,
    _query: web::Query<ServiceVisibilityQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Catalog));
    consul_ok(&meta).json(connect_service.list_exported_services())
}

/// GET /v1/imported-services (persistent)
pub async fn list_imported_services_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    connect_service: web::Data<ConsulConnectService>,
    _query: web::Query<ServiceVisibilityQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Catalog));
    consul_ok(&meta).json(connect_service.list_imported_services())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discovery_chain_default() {
        let service = ConsulConnectService::new();
        let chain = service.get_discovery_chain("web");
        assert_eq!(chain.chain.service_name, "web");
        assert_eq!(chain.chain.protocol, "tcp");
        assert_eq!(chain.chain.namespace, "default");
        assert_eq!(chain.chain.datacenter, "dc1");
        assert!(!chain.chain.start_node.is_empty());
        assert!(chain.chain.start_node.contains("resolver:"));
        assert_eq!(chain.chain.nodes.len(), 1);
        assert_eq!(chain.chain.targets.len(), 1);
    }

    #[test]
    fn test_discovery_chain_resolver_target() {
        let service = ConsulConnectService::new();
        let chain = service.get_discovery_chain("api");
        let node = chain.chain.nodes.values().next().unwrap();
        assert_eq!(node.node_type, DiscoveryGraphNodeType::Resolver);
        assert!(node.resolver.as_ref().unwrap().default);
        assert_eq!(node.resolver.as_ref().unwrap().connect_timeout, "5s");
    }

    #[test]
    fn test_exported_services() {
        let service = ConsulConnectService::new();
        service.add_exported_service(ResolvedExportedService {
            service: "web".to_string(),
            consumers: ResolvedConsumers {
                peers: vec!["east".to_string()],
                partitions: vec![],
            },
        });
        service.add_exported_service(ResolvedExportedService {
            service: "api".to_string(),
            consumers: ResolvedConsumers {
                peers: vec!["west".to_string()],
                partitions: vec![],
            },
        });

        let exported = service.list_exported_services();
        assert_eq!(exported.len(), 2);
        assert_eq!(exported[0].service, "api"); // sorted
        assert_eq!(exported[1].service, "web");
        assert_eq!(exported[0].consumers.peers, vec!["west"]);
        assert_eq!(exported[1].consumers.peers, vec!["east"]);
    }

    #[test]
    fn test_imported_services() {
        let service = ConsulConnectService::new();
        service.add_imported_service(ImportedService {
            service: "db".to_string(),
            source_peer: "east".to_string(),
        });

        let imported = service.list_imported_services();
        assert_eq!(imported.len(), 1);
        assert_eq!(imported[0].service, "db");
        assert_eq!(imported[0].source_peer, "east");
    }

    #[test]
    fn test_empty_services() {
        let service = ConsulConnectService::new();
        assert!(service.list_exported_services().is_empty());
        assert!(service.list_imported_services().is_empty());
    }

    #[test]
    fn test_discovery_chain_different_services() {
        let service = ConsulConnectService::new();

        let chain1 = service.get_discovery_chain("svc-a");
        let chain2 = service.get_discovery_chain("svc-b");

        assert_eq!(chain1.chain.service_name, "svc-a");
        assert_eq!(chain2.chain.service_name, "svc-b");

        // Each chain should have its own target
        let target1 = chain1.chain.targets.values().next().unwrap();
        let target2 = chain2.chain.targets.values().next().unwrap();
        assert_eq!(target1.service, "svc-a");
        assert_eq!(target2.service, "svc-b");
    }

    #[test]
    fn test_discovery_chain_target_defaults() {
        let service = ConsulConnectService::new();
        let chain = service.get_discovery_chain("my-service");

        let target = chain.chain.targets.values().next().unwrap();
        assert_eq!(target.service, "my-service");
        assert_eq!(target.namespace, "default");
        assert_eq!(target.datacenter, "dc1");
        assert_eq!(target.connect_timeout, "5s");
    }

    #[test]
    fn test_exported_services_sorted() {
        let service = ConsulConnectService::new();

        service.add_exported_service(ResolvedExportedService {
            service: "zzz".to_string(),
            consumers: ResolvedConsumers {
                peers: vec![],
                partitions: vec![],
            },
        });
        service.add_exported_service(ResolvedExportedService {
            service: "aaa".to_string(),
            consumers: ResolvedConsumers {
                peers: vec![],
                partitions: vec![],
            },
        });

        let exported = service.list_exported_services();
        assert_eq!(exported[0].service, "aaa");
        assert_eq!(exported[1].service, "zzz");
    }

    #[test]
    fn test_imported_services_sorted() {
        let service = ConsulConnectService::new();

        service.add_imported_service(ImportedService {
            service: "zeta".to_string(),
            source_peer: "peer-1".to_string(),
        });
        service.add_imported_service(ImportedService {
            service: "alpha".to_string(),
            source_peer: "peer-2".to_string(),
        });

        let imported = service.list_imported_services();
        assert_eq!(imported[0].service, "alpha");
        assert_eq!(imported[1].service, "zeta");
    }

    #[test]
    fn test_exported_service_with_partitions() {
        let service = ConsulConnectService::new();

        service.add_exported_service(ResolvedExportedService {
            service: "shared-svc".to_string(),
            consumers: ResolvedConsumers {
                peers: vec!["peer-east".to_string()],
                partitions: vec!["partition-1".to_string(), "partition-2".to_string()],
            },
        });

        let exported = service.list_exported_services();
        assert_eq!(exported[0].consumers.partitions.len(), 2);
        assert_eq!(exported[0].consumers.peers.len(), 1);
    }

    #[test]
    fn test_imported_services_merged_from_peering() {
        use crate::peering::{Peering, PeeringRemoteInfo, PeeringState, PeeringStreamStatus};
        use std::sync::Arc;

        let peering = Arc::new(crate::peering::ConsulPeeringService::new());
        // Seed an active peering with imported services.
        peering.peerings_for_test().insert(
            "peer-east".to_string(),
            Peering {
                id: uuid::Uuid::new_v4().to_string(),
                name: "peer-east".to_string(),
                partition: String::new(),
                state: PeeringState::Active,
                peer_id: uuid::Uuid::new_v4().to_string(),
                peer_server_name: String::new(),
                peer_server_addresses: Vec::new(),
                peer_ca_pems: Vec::new(),
                meta: Default::default(),
                stream_status: PeeringStreamStatus {
                    imported_services: vec!["db".to_string(), "cache".to_string()],
                    ..Default::default()
                },
                create_index: 1,
                modify_index: 1,
                remote: PeeringRemoteInfo::default(),
                deleted_at: None,
            },
        );

        let service = ConsulConnectService::new().with_peering_service(peering);

        // Also add a manual imported service.
        service.add_imported_service(ImportedService {
            service: "api".to_string(),
            source_peer: "peer-west".to_string(),
        });

        let imported = service.list_imported_services();
        let names: Vec<&str> = imported.iter().map(|i| i.service.as_str()).collect();
        // Sorted: api, cache, db
        assert_eq!(names, vec!["api", "cache", "db"]);

        let db_entry = imported.iter().find(|i| i.service == "db").unwrap();
        assert_eq!(db_entry.source_peer, "peer-east");
    }

    // ========================================================================
    // Discovery chain compilation helper tests
    // ========================================================================

    #[test]
    fn test_parse_failover_target_service_only() {
        let (svc, ns, part, dc) = ConsulConnectService::parse_failover_target(
            "web", "default", "default", "dc1",
        );
        assert_eq!(svc, "web");
        assert_eq!(ns, "default");
        assert_eq!(part, "default");
        assert_eq!(dc, "dc1"); // defaults to local DC
    }

    #[test]
    fn test_parse_failover_target_with_namespace() {
        let (svc, ns, part, dc) = ConsulConnectService::parse_failover_target(
            "web.team-a", "default", "default", "dc1",
        );
        assert_eq!(svc, "web");
        assert_eq!(ns, "team-a");
        assert_eq!(part, "default");
        assert_eq!(dc, "dc1");
    }

    #[test]
    fn test_parse_failover_target_with_namespace_and_dc() {
        let (svc, ns, part, dc) = ConsulConnectService::parse_failover_target(
            "web.team-a.dc2", "default", "default", "dc1",
        );
        assert_eq!(svc, "web");
        assert_eq!(ns, "team-a");
        assert_eq!(part, "default");
        assert_eq!(dc, "dc2");
    }

    #[test]
    fn test_resolve_mesh_gateway_mode_default_none() {
        let mode = ConsulConnectService::resolve_mesh_gateway_mode(None, None);
        assert_eq!(mode, "none");
    }

    #[test]
    fn test_resolve_mesh_gateway_mode_from_proxy_defaults() {
        let mut extra = HashMap::new();
        extra.insert(
            "MeshGateway".to_string(),
            serde_json::json!({"Mode": "local"}),
        );
        let proxy_defaults = crate::config_entry::ConfigEntry {
            kind: "proxy-defaults".to_string(),
            name: "global".to_string(),
            namespace: None,
            partition: None,
            meta: None,
            extra,
            create_index: 1,
            modify_index: 1,
        };
        let mode = ConsulConnectService::resolve_mesh_gateway_mode(None, Some(&proxy_defaults));
        assert_eq!(mode, "local");
    }

    #[test]
    fn test_resolve_mesh_gateway_mode_resolver_overrides_proxy_defaults() {
        let mut resolver_extra = HashMap::new();
        resolver_extra.insert(
            "MeshGateway".to_string(),
            serde_json::json!({"Mode": "remote"}),
        );
        let resolver = crate::config_entry::ConfigEntry {
            kind: "service-resolver".to_string(),
            name: "web".to_string(),
            namespace: None,
            partition: None,
            meta: None,
            extra: resolver_extra,
            create_index: 1,
            modify_index: 1,
        };

        let mut proxy_extra = HashMap::new();
        proxy_extra.insert(
            "MeshGateway".to_string(),
            serde_json::json!({"Mode": "local"}),
        );
        let proxy_defaults = crate::config_entry::ConfigEntry {
            kind: "proxy-defaults".to_string(),
            name: "global".to_string(),
            namespace: None,
            partition: None,
            meta: None,
            extra: proxy_extra,
            create_index: 1,
            modify_index: 1,
        };

        let mode =
            ConsulConnectService::resolve_mesh_gateway_mode(Some(&resolver), Some(&proxy_defaults));
        assert_eq!(mode, "remote"); // resolver takes precedence
    }

    #[test]
    fn test_resolve_connect_timeout_default() {
        let ct = ConsulConnectService::resolve_connect_timeout(None, None);
        assert_eq!(ct, "5s");
    }

    #[test]
    fn test_resolve_connect_timeout_from_resolver() {
        let mut extra = HashMap::new();
        extra.insert("ConnectTimeout".to_string(), serde_json::json!("10s"));
        let resolver = crate::config_entry::ConfigEntry {
            kind: "service-resolver".to_string(),
            name: "web".to_string(),
            namespace: None,
            partition: None,
            meta: None,
            extra,
            create_index: 1,
            modify_index: 1,
        };
        let ct = ConsulConnectService::resolve_connect_timeout(Some(&resolver), None);
        assert_eq!(ct, "10s");
    }

    #[test]
    fn test_resolve_protocol_default() {
        let p = ConsulConnectService::resolve_protocol(None, None);
        assert_eq!(p, "tcp");
    }

    #[test]
    fn test_resolve_protocol_from_service_defaults() {
        let mut extra = HashMap::new();
        extra.insert("Protocol".to_string(), serde_json::json!("http"));
        let sd = crate::config_entry::ConfigEntry {
            kind: "service-defaults".to_string(),
            name: "web".to_string(),
            namespace: None,
            partition: None,
            meta: None,
            extra,
            create_index: 1,
            modify_index: 1,
        };
        let p = ConsulConnectService::resolve_protocol(Some(&sd), None);
        assert_eq!(p, "http");
    }

    #[test]
    fn test_build_target_sni_local_no_mesh_gateway() {
        let target = ConsulConnectService::build_target(
            "web.default.default.dc1",
            "web",
            "",
            "default",
            "default",
            "dc1",
            "dc1", // local_dc == datacenter => local
            "none",
            "5s",
            "consul",
            DiscoveryTargetSubset::default(),
        );
        // Local DC + no mesh gateway => .internal SNI
        assert_eq!(target.sni, "web.default.dc1.internal");
        assert_eq!(target.mesh_gateway.mode, "none");
    }

    #[test]
    fn test_build_target_sni_cross_dc_mesh_gateway() {
        let target = ConsulConnectService::build_target(
            "web.default.default.dc2",
            "web",
            "",
            "default",
            "default",
            "dc2",
            "dc1", // local_dc != datacenter => cross-DC
            "local",
            "5s",
            "consul",
            DiscoveryTargetSubset::default(),
        );
        // Cross-DC or mesh gateway => .alt.consul SNI
        assert!(target.sni.ends_with(".alt.consul"));
        assert!(target.sni.contains("dc2"));
        assert_eq!(target.mesh_gateway.mode, "local");
    }
}
