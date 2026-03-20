//! Consul Connect/Service Mesh API handlers with full-path route macros.

use actix_web::{get, post, web, HttpRequest, Responder};

use crate::acl::AclService;
use crate::connect::{
    ConsulConnectService, DiscoveryChainOverrides, DiscoveryChainQueryParams,
    ServiceVisibilityQueryParams,
};
use crate::index_provider::ConsulIndexProvider;

// ============================================================================
// In-memory handlers
// ============================================================================

#[get("/v1/discovery-chain/{service}")]
pub async fn get_discovery_chain(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    connect_service: web::Data<ConsulConnectService>,
    path: web::Path<String>,
    _query: web::Query<DiscoveryChainQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> impl Responder {
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

#[post("/v1/discovery-chain/{service}")]
pub async fn post_discovery_chain(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    connect_service: web::Data<ConsulConnectService>,
    path: web::Path<String>,
    _query: web::Query<DiscoveryChainQueryParams>,
    body: web::Json<DiscoveryChainOverrides>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> impl Responder {
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

#[get("/v1/exported-services")]
pub async fn list_exported_services(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    connect_service: web::Data<ConsulConnectService>,
    _query: web::Query<ServiceVisibilityQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> impl Responder {
    crate::connect::list_exported_services(
        req,
        acl_service,
        connect_service,
        _query,
        index_provider,
    )
    .await
}

#[get("/v1/imported-services")]
pub async fn list_imported_services(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    connect_service: web::Data<ConsulConnectService>,
    _query: web::Query<ServiceVisibilityQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> impl Responder {
    crate::connect::list_imported_services(
        req,
        acl_service,
        connect_service,
        _query,
        index_provider,
    )
    .await
}

// ============================================================================
// Persistent handlers
// ============================================================================

#[get("/v1/discovery-chain/{service}")]
pub async fn get_discovery_chain_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    connect_service: web::Data<ConsulConnectService>,
    path: web::Path<String>,
    _query: web::Query<DiscoveryChainQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> impl Responder {
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

#[post("/v1/discovery-chain/{service}")]
pub async fn post_discovery_chain_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    connect_service: web::Data<ConsulConnectService>,
    path: web::Path<String>,
    _query: web::Query<DiscoveryChainQueryParams>,
    body: web::Json<DiscoveryChainOverrides>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> impl Responder {
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

#[get("/v1/exported-services")]
pub async fn list_exported_services_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    connect_service: web::Data<ConsulConnectService>,
    _query: web::Query<ServiceVisibilityQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> impl Responder {
    crate::connect::list_exported_services_persistent(
        req,
        acl_service,
        connect_service,
        _query,
        index_provider,
    )
    .await
}

#[get("/v1/imported-services")]
pub async fn list_imported_services_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    connect_service: web::Data<ConsulConnectService>,
    _query: web::Query<ServiceVisibilityQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> impl Responder {
    crate::connect::list_imported_services_persistent(
        req,
        acl_service,
        connect_service,
        _query,
        index_provider,
    )
    .await
}
