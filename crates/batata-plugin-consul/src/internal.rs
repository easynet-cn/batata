//! Consul Internal/UI API endpoints
//!
//! Provides handlers for `/v1/internal/*` endpoints used by the Consul UI
//! and internal operations (federation, VIP, ACL authorize).

use std::collections::HashMap;

use actix_web::{HttpRequest, HttpResponse, web};
use serde::{Deserialize, Serialize};

use crate::acl::{AclService, ResourceType};
use crate::catalog::ConsulCatalogService;
use crate::config_entry::ConsulConfigEntryService;
use crate::connect::ConsulConnectService;
use crate::connect_ca::ConsulConnectCAService;
use crate::health::ConsulHealthService;
use crate::index_provider::{ConsulIndexProvider, ConsulTable};
use crate::model::{AgentServiceRegistration, ConsulDatacenterConfig, ConsulError};
use crate::naming_store::ConsulNamingStore;

// ============================================================================
// UI Models
// ============================================================================

/// Node info for UI (matches Consul's NodeInfo struct)
#[derive(Debug, Clone, Serialize)]
pub struct UINode {
    #[serde(rename = "ID")]
    pub id: String,
    #[serde(rename = "Node")]
    pub node: String,
    #[serde(rename = "Address")]
    pub address: String,
    #[serde(rename = "TaggedAddresses")]
    pub tagged_addresses: HashMap<String, String>,
    #[serde(rename = "Meta")]
    pub meta: HashMap<String, String>,
    #[serde(rename = "Services")]
    pub services: Vec<crate::model::AgentService>,
    #[serde(rename = "Checks")]
    pub checks: Vec<crate::model::HealthCheck>,
}

/// Query parameters for UI node list
#[derive(Debug, Deserialize)]
pub struct UINodeQueryParams {
    pub dc: Option<String>,
}

/// Query parameters for UI exported services
#[derive(Debug, Deserialize)]
pub struct UIExportedServicesQueryParams {
    pub dc: Option<String>,
}

/// Service topology for UI
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct ServiceTopology {
    pub protocol: String,
    pub transparent_proxy: bool,
    pub upstreams: Vec<ServiceTopologySummary>,
    pub downstreams: Vec<ServiceTopologySummary>,
    #[serde(rename = "FilteredByACLs")]
    pub filtered_by_acls: bool,
}

/// Service topology summary entry
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct ServiceTopologySummary {
    pub name: String,
    pub datacenter: String,
    pub namespace: String,
    pub intention: ServiceTopologyIntention,
}

/// Service topology intention
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct ServiceTopologyIntention {
    pub allowed: bool,
    pub has_permissions: bool,
    pub external_source: String,
}

/// Query parameters for service topology
#[derive(Debug, Deserialize)]
pub struct UIServiceTopologyQueryParams {
    pub dc: Option<String>,
    pub kind: Option<String>,
}

/// Catalog summary for UI
#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct CatalogSummary {
    pub nodes: CatalogCountSummary,
    pub services: CatalogCountSummary,
    pub checks: CatalogCountSummary,
}

/// Category count in catalog summary
#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct CatalogCountSummary {
    pub total: i64,
    pub passing: i64,
    pub warning: i64,
    pub critical: i64,
}

/// Federation state
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct FederationState {
    pub datacenter: String,
    pub mesh_gateways: Vec<serde_json::Value>,
    pub primary_datacenter: String,
    #[serde(rename = "PrimaryModifyIndex")]
    pub primary_modifyindex: u64,
}

/// Assign service VIPs request
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AssignServiceVIPsRequest {
    pub service_name: String,
}

/// Assign service VIPs response
#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct AssignServiceVIPsResponse {
    pub service_name: String,
    pub found: bool,
}

/// Query parameters for UI catalog overview
#[derive(Debug, Deserialize)]
pub struct UICatalogOverviewQueryParams {
    pub dc: Option<String>,
}

// ============================================================================
// UI Handlers
// ============================================================================

