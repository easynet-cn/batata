//! Consul Connect CA & Intentions API handlers with scope-relative route macros.
//!
//! CA and intentions handlers use scope "/connect".
//! Agent connect routes (leaf cert, authorize) use a separate scope for /agent/connect.

use actix_web::{delete, get, post, put, web, HttpRequest, HttpResponse, Scope};

use crate::acl::AclService;
use crate::connect_ca::{
    AgentAuthorizeRequest, CAConfig, CARootQueryParams, ConsulConnectCAService,
    IntentionExactQuery, IntentionMatchQuery, IntentionQueryParams, IntentionRequest,
};
use crate::index_provider::ConsulIndexProvider;

// ============================================================================
// In-memory handlers
// ============================================================================

#[get("/ca/roots")]
async fn get_ca_roots(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    _query: web::Query<CARootQueryParams>,
) -> HttpResponse {
    crate::connect_ca::get_ca_roots(req, acl_service, ca_service, index_provider, _query).await
}

#[get("/ca/configuration")]
async fn get_ca_configuration(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::connect_ca::get_ca_configuration(req, acl_service, ca_service, index_provider).await
}

#[put("/ca/configuration")]
async fn set_ca_configuration(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    body: web::Json<CAConfig>,
) -> HttpResponse {
    crate::connect_ca::set_ca_configuration(req, acl_service, ca_service, index_provider, body)
        .await
}

#[get("/intentions/check")]
async fn check_intention(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    query: web::Query<IntentionQueryParams>,
) -> HttpResponse {
    crate::connect_ca::check_intention(req, acl_service, ca_service, index_provider, query).await
}

#[get("/intentions/match")]
async fn match_intentions(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    query: web::Query<IntentionMatchQuery>,
) -> HttpResponse {
    crate::connect_ca::match_intentions(req, acl_service, ca_service, index_provider, query).await
}

#[get("/intentions/exact")]
async fn get_intention_exact(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    query: web::Query<IntentionExactQuery>,
) -> HttpResponse {
    crate::connect_ca::get_intention_exact(req, acl_service, ca_service, index_provider, query)
        .await
}

#[put("/intentions/exact")]
async fn upsert_intention_exact(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    body: web::Json<IntentionRequest>,
) -> HttpResponse {
    crate::connect_ca::upsert_intention_exact(req, acl_service, ca_service, index_provider, body)
        .await
}

#[delete("/intentions/exact")]
async fn delete_intention_exact(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    query: web::Query<IntentionExactQuery>,
) -> HttpResponse {
    crate::connect_ca::delete_intention_exact(req, acl_service, ca_service, index_provider, query)
        .await
}

#[get("/intentions")]
async fn list_intentions(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    _query: web::Query<IntentionQueryParams>,
) -> HttpResponse {
    crate::connect_ca::list_intentions(req, acl_service, ca_service, index_provider, _query).await
}

#[post("/intentions")]
async fn create_intention(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    body: web::Json<IntentionRequest>,
) -> HttpResponse {
    crate::connect_ca::create_intention(req, acl_service, ca_service, index_provider, body).await
}

#[get("/intentions/{id}")]
async fn get_intention(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    path: web::Path<String>,
) -> HttpResponse {
    crate::connect_ca::get_intention(req, acl_service, ca_service, index_provider, path).await
}

#[put("/intentions/{id}")]
async fn update_intention(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    path: web::Path<String>,
    body: web::Json<IntentionRequest>,
) -> HttpResponse {
    crate::connect_ca::update_intention(req, acl_service, ca_service, index_provider, path, body)
        .await
}

#[delete("/intentions/{id}")]
async fn delete_intention(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    path: web::Path<String>,
) -> HttpResponse {
    crate::connect_ca::delete_intention(req, acl_service, ca_service, index_provider, path).await
}

// ============================================================================
// Persistent handlers
// ============================================================================

#[get("/ca/roots")]
async fn get_ca_roots_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    _query: web::Query<CARootQueryParams>,
) -> HttpResponse {
    crate::connect_ca::get_ca_roots_persistent(req, acl_service, ca_service, index_provider, _query)
        .await
}

#[get("/ca/configuration")]
async fn get_ca_configuration_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::connect_ca::get_ca_configuration_persistent(
        req,
        acl_service,
        ca_service,
        index_provider,
    )
    .await
}

#[put("/ca/configuration")]
async fn set_ca_configuration_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    body: web::Json<CAConfig>,
) -> HttpResponse {
    crate::connect_ca::set_ca_configuration_persistent(
        req,
        acl_service,
        ca_service,
        index_provider,
        body,
    )
    .await
}

#[get("/intentions/check")]
async fn check_intention_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    query: web::Query<IntentionQueryParams>,
) -> HttpResponse {
    crate::connect_ca::check_intention_persistent(
        req,
        acl_service,
        ca_service,
        index_provider,
        query,
    )
    .await
}

