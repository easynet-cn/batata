#![allow(clippy::too_many_arguments)]
//! Consul Internal/UI API handlers with scope-relative route macros.
//!
//! Thin wrappers that delegate to the original handler functions in
//! `crate::internal`, `crate::catalog` (for ui_services), and `crate::acl`
//! (for acl_authorize).

use std::sync::Arc;

use actix_web::{HttpRequest, HttpResponse, Scope, get, post, put, web};

use batata_common::ClusterManager;

use crate::acl::{AclAuthorizationCheck, AclService};
use crate::catalog::ConsulCatalogService;
use crate::config_entry::ConsulConfigEntryService;
use crate::connect::ConsulConnectService;
use crate::connect_ca::ConsulConnectCAService;
use crate::health::ConsulHealthService;
use crate::index_provider::ConsulIndexProvider;
use crate::internal::{
    AssignServiceVIPsRequest, UICatalogOverviewQueryParams, UIExportedServicesQueryParams,
    UINodeQueryParams, UIServiceTopologyQueryParams,
};
use crate::model::ConsulDatacenterConfig;
use crate::naming_store::ConsulNamingStore;

// ============================================================================
// UI Handlers
// ============================================================================

#[get("/ui/services")]
async fn ui_services(
    req: HttpRequest,
    catalog: web::Data<ConsulCatalogService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    query: web::Query<crate::catalog::CatalogQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::catalog::ui_services(req, catalog, acl_service, dc_config, query, index_provider).await
}

#[get("/ui/nodes")]
async fn ui_nodes(
    req: HttpRequest,
    naming_store: web::Data<ConsulNamingStore>,
    health_service: web::Data<crate::health::ConsulHealthService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    member_manager: web::Data<Arc<dyn ClusterManager>>,
    query: web::Query<UINodeQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::internal::ui_nodes(
        req,
        naming_store,
        health_service,
        acl_service,
        dc_config,
        member_manager,
        query,
        index_provider,
    )
    .await
}

#[get("/ui/node/{node}")]
async fn ui_node_info(
    req: HttpRequest,
    naming_store: web::Data<ConsulNamingStore>,
    health_service: web::Data<crate::health::ConsulHealthService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::internal::ui_node_info(
        req,
        naming_store,
        health_service,
        acl_service,
        dc_config,
        path,
        index_provider,
    )
    .await
}

#[get("/ui/exported-services")]
async fn ui_exported_services(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    connect_service: web::Data<ConsulConnectService>,
    _query: web::Query<UIExportedServicesQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::internal::ui_exported_services(req, acl_service, connect_service, _query, index_provider)
        .await
}

#[get("/ui/catalog-overview")]
async fn ui_catalog_overview(
    req: HttpRequest,
    naming_store: web::Data<ConsulNamingStore>,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    _query: web::Query<UICatalogOverviewQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::internal::ui_catalog_overview(
        req,
        naming_store,
        health_service,
        acl_service,
        dc_config,
        _query,
        index_provider,
    )
    .await
}

#[get("/ui/gateway-services-nodes/{gateway}")]
async fn ui_gateway_services_nodes(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    catalog: web::Data<ConsulCatalogService>,
    config_entry_service: web::Data<ConsulConfigEntryService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::internal::ui_gateway_services_nodes(
        req,
        acl_service,
        catalog,
        config_entry_service,
        path,
        index_provider,
    )
    .await
}

#[get("/ui/gateway-intentions/{gateway}")]
async fn ui_gateway_intentions(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::internal::ui_gateway_intentions(req, acl_service, ca_service, path, index_provider).await
}

#[get("/ui/service-topology/{service}")]
async fn ui_service_topology(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    ca_service: web::Data<ConsulConnectCAService>,
    naming_store: web::Data<ConsulNamingStore>,
    path: web::Path<String>,
    query: web::Query<UIServiceTopologyQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::internal::ui_service_topology(
        req,
        acl_service,
        dc_config,
        ca_service,
        naming_store,
        path,
        query,
        index_provider,
    )
    .await
}

#[get("/ui/metrics-proxy/{path:.*}")]
async fn ui_metrics_proxy(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
) -> HttpResponse {
    crate::internal::ui_metrics_proxy(req, acl_service, dc_config).await
}

// ============================================================================
// Federation State Handlers
// ============================================================================

#[get("/federation-states")]
async fn federation_state_list(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    naming_store: web::Data<ConsulNamingStore>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::internal::federation_state_list(
        req,
        acl_service,
        dc_config,
        naming_store,
        index_provider,
    )
    .await
}

