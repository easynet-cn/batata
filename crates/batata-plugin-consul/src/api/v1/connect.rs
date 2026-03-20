//! Consul Connect/Service Mesh API handlers with scope-relative route macros.
//!
//! Discovery chain handlers use scope "/discovery-chain".
//! Exported/imported services use standalone resources.

use actix_web::{get, post, web, HttpRequest, HttpResponse, Scope};

use crate::acl::AclService;
use crate::connect::{
    ConsulConnectService, DiscoveryChainOverrides, DiscoveryChainQueryParams,
    ServiceVisibilityQueryParams,
};
use crate::index_provider::ConsulIndexProvider;

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

pub fn routes() -> Scope {
    web::scope("/discovery-chain")
        .service(get_discovery_chain)
        .service(post_discovery_chain)
        .service(get_discovery_chain_persistent)
        .service(post_discovery_chain_persistent)
}

pub fn exported_services_resource() -> actix_web::Resource {
    web::resource("/exported-services")
        .route(web::get().to(list_exported_services_handler))
        .route(web::get().to(list_exported_services_persistent_handler))
}

pub fn imported_services_resource() -> actix_web::Resource {
    web::resource("/imported-services")
        .route(web::get().to(list_imported_services_handler))
        .route(web::get().to(list_imported_services_persistent_handler))
}
