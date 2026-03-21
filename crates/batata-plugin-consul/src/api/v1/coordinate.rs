//! Consul Coordinate API handlers with scope-relative route macros.
//!
//! These use `#[get("/datacenters")]` style macros under a "/coordinate" scope.

use actix_web::{HttpRequest, HttpResponse, Scope, get, put, web};

use crate::acl::AclService;
use crate::coordinate::{
    ConsulCoordinateService, ConsulCoordinateServicePersistent, CoordinateQueryParams,
    CoordinateUpdateRequest,
};
use crate::index_provider::ConsulIndexProvider;
use crate::peering::ConsulPeeringService;

// ============================================================================
// In-memory handlers
// ============================================================================

#[get("/datacenters")]
async fn get_coordinate_datacenters(
    coord_service: web::Data<ConsulCoordinateService>,
    peering_service: web::Data<ConsulPeeringService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::coordinate::get_coordinate_datacenters(coord_service, peering_service, index_provider)
        .await
}

#[get("/nodes")]
async fn get_coordinate_nodes(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    coord_service: web::Data<ConsulCoordinateService>,
    query: web::Query<CoordinateQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::coordinate::get_coordinate_nodes(req, acl_service, coord_service, query, index_provider)
        .await
}

#[get("/node/{node}")]
async fn get_coordinate_node(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    coord_service: web::Data<ConsulCoordinateService>,
    path: web::Path<String>,
    _query: web::Query<CoordinateQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::coordinate::get_coordinate_node(
        req,
        acl_service,
        coord_service,
        path,
        _query,
        index_provider,
    )
    .await
}

#[put("/update")]
async fn update_coordinate(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    coord_service: web::Data<ConsulCoordinateService>,
    body: web::Json<CoordinateUpdateRequest>,
) -> HttpResponse {
    crate::coordinate::update_coordinate(req, acl_service, coord_service, body).await
}

// ============================================================================
// Persistent handlers
// ============================================================================

#[get("/datacenters")]
async fn get_coordinate_datacenters_persistent(
    coord_service: web::Data<ConsulCoordinateServicePersistent>,
    peering_service: web::Data<ConsulPeeringService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::coordinate::get_coordinate_datacenters_persistent(
        coord_service,
        peering_service,
        index_provider,
    )
    .await
}

#[get("/nodes")]
async fn get_coordinate_nodes_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    coord_service: web::Data<ConsulCoordinateServicePersistent>,
    query: web::Query<CoordinateQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::coordinate::get_coordinate_nodes_persistent(
        req,
        acl_service,
        coord_service,
        query,
        index_provider,
    )
    .await
}

#[get("/node/{node}")]
async fn get_coordinate_node_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    coord_service: web::Data<ConsulCoordinateServicePersistent>,
    path: web::Path<String>,
    _query: web::Query<CoordinateQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::coordinate::get_coordinate_node_persistent(
        req,
        acl_service,
        coord_service,
        path,
        _query,
        index_provider,
    )
    .await
}

#[put("/update")]
async fn update_coordinate_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    coord_service: web::Data<ConsulCoordinateServicePersistent>,
    body: web::Json<CoordinateUpdateRequest>,
) -> HttpResponse {
    crate::coordinate::update_coordinate_persistent(req, acl_service, coord_service, body).await
}

// ============================================================================
// Route registration
// ============================================================================

pub fn routes() -> Scope {
    web::scope("/coordinate")
        .service(get_coordinate_datacenters)
        .service(get_coordinate_nodes)
        .service(get_coordinate_node)
        .service(update_coordinate)
}

pub fn routes_persistent() -> Scope {
    web::scope("/coordinate")
        .service(get_coordinate_datacenters_persistent)
        .service(get_coordinate_nodes_persistent)
        .service(get_coordinate_node_persistent)
        .service(update_coordinate_persistent)
}
