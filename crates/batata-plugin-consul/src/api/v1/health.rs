#![allow(clippy::too_many_arguments)]
//! Consul Health API handlers with scope-relative route macros.
//!
//! Thin wrappers that delegate to the original handler functions in
//! `crate::health`.

use std::sync::Arc;

use actix_web::{HttpRequest, HttpResponse, Scope, get, web};

use batata_api::naming::NamingServiceProvider;

use crate::acl::AclService;
use crate::health::ConsulHealthService;
use crate::index_provider::ConsulIndexProvider;
use crate::model::{ConsulDatacenterConfig, HealthQueryParams};

#[get("/service/{service}")]
async fn get_service_health(
    req: HttpRequest,
    naming_service: web::Data<Arc<dyn NamingServiceProvider>>,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    path: web::Path<String>,
    query: web::Query<HealthQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::health::get_service_health(
        req,
        naming_service,
        health_service,
        acl_service,
        dc_config,
        path,
        query,
        index_provider,
    )
    .await
}

#[get("/checks/{service}")]
async fn get_service_checks(
    req: HttpRequest,
    naming_service: web::Data<Arc<dyn NamingServiceProvider>>,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    path: web::Path<String>,
    query: web::Query<HealthQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::health::get_service_checks(
        req,
        naming_service,
        health_service,
        acl_service,
        dc_config,
        path,
        query,
        index_provider,
    )
    .await
}

#[get("/state/{state}")]
async fn get_checks_by_state(
    req: HttpRequest,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    _query: web::Query<HealthQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::health::get_checks_by_state(
        req,
        health_service,
        acl_service,
        path,
        _query,
        index_provider,
    )
    .await
}

#[get("/node/{node}")]
async fn get_node_checks(
    req: HttpRequest,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    _query: web::Query<HealthQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::health::get_node_checks(
        req,
        health_service,
        acl_service,
        path,
        _query,
        index_provider,
    )
    .await
}

#[get("/connect/{service}")]
async fn get_connect_health(
    req: HttpRequest,
    naming_service: web::Data<Arc<dyn NamingServiceProvider>>,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    path: web::Path<String>,
    query: web::Query<HealthQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::health::get_connect_health(
        req,
        naming_service,
        health_service,
        acl_service,
        dc_config,
        path,
        query,
        index_provider,
    )
    .await
}

#[get("/ingress/{service}")]
async fn get_ingress_health(
    req: HttpRequest,
    naming_service: web::Data<Arc<dyn NamingServiceProvider>>,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    path: web::Path<String>,
    query: web::Query<HealthQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::health::get_ingress_health(
        req,
        naming_service,
        health_service,
        acl_service,
        dc_config,
        path,
        query,
        index_provider,
    )
    .await
}

pub fn routes() -> Scope {
    web::scope("/health")
        .service(get_service_health)
        .service(get_service_checks)
        .service(get_checks_by_state)
        .service(get_node_checks)
        .service(get_connect_health)
        .service(get_ingress_health)
}
