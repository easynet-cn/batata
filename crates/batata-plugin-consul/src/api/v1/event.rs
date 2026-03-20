//! Consul Event API handlers with scope-relative route macros.
//!
//! These delegate to the original handlers in `crate::event`.
//! Includes both in-memory and persistent (database-backed) variants.

use actix_web::{HttpRequest, HttpResponse, Scope, get, put, web};

use crate::acl::AclService;
use crate::event::ConsulEventServicePersistent;
use crate::index_provider::ConsulIndexProvider;
use crate::model::{EventFireParams, EventFireRequest, EventListParams};

// ============================================================================
// In-memory handlers
// ============================================================================

#[put("/fire/{name}")]
async fn fire_event(
    req: HttpRequest,
    path: web::Path<String>,
    query: web::Query<EventFireParams>,
    body: Option<web::Json<EventFireRequest>>,
    acl_service: web::Data<AclService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::event::fire_event(req, path, query, body, acl_service, index_provider).await
}

#[get("/list")]
async fn list_events(
    req: HttpRequest,
    query: web::Query<EventListParams>,
    acl_service: web::Data<AclService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::event::list_events(req, query, acl_service, index_provider).await
}

// ============================================================================
// Persistent handlers
// ============================================================================

#[put("/fire/{name}")]
async fn fire_event_persistent(
    req: HttpRequest,
    path: web::Path<String>,
    query: web::Query<EventFireParams>,
    body: Option<web::Json<EventFireRequest>>,
    acl_service: web::Data<AclService>,
    event_service: web::Data<ConsulEventServicePersistent>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::event::fire_event_persistent(
        req,
        path,
        query,
        body,
        acl_service,
        event_service,
        index_provider,
    )
    .await
}

#[get("/list")]
async fn list_events_persistent(
    req: HttpRequest,
    query: web::Query<EventListParams>,
    acl_service: web::Data<AclService>,
    event_service: web::Data<ConsulEventServicePersistent>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::event::list_events_persistent(req, query, acl_service, event_service, index_provider)
        .await
}

// ============================================================================
// Scope builders
// ============================================================================

/// In-memory event routes
pub fn routes() -> Scope {
    web::scope("/event")
        .service(fire_event)
        .service(list_events)
}

/// Persistent (database-backed) event routes
pub fn routes_persistent() -> Scope {
    web::scope("/event")
        .service(fire_event_persistent)
        .service(list_events_persistent)
}
