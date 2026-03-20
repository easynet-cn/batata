//! Consul Peering API handlers with full-path route macros.

use actix_web::{delete, get, post, web, HttpRequest, Responder};

use crate::acl::AclService;
use crate::index_provider::ConsulIndexProvider;
use crate::peering::{
    ConsulPeeringService, PeeringEstablishRequest, PeeringGenerateTokenRequest, PeeringQueryParams,
};

// ============================================================================
// In-memory handlers
// ============================================================================

#[post("/v1/peering/token")]
pub async fn generate_peering_token(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    peering_service: web::Data<ConsulPeeringService>,
    body: web::Json<PeeringGenerateTokenRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> impl Responder {
    crate::peering::generate_peering_token(req, acl_service, peering_service, body, index_provider)
        .await
}

#[post("/v1/peering/establish")]
pub async fn establish_peering(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    peering_service: web::Data<ConsulPeeringService>,
    body: web::Json<PeeringEstablishRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> impl Responder {
    crate::peering::establish_peering(req, acl_service, peering_service, body, index_provider).await
}

#[get("/v1/peering/{name}")]
pub async fn get_peering(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    peering_service: web::Data<ConsulPeeringService>,
    path: web::Path<String>,
    _query: web::Query<PeeringQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> impl Responder {
    crate::peering::get_peering(req, acl_service, peering_service, path, _query, index_provider)
        .await
}

#[delete("/v1/peering/{name}")]
pub async fn delete_peering(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    peering_service: web::Data<ConsulPeeringService>,
    path: web::Path<String>,
    _query: web::Query<PeeringQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> impl Responder {
    crate::peering::delete_peering(req, acl_service, peering_service, path, _query, index_provider)
        .await
}

#[get("/v1/peerings")]
pub async fn list_peerings(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    peering_service: web::Data<ConsulPeeringService>,
    _query: web::Query<PeeringQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> impl Responder {
    crate::peering::list_peerings(req, acl_service, peering_service, _query, index_provider).await
}

// ============================================================================
// Persistent handlers
// ============================================================================

#[post("/v1/peering/token")]
pub async fn generate_peering_token_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    peering_service: web::Data<ConsulPeeringService>,
    body: web::Json<PeeringGenerateTokenRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> impl Responder {
    crate::peering::generate_peering_token_persistent(
        req,
        acl_service,
        peering_service,
        body,
        index_provider,
    )
    .await
}

#[post("/v1/peering/establish")]
pub async fn establish_peering_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    peering_service: web::Data<ConsulPeeringService>,
    body: web::Json<PeeringEstablishRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> impl Responder {
    crate::peering::establish_peering_persistent(
        req,
        acl_service,
        peering_service,
        body,
        index_provider,
    )
    .await
}

#[get("/v1/peering/{name}")]
pub async fn get_peering_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    peering_service: web::Data<ConsulPeeringService>,
    path: web::Path<String>,
    _query: web::Query<PeeringQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> impl Responder {
    crate::peering::get_peering_persistent(
        req,
        acl_service,
        peering_service,
        path,
        _query,
        index_provider,
    )
    .await
}

#[delete("/v1/peering/{name}")]
pub async fn delete_peering_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    peering_service: web::Data<ConsulPeeringService>,
    path: web::Path<String>,
    _query: web::Query<PeeringQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> impl Responder {
    crate::peering::delete_peering_persistent(
        req,
        acl_service,
        peering_service,
        path,
        _query,
        index_provider,
    )
    .await
}

#[get("/v1/peerings")]
pub async fn list_peerings_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    peering_service: web::Data<ConsulPeeringService>,
    _query: web::Query<PeeringQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> impl Responder {
    crate::peering::list_peerings_persistent(
        req,
        acl_service,
        peering_service,
        _query,
        index_provider,
    )
    .await
}