#[get("/federation-states/mesh-gateways")]
async fn federation_state_mesh_gateways(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    naming_store: web::Data<ConsulNamingStore>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::internal::federation_state_mesh_gateways(
        req,
        acl_service,
        dc_config,
        naming_store,
        index_provider,
    )
    .await
}

#[get("/federation-state/{dc}")]
async fn federation_state_get(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    naming_store: web::Data<ConsulNamingStore>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::internal::federation_state_get(
        req,
        acl_service,
        dc_config,
        naming_store,
        path,
        index_provider,
    )
    .await
}

// ============================================================================
// Service Virtual IP Handler
// ============================================================================

#[put("/service-virtual-ip")]
async fn assign_service_virtual_ip(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    naming_store: web::Data<ConsulNamingStore>,
    body: web::Json<AssignServiceVIPsRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::internal::assign_service_virtual_ip(req, acl_service, naming_store, body, index_provider)
        .await
}

// ============================================================================
// ACL Authorize Handler
// ============================================================================

#[post("/acl/authorize")]
async fn acl_authorize(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    body: web::Json<Vec<AclAuthorizationCheck>>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::acl::acl_authorize(req, acl_service, body, index_provider).await
}

/// GET /v1/internal/rpc-methods - Lists RPC method names exposed by this agent.
///
/// Used by gRPC-capable Consul clients to discover available RPC methods.
/// We advertise the stable subset Batata implements via its Consul plugin.
#[get("/rpc-methods")]
async fn internal_rpc_methods() -> HttpResponse {
    // Match the shape of Consul's response: a JSON array of strings.
    let methods: &[&str] = &[
        "ACL.TokenSet",
        "ACL.TokenRead",
        "ACL.TokenDelete",
        "ACL.TokenList",
        "ACL.PolicySet",
        "ACL.PolicyRead",
        "ACL.PolicyDelete",
        "ACL.PolicyList",
        "ACL.RoleSet",
        "ACL.RoleRead",
        "ACL.RoleDelete",
        "ACL.RoleList",
        "Catalog.Register",
        "Catalog.Deregister",
        "Catalog.ListServices",
        "Catalog.ListNodes",
        "Catalog.ServiceNodes",
        "Catalog.NodeServices",
        "ConfigEntry.Apply",
        "ConfigEntry.Get",
        "ConfigEntry.List",
        "ConfigEntry.Delete",
        "ConnectCA.Roots",
        "ConnectCA.ConfigurationGet",
        "ConnectCA.ConfigurationSet",
        "Coordinate.Update",
        "Coordinate.ListDatacenters",
        "Coordinate.ListNodes",
        "Health.ChecksInState",
        "Health.NodeChecks",
        "Health.ServiceChecks",
        "Health.ServiceNodes",
        "Intention.Apply",
        "Intention.Get",
        "Intention.List",
        "Intention.Match",
        "Internal.NodeDump",
        "Internal.NodeInfo",
        "Internal.ServiceTopology",
        "KVS.Apply",
        "KVS.Get",
        "KVS.List",
        "KVS.ListKeys",
        "Namespace.Apply",
        "Namespace.Get",
        "Namespace.List",
        "Namespace.Delete",
        "Operator.RaftGetConfiguration",
        "Operator.RaftRemovePeerByID",
        "Operator.AutopilotGetConfiguration",
        "Operator.AutopilotSetConfiguration",
        "Operator.ServerHealth",
        "PreparedQuery.Apply",
        "PreparedQuery.Get",
        "PreparedQuery.List",
        "PreparedQuery.Execute",
        "Session.Apply",
        "Session.Get",
        "Session.List",
        "Session.Renew",
        "Txn.Apply",
        "Txn.Read",
    ];
    HttpResponse::Ok().json(methods)
}

pub fn routes() -> Scope {
    web::scope("/internal")
        .service(ui_services)
        .service(ui_nodes)
        .service(ui_node_info)
        .service(ui_exported_services)
        .service(ui_catalog_overview)
        .service(ui_gateway_services_nodes)
        .service(ui_gateway_intentions)
        .service(ui_service_topology)
        .service(ui_metrics_proxy)
        .service(federation_state_list)
        .service(federation_state_mesh_gateways)
        .service(federation_state_get)
        .service(assign_service_virtual_ip)
        .service(acl_authorize)
        .service(internal_rpc_methods)
}
