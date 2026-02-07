//! V3 Admin A2A agent management endpoints

use std::sync::Arc;

use actix_web::{HttpResponse, Responder, delete, get, post, put, web};

use crate::{
    api::ai::{AgentRegistry, model::AgentQuery, model::AgentRegistrationRequest},
    model::response::RestResult,
};

/// POST /v3/admin/ai/a2a
#[post("")]
async fn register_agent(
    registry: web::Data<Arc<AgentRegistry>>,
    body: web::Json<AgentRegistrationRequest>,
) -> impl Responder {
    match registry.register(body.into_inner()) {
        Ok(agent) => HttpResponse::Ok().json(RestResult::ok(Some(agent))),
        Err(e) => HttpResponse::BadRequest().json(RestResult::<()>::err(400, &e)),
    }
}

/// GET /v3/admin/ai/a2a/{namespace}/{name}
#[get("{namespace}/{name}")]
async fn get_agent(
    registry: web::Data<Arc<AgentRegistry>>,
    path: web::Path<(String, String)>,
) -> impl Responder {
    let (namespace, name) = path.into_inner();

    match registry.get(&namespace, &name) {
        Some(agent) => HttpResponse::Ok().json(RestResult::ok(Some(agent))),
        None => HttpResponse::NotFound().json(RestResult::<()>::err(
            404,
            &format!("Agent '{}' not found in namespace '{}'", name, namespace),
        )),
    }
}

/// PUT /v3/admin/ai/a2a/{namespace}/{name}
#[put("{namespace}/{name}")]
async fn update_agent(
    registry: web::Data<Arc<AgentRegistry>>,
    path: web::Path<(String, String)>,
    body: web::Json<AgentRegistrationRequest>,
) -> impl Responder {
    let (namespace, name) = path.into_inner();

    match registry.update(&namespace, &name, body.into_inner()) {
        Ok(agent) => HttpResponse::Ok().json(RestResult::ok(Some(agent))),
        Err(e) => HttpResponse::NotFound().json(RestResult::<()>::err(404, &e)),
    }
}

/// DELETE /v3/admin/ai/a2a/{namespace}/{name}
#[delete("{namespace}/{name}")]
async fn delete_agent(
    registry: web::Data<Arc<AgentRegistry>>,
    path: web::Path<(String, String)>,
) -> impl Responder {
    let (namespace, name) = path.into_inner();

    match registry.deregister(&namespace, &name) {
        Ok(()) => HttpResponse::Ok().json(RestResult::ok(Some(true))),
        Err(e) => HttpResponse::NotFound().json(RestResult::<()>::err(404, &e)),
    }
}

/// GET /v3/admin/ai/a2a/list
#[get("list")]
async fn list_agents(
    registry: web::Data<Arc<AgentRegistry>>,
    query: web::Query<AgentQuery>,
) -> impl Responder {
    let result = registry.list(&query.into_inner());
    HttpResponse::Ok().json(RestResult::ok(Some(result)))
}

pub fn routes() -> actix_web::Scope {
    web::scope("/a2a")
        .service(list_agents)
        .service(register_agent)
        .service(get_agent)
        .service(update_agent)
        .service(delete_agent)
}
