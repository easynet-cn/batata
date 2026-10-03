
// actix-web route macros (`#[get]`, `#[post]`, `#[delete]`, ...) expand to a
// struct that cannot carry a doc comment, which trips `missing_docs`. The
// generated struct is an internal implementation detail, so the lint is allowed
// for this module.
#![allow(missing_docs)]
#![allow(clippy::too_many_arguments)]
//! Consul Health API handlers with scope-relative route macros.
//!
//! Thin wrappers that delegate to the original handler functions in
//! `crate::health`.

use actix_web::{HttpRequest, HttpResponse, Scope, get, web};

use crate::acl::AclService;
use crate::health::ConsulHealthService;
use crate::index_provider::ConsulIndexProvider;
use crate::model::{ConsulDatacenterConfig, HealthQueryParams};
use crate::naming_store::ConsulNamingStore;
use crate::peering::ConsulPeeringService;

#[get("/service/{service}")]
async fn get_service_health(
    req: HttpRequest,
    naming_store: web::Data<ConsulNamingStore>,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    path: web::Path<String>,
    query: web::Query<HealthQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
    config_entry_service: web::Data<crate::config_entry::ConsulConfigEntryService>,
    peering_service: web::Data<ConsulPeeringService>,
    coord_service: web::Data<crate::coordinate::ConsulCoordinateService>,
) -> HttpResponse {
    crate::health::get_service_health(
        req,
        naming_store,
        health_service,
        acl_service,
        dc_config,
        path,
        query,
        index_provider,
        config_entry_service,
        peering_service,
        coord_service,
    )
    .await
}

#[get("/checks/{service}")]
async fn get_service_checks(
    req: HttpRequest,
    naming_store: web::Data<ConsulNamingStore>,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    path: web::Path<String>,
    query: web::Query<HealthQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::health::get_service_checks(
        req,
        naming_store,
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
    naming_store: web::Data<ConsulNamingStore>,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    path: web::Path<String>,
    query: web::Query<HealthQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::health::get_connect_health(
        req,
        naming_store,
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
    naming_store: web::Data<ConsulNamingStore>,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    config_entry_service: web::Data<crate::config_entry::ConsulConfigEntryService>,
    path: web::Path<String>,
    query: web::Query<HealthQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::health::get_ingress_health(
        req,
        naming_store,
        health_service,
        acl_service,
        dc_config,
        config_entry_service,
        path,
        query,
        index_provider,
    )
    .await
}

/// GET /v1/health/stream/{service} - SSE stream of service health changes
///
/// Returns a `text/event-stream` response that pushes the service's current
/// health snapshot immediately, then a fresh snapshot whenever the catalog
/// index advances. Used by Envoy sidecars and service-mesh components for
/// low-latency health updates.
#[get("/stream/{service}")]
async fn health_stream_service(
    req: HttpRequest,
    naming_store: web::Data<ConsulNamingStore>,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    path: web::Path<String>,
    query: web::Query<HealthQueryParams>,
    config_entry_service: web::Data<crate::config_entry::ConsulConfigEntryService>,
    coord_service: web::Data<crate::coordinate::ConsulCoordinateService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let service_name = path.into_inner();

    // ACL check: service read access
    let authz = acl_service.authorize_request(
        &req,
        crate::acl::ResourceType::Service,
        &service_name,
        false,
    );
    if !authz.allowed {
        crate::api_metrics::incr_endpoint("health_stream_service", "error");
        return HttpResponse::Forbidden().json(serde_json::json!({
            "error": authz.reason
        }));
    }
    crate::api_metrics::incr_endpoint("health_stream_service", "success");

    let query_inner = query.into_inner();
    let stream = crate::health::stream_service_health(
        req,
        naming_store,
        health_service,
        dc_config,
        service_name,
        query_inner,
        config_entry_service,
        coord_service,
        index_provider,
    );

    HttpResponse::Ok()
        .insert_header(("Content-Type", "text/event-stream"))
        .insert_header(("Cache-Control", "no-cache"))
        .insert_header(("X-Accel-Buffering", "no"))
        .streaming(stream)
}

/// GET /v1/health/stream - SSE stream of all services' health changes
///
/// Streams the health of every registered service. Same SSE semantics as
/// `/health/stream/{service}` but without a service-name filter.
#[get("/stream")]
async fn health_stream_all(
    req: HttpRequest,
    naming_store: web::Data<ConsulNamingStore>,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    query: web::Query<HealthQueryParams>,
    config_entry_service: web::Data<crate::config_entry::ConsulConfigEntryService>,
    coord_service: web::Data<crate::coordinate::ConsulCoordinateService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    // ACL check: service list access (empty name = wildcard read)
    let authz = acl_service.authorize_request(
        &req,
        crate::acl::ResourceType::Service,
        "",
        false,
    );
    if !authz.allowed {
        crate::api_metrics::incr_endpoint("health_stream_all", "error");
        return HttpResponse::Forbidden().json(serde_json::json!({
            "error": authz.reason
        }));
    }
    crate::api_metrics::incr_endpoint("health_stream_all", "success");

    let query_inner = query.into_inner();
    let stream = crate::health::stream_service_health(
        req,
        naming_store,
        health_service,
        dc_config,
        String::new(), // empty service name = all services
        query_inner,
        config_entry_service,
        coord_service,
        index_provider,
    );

    HttpResponse::Ok()
        .insert_header(("Content-Type", "text/event-stream"))
        .insert_header(("Cache-Control", "no-cache"))
        .insert_header(("X-Accel-Buffering", "no"))
        .streaming(stream)
}

/// The `routes` function.
pub fn routes() -> Scope {
    web::scope("/health")
        .service(get_service_health)
        .service(get_service_checks)
        .service(get_checks_by_state)
        .service(get_node_checks)
        .service(get_connect_health)
        .service(get_ingress_health)
        .service(health_stream_service)
        .service(health_stream_all)
}