#[get("/intentions/match")]
async fn match_intentions_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    query: web::Query<IntentionMatchQuery>,
) -> HttpResponse {
    crate::connect_ca::match_intentions_persistent(
        req,
        acl_service,
        ca_service,
        index_provider,
        query,
    )
    .await
}

#[get("/intentions/exact")]
async fn get_intention_exact_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    query: web::Query<IntentionExactQuery>,
) -> HttpResponse {
    crate::connect_ca::get_intention_exact_persistent(
        req,
        acl_service,
        ca_service,
        index_provider,
        query,
    )
    .await
}

#[put("/intentions/exact")]
async fn upsert_intention_exact_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    body: web::Json<IntentionRequest>,
) -> HttpResponse {
    crate::connect_ca::upsert_intention_exact_persistent(
        req,
        acl_service,
        ca_service,
        index_provider,
        body,
    )
    .await
}

#[delete("/intentions/exact")]
async fn delete_intention_exact_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    query: web::Query<IntentionExactQuery>,
) -> HttpResponse {
    crate::connect_ca::delete_intention_exact_persistent(
        req,
        acl_service,
        ca_service,
        index_provider,
        query,
    )
    .await
}

#[get("/intentions")]
async fn list_intentions_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    _query: web::Query<IntentionQueryParams>,
) -> HttpResponse {
    crate::connect_ca::list_intentions_persistent(
        req,
        acl_service,
        ca_service,
        index_provider,
        _query,
    )
    .await
}

#[post("/intentions")]
async fn create_intention_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    body: web::Json<IntentionRequest>,
) -> HttpResponse {
    crate::connect_ca::create_intention_persistent(
        req,
        acl_service,
        ca_service,
        index_provider,
        body,
    )
    .await
}

#[get("/intentions/{id}")]
async fn get_intention_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    path: web::Path<String>,
) -> HttpResponse {
    crate::connect_ca::get_intention_persistent(req, acl_service, ca_service, index_provider, path)
        .await
}

#[put("/intentions/{id}")]
async fn update_intention_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    path: web::Path<String>,
    body: web::Json<IntentionRequest>,
) -> HttpResponse {
    crate::connect_ca::update_intention_persistent(
        req,
        acl_service,
        ca_service,
        index_provider,
        path,
        body,
    )
    .await
}

#[delete("/intentions/{id}")]
async fn delete_intention_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    path: web::Path<String>,
) -> HttpResponse {
    crate::connect_ca::delete_intention_persistent(
        req,
        acl_service,
        ca_service,
        index_provider,
        path,
    )
    .await
}

// ============================================================================
// Agent connect routes (registered under /agent/connect)
// ============================================================================

#[get("/ca/leaf/{service}")]
async fn get_leaf_cert(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    path: web::Path<String>,
) -> HttpResponse {
    crate::connect_ca::get_leaf_cert(req, acl_service, ca_service, index_provider, path).await
}

#[post("/authorize")]
async fn connect_authorize(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    body: web::Json<AgentAuthorizeRequest>,
) -> HttpResponse {
    crate::connect_ca::connect_authorize(req, acl_service, ca_service, index_provider, body).await
}

#[get("/ca/leaf/{service}")]
async fn get_leaf_cert_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    path: web::Path<String>,
) -> HttpResponse {
    crate::connect_ca::get_leaf_cert_persistent(req, acl_service, ca_service, index_provider, path)
        .await
}

#[post("/authorize")]
async fn connect_authorize_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    ca_service: web::Data<ConsulConnectCAService>,
    index_provider: web::Data<ConsulIndexProvider>,
    body: web::Json<AgentAuthorizeRequest>,
) -> HttpResponse {
    crate::connect_ca::connect_authorize_persistent(
        req,
        acl_service,
        ca_service,
        index_provider,
        body,
    )
    .await
}

pub fn routes() -> Scope {
    web::scope("/connect")
        .service(get_ca_roots)
        .service(get_ca_configuration)
        .service(set_ca_configuration)
        .service(check_intention)
        .service(match_intentions)
        .service(get_intention_exact)
        .service(upsert_intention_exact)
        .service(delete_intention_exact)
        .service(list_intentions)
        .service(create_intention)
        .service(get_intention)
        .service(update_intention)
        .service(delete_intention)
        .service(get_ca_roots_persistent)
        .service(get_ca_configuration_persistent)
        .service(set_ca_configuration_persistent)
        .service(check_intention_persistent)
        .service(match_intentions_persistent)
        .service(get_intention_exact_persistent)
        .service(upsert_intention_exact_persistent)
        .service(delete_intention_exact_persistent)
        .service(list_intentions_persistent)
        .service(create_intention_persistent)
        .service(get_intention_persistent)
        .service(update_intention_persistent)
        .service(delete_intention_persistent)
}

/// Agent connect routes to be registered under /agent/connect scope
pub fn agent_connect_routes() -> Scope {
    web::scope("/connect")
        .service(get_leaf_cert)
        .service(connect_authorize)
        .service(get_leaf_cert_persistent)
        .service(connect_authorize_persistent)
}
