//! Consul Connect/Service Mesh API
//!
//! Provides discovery chain, exported services, and imported services endpoints.

use actix_web::{HttpRequest, HttpResponse, web};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::acl::{AclService, ResourceType};
use crate::config_entry::ConsulConfigEntryService;
use crate::consul_meta::{ConsulResponseMeta, consul_ok};
use crate::index_provider::{ConsulIndexProvider, ConsulTable};
use crate::model::ConsulError;
use crate::model::ConsulErrorBody;

// ============================================================================
// Discovery Chain Models
// ============================================================================

/// Graph node types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DiscoveryGraphNodeType {
    Router,
    Splitter,
    Resolver,
}

/// A route match condition
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DiscoveryRouteMatch {
    #[serde(rename = "HTTP")]
    pub http: Option<DiscoveryHTTPRouteMatch>,
}

/// HTTP route match
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DiscoveryHTTPRouteMatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_exact: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_prefix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_regex: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub header: Vec<DiscoveryHTTPHeaderMatch>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub query_param: Vec<DiscoveryHTTPQueryMatch>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub methods: Vec<String>,
}

/// HTTP header match
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DiscoveryHTTPHeaderMatch {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exact: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suffix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub regex: Option<String>,
    #[serde(default)]
    pub present: bool,
    #[serde(default)]
    pub invert: bool,
}

/// HTTP query parameter match
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DiscoveryHTTPQueryMatch {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exact: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub regex: Option<String>,
    #[serde(default)]
    pub present: bool,
}

/// A route definition within a router node
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DiscoveryRoute {
    pub definition: Option<DiscoveryRouteMatch>,
    pub next_node: String,
}

/// A split definition within a splitter node
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DiscoverySplit {
    pub definition: Option<DiscoverySplitDefinition>,
    pub weight: f64,
    pub next_node: String,
}

/// Split definition details
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DiscoverySplitDefinition {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_subset: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partition: Option<String>,
}

/// Resolver configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DiscoveryResolver {
    pub default: bool,
    pub connect_timeout: String,
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failover: Option<DiscoveryFailover>,
}

/// Failover configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DiscoveryFailover {
    pub targets: Vec<String>,
}

/// A node in the discovery graph
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DiscoveryGraphNode {
    #[serde(rename = "Type")]
    pub node_type: DiscoveryGraphNodeType,
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub routes: Vec<DiscoveryRoute>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub splits: Vec<DiscoverySplit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolver: Option<DiscoveryResolver>,
}

/// A target in the discovery chain
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DiscoveryTarget {
    #[serde(rename = "ID")]
    pub id: String,
    pub service: String,
    pub service_subset: String,
    pub namespace: String,
    pub partition: String,
    pub datacenter: String,
    #[serde(rename = "MeshGateway")]
    pub mesh_gateway: MeshGatewayConfig,
    pub subset: DiscoveryTargetSubset,
    pub connect_timeout: String,
    #[serde(rename = "SNI")]
    pub sni: String,
    pub name: String,
}

/// Mesh gateway configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MeshGatewayConfig {
    #[serde(default)]
    pub mode: String,
}

/// Subset configuration for a target
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DiscoveryTargetSubset {
    #[serde(default)]
    pub filter: String,
    #[serde(default)]
    pub only_passing: bool,
}

/// The compiled discovery chain
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CompiledDiscoveryChain {
    pub service_name: String,
    pub namespace: String,
    pub datacenter: String,
    #[serde(default)]
    pub customization_hash: String,
    pub protocol: String,
    pub start_node: String,
    pub nodes: HashMap<String, DiscoveryGraphNode>,
    pub targets: HashMap<String, DiscoveryTarget>,
}

/// Discovery chain API response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DiscoveryChainResponse {
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
    pub peers: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub partitions: Vec<String>,
}

/// An exported service entry
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ResolvedExportedService {
    pub service: String,
    pub consumers: ResolvedConsumers,
}

/// An imported service entry
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ImportedService {
    pub service: String,
    #[serde(default)]
    pub source_peer: String,
}

// ============================================================================
// Query Parameters
// ============================================================================

/// Query parameters for discovery chain
#[derive(Debug, Deserialize)]
pub struct DiscoveryChainQueryParams {
    pub dc: Option<String>,
    pub ns: Option<String>,
    pub partition: Option<String>,
    #[serde(rename = "compile-dc")]
    pub compile_dc: Option<String>,
}

/// Request body for POST /v1/discovery-chain/{service} with overrides
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DiscoveryChainOverrides {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub override_protocol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub override_connect_timeout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub override_mesh_gateway: Option<MeshGatewayConfig>,
}

