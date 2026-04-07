//! Consul Event API handlers with scope-relative route macros.
//!
//! These delegate to the original handlers in `crate::event`.
//! Events are ephemeral (ring buffer) — no persistent variant needed.

use actix_web::{HttpRequest, HttpResponse, Scope, get, put, web};

use crate::acl::AclService;
use crate::event::ConsulEventService;
use crate::index_provider::ConsulIndexProvider;
use crate::model::{EventFireParams, EventListParams};

#[put("/fire/{name}")]
async fn fire_event(
    req: HttpRequest,
    path: web::Path<String>,
    query: web::Query<EventFireParams>,
    body: web::Bytes,
    acl_service: web::Data<AclService>,
    event_service: web::Data<ConsulEventService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::event::fire_event(
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
async fn list_events(
    req: HttpRequest,
    query: web::Query<EventListParams>,
    acl_service: web::Data<AclService>,
    event_service: web::Data<ConsulEventService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::event::list_events(req, query, acl_service, event_service, index_provider).await
}

/// Event routes (used for both standalone and cluster modes)
pub fn routes() -> Scope {
    web::scope("/event")
        .service(fire_event)
        .service(list_events)
}

/// Persistent event routes (same as routes — events are always in-memory)
pub fn routes_persistent() -> Scope {
    routes()
}
