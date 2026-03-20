//! Consul Peering API handlers with scope-relative route macros.
//!
//! Peering handlers use scope "/peering", list uses standalone resource "/peerings".

use actix_web::{delete, get, post, web, HttpRequest, HttpResponse, Scope};

use crate::acl::AclService;
use crate::index_provider::ConsulIndexProvider;
use crate::peering::{
    ConsulPeeringService, PeeringEstablishRequest, PeeringGenerateTokenRequest, PeeringQueryParams,
};

// ============================================================================
// In-memory handlers
// ============================================================================

#[post("/token")]
async fn generate_peering_token(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    peering_service: web::Data<ConsulPeeringService>,
    body: web::Json<PeeringGenerateTokenRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::peering::generate_peering_token(req, acl_service, peering_service, body, index_provider)
        .await
}

#[post("/establish")]
async fn establish_peering(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    peering_service: web::Data<ConsulPeeringService>,
    body: web::Json<PeeringEstablishRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::peering::establish_peering(req, acl_service, peering_service, body, index_provider).await
}

#[get("/{name}")]
async fn get_peering(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    peering_service: web::Data<ConsulPeeringService>,
    path: web::Path<String>,
    _query: web::Query<PeeringQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::peering::get_peering(req, acl_service, peering_service, path, _query, index_provider)
        .await
}

#[delete("/{name}")]
async fn delete_peering(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    peering_service: web::Data<ConsulPeeringService>,
    path: web::Path<String>,
    _query: web::Query<PeeringQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::peering::delete_peering(req, acl_service, peering_service, path, _query, index_provider)
        .await
}

// ============================================================================
// Persistent handlers
// ============================================================================

#[post("/token")]
async fn generate_peering_token_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    peering_service: web::Data<ConsulPeeringService>,
    body: web::Json<PeeringGenerateTokenRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::peering::generate_peering_token_persistent(
        req,
        acl_service,
        peering_service,
        body,
        index_provider,
    )
    .await
}

#[post("/establish")]
async fn establish_peering_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    peering_service: web::Data<ConsulPeeringService>,
    body: web::Json<PeeringEstablishRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::peering::establish_peering_persistent(
        req,
        acl_service,
        peering_service,
        body,
        index_provider,
    )
    .await
}

#[get("/{name}")]
async fn get_peering_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    peering_service: web::Data<ConsulPeeringService>,
    path: web::Path<String>,
    _query: web::Query<PeeringQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
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

#[delete("/{name}")]
async fn delete_peering_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    peering_service: web::Data<ConsulPeeringService>,
    path: web::Path<String>,
    _query: web::Query<PeeringQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
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

// ============================================================================
// List peerings (standalone resource at /peerings)
// ============================================================================

async fn list_peerings_handler(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    peering_service: web::Data<ConsulPeeringService>,
    _query: web::Query<PeeringQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::peering::list_peerings(req, acl_service, peering_service, _query, index_provider).await
}

async fn list_peerings_persistent_handler(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    peering_service: web::Data<ConsulPeeringService>,
    _query: web::Query<PeeringQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::peering::list_peerings_persistent(
        req,
        acl_service,
        peering_service,
        _query,
        index_provider,
    )
    .await
}

pub fn routes() -> Scope {
    web::scope("/peering")
        .service(generate_peering_token)
        .service(establish_peering)
        .service(get_peering)
        .service(delete_peering)
        .service(generate_peering_token_persistent)
        .service(establish_peering_persistent)
        .service(get_peering_persistent)
        .service(delete_peering_persistent)
}

pub fn list_resource() -> actix_web::Resource {
    web::resource("/peerings")
        .route(web::get().to(list_peerings_handler))
        .route(web::get().to(list_peerings_persistent_handler))
}
