// Console MCP server management API endpoints
// This module provides console endpoints for MCP server registration and management

use std::sync::Arc;

use actix_web::{HttpMessage, HttpRequest, Responder, Scope, delete, get, post, put, web};

use crate::{
    ActionTypes, ApiType, Secured, SignType,
    api::ai::model::{
        BatchRegistrationResponse, McpServer, McpServerImportRequest, McpServerListResponse,
        McpServerQuery, McpServerRegistration,
    },
    api::ai::{McpRegistryStats, McpServerRegistry},
    model::{self, common::AppState},
    secured,
};

/// List MCP servers
#[get("")]
async fn list_servers(
    req: HttpRequest,
    data: web::Data<AppState>,
    registry: web::Data<Arc<McpServerRegistry>>,
    params: web::Query<McpServerQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/mcp")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let result = registry.list(&params.into_inner());
    model::common::Result::<McpServerListResponse>::http_success(result)
}

/// Get MCP server by namespace and name
#[get("/{namespace}/{name}")]
async fn get_server(
    req: HttpRequest,
    data: web::Data<AppState>,
    registry: web::Data<Arc<McpServerRegistry>>,
    path: web::Path<(String, String)>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/mcp")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let (namespace, name) = path.into_inner();
    match registry.get(&namespace, &name) {
        Some(server) => model::common::Result::<McpServer>::http_success(server),
        None => model::common::Result::<String>::http_response(
            404,
            404,
            format!("Server '{}' not found in namespace '{}'", name, namespace),
            String::new(),
        ),
    }
}

/// Register a new MCP server
#[post("")]
async fn register_server(
    req: HttpRequest,
    data: web::Data<AppState>,
    registry: web::Data<Arc<McpServerRegistry>>,
    body: web::Json<McpServerRegistration>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/mcp")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    match registry.register(body.into_inner()) {
        Ok(server) => model::common::Result::<McpServer>::http_success(server),
        Err(e) => model::common::Result::<String>::http_response(400, 400, e, String::new()),
    }
}

/// Update an existing MCP server
#[put("/{namespace}/{name}")]
async fn update_server(
    req: HttpRequest,
    data: web::Data<AppState>,
    registry: web::Data<Arc<McpServerRegistry>>,
    path: web::Path<(String, String)>,
    body: web::Json<McpServerRegistration>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/mcp")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let (namespace, name) = path.into_inner();
    match registry.update(&namespace, &name, body.into_inner()) {
        Ok(server) => model::common::Result::<McpServer>::http_success(server),
        Err(e) => model::common::Result::<String>::http_response(404, 404, e, String::new()),
    }
}

/// Delete an MCP server
#[delete("/{namespace}/{name}")]
async fn delete_server(
    req: HttpRequest,
    data: web::Data<AppState>,
    registry: web::Data<Arc<McpServerRegistry>>,
    path: web::Path<(String, String)>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/mcp")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let (namespace, name) = path.into_inner();
    match registry.deregister(&namespace, &name) {
        Ok(()) => model::common::Result::<bool>::http_success(true),
        Err(e) => model::common::Result::<String>::http_response(404, 404, e, String::new()),
    }
}

/// Import MCP servers from JSON config
#[post("/import")]
async fn import_servers(
    req: HttpRequest,
    data: web::Data<AppState>,
    registry: web::Data<Arc<McpServerRegistry>>,
    body: web::Json<McpServerImportRequest>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/mcp")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let result = registry.import(body.into_inner());
    model::common::Result::<BatchRegistrationResponse>::http_success(result)
}

/// Get MCP registry statistics
#[get("/stats")]
async fn get_stats(
    req: HttpRequest,
    data: web::Data<AppState>,
    registry: web::Data<Arc<McpServerRegistry>>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/mcp")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let stats = registry.stats();
    model::common::Result::<McpRegistryStats>::http_success(stats)
}

pub fn routes() -> Scope {
    web::scope("/ai/mcp/servers")
        .service(get_stats)
        .service(import_servers)
        .service(list_servers)
        .service(get_server)
        .service(register_server)
        .service(update_server)
        .service(delete_server)
}
