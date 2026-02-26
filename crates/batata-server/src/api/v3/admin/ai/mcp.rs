//! V3 Admin MCP server management endpoints
//! Aligned with Nacos V3 Admin API contract

use std::sync::Arc;

use crate::{
    api::ai::{
        McpServerRegistry,
        model::{McpDeleteQuery, McpDetailQuery, McpListQuery, McpServerRegistration},
    },
    model::response::RestResult,
};
use actix_web::{HttpResponse, Responder, delete, get, post, put, web};

/// GET /v3/admin/ai/mcp/list
#[get("list")]
async fn list_mcp(
    registry: web::Data<Arc<McpServerRegistry>>,
    query: web::Query<McpListQuery>,
) -> impl Responder {
    let result = registry.list_with_search(&query.into_inner());
    HttpResponse::Ok().json(RestResult::ok(Some(result)))
}

/// GET /v3/admin/ai/mcp?namespaceId=xxx&mcpName=xxx
#[get("")]
async fn get_mcp(
    registry: web::Data<Arc<McpServerRegistry>>,
    query: web::Query<McpDetailQuery>,
) -> impl Responder {
    match registry.get_by_query(&query.into_inner()) {
        Some(server) => HttpResponse::Ok().json(RestResult::ok(Some(server))),
        None => HttpResponse::NotFound().json(RestResult::<()>::err(404, "MCP server not found")),
    }
}

/// POST /v3/admin/ai/mcp
#[post("")]
async fn create_mcp(
    registry: web::Data<Arc<McpServerRegistry>>,
    body: web::Json<McpServerRegistration>,
) -> impl Responder {
    match registry.register(body.into_inner()) {
        Ok(server) => HttpResponse::Ok().json(RestResult::ok(Some(server))),
        Err(e) => HttpResponse::BadRequest().json(RestResult::<()>::err(400, &e)),
    }
}

/// PUT /v3/admin/ai/mcp?namespaceId=xxx&mcpName=xxx
#[put("")]
async fn update_mcp(
    registry: web::Data<Arc<McpServerRegistry>>,
    query: web::Query<McpDetailQuery>,
    body: web::Json<McpServerRegistration>,
) -> impl Responder {
    let q = query.into_inner();
    let namespace = q.namespace_id.as_deref().unwrap_or("public").to_string();
    let fallback_name = body.name.clone();
    let name = q.mcp_name.unwrap_or(fallback_name);

    match registry.update(&namespace, &name, body.into_inner()) {
        Ok(server) => HttpResponse::Ok().json(RestResult::ok(Some(server))),
        Err(e) => HttpResponse::NotFound().json(RestResult::<()>::err(404, &e)),
    }
}

/// DELETE /v3/admin/ai/mcp?namespaceId=xxx&mcpName=xxx
#[delete("")]
async fn delete_mcp(
    registry: web::Data<Arc<McpServerRegistry>>,
    query: web::Query<McpDeleteQuery>,
) -> impl Responder {
    match registry.delete_by_query(&query.into_inner()) {
        Ok(()) => HttpResponse::Ok().json(RestResult::ok(Some(true))),
        Err(e) => HttpResponse::NotFound().json(RestResult::<()>::err(404, &e)),
    }
}

pub fn routes() -> actix_web::Scope {
    web::scope("/mcp")
        .service(list_mcp)
        .service(create_mcp)
        .service(update_mcp)
        .service(get_mcp)
        .service(delete_mcp)
}
