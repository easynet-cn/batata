//! Consul Lock & Semaphore API handlers with full-path route macros.

use actix_web::{delete, get, post, put, web, HttpRequest, Responder};

use crate::acl::AclService;
use crate::lock::{
    ConsulLockService, ConsulSemaphoreService, LockOptions, LockReleaseQuery, SemaphoreOptions,
};

#[post("/v1/lock/acquire")]
pub async fn acquire_lock(
    req: HttpRequest,
    lock_service: web::Data<ConsulLockService>,
    acl_service: web::Data<AclService>,
    body: web::Json<LockOptions>,
) -> impl Responder {
    crate::lock::acquire_lock(req, lock_service, acl_service, body).await
}

#[put("/v1/lock/release/{key:.*}")]
pub async fn release_lock(
    req: HttpRequest,
    lock_service: web::Data<ConsulLockService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    query: web::Query<LockReleaseQuery>,
) -> impl Responder {
    crate::lock::release_lock(req, lock_service, acl_service, path, query).await
}

#[get("/v1/lock/{key:.*}")]
pub async fn get_lock(
    req: HttpRequest,
    lock_service: web::Data<ConsulLockService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
) -> impl Responder {
    crate::lock::get_lock(req, lock_service, acl_service, path).await
}

#[delete("/v1/lock/{key:.*}")]
pub async fn destroy_lock(
    req: HttpRequest,
    lock_service: web::Data<ConsulLockService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
) -> impl Responder {
    crate::lock::destroy_lock(req, lock_service, acl_service, path).await
}

#[put("/v1/lock/renew/{key:.*}")]
pub async fn renew_lock(
    req: HttpRequest,
    lock_service: web::Data<ConsulLockService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    query: web::Query<LockReleaseQuery>,
) -> impl Responder {
    crate::lock::renew_lock(req, lock_service, acl_service, path, query).await
}

#[post("/v1/semaphore/acquire")]
pub async fn acquire_semaphore(
    req: HttpRequest,
    semaphore_service: web::Data<ConsulSemaphoreService>,
    acl_service: web::Data<AclService>,
    body: web::Json<SemaphoreOptions>,
) -> impl Responder {
    crate::lock::acquire_semaphore(req, semaphore_service, acl_service, body).await
}

#[put("/v1/semaphore/release/{prefix:.*}")]
pub async fn release_semaphore(
    req: HttpRequest,
    semaphore_service: web::Data<ConsulSemaphoreService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    query: web::Query<LockReleaseQuery>,
) -> impl Responder {
    crate::lock::release_semaphore(req, semaphore_service, acl_service, path, query).await
}

#[get("/v1/semaphore/{prefix:.*}")]
pub async fn get_semaphore(
    req: HttpRequest,
    semaphore_service: web::Data<ConsulSemaphoreService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
) -> impl Responder {
    crate::lock::get_semaphore(req, semaphore_service, acl_service, path).await
}