/// Build a UINode from real data in the store.
fn build_node_from_store(
    dc_config: &ConsulDatacenterConfig,
    naming_store: &ConsulNamingStore,
    health_service: &ConsulHealthService,
    index: u64,
) -> UINode {
    let ip = batata_common::local_ip();

    let mut tagged = HashMap::new();
    tagged.insert("lan".to_string(), ip.clone());
    tagged.insert("lan_ipv4".to_string(), ip.clone());
    tagged.insert("wan".to_string(), ip.clone());
    tagged.insert("wan_ipv4".to_string(), ip.clone());

    let mut meta = HashMap::new();
    meta.insert("consul-network-segment".to_string(), "".to_string());
    meta.insert("consul-version".to_string(), dc_config.full_version());

    let mut services = Vec::new();
    for (_key, data) in naming_store.scan_ns(crate::namespace::DEFAULT_NAMESPACE) {
        if let Ok(reg) = serde_json::from_slice::<AgentServiceRegistration>(&data) {
            let svc = crate::model::AgentService::from(&reg);
            services.push(svc);
        }
    }

    let all_checks = health_service.get_all_checks_sync();
    let mut checks = Vec::new();
    for mut check in all_checks {
        check.node = dc_config.node_name.clone();
        if check.definition.is_none() {
            check.definition = Some(crate::model::HealthCheckDefinition::default());
        }
        checks.push(check);
    }

    UINode {
        id: dc_config.node_id.clone(),
        node: dc_config.node_name.clone(),
        address: ip,
        tagged_addresses: tagged,
        meta,
        services,
        checks,
    }
}

/// GET /v1/internal/ui/nodes - List nodes for UI
pub async fn ui_nodes(
    req: HttpRequest,
    naming_store: web::Data<ConsulNamingStore>,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    _query: web::Query<UINodeQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Node, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let index = index_provider.current_index(ConsulTable::Catalog);
    let node = build_node_from_store(&dc_config, &naming_store, &health_service, index);

    HttpResponse::Ok()
        .insert_header(("X-Consul-Index", index.to_string()))
        .json(vec![node])
}

/// GET /v1/internal/ui/node/{node} - Get node info for UI
pub async fn ui_node_info(
    req: HttpRequest,
    naming_store: web::Data<ConsulNamingStore>,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Node, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let node_name = path.into_inner();
    let index = index_provider.current_index(ConsulTable::Catalog);

    // In single-node mode, return the local node for any valid query.
    // Match by: configured node name, local IP, or any registered service address.
    let local_ip = batata_common::local_ip();
    let is_local = node_name == dc_config.node_name
        || node_name == local_ip
        || naming_store
            .scan_ns(crate::namespace::DEFAULT_NAMESPACE)
            .iter()
            .any(|(_, data)| {
                serde_json::from_slice::<AgentServiceRegistration>(data)
                    .map(|r| r.effective_address() == node_name)
                    .unwrap_or(false)
            });
    if is_local {
        let node = build_node_from_store(&dc_config, &naming_store, &health_service, index);
        return HttpResponse::Ok()
            .insert_header(("X-Consul-Index", index.to_string()))
            .json(node);
    }

    HttpResponse::NotFound().json(ConsulError::new(format!("Node '{}' not found", node_name)))
}

/// GET /v1/internal/ui/exported-services - List exported services for UI
pub async fn ui_exported_services(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    connect_service: web::Data<ConsulConnectService>,
    _query: web::Query<UIExportedServicesQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    HttpResponse::Ok()
        .insert_header((
            "X-Consul-Index",
            index_provider
                .current_index(ConsulTable::Catalog)
                .to_string(),
        ))
        .json(connect_service.list_exported_services())
}

/// GET /v1/internal/ui/catalog-overview - Get catalog overview for UI
pub async fn ui_catalog_overview(
    req: HttpRequest,
    naming_store: web::Data<ConsulNamingStore>,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    _dc_config: web::Data<ConsulDatacenterConfig>,
    _query: web::Query<UICatalogOverviewQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let service_names = naming_store.service_names(crate::namespace::DEFAULT_NAMESPACE);
    let service_count = service_names.len();

    let mut node_set: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut total_checks: i64 = 0;
    let mut passing_checks: i64 = 0;
    let mut warning_checks: i64 = 0;
    let mut critical_checks: i64 = 0;

    for service_name in &service_names {
        let entries =
            naming_store.get_service_entries(crate::namespace::DEFAULT_NAMESPACE, service_name);
        for entry_bytes in &entries {
            if let Ok(reg) = serde_json::from_slice::<AgentServiceRegistration>(entry_bytes) {
                let ip = reg.effective_address();
                let node_name = format!("node-{}", ip.replace('.', "-"));
                node_set.insert(node_name);

                let service_id = reg.service_id();
                let checks = health_service.get_service_checks(&service_id).await;
                for check in &checks {
                    total_checks += 1;
                    match check.status.as_str() {
                        "passing" => passing_checks += 1,
                        "warning" => warning_checks += 1,
                        "critical" => critical_checks += 1,
                        _ => {}
                    }
                }
            }
        }
    }

    let summary = CatalogSummary {
        nodes: CatalogCountSummary {
            total: node_set.len() as i64,
            passing: node_set.len() as i64,
            warning: 0,
            critical: 0,
        },
        services: CatalogCountSummary {
            total: service_count as i64,
            passing: service_count as i64,
            warning: 0,
            critical: 0,
        },
        checks: CatalogCountSummary {
            total: total_checks,
            passing: passing_checks,
            warning: warning_checks,
            critical: critical_checks,
        },
    };

    HttpResponse::Ok()
        .insert_header((
            "X-Consul-Index",
            index_provider
                .current_index(ConsulTable::Catalog)
                .to_string(),
        ))
        .json(summary)
}

