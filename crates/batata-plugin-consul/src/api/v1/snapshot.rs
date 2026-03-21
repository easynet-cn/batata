//! Consul Snapshot API handlers.
//!
//! Uses a resource (not scope) since GET and PUT share the same "/snapshot" path.

use actix_web::{HttpRequest, HttpResponse, web};

use crate::acl::AclService;
use crate::snapshot::{
    ConsulSnapshotService, ConsulSnapshotServicePersistent, SnapshotQueryParams,
};

// ============================================================================
// In-memory handlers
// ============================================================================

async fn save_snapshot(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    snapshot_service: web::Data<ConsulSnapshotService>,
    query: web::Query<SnapshotQueryParams>,
) -> HttpResponse {
    crate::snapshot::save_snapshot(req, acl_service, snapshot_service, query).await
}

async fn restore_snapshot(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    snapshot_service: web::Data<ConsulSnapshotService>,
    body: web::Bytes,
    query: web::Query<SnapshotQueryParams>,
) -> HttpResponse {
    crate::snapshot::restore_snapshot(req, acl_service, snapshot_service, body, query).await
}

// ============================================================================
// Persistent handlers
// ============================================================================

async fn save_snapshot_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    snapshot_service: web::Data<ConsulSnapshotServicePersistent>,
    query: web::Query<SnapshotQueryParams>,
) -> HttpResponse {
    crate::snapshot::save_snapshot_persistent(req, acl_service, snapshot_service, query).await
}

async fn restore_snapshot_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    snapshot_service: web::Data<ConsulSnapshotServicePersistent>,
    body: web::Bytes,
    query: web::Query<SnapshotQueryParams>,
) -> HttpResponse {
    crate::snapshot::restore_snapshot_persistent(req, acl_service, snapshot_service, body, query)
        .await
}

pub fn routes() -> actix_web::Resource {
    web::resource("/snapshot")
        .route(web::get().to(save_snapshot))
        .route(web::put().to(restore_snapshot))
        .route(web::get().to(save_snapshot_persistent))
        .route(web::put().to(restore_snapshot_persistent))
}
