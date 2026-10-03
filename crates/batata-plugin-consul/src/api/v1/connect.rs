//! Consul Connect/Service Mesh API handlers with scope-relative route macros.
//!
//! Discovery chain handlers use scope "/discovery-chain".
//! Exported/imported services use standalone resources.

// actix-web route macros (`#[get]`, `#[post]`, `#[delete]`, ...) expand to a
// struct that cannot carry a doc comment, which trips `missing_docs`. The
// generated struct is an internal implementation detail, so the lint is allowed
// for this module.
#![allow(missing_docs)]

use actix_web::{HttpRequest, HttpResponse, Scope, get, post, web};

use crate::acl::AclService;
use crate::agent::ConsulAgentService;
use crate::connect::{
    ConsulConnectService, DiscoveryChainOverrides, DiscoveryChainQueryParams,
    ProxyConfigQueryParams, ServiceVisibilityQueryParams,
};
use crate::index_provider::ConsulIndexProvider;
use crate::model::ConsulDatacenterConfig;

// ============================================================================
// In-memory handlers
// ============================================================================

#[get("/{service}")]
async fn get_discovery_chain(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    connect_service: web::Data<ConsulConnectService>,
    path: web::Path<String>,
    _query: web::Query<DiscoveryChainQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::connect::get_discovery_chain(
        req,
        acl_service,
        connect_service,
        path,
        _query,
        index_provider,
    )
    .await
}

#[post("/{service}")]
async fn post_discovery_chain(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    connect_service: web::Data<ConsulConnectService>,
    path: web::Path<String>,
    _query: web::Query<DiscoveryChainQueryParams>,
    body: web::Json<DiscoveryChainOverrides>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::connect::post_discovery_chain(
        req,
        acl_service,
        connect_service,
        path,
        _query,
        body,
        index_provider,
    )
    .await
}

// ============================================================================
// Persistent handlers
// ============================================================================

#[get("/{service}")]
async fn get_discovery_chain_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    connect_service: web::Data<ConsulConnectService>,
    path: web::Path<String>,
    _query: web::Query<DiscoveryChainQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::connect::get_discovery_chain_persistent(
        req,
        acl_service,
        connect_service,
        path,
        _query,
        index_provider,
    )
    .await
}

#[post("/{service}")]
async fn post_discovery_chain_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    connect_service: web::Data<ConsulConnectService>,
    path: web::Path<String>,
    _query: web::Query<DiscoveryChainQueryParams>,
    body: web::Json<DiscoveryChainOverrides>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::connect::post_discovery_chain_persistent(
        req,
        acl_service,
        connect_service,
        path,
        _query,
        body,
        index_provider,
    )
    .await
}

// ============================================================================
// Exported/Imported services (standalone resources)
// ============================================================================

async fn list_exported_services_handler(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    connect_service: web::Data<ConsulConnectService>,
    _query: web::Query<ServiceVisibilityQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::connect::list_exported_services(
        req,
        acl_service,
        connect_service,
        _query,
        index_provider,
    )
    .await
}

async fn list_exported_services_persistent_handler(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    connect_service: web::Data<ConsulConnectService>,
    _query: web::Query<ServiceVisibilityQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::connect::list_exported_services_persistent(
        req,
        acl_service,
        connect_service,
        _query,
        index_provider,
    )
    .await
}

async fn list_imported_services_handler(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    connect_service: web::Data<ConsulConnectService>,
    _query: web::Query<ServiceVisibilityQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::connect::list_imported_services(
        req,
        acl_service,
        connect_service,
        _query,
        index_provider,
    )
    .await
}

async fn list_imported_services_persistent_handler(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    connect_service: web::Data<ConsulConnectService>,
    _query: web::Query<ServiceVisibilityQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::connect::list_imported_services_persistent(
        req,
        acl_service,
        connect_service,
        _query,
        index_provider,
    )
    .await
}

// ============================================================================
// Connect Proxy config (Envoy bootstrap)
// ============================================================================

async fn get_proxy_config_handler(
    req: HttpRequest,
    agent: web::Data<ConsulAgentService>,
    acl_service: web::Data<AclService>,
    connect_service: web::Data<ConsulConnectService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    index_provider: web::Data<ConsulIndexProvider>,
    path: web::Path<String>,
    query: web::Query<ProxyConfigQueryParams>,
) -> HttpResponse {
    crate::connect::get_proxy_config(
        req, agent, acl_service, connect_service, dc_config, index_provider, path, query,
    )
    .await
}

/// The `routes` function.
pub fn routes() -> Scope {
    web::scope("/discovery-chain")
        .service(get_discovery_chain)
        .service(post_discovery_chain)
        .service(get_discovery_chain_persistent)
        .service(post_discovery_chain_persistent)
}

/// The `proxy_resource` function.
pub fn proxy_resource() -> actix_web::Resource {
    web::resource("/connect/proxy/{service_id}").route(web::get().to(get_proxy_config_handler))
}

/// The `exported_services_resource` function.
pub fn exported_services_resource() -> actix_web::Resource {
    web::resource("/exported-services")
        .route(web::get().to(list_exported_services_handler))
        .route(web::get().to(list_exported_services_persistent_handler))
}

/// The `imported_services_resource` function.
pub fn imported_services_resource() -> actix_web::Resource {
    web::resource("/imported-services")
        .route(web::get().to(list_imported_services_handler))
        .route(web::get().to(list_imported_services_persistent_handler))
}
