//! Consul Internal/UI API endpoints
//!
//! Provides handlers for `/v1/internal/*` endpoints used by the Consul UI
//! and internal operations (federation, VIP, ACL authorize).

use std::collections::HashMap;
use std::sync::Arc;

use actix_web::{HttpRequest, HttpResponse, web};
use serde::{Deserialize, Serialize};

use batata_naming::service::NamingService;

use crate::acl::{AclService, ResourceType};
use crate::connect::ConsulConnectService;
use crate::connect_ca::ConsulConnectCAService;
use crate::health::ConsulHealthService;
use crate::model::ConsulError;

// ============================================================================
// UI Models
// ============================================================================

/// Node summary for UI
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct UINode {
    #[serde(rename = "ID")]
    pub id: String,
    pub node: String,
    pub address: String,
    pub datacenter: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<HashMap<String, String>>,
    pub create_index: u64,
    pub modify_index: u64,
}

/// Catalog overview summary
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct CatalogSummary {
    pub nodes: CatalogCountSummary,
    pub services: CatalogCountSummary,
    pub checks: CatalogCountSummary,
}

/// Count summary with health status breakdown
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct CatalogCountSummary {
    pub total: i64,
    pub passing: i64,
    pub warning: i64,
    pub critical: i64,
}

/// Service topology response
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

/// Service topology node summary
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct ServiceTopologySummary {
    pub name: String,
    pub datacenter: String,
    pub namespace: String,
    pub intention: ServiceTopologyIntention,
}

/// Intention status in topology
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct ServiceTopologyIntention {
    pub allowed: bool,
    pub has_permissions: bool,
    pub external_source: String,
}

// ============================================================================
// Federation Models
// ============================================================================

/// Federation state for a datacenter
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct FederationState {
    pub datacenter: String,
    pub mesh_gateways: Vec<serde_json::Value>,
    pub primary_datacenter: String,
    pub primary_modifyindex: u64,
}

// ============================================================================
// VIP Models
// ============================================================================

/// Request to assign service virtual IPs
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AssignServiceVIPsRequest {
    pub service_name: String,
    #[serde(default)]
    pub manual_vips: Vec<String>,
}

/// Response for VIP assignment
#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct AssignServiceVIPsResponse {
    pub service_name: String,
    pub found: bool,
}

// ============================================================================
// Query Parameters
// ============================================================================

/// Query parameters for UI node endpoints
#[derive(Debug, Deserialize)]
pub struct UINodeQueryParams {
    pub dc: Option<String>,
    pub filter: Option<String>,
}

/// Query parameters for UI service topology
#[derive(Debug, Deserialize)]
pub struct UIServiceTopologyQueryParams {
    pub dc: Option<String>,
    pub kind: Option<String>,
}

/// Query parameters for UI exported services
#[derive(Debug, Deserialize)]
pub struct UIExportedServicesQueryParams {
    pub dc: Option<String>,
    pub partition: Option<String>,
}

/// Query parameters for UI catalog overview
#[derive(Debug, Deserialize)]
pub struct UICatalogOverviewQueryParams {
    pub dc: Option<String>,
}

// ============================================================================
// UI Handlers
// ============================================================================

/// GET /v1/internal/ui/nodes - List nodes for UI
pub async fn ui_nodes(
    req: HttpRequest,
    naming_service: web::Data<Arc<NamingService>>,
    acl_service: web::Data<AclService>,
    _query: web::Query<UINodeQueryParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Node, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    // Collect unique nodes from all service instances
    let (_, service_names) = naming_service.list_services("public", "DEFAULT_GROUP", 1, 10000);
    let mut node_map: HashMap<String, UINode> = HashMap::new();

    for service_name in &service_names {
        let instances =
            naming_service.get_instances("public", "DEFAULT_GROUP", service_name, "", false);
        for instance in &instances {
            let node_name = instance
                .metadata
                .get("consul_node")
                .cloned()
                .unwrap_or_else(|| instance.ip.clone());
            node_map.entry(node_name.clone()).or_insert_with(|| UINode {
                id: uuid::Uuid::new_v4().to_string(),
                node: node_name,
                address: instance.ip.clone(),
                datacenter: "dc1".to_string(),
                meta: None,
                create_index: 1,
                modify_index: 1,
            });
        }
    }

    let nodes: Vec<UINode> = node_map.into_values().collect();
    HttpResponse::Ok()
        .insert_header(("X-Consul-Index", "1"))
        .json(nodes)
}

/// GET /v1/internal/ui/node/{node} - Get node info for UI
pub async fn ui_node_info(
    req: HttpRequest,
    naming_service: web::Data<Arc<NamingService>>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Node, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let node_name = path.into_inner();
    let (_, service_names) = naming_service.list_services("public", "DEFAULT_GROUP", 1, 10000);

    for service_name in &service_names {
        let instances =
            naming_service.get_instances("public", "DEFAULT_GROUP", service_name, "", false);
        for instance in &instances {
            let instance_node = instance
                .metadata
                .get("consul_node")
                .cloned()
                .unwrap_or_else(|| instance.ip.clone());
            if instance_node == node_name {
                let node = UINode {
                    id: uuid::Uuid::new_v4().to_string(),
                    node: node_name,
                    address: instance.ip.clone(),
                    datacenter: "dc1".to_string(),
                    meta: None,
                    create_index: 1,
                    modify_index: 1,
                };
                return HttpResponse::Ok().json(node);
            }
        }
    }

    HttpResponse::NotFound().json(ConsulError::new(format!("Node '{}' not found", node_name)))
}

