//! Consul Session API handlers with scope-relative route macros.
//!
//! These delegate to the original handlers in `crate::session`.

use actix_web::{HttpRequest, HttpResponse, Scope, get, put, web};

use crate::acl::AclService;
use crate::index_provider::ConsulIndexProvider;
use crate::kv::ConsulKVService;
use crate::model::SessionCreateRequest;
use crate::session::ConsulSessionService;

#[put("/create")]
async fn create_session(
    req: HttpRequest,
    session_service: web::Data<ConsulSessionService>,
    acl_service: web::Data<AclService>,
    body: web::Json<SessionCreateRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::session::create_session(req, session_service, acl_service, body, index_provider).await
}

#[put("/destroy/{uuid}")]
async fn destroy_session(
    req: HttpRequest,
    session_service: web::Data<ConsulSessionService>,
    kv_service: web::Data<ConsulKVService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::session::destroy_session(
        req,
        session_service,
        kv_service,
        acl_service,
        path,
        index_provider,
    )
    .await
}

#[get("/info/{uuid}")]
async fn get_session_info(
    req: HttpRequest,
    session_service: web::Data<ConsulSessionService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::session::get_session_info(req, session_service, acl_service, path, index_provider).await
}

#[get("/list")]
async fn list_sessions(
    req: HttpRequest,
    session_service: web::Data<ConsulSessionService>,
    acl_service: web::Data<AclService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::session::list_sessions(req, session_service, acl_service, index_provider).await
}

#[get("/node/{node}")]
async fn list_node_sessions(
    req: HttpRequest,
    session_service: web::Data<ConsulSessionService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::session::list_node_sessions(req, session_service, acl_service, path, index_provider)
        .await
}

#[put("/renew/{uuid}")]
async fn renew_session(
    req: HttpRequest,
    session_service: web::Data<ConsulSessionService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::session::renew_session(req, session_service, acl_service, path, index_provider).await
}

pub fn routes() -> Scope {
    web::scope("/session")
        .service(create_session)
        .service(destroy_session)
        .service(get_session_info)
        .service(list_sessions)
        .service(list_node_sessions)
        .service(renew_session)
}
