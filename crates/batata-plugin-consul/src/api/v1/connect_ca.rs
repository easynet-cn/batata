//! Consul Connect CA & Intentions API handlers with full-path route macros.

use actix_web::{delete, get, post, put, web, HttpRequest, Responder};

use crate::acl::AclService;
use crate::connect_ca::{
    AgentAuthorizeRequest, CAConfig, CARootQueryParams, ConsulConnectCAService,
    IntentionExactQuery, IntentionMatchQuery, IntentionQueryParams, IntentionRequest,
};
use crate::index_provider::ConsulIndexProvider;

// ============================================================================
// In-memory handlers
// ============================================================================

#[get("/v1/connect/ca/roots")]
pub async fn get_ca_roots(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    _query: web::Query<CARootQueryParams>,
) -> impl Responder {
    crate::connect_ca::get_ca_roots(req, acl_service, ca_service, index_provider, _query).await
}

#[get("/v1/connect/ca/configuration")]
pub async fn get_ca_configuration(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> impl Responder {
    crate::connect_ca::get_ca_configuration(req, acl_service, ca_service, index_provider).await
}

#[put("/v1/connect/ca/configuration")]
pub async fn set_ca_configuration(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    body: web::Json<CAConfig>,
) -> impl Responder {
    crate::connect_ca::set_ca_configuration(req, acl_service, ca_service, index_provider, body)
        .await
}

#[get("/v1/agent/connect/ca/leaf/{service}")]
pub async fn get_leaf_cert(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    path: web::Path<String>,
) -> impl Responder {
    crate::connect_ca::get_leaf_cert(req, acl_service, ca_service, index_provider, path).await
}

#[post("/v1/agent/connect/authorize")]
pub async fn connect_authorize(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    body: web::Json<AgentAuthorizeRequest>,
) -> impl Responder {
    crate::connect_ca::connect_authorize(req, acl_service, ca_service, index_provider, body).await
}

#[get("/v1/connect/intentions/check")]
pub async fn check_intention(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    query: web::Query<IntentionQueryParams>,
) -> impl Responder {
    crate::connect_ca::check_intention(req, acl_service, ca_service, index_provider, query).await
}

#[get("/v1/connect/intentions/match")]
pub async fn match_intentions(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    query: web::Query<IntentionMatchQuery>,
) -> impl Responder {
    crate::connect_ca::match_intentions(req, acl_service, ca_service, index_provider, query).await
}

#[get("/v1/connect/intentions/exact")]
pub async fn get_intention_exact(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    query: web::Query<IntentionExactQuery>,
) -> impl Responder {
    crate::connect_ca::get_intention_exact(req, acl_service, ca_service, index_provider, query)
        .await
}

#[put("/v1/connect/intentions/exact")]
pub async fn upsert_intention_exact(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    body: web::Json<IntentionRequest>,
) -> impl Responder {
    crate::connect_ca::upsert_intention_exact(req, acl_service, ca_service, index_provider, body)
        .await
}

#[delete("/v1/connect/intentions/exact")]
pub async fn delete_intention_exact(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    query: web::Query<IntentionExactQuery>,
) -> impl Responder {
    crate::connect_ca::delete_intention_exact(req, acl_service, ca_service, index_provider, query)
        .await
}

#[get("/v1/connect/intentions")]
pub async fn list_intentions(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    _query: web::Query<IntentionQueryParams>,
) -> impl Responder {
    crate::connect_ca::list_intentions(req, acl_service, ca_service, index_provider, _query).await
}

#[post("/v1/connect/intentions")]
pub async fn create_intention(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    body: web::Json<IntentionRequest>,
) -> impl Responder {
    crate::connect_ca::create_intention(req, acl_service, ca_service, index_provider, body).await
}

#[get("/v1/connect/intentions/{id}")]
pub async fn get_intention(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    path: web::Path<String>,
) -> impl Responder {
    crate::connect_ca::get_intention(req, acl_service, ca_service, index_provider, path).await
}

#[put("/v1/connect/intentions/{id}")]
pub async fn update_intention(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    path: web::Path<String>,
    body: web::Json<IntentionRequest>,
) -> impl Responder {
    crate::connect_ca::update_intention(req, acl_service, ca_service, index_provider, path, body)
        .await
}

#[delete("/v1/connect/intentions/{id}")]
pub async fn delete_intention(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    path: web::Path<String>,
) -> impl Responder {
    crate::connect_ca::delete_intention(req, acl_service, ca_service, index_provider, path).await
}

// ============================================================================
// Persistent handlers
// ============================================================================

#[get("/v1/connect/ca/roots")]
pub async fn get_ca_roots_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    _query: web::Query<CARootQueryParams>,
) -> impl Responder {
    crate::connect_ca::get_ca_roots_persistent(req, acl_service, ca_service, index_provider, _query).await
}

