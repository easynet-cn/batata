//! V3 Admin A2A agent management endpoints
//! Aligned with Nacos V3 Admin API contract

use std::sync::Arc;

use actix_web::{HttpResponse, Responder, delete, get, post, put, web};

use crate::{
    api::ai::{
        AgentRegistry,
        model::{
            AgentDeleteQuery, AgentDetailQuery, AgentListQuery, AgentRegistrationRequest,
            AgentVersionListQuery,
        },
    },
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

/// GET /v3/admin/ai/a2a?namespaceId=xxx&agentName=xxx
#[get("")]
async fn get_agent(
    registry: web::Data<Arc<AgentRegistry>>,
    query: web::Query<AgentDetailQuery>,
) -> impl Responder {
    match registry.get_by_query(&query.into_inner()) {
        Some(agent) => HttpResponse::Ok().json(RestResult::ok(Some(agent))),
        None => HttpResponse::NotFound().json(RestResult::<()>::err(404, "Agent not found")),
    }
}

/// PUT /v3/admin/ai/a2a?namespaceId=xxx&agentName=xxx
#[put("")]
async fn update_agent(
    registry: web::Data<Arc<AgentRegistry>>,
    query: web::Query<AgentDetailQuery>,
    body: web::Json<AgentRegistrationRequest>,
) -> impl Responder {
    let q = query.into_inner();
    let namespace = q.namespace_id.as_deref().unwrap_or("public").to_string();
    let fallback_name = body.card.name.clone();
    let name = q.agent_name.unwrap_or(fallback_name);

    match registry.update(&namespace, &name, body.into_inner()) {
        Ok(agent) => HttpResponse::Ok().json(RestResult::ok(Some(agent))),
        Err(e) => HttpResponse::NotFound().json(RestResult::<()>::err(404, &e)),
    }
}

/// DELETE /v3/admin/ai/a2a?namespaceId=xxx&agentName=xxx
#[delete("")]
async fn delete_agent(
    registry: web::Data<Arc<AgentRegistry>>,
    query: web::Query<AgentDeleteQuery>,
) -> impl Responder {
    match registry.delete_by_query(&query.into_inner()) {
        Ok(()) => HttpResponse::Ok().json(RestResult::ok(Some(true))),
        Err(e) => HttpResponse::NotFound().json(RestResult::<()>::err(404, &e)),
    }
}

/// GET /v3/admin/ai/a2a/list
#[get("list")]
async fn list_agents(
    registry: web::Data<Arc<AgentRegistry>>,
    query: web::Query<AgentListQuery>,
) -> impl Responder {
    let result = registry.list_with_search(&query.into_inner());
    HttpResponse::Ok().json(RestResult::ok(Some(result)))
}

/// GET /v3/admin/ai/a2a/version/list
#[get("version/list")]
async fn list_versions(
    registry: web::Data<Arc<AgentRegistry>>,
    query: web::Query<AgentVersionListQuery>,
) -> impl Responder {
    let q = query.into_inner();
    let namespace = q.namespace_id.as_deref().unwrap_or("public");
    let name = q.agent_name.as_deref().unwrap_or("");

    let versions = registry.list_versions(namespace, name);
    HttpResponse::Ok().json(RestResult::ok(Some(versions)))
}

pub fn routes() -> actix_web::Scope {
    web::scope("/a2a")
        .service(list_agents)
        .service(list_versions)
        .service(register_agent)
        .service(update_agent)
        .service(get_agent)
        .service(delete_agent)
}
