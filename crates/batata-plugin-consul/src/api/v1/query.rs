#![allow(clippy::too_many_arguments)]
//! Consul Prepared Query API handlers with scope-relative route macros.
//!
//! These use `#[get("/{uuid}")]` style macros under a "/query" scope.

use actix_web::{HttpRequest, HttpResponse, Scope, delete, get, post, put, web};

use crate::acl::AclService;
use crate::index_provider::ConsulIndexProvider;
use crate::model::{ConsulDatacenterConfig, PreparedQueryCreateRequest, PreparedQueryParams};
use crate::query::ConsulQueryService;

// ============================================================================
// Handlers
// ============================================================================

#[post("")]
async fn create_query(
    req: HttpRequest,
    body: web::Json<PreparedQueryCreateRequest>,
    acl_service: web::Data<AclService>,
    query_service: web::Data<ConsulQueryService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::query::create_query(req, body, acl_service, query_service, index_provider).await
}

#[get("")]
async fn list_queries(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    query_service: web::Data<ConsulQueryService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::query::list_queries(req, acl_service, query_service, index_provider).await
}

#[get("/{uuid}")]
async fn get_query(
    req: HttpRequest,
    path: web::Path<String>,
    acl_service: web::Data<AclService>,
    query_service: web::Data<ConsulQueryService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::query::get_query(req, path, acl_service, query_service, index_provider).await
}

#[put("/{uuid}")]
async fn update_query(
    req: HttpRequest,
    path: web::Path<String>,
    body: web::Json<PreparedQueryCreateRequest>,
    acl_service: web::Data<AclService>,
    query_service: web::Data<ConsulQueryService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::query::update_query(req, path, body, acl_service, query_service, index_provider).await
}

#[delete("/{uuid}")]
async fn delete_query(
    req: HttpRequest,
    path: web::Path<String>,
    acl_service: web::Data<AclService>,
    query_service: web::Data<ConsulQueryService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::query::delete_query(req, path, acl_service, query_service, index_provider).await
}

#[get("/{uuid}/execute")]
async fn execute_query(
    req: HttpRequest,
    path: web::Path<String>,
    query_params: web::Query<PreparedQueryParams>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    query_service: web::Data<ConsulQueryService>,
    naming_store: web::Data<crate::naming_store::ConsulNamingStore>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::query::execute_query(
        req,
        path,
        query_params,
        acl_service,
        dc_config,
        query_service,
        naming_store,
        index_provider,
    )
    .await
}

#[get("/{uuid}/explain")]
async fn explain_query(
    req: HttpRequest,
    path: web::Path<String>,
    acl_service: web::Data<AclService>,
    query_service: web::Data<ConsulQueryService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::query::explain_query(req, path, acl_service, query_service, index_provider).await
}

pub fn routes() -> Scope {
    web::scope("/query")
        .service(create_query)
        .service(list_queries)
        .service(execute_query)
        .service(explain_query)
        .service(get_query)
        .service(update_query)
        .service(delete_query)
}