#[get("/v1/connect/ca/configuration")]
pub async fn get_ca_configuration_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> impl Responder {
    crate::connect_ca::get_ca_configuration_persistent(req, acl_service, ca_service, index_provider).await
}

#[put("/v1/connect/ca/configuration")]
pub async fn set_ca_configuration_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    body: web::Json<CAConfig>,
) -> impl Responder {
    crate::connect_ca::set_ca_configuration_persistent(req, acl_service, ca_service, index_provider, body).await
}

#[get("/v1/agent/connect/ca/leaf/{service}")]
pub async fn get_leaf_cert_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    path: web::Path<String>,
) -> impl Responder {
    crate::connect_ca::get_leaf_cert_persistent(req, acl_service, ca_service, index_provider, path).await
}

#[post("/v1/agent/connect/authorize")]
pub async fn connect_authorize_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    body: web::Json<AgentAuthorizeRequest>,
) -> impl Responder {
    crate::connect_ca::connect_authorize_persistent(req, acl_service, ca_service, index_provider, body).await
}

#[get("/v1/connect/intentions/check")]
pub async fn check_intention_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    query: web::Query<IntentionQueryParams>,
) -> impl Responder {
    crate::connect_ca::check_intention_persistent(req, acl_service, ca_service, index_provider, query).await
}

#[get("/v1/connect/intentions/match")]
pub async fn match_intentions_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    query: web::Query<IntentionMatchQuery>,
) -> impl Responder {
    crate::connect_ca::match_intentions_persistent(req, acl_service, ca_service, index_provider, query).await
}

#[get("/v1/connect/intentions/exact")]
pub async fn get_intention_exact_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    query: web::Query<IntentionExactQuery>,
) -> impl Responder {
    crate::connect_ca::get_intention_exact_persistent(req, acl_service, ca_service, index_provider, query).await
}

#[put("/v1/connect/intentions/exact")]
pub async fn upsert_intention_exact_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    body: web::Json<IntentionRequest>,
) -> impl Responder {
    crate::connect_ca::upsert_intention_exact_persistent(req, acl_service, ca_service, index_provider, body).await
}

#[delete("/v1/connect/intentions/exact")]
pub async fn delete_intention_exact_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    query: web::Query<IntentionExactQuery>,
) -> impl Responder {
    crate::connect_ca::delete_intention_exact_persistent(req, acl_service, ca_service, index_provider, query).await
}

#[get("/v1/connect/intentions")]
pub async fn list_intentions_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    _query: web::Query<IntentionQueryParams>,
) -> impl Responder {
    crate::connect_ca::list_intentions_persistent(req, acl_service, ca_service, index_provider, _query).await
}

#[post("/v1/connect/intentions")]
pub async fn create_intention_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    body: web::Json<IntentionRequest>,
) -> impl Responder {
    crate::connect_ca::create_intention_persistent(req, acl_service, ca_service, index_provider, body).await
}

#[get("/v1/connect/intentions/{id}")]
pub async fn get_intention_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    path: web::Path<String>,
) -> impl Responder {
    crate::connect_ca::get_intention_persistent(req, acl_service, ca_service, index_provider, path).await
}

#[put("/v1/connect/intentions/{id}")]
pub async fn update_intention_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    path: web::Path<String>,
    body: web::Json<IntentionRequest>,
) -> impl Responder {
    crate::connect_ca::update_intention_persistent(req, acl_service, ca_service, index_provider, path, body).await
}

#[delete("/v1/connect/intentions/{id}")]
pub async fn delete_intention_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    path: web::Path<String>,
) -> impl Responder {
    crate::connect_ca::delete_intention_persistent(req, acl_service, ca_service, index_provider, path).await
}
