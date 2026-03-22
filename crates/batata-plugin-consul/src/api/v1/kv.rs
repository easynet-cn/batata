//! Consul KV API handlers with scope-relative route macros.
//!
//! These use `#[get("/{key:.*}")]` style macros under a "/kv" scope.

use actix_web::{HttpRequest, HttpResponse, Scope, delete, get, post, put, web};

use crate::acl::AclService;
use crate::index_provider::ConsulIndexProvider;
use crate::kv::{ConsulKVService, KVPair, KVQueryParams};

// ============================================================================
// In-memory handlers
// ============================================================================

#[get("/export")]
async fn export_kv(
    kv_service: web::Data<ConsulKVService>,
    acl_service: web::Data<AclService>,
    req: HttpRequest,
) -> HttpResponse {
    crate::kv::export_kv(kv_service, acl_service, req).await
}

#[post("/import")]
async fn import_kv(
    kv_service: web::Data<ConsulKVService>,
    acl_service: web::Data<AclService>,
    req: HttpRequest,
    body: web::Json<Vec<KVPair>>,
) -> HttpResponse {
    crate::kv::import_kv(kv_service, acl_service, req, body).await
}

#[get("/{key:.*}")]
async fn get_kv(
    kv_service: web::Data<ConsulKVService>,
    index_provider: web::Data<ConsulIndexProvider>,
    acl_service: web::Data<AclService>,
    req: HttpRequest,
    query: web::Query<KVQueryParams>,
) -> HttpResponse {
    crate::kv::get_kv(kv_service, index_provider, acl_service, req, query).await
}

#[put("/{key:.*}")]
async fn put_kv(
    kv_service: web::Data<ConsulKVService>,
    acl_service: web::Data<AclService>,
    req: HttpRequest,
    query: web::Query<KVQueryParams>,
    body: web::Bytes,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::kv::put_kv(kv_service, acl_service, req, query, body, index_provider).await
}

#[delete("/{key:.*}")]
async fn delete_kv(
    kv_service: web::Data<ConsulKVService>,
    index_provider: web::Data<ConsulIndexProvider>,
    acl_service: web::Data<AclService>,
    req: HttpRequest,
    query: web::Query<KVQueryParams>,
) -> HttpResponse {
    crate::kv::delete_kv(kv_service, index_provider, acl_service, req, query).await
}

pub fn routes() -> Scope {
    web::scope("/kv")
        .service(export_kv)
        .service(import_kv)
        .service(get_kv)
        .service(put_kv)
        .service(delete_kv)
}

pub fn txn_resource() -> actix_web::Resource {
    web::resource("/txn").route(web::put().to(crate::kv::txn))
}
