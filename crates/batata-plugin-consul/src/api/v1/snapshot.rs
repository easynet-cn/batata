//! Consul Snapshot API handlers with full-path route macros.

use actix_web::{HttpRequest, HttpResponse, Responder, get, put, web};

use crate::acl::AclService;
use crate::index_provider::ConsulIndexProvider;
use crate::snapshot::{ConsulSnapshotService, ConsulSnapshotServicePersistent};

// ============================================================================
// In-memory handlers
// ============================================================================

#[get("/v1/snapshot")]
pub async fn save_snapshot(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    snapshot_service: web::Data<ConsulSnapshotService>,
    query: web::Query<crate::snapshot::SnapshotQueryParams>,
) -> impl Responder {
    crate::snapshot::save_snapshot(req, acl_service, snapshot_service, query).await
}

#[put("/v1/snapshot")]
pub async fn restore_snapshot(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    snapshot_service: web::Data<ConsulSnapshotService>,
    body: web::Bytes,
    query: web::Query<crate::snapshot::SnapshotQueryParams>,
) -> impl Responder {
    crate::snapshot::restore_snapshot(req, acl_service, snapshot_service, body, query).await
}

// ============================================================================
// Persistent handlers
// ============================================================================

#[get("/v1/snapshot")]
pub async fn save_snapshot_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    snapshot_service: web::Data<ConsulSnapshotServicePersistent>,
    query: web::Query<crate::snapshot::SnapshotQueryParams>,
) -> impl Responder {
    crate::snapshot::save_snapshot_persistent(req, acl_service, snapshot_service, query).await
}

#[put("/v1/snapshot")]
pub async fn restore_snapshot_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    snapshot_service: web::Data<ConsulSnapshotServicePersistent>,
    body: web::Bytes,
    query: web::Query<crate::snapshot::SnapshotQueryParams>,
) -> impl Responder {
    crate::snapshot::restore_snapshot_persistent(req, acl_service, snapshot_service, body, query)
        .await
}
