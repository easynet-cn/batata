//! V3 Admin MCP server management endpoints

use std::sync::Arc;

use crate::{
    api::ai::{McpServerRegistry, model::McpServerQuery, model::McpServerRegistration},
    model::response::RestResult,
};
use actix_web::{HttpResponse, Responder, delete, get, post, put, web};

/// GET /v3/admin/ai/mcp/list
#[get("list")]
async fn list_mcp(
    registry: web::Data<Arc<McpServerRegistry>>,
    query: web::Query<McpServerQuery>,
) -> impl Responder {
    let result = registry.list(&query.into_inner());
    HttpResponse::Ok().json(RestResult::ok(Some(result)))
}

/// GET /v3/admin/ai/mcp
#[get("{namespace}/{name}")]
async fn get_mcp(
    registry: web::Data<Arc<McpServerRegistry>>,
    path: web::Path<(String, String)>,
) -> impl Responder {
    let (namespace, name) = path.into_inner();

    match registry.get(&namespace, &name) {
        Some(server) => HttpResponse::Ok().json(RestResult::ok(Some(server))),
        None => HttpResponse::NotFound().json(RestResult::<()>::err(
            404,
            &format!("Server '{}' not found in namespace '{}'", name, namespace),
        )),
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

/// PUT /v3/admin/ai/mcp/{namespace}/{name}
#[put("{namespace}/{name}")]
async fn update_mcp(
    registry: web::Data<Arc<McpServerRegistry>>,
    path: web::Path<(String, String)>,
    body: web::Json<McpServerRegistration>,
) -> impl Responder {
    let (namespace, name) = path.into_inner();

    match registry.update(&namespace, &name, body.into_inner()) {
        Ok(server) => HttpResponse::Ok().json(RestResult::ok(Some(server))),
        Err(e) => HttpResponse::NotFound().json(RestResult::<()>::err(404, &e)),
    }
}

/// DELETE /v3/admin/ai/mcp/{namespace}/{name}
#[delete("{namespace}/{name}")]
async fn delete_mcp(
    registry: web::Data<Arc<McpServerRegistry>>,
    path: web::Path<(String, String)>,
) -> impl Responder {
    let (namespace, name) = path.into_inner();

    match registry.deregister(&namespace, &name) {
        Ok(()) => HttpResponse::Ok().json(RestResult::ok(Some(true))),
        Err(e) => HttpResponse::NotFound().json(RestResult::<()>::err(404, &e)),
    }
}

pub fn routes() -> actix_web::Scope {
    web::scope("/mcp")
        .service(list_mcp)
        .service(create_mcp)
        .service(get_mcp)
        .service(update_mcp)
        .service(delete_mcp)
}