/// GET /v1/internal/ui/exported-services - List exported services for UI
pub async fn ui_exported_services(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    connect_service: web::Data<ConsulConnectService>,
    _query: web::Query<UIExportedServicesQueryParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    HttpResponse::Ok()
        .insert_header(("X-Consul-Index", "1"))
        .json(connect_service.list_exported_services())
}

/// GET /v1/internal/ui/catalog-overview - Get catalog overview for UI
pub async fn ui_catalog_overview(
    req: HttpRequest,
    naming_service: web::Data<Arc<NamingService>>,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    _query: web::Query<UICatalogOverviewQueryParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let (service_count, service_names) =
        naming_service.list_services("public", "DEFAULT_GROUP", 1, 10000);

    // Count total instances and collect unique nodes
    let mut total_instances: i64 = 0;
    let mut node_set: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut total_checks: i64 = 0;
    let mut passing_checks: i64 = 0;
    let mut warning_checks: i64 = 0;
    let mut critical_checks: i64 = 0;

    for service_name in &service_names {
        let instances =
            naming_service.get_instances("public", "DEFAULT_GROUP", service_name, "", false);
        for instance in &instances {
            total_instances += 1;
            let node_name = instance
                .metadata
                .get("consul_node")
                .cloned()
                .unwrap_or_else(|| instance.ip.clone());
            node_set.insert(node_name);

            // Get checks for this instance
            let checks = health_service
                .get_service_checks(&instance.instance_id)
                .await;
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

    let _ = total_instances; // used for node counting
    HttpResponse::Ok()
        .insert_header(("X-Consul-Index", "1"))
        .json(summary)
}

/// GET /v1/internal/ui/gateway-services-nodes/{gateway} - List gateway service nodes
pub async fn ui_gateway_services_nodes(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    _path: web::Path<String>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    // Stub: gateway services not implemented
    HttpResponse::Ok()
        .insert_header(("X-Consul-Index", "1"))
        .json(Vec::<serde_json::Value>::new())
}

/// GET /v1/internal/ui/gateway-intentions/{gateway} - List gateway intentions
pub async fn ui_gateway_intentions(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    path: web::Path<String>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let gateway = path.into_inner();
    let matched = ca_service.match_intentions("destination", &gateway);
    HttpResponse::Ok()
        .insert_header(("X-Consul-Index", "1"))
        .json(matched)
}

/// GET /v1/internal/ui/service-topology/{service} - Get service topology
pub async fn ui_service_topology(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    _path: web::Path<String>,
    _query: web::Query<UIServiceTopologyQueryParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    // Stub: service topology requires deep integration
    let topology = ServiceTopology {
        protocol: "tcp".to_string(),
        transparent_proxy: false,
        upstreams: Vec::new(),
        downstreams: Vec::new(),
        filtered_by_acls: false,
    };

    HttpResponse::Ok()
        .insert_header(("X-Consul-Index", "1"))
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
// Federation State Handlers
// ============================================================================

/// GET /v1/internal/federation-states - List federation states
pub async fn federation_state_list(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let state = FederationState {
        datacenter: "dc1".to_string(),
        mesh_gateways: Vec::new(),
        primary_datacenter: "dc1".to_string(),
        primary_modifyindex: 1,
    };

    HttpResponse::Ok()
        .insert_header(("X-Consul-Index", "1"))
        .json(vec![state])
}

/// GET /v1/internal/federation-states/mesh-gateways - List mesh gateway federation states
pub async fn federation_state_mesh_gateways(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    HttpResponse::Ok()
        .insert_header(("X-Consul-Index", "1"))
        .json(HashMap::<String, Vec<serde_json::Value>>::new())
}

/// GET /v1/internal/federation-state/{dc} - Get federation state for a datacenter
pub async fn federation_state_get(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let dc = path.into_inner();
    let state = FederationState {
        datacenter: dc,
        mesh_gateways: Vec::new(),
        primary_datacenter: "dc1".to_string(),
        primary_modifyindex: 1,
    };

    HttpResponse::Ok()
        .insert_header(("X-Consul-Index", "1"))
        .json(state)
}

// ============================================================================
// Service Virtual IP Handler
// ============================================================================

/// PUT /v1/internal/service-virtual-ip - Assign service virtual IPs (Enterprise stub)
pub async fn assign_service_virtual_ip(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    body: web::Json<AssignServiceVIPsRequest>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Operator, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let request = body.into_inner();
    HttpResponse::Ok().json(AssignServiceVIPsResponse {
        service_name: request.service_name,
        found: false,
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
            datacenter: "dc1".to_string(),
            meta: None,
            create_index: 1,
            modify_index: 1,
        };
        let json = serde_json::to_value(&node).unwrap();
        assert_eq!(json["ID"], "test-id");
        assert_eq!(json["Node"], "node-1");
        assert_eq!(json["Address"], "10.0.0.1");
    }
}
