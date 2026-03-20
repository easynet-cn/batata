//! Consul KV API handlers with full-path route macros.

use actix_web::{delete, get, post, put, web, HttpRequest, HttpResponse, Responder};

use crate::acl::AclService;
use crate::index_provider::ConsulIndexProvider;
use crate::kv::{ConsulKVService, KVPair, KVQueryParams, TxnOp};

#[get("/v1/kv/{key:.*}")]
pub async fn get_kv(
    kv_service: web::Data<ConsulKVService>,
    acl_service: web::Data<AclService>,
    req: HttpRequest,
    query: web::Query<KVQueryParams>,
) -> impl Responder {
    crate::kv::get_kv(kv_service, acl_service, req, query).await
}

#[put("/v1/kv/{key:.*}")]
pub async fn put_kv(
    kv_service: web::Data<ConsulKVService>,
    acl_service: web::Data<AclService>,
    req: HttpRequest,
    query: web::Query<KVQueryParams>,
    body: web::Bytes,
    index_provider: web::Data<ConsulIndexProvider>,
) -> impl Responder {
    crate::kv::put_kv(kv_service, acl_service, req, query, body, index_provider).await
}

#[delete("/v1/kv/{key:.*}")]
pub async fn delete_kv(
    kv_service: web::Data<ConsulKVService>,
    acl_service: web::Data<AclService>,
    req: HttpRequest,
    query: web::Query<KVQueryParams>,
) -> impl Responder {
    crate::kv::delete_kv(kv_service, acl_service, req, query).await
}

#[put("/v1/txn")]
pub async fn txn(
    kv_service: web::Data<ConsulKVService>,
    acl_service: web::Data<AclService>,
    req: HttpRequest,
    body: web::Json<Vec<TxnOp>>,
) -> impl Responder {
    crate::kv::txn(kv_service, acl_service, req, body).await
}

#[get("/v1/kv/export")]
pub async fn export_kv(
    kv_service: web::Data<ConsulKVService>,
    acl_service: web::Data<AclService>,
    req: HttpRequest,
) -> impl Responder {
    crate::kv::export_kv(kv_service, acl_service, req).await
}

#[post("/v1/kv/import")]
pub async fn import_kv(
    kv_service: web::Data<ConsulKVService>,
    acl_service: web::Data<AclService>,
    req: HttpRequest,
    body: web::Json<Vec<KVPair>>,
) -> impl Responder {
    crate::kv::import_kv(kv_service, acl_service, req, body).await
}
