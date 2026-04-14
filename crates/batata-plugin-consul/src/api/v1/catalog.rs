//! Consul Catalog API handlers with scope-relative route macros.
//!
//! Thin wrappers that delegate to the original handler functions in
//! `crate::catalog`.

use actix_web::{HttpRequest, HttpResponse, Scope, get, put, web};

use crate::acl::AclService;
use crate::catalog::{
    CatalogDeregistration, CatalogQueryParams, CatalogRegistration, ConsulCatalogService,
};
use crate::config_entry::ConsulConfigEntryService;
use crate::index_provider::ConsulIndexProvider;
use crate::model::ConsulDatacenterConfig;
use crate::peering::ConsulPeeringService;

#[get("/datacenters")]
async fn list_datacenters(
    dc_config: web::Data<ConsulDatacenterConfig>,
    peering_service: web::Data<ConsulPeeringService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::catalog::list_datacenters(dc_config, peering_service, index_provider).await
}

#[get("/services")]
async fn list_services(
    req: HttpRequest,
    catalog: web::Data<ConsulCatalogService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    query: web::Query<CatalogQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::catalog::list_services(req, catalog, acl_service, dc_config, query, index_provider).await
}

#[get("/service/{service}")]
async fn get_service(
    req: HttpRequest,
    catalog: web::Data<ConsulCatalogService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    path: web::Path<String>,
    query: web::Query<CatalogQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
    config_entry_service: web::Data<crate::config_entry::ConsulConfigEntryService>,
) -> HttpResponse {
    crate::catalog::get_service(
        req,
        catalog,
        acl_service,
        dc_config,
        path,
        query,
        index_provider,
        config_entry_service,
    )
    .await
}

#[get("/nodes")]
async fn list_nodes(
    req: HttpRequest,
    catalog: web::Data<ConsulCatalogService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    query: web::Query<CatalogQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::catalog::list_nodes(req, catalog, acl_service, dc_config, query, index_provider).await
}

#[get("/node/{node}")]
async fn get_node(
    req: HttpRequest,
    catalog: web::Data<ConsulCatalogService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    path: web::Path<String>,
    query: web::Query<CatalogQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::catalog::get_node(
        req,
        catalog,
        acl_service,
        dc_config,
        path,
        query,
        index_provider,
    )
    .await
}

#[put("/register")]
async fn register(
    req: HttpRequest,
    catalog: web::Data<ConsulCatalogService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    query: web::Query<CatalogQueryParams>,
    body: web::Json<CatalogRegistration>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::catalog::register(
        req,
        catalog,
        acl_service,
        dc_config,
        query,
        body,
        index_provider,
    )
    .await
}

#[put("/deregister")]
async fn deregister(
    req: HttpRequest,
    catalog: web::Data<ConsulCatalogService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    query: web::Query<CatalogQueryParams>,
    body: web::Json<CatalogDeregistration>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::catalog::deregister(
        req,
        catalog,
        acl_service,
        dc_config,
        query,
        body,
        index_provider,
    )
    .await
}

#[get("/connect/{service}")]
async fn get_connect_service(
    req: HttpRequest,
    catalog: web::Data<ConsulCatalogService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    path: web::Path<String>,
    query: web::Query<CatalogQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::catalog::get_connect_service(
        req,
        catalog,
        acl_service,
        dc_config,
        path,
        query,
        index_provider,
    )
    .await
}

#[get("/node-services/{node}")]
async fn get_node_services(
    req: HttpRequest,
    catalog: web::Data<ConsulCatalogService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    path: web::Path<String>,
    query: web::Query<CatalogQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::catalog::get_node_services(
        req,
        catalog,
        acl_service,
        dc_config,
        path,
        query,
        index_provider,
    )
    .await
}

#[get("/gateway-services/{gateway}")]
async fn get_gateway_services(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    catalog: web::Data<ConsulCatalogService>,
    config_entry_service: web::Data<ConsulConfigEntryService>,
    path: web::Path<String>,
    query: web::Query<crate::catalog::CatalogQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::catalog::get_gateway_services(
        req,
        acl_service,
        catalog,
        config_entry_service,
        path,
        query,
        index_provider,
    )
    .await
}

pub fn routes() -> Scope {
    web::scope("/catalog")
        .service(list_datacenters)
        .service(list_services)
        .service(get_service)
        .service(list_nodes)
        .service(get_node)
        .service(register)
        .service(deregister)
        .service(get_connect_service)
        .service(get_node_services)
        .service(get_gateway_services)
}