/// GET /v1/internal/ui/gateway-services-nodes/{gateway} - List gateway service nodes
pub async fn ui_gateway_services_nodes(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    catalog: web::Data<ConsulCatalogService>,
    config_entry_service: web::Data<ConsulConfigEntryService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let gateway_name = path.into_inner();
    let gateway_services =
        catalog.get_gateway_services_from_config(&gateway_name, &config_entry_service);

    HttpResponse::Ok()
        .insert_header((
            "X-Consul-Index",
            index_provider
                .current_index(ConsulTable::Catalog)
                .to_string(),
        ))
        .json(gateway_services)
}

/// GET /v1/internal/ui/gateway-intentions/{gateway} - List gateway intentions
pub async fn ui_gateway_intentions(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let gateway = path.into_inner();
    let matched = ca_service.match_intentions("destination", &gateway);
    HttpResponse::Ok()
        .insert_header((
            "X-Consul-Index",
            index_provider
                .current_index(ConsulTable::Catalog)
                .to_string(),
        ))
        .json(matched)
}

/// GET /v1/internal/ui/service-topology/{service} - Get service topology
#[allow(clippy::too_many_arguments)]
pub async fn ui_service_topology(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    ca_service: web::Data<ConsulConnectCAService>,
    naming_store: web::Data<ConsulNamingStore>,
    path: web::Path<String>,
    query: web::Query<UIServiceTopologyQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let service_name = path.into_inner();
    let dc = dc_config.resolve_dc(&query.dc);

    let mut upstreams = Vec::new();
    let mut downstreams = Vec::new();

    // Get all intentions where this service is the source (upstreams)
    let source_intentions = ca_service.match_intentions("source", &service_name);
    for intention in &source_intentions {
        upstreams.push(ServiceTopologySummary {
            name: intention.destination_name.clone(),
            datacenter: dc.clone(),
            namespace: "default".to_string(),
            intention: ServiceTopologyIntention {
                allowed: intention.action == crate::connect_ca::IntentionAction::Allow,
                has_permissions: !intention.permissions.is_empty(),
                external_source: String::new(),
            },
        });
    }

    // Get all intentions where this service is the destination (downstreams)
    let dest_intentions = ca_service.match_intentions("destination", &service_name);
    for intention in &dest_intentions {
        downstreams.push(ServiceTopologySummary {
            name: intention.source_name.clone(),
            datacenter: dc.clone(),
            namespace: "default".to_string(),
            intention: ServiceTopologyIntention {
                allowed: intention.action == crate::connect_ca::IntentionAction::Allow,
                has_permissions: !intention.permissions.is_empty(),
                external_source: String::new(),
            },
        });
    }

    // Also check proxy config for upstream dependencies
    for (_key, data) in naming_store.scan_ns(crate::namespace::DEFAULT_NAMESPACE) {
        if let Ok(reg) = serde_json::from_slice::<AgentServiceRegistration>(&data) {
            if reg.kind.as_deref() == Some("connect-proxy") {
                if let Some(ref proxy) = reg.proxy {
                    let dest = proxy
                        .get("DestinationServiceName")
                        .or_else(|| proxy.get("destination_service_name"))
                        .and_then(|v| v.as_str());
                    if dest == Some(&service_name) {
                        if let Some(upstream_arr) = proxy
                            .get("Upstreams")
                            .or_else(|| proxy.get("upstreams"))
                            .and_then(|v| v.as_array())
                        {
                            for upstream in upstream_arr {
                                let upstream_name = upstream
                                    .get("DestinationName")
                                    .or_else(|| upstream.get("destination_name"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                if !upstream_name.is_empty()
                                    && !upstreams.iter().any(|u| u.name == upstream_name)
                                {
                                    upstreams.push(ServiceTopologySummary {
                                        name: upstream_name,
                                        datacenter: dc.clone(),
                                        namespace: "default".to_string(),
                                        intention: ServiceTopologyIntention {
                                            allowed: true,
                                            has_permissions: false,
                                            external_source: String::new(),
                                        },
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let protocol = query.kind.as_deref().unwrap_or("tcp").to_string();

    let topology = ServiceTopology {
        protocol,
        transparent_proxy: false,
        upstreams,
        downstreams,
        filtered_by_acls: false,
    };

    HttpResponse::Ok()
        .insert_header((
            "X-Consul-Index",
            index_provider
                .current_index(ConsulTable::Catalog)
                .to_string(),
        ))
        .json(topology)
}

/// GET /v1/internal/ui/metrics-proxy/{path:.*} - Proxy metrics requests
pub async fn ui_metrics_proxy(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    HttpResponse::NotFound().json(ConsulError::new("metrics proxy not configured"))
}

// ============================================================================
// Federation State Helpers
// ============================================================================

/// Collect mesh-gateway service instances as JSON values for federation state
fn collect_mesh_gateways(
    naming_store: &ConsulNamingStore,
    datacenter: &str,
) -> Vec<serde_json::Value> {
    let mut gateways = Vec::new();

    for (_key, data) in naming_store.scan_ns(crate::namespace::DEFAULT_NAMESPACE) {
        if let Ok(reg) = serde_json::from_slice::<AgentServiceRegistration>(&data) {
            if reg.kind.as_deref() == Some("mesh-gateway") {
                let ip = reg.effective_address();
                let port = reg.effective_port();
                let svc_id = reg.service_id();
                let node_name = format!("node-{}", ip.replace('.', "-"));
                gateways.push(serde_json::json!({
                    "WAN": { "Address": ip, "Port": port },
                    "LAN": { "Address": ip, "Port": port },
                    "Service": {
                        "ID": svc_id,
                        "Service": reg.name,
                        "Address": ip,
                        "Port": port,
                        "Meta": {},
                        "Datacenter": datacenter
                    },
                    "Node": {
                        "Node": node_name,
                        "Address": ip,
                        "Datacenter": datacenter
                    }
                }));
            }
        }
    }

    gateways
}

// ============================================================================
// Federation State Handlers
// ============================================================================

/// GET /v1/internal/federation-states - List federation states
pub async fn federation_state_list(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    naming_store: web::Data<ConsulNamingStore>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let mesh_gateways = collect_mesh_gateways(&naming_store, &dc_config.datacenter);

    let state = FederationState {
        datacenter: dc_config.datacenter.clone(),
        mesh_gateways,
        primary_datacenter: dc_config.primary_datacenter.clone(),
        primary_modifyindex: 1,
    };

    HttpResponse::Ok()
        .insert_header((
            "X-Consul-Index",
            index_provider
                .current_index(ConsulTable::Catalog)
                .to_string(),
        ))
        .json(vec![state])
}

/// GET /v1/internal/federation-states/mesh-gateways - List mesh gateway federation states
pub async fn federation_state_mesh_gateways(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    naming_store: web::Data<ConsulNamingStore>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let mut gateways_by_dc: HashMap<String, Vec<serde_json::Value>> = HashMap::new();

    for (_key, data) in naming_store.scan_ns(crate::namespace::DEFAULT_NAMESPACE) {
        if let Ok(reg) = serde_json::from_slice::<AgentServiceRegistration>(&data) {
            if reg.kind.as_deref() == Some("mesh-gateway") {
                let dc = dc_config.datacenter.clone();
                let entry = serde_json::json!({
                    "Address": reg.effective_address(),
                    "Port": reg.effective_port(),
                    "Service": reg.name,
                });
                gateways_by_dc.entry(dc).or_default().push(entry);
            }
        }
    }

    HttpResponse::Ok()
        .insert_header((
            "X-Consul-Index",
            index_provider
                .current_index(ConsulTable::Catalog)
                .to_string(),
        ))
        .json(gateways_by_dc)
}

/// GET /v1/internal/federation-state/{dc} - Get federation state for a datacenter
pub async fn federation_state_get(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    naming_store: web::Data<ConsulNamingStore>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let dc = path.into_inner();
    let mesh_gateways = if dc == dc_config.datacenter {
        collect_mesh_gateways(&naming_store, &dc)
    } else {
        Vec::new()
    };

    let state = FederationState {
        datacenter: dc,
        mesh_gateways,
        primary_datacenter: dc_config.primary_datacenter.clone(),
        primary_modifyindex: 1,
    };

    HttpResponse::Ok()
        .insert_header((
            "X-Consul-Index",
            index_provider
                .current_index(ConsulTable::Catalog)
                .to_string(),
        ))
        .json(state)
}

// ============================================================================
// Service Virtual IP Handler
// ============================================================================

/// PUT /v1/internal/service-virtual-ip - Assign service virtual IPs
pub async fn assign_service_virtual_ip(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    naming_store: web::Data<ConsulNamingStore>,
    body: web::Json<AssignServiceVIPsRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let request = body.into_inner();
    let found = !naming_store
        .get_service_entries(crate::namespace::DEFAULT_NAMESPACE, &request.service_name)
        .is_empty();

    HttpResponse::Ok()
        .insert_header((
            "X-Consul-Index",
            index_provider
                .current_index(ConsulTable::Catalog)
                .to_string(),
        ))
        .json(AssignServiceVIPsResponse {
            service_name: request.service_name,
            found,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_catalog_count_summary_serialization() {
        let summary = CatalogCountSummary {
            total: 10,
            passing: 8,
            warning: 1,
            critical: 1,
        };
        let json = serde_json::to_value(&summary).unwrap();
        assert_eq!(json["Total"], 10);
        assert_eq!(json["Passing"], 8);
    }

    #[test]
    fn test_federation_state_serialization() {
        let state = FederationState {
            datacenter: "dc1".to_string(),
            mesh_gateways: Vec::new(),
            primary_datacenter: "dc1".to_string(),
            primary_modifyindex: 1,
        };
        let json = serde_json::to_value(&state).unwrap();
        assert_eq!(json["Datacenter"], "dc1");
        assert_eq!(json["PrimaryDatacenter"], "dc1");
    }

    #[test]
    fn test_assign_vip_response() {
        let resp = AssignServiceVIPsResponse {
            service_name: "web".to_string(),
            found: false,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["ServiceName"], "web");
        assert_eq!(json["Found"], false);
    }

    #[test]
    fn test_service_topology_serialization() {
        let topology = ServiceTopology {
            protocol: "http".to_string(),
            transparent_proxy: false,
            upstreams: Vec::new(),
            downstreams: Vec::new(),
            filtered_by_acls: false,
        };
        let json = serde_json::to_value(&topology).unwrap();
        assert_eq!(json["Protocol"], "http");
        assert_eq!(json["FilteredByACLs"], false);
    }

    #[test]
    fn test_ui_node_serialization() {
        let node = UINode {
            id: "test-id".to_string(),
            node: "node-1".to_string(),
            address: "10.0.0.1".to_string(),
            tagged_addresses: HashMap::new(),
            meta: HashMap::new(),
            services: vec![],
            checks: vec![],
        };
        let json = serde_json::to_value(&node).unwrap();
        assert_eq!(json["ID"], "test-id");
        assert_eq!(json["Node"], "node-1");
        assert_eq!(json["Address"], "10.0.0.1");
        assert!(json["Services"].is_array());
        assert!(json["Checks"].is_array());
    }

    #[test]
    fn test_ui_node_with_meta() {
        let node = UINode {
            id: uuid::Uuid::new_v4().to_string(),
            node: "member-node-1".to_string(),
            address: "192.168.1.100".to_string(),
            tagged_addresses: HashMap::from([
                ("lan".to_string(), "192.168.1.100".to_string()),
            ]),
            meta: HashMap::from([
                ("consul-version".to_string(), "1.22.5".to_string()),
            ]),
            services: vec![],
            checks: vec![],
        };
        let json = serde_json::to_value(&node).unwrap();
        assert_eq!(json["Node"], "member-node-1");
        let tagged = json["TaggedAddresses"].as_object().unwrap();
        assert_eq!(tagged["lan"], "192.168.1.100");
        let meta = json["Meta"].as_object().unwrap();
        assert_eq!(meta["consul-version"], "1.22.5");
    }
}
