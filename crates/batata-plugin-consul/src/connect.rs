//! Consul Connect/Service Mesh API
//!
//! Provides discovery chain, exported services, and imported services endpoints.

use actix_web::{HttpRequest, HttpResponse, web};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::acl::{AclService, AclServicePersistent, ResourceType};
use crate::model::ConsulError;

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

/// Query parameters for exported/imported services
#[derive(Debug, Deserialize)]
pub struct ServiceVisibilityQueryParams {
    pub partition: Option<String>,
}

// ============================================================================
// Service (In-Memory)
// ============================================================================

/// In-memory connect service for discovery chain and service visibility
pub struct ConsulConnectService {
    /// Exported services configuration
    exported_services: Arc<DashMap<String, ResolvedExportedService>>,
    /// Imported services configuration
    imported_services: Arc<DashMap<String, ImportedService>>,
    /// Datacenter name
    datacenter: String,
}

impl ConsulConnectService {
    pub fn new() -> Self {
        Self {
            exported_services: Arc::new(DashMap::new()),
            imported_services: Arc::new(DashMap::new()),
            datacenter: "dc1".to_string(),
        }
    }

    /// Get the compiled discovery chain for a service.
    /// Returns a default chain with a single resolver node.
    pub fn get_discovery_chain(&self, service_name: &str) -> DiscoveryChainResponse {
        let target_id = format!(
            "{}.default.default.{}.internal",
            service_name, self.datacenter
        );
        let resolver_name = format!(
            "resolver:{}.default.default.{}",
            service_name, self.datacenter
        );

        let resolver_node = DiscoveryGraphNode {
            node_type: DiscoveryGraphNodeType::Resolver,
            name: resolver_name.clone(),
            routes: Vec::new(),
            splits: Vec::new(),
            resolver: Some(DiscoveryResolver {
                default: true,
                connect_timeout: "5s".to_string(),
                target: target_id.clone(),
                failover: None,
            }),
        };

        let target = DiscoveryTarget {
            id: target_id.clone(),
            service: service_name.to_string(),
            service_subset: String::new(),
            namespace: "default".to_string(),
            partition: "default".to_string(),
            datacenter: self.datacenter.clone(),
            mesh_gateway: MeshGatewayConfig::default(),
            subset: DiscoveryTargetSubset::default(),
            connect_timeout: "5s".to_string(),
            sni: format!("{}.default.{}.internal", service_name, self.datacenter),
            name: format!("{}.default.default.{}", service_name, self.datacenter),
        };

        let mut nodes = HashMap::new();
        nodes.insert(resolver_name.clone(), resolver_node);

        let mut targets = HashMap::new();
        targets.insert(target_id, target);

        DiscoveryChainResponse {
            chain: CompiledDiscoveryChain {
                service_name: service_name.to_string(),
                namespace: "default".to_string(),
                datacenter: self.datacenter.clone(),
                customization_hash: String::new(),
                protocol: "tcp".to_string(),
                start_node: resolver_name,
                nodes,
                targets,
            },
        }
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
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let service_name = path.into_inner();
    HttpResponse::Ok().json(connect_service.get_discovery_chain(&service_name))
}

/// GET /v1/exported-services - List exported services
pub async fn list_exported_services(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    connect_service: web::Data<ConsulConnectService>,
    _query: web::Query<ServiceVisibilityQueryParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    HttpResponse::Ok().json(connect_service.list_exported_services())
}

/// GET /v1/imported-services - List imported services
pub async fn list_imported_services(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    connect_service: web::Data<ConsulConnectService>,
    _query: web::Query<ServiceVisibilityQueryParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    HttpResponse::Ok().json(connect_service.list_imported_services())
}

// ============================================================================
// HTTP Handlers (Persistent)
// ============================================================================

/// GET /v1/discovery-chain/{service} (persistent)
pub async fn get_discovery_chain_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclServicePersistent>,
    connect_service: web::Data<ConsulConnectService>,
    path: web::Path<String>,
    _query: web::Query<DiscoveryChainQueryParams>,
) -> HttpResponse {
    let authz = acl_service
        .authorize_request(&req, ResourceType::Service, "", false)
        .await;
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let service_name = path.into_inner();
    HttpResponse::Ok().json(connect_service.get_discovery_chain(&service_name))
}

/// GET /v1/exported-services (persistent)
pub async fn list_exported_services_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclServicePersistent>,
    connect_service: web::Data<ConsulConnectService>,
    _query: web::Query<ServiceVisibilityQueryParams>,
) -> HttpResponse {
    let authz = acl_service
        .authorize_request(&req, ResourceType::Operator, "", false)
        .await;
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    HttpResponse::Ok().json(connect_service.list_exported_services())
}

/// GET /v1/imported-services (persistent)
pub async fn list_imported_services_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclServicePersistent>,
    connect_service: web::Data<ConsulConnectService>,
    _query: web::Query<ServiceVisibilityQueryParams>,
) -> HttpResponse {
    let authz = acl_service
        .authorize_request(&req, ResourceType::Operator, "", false)
        .await;
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    HttpResponse::Ok().json(connect_service.list_imported_services())
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
        assert!(!chain.chain.start_node.is_empty());
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