/// Query parameters for exported/imported services
#[derive(Debug, Deserialize)]
pub struct ServiceVisibilityQueryParams {
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
    /// Datacenter name
    datacenter: String,
}

impl ConsulConnectService {
    pub fn new() -> Self {
        Self::with_datacenter("dc1".to_string())
    }

    pub fn with_datacenter(datacenter: String) -> Self {
        Self {
            exported_services: Arc::new(DashMap::new()),
            imported_services: Arc::new(DashMap::new()),
            config_entry_service: None,
            datacenter,
        }
    }

    /// Set the config entry service for building discovery chains from config entries
    pub fn with_config_entry_service(mut self, service: Arc<ConsulConfigEntryService>) -> Self {
        self.config_entry_service = Some(service);
        self
    }

    /// Get the compiled discovery chain for a service.
    /// Builds the chain from config entries (service-router, service-splitter, service-resolver)
    /// if available, otherwise returns a default chain with a single resolver node.
    pub fn get_discovery_chain(&self, service_name: &str) -> DiscoveryChainResponse {
        let target_id = format!(
            "{}.default.default.{}.internal",
            service_name, self.datacenter
        );
        let resolver_key = format!(
            "resolver:{}.default.default.{}",
            service_name, self.datacenter
        );

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

        // Determine connect timeout from resolver config entry or use default
        let connect_timeout = resolver_entry
            .as_ref()
            .and_then(|e| e.extra.get("ConnectTimeout"))
            .and_then(|v| v.as_str())
            .unwrap_or("5s")
            .to_string();

        // Determine protocol from service-defaults config entry or use default
        let protocol = self
            .config_entry_service
            .as_ref()
            .and_then(|s| s.get_entry("service-defaults", service_name))
            .and_then(|e| {
                e.extra
                    .get("Protocol")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| "tcp".to_string());

        let mut nodes = HashMap::new();
        let mut start_node = resolver_key.clone();

        // If a service-router config entry exists, add a router node
        if let Some(ref router) = router_entry {
            let router_name = format!(
                "router:{}.default.default.{}",
                service_name, self.datacenter
            );
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

        // If a service-splitter config entry exists, add a splitter node
        if let Some(ref splitter) = splitter_entry {
            let splitter_name = format!(
                "splitter:{}.default.default.{}",
                service_name, self.datacenter
            );
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

        // Always add the resolver node
        let resolver_node = DiscoveryGraphNode {
            node_type: DiscoveryGraphNodeType::Resolver,
            name: resolver_key.clone(),
            routes: Vec::new(),
            splits: Vec::new(),
            resolver: Some(DiscoveryResolver {
                default: resolver_entry.is_none(),
                connect_timeout: connect_timeout.clone(),
                target: target_id.clone(),
                failover: resolver_entry
                    .as_ref()
                    .and_then(|e| e.extra.get("Failover"))
                    .and_then(|v| v.as_object())
                    .and_then(|obj| {
                        obj.get("Targets")
                            .and_then(|t| t.as_array())
                            .map(|arr| DiscoveryFailover {
                                targets: arr
                                    .iter()
                                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                    .collect(),
                            })
                    }),
            }),
        };
        nodes.insert(resolver_key.clone(), resolver_node);

        let target = DiscoveryTarget {
            id: target_id.clone(),
            service: service_name.to_string(),
            service_subset: String::new(),
            namespace: "default".to_string(),
            partition: "default".to_string(),
            datacenter: self.datacenter.clone(),
            mesh_gateway: MeshGatewayConfig::default(),
            subset: DiscoveryTargetSubset::default(),
            connect_timeout,
            sni: format!("{}.default.{}.internal", service_name, self.datacenter),
            name: format!("{}.default.default.{}", service_name, self.datacenter),
        };

        let mut targets = HashMap::new();
        targets.insert(target_id, target);

        DiscoveryChainResponse {
            chain: CompiledDiscoveryChain {
                service_name: service_name.to_string(),
                namespace: "default".to_string(),
                datacenter: self.datacenter.clone(),
                customization_hash: String::new(),
                protocol,
                start_node,
                nodes,
                targets,
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

    pub fn list_exported_services(&self) -> Vec<ResolvedExportedService> {
        let mut services: Vec<ResolvedExportedService> = self
            .exported_services
            .iter()
            .map(|r| r.value().clone())
            .collect();
        services.sort_by(|a, b| a.service.cmp(&b.service));
        services
    }

    pub fn list_imported_services(&self) -> Vec<ImportedService> {
        let mut services: Vec<ImportedService> = self
            .imported_services
            .iter()
            .map(|r| r.value().clone())
            .collect();
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
}
