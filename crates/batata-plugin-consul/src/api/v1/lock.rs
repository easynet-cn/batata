//! Consul Lock & Semaphore API handlers with scope-relative route macros.
//!
//! Lock handlers use scope "/lock", semaphore handlers use scope "/semaphore".

use actix_web::{HttpRequest, HttpResponse, Scope, delete, get, post, put, web};

use crate::acl::AclService;
use crate::lock::{
    ConsulLockService, ConsulSemaphoreService, LockOptions, LockReleaseQuery, SemaphoreOptions,
};

// ============================================================================
// Lock handlers
// ============================================================================

#[post("/acquire")]
async fn acquire_lock(
    req: HttpRequest,
    lock_service: web::Data<ConsulLockService>,
    acl_service: web::Data<AclService>,
    body: web::Json<LockOptions>,
) -> HttpResponse {
    crate::lock::acquire_lock(req, lock_service, acl_service, body).await
}

#[put("/release/{key:.*}")]
async fn release_lock(
    req: HttpRequest,
    lock_service: web::Data<ConsulLockService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    query: web::Query<LockReleaseQuery>,
) -> HttpResponse {
    crate::lock::release_lock(req, lock_service, acl_service, path, query).await
}

#[put("/renew/{key:.*}")]
async fn renew_lock(
    req: HttpRequest,
    lock_service: web::Data<ConsulLockService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    query: web::Query<LockReleaseQuery>,
) -> HttpResponse {
    crate::lock::renew_lock(req, lock_service, acl_service, path, query).await
}

#[get("/{key:.*}")]
async fn get_lock(
    req: HttpRequest,
    lock_service: web::Data<ConsulLockService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
) -> HttpResponse {
    crate::lock::get_lock(req, lock_service, acl_service, path).await
}

#[delete("/{key:.*}")]
async fn destroy_lock(
    req: HttpRequest,
    lock_service: web::Data<ConsulLockService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
) -> HttpResponse {
    crate::lock::destroy_lock(req, lock_service, acl_service, path).await
}

pub fn routes() -> Scope {
    web::scope("/lock")
        .service(acquire_lock)
        .service(release_lock)
        .service(renew_lock)
        .service(get_lock)
        .service(destroy_lock)
}

// ============================================================================
// Semaphore handlers
// ============================================================================

#[post("/acquire")]
async fn acquire_semaphore(
    req: HttpRequest,
    semaphore_service: web::Data<ConsulSemaphoreService>,
    acl_service: web::Data<AclService>,
    body: web::Json<SemaphoreOptions>,
) -> HttpResponse {
    crate::lock::acquire_semaphore(req, semaphore_service, acl_service, body).await
}

#[put("/release/{prefix:.*}")]
async fn release_semaphore(
    req: HttpRequest,
    semaphore_service: web::Data<ConsulSemaphoreService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    query: web::Query<LockReleaseQuery>,
) -> HttpResponse {
    crate::lock::release_semaphore(req, semaphore_service, acl_service, path, query).await
}

#[get("/{prefix:.*}")]
async fn get_semaphore(
    req: HttpRequest,
    semaphore_service: web::Data<ConsulSemaphoreService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
) -> HttpResponse {
    crate::lock::get_semaphore(req, semaphore_service, acl_service, path).await
}

pub fn semaphore_routes() -> Scope {
    web::scope("/semaphore")
        .service(acquire_semaphore)
        .service(release_semaphore)
        .service(get_semaphore)
}
