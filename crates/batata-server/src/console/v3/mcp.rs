// Console MCP server management API endpoints
// Aligned with Nacos V3 Console API contract

use std::collections::HashMap;
use std::sync::Arc;

use actix_web::{HttpMessage, HttpRequest, Responder, Scope, delete, get, post, put, web};

use crate::{
    ActionTypes, ApiType, Secured, SignType,
    api::ai::model::{
        BatchRegistrationResponse, McpDeleteQuery, McpDetailQuery, McpImportExecuteRequest,
        McpImportValidateRequest, McpImportValidateResponse, McpListQuery, McpServer,
        McpServerConfig, McpServerImportRequest, McpServerListResponse, McpServerRegistration,
    },
    api::ai::{McpRegistryStats, McpServerRegistry},
    error,
    model::{self, common::AppState},
    secured,
};

/// List MCP servers
/// GET /v3/console/ai/mcp/list
#[get("/list")]
async fn list_servers(
    req: HttpRequest,
    data: web::Data<AppState>,
    registry: web::Data<Arc<McpServerRegistry>>,
    params: web::Query<McpListQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/mcp")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let result = registry.list_with_search(&params.into_inner());
    model::common::Result::<McpServerListResponse>::http_success(result)
}

/// Get MCP server by query params
/// GET /v3/console/ai/mcp?namespaceId=xxx&mcpName=xxx
#[get("")]
async fn get_server(
    req: HttpRequest,
    data: web::Data<AppState>,
    registry: web::Data<Arc<McpServerRegistry>>,
    query: web::Query<McpDetailQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/mcp")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    match registry.get_by_query(&query.into_inner()) {
        Some(server) => model::common::Result::<McpServer>::http_success(server),
        None => model::common::Result::<String>::http_response(
            404,
            404,
            "MCP server not found".to_string(),
            String::new(),
        ),
    }
}

/// Register a new MCP server
/// POST /v3/console/ai/mcp
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
        Err(e) => model::common::Result::<String>::http_response(
            400,
            error::PARAMETER_VALIDATE_ERROR.code,
            e,
            String::new(),
        ),
    }
}

/// Update an existing MCP server
/// PUT /v3/console/ai/mcp?namespaceId=xxx&mcpName=xxx
#[put("")]
async fn update_server(
    req: HttpRequest,
    data: web::Data<AppState>,
    registry: web::Data<Arc<McpServerRegistry>>,
    query: web::Query<McpDetailQuery>,
    body: web::Json<McpServerRegistration>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/mcp")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let q = query.into_inner();
    let namespace = q.namespace_id.as_deref().unwrap_or("public").to_string();
    let fallback_name = body.name.clone();
    let name = q.mcp_name.unwrap_or(fallback_name);

    match registry.update(&namespace, &name, body.into_inner()) {
        Ok(server) => model::common::Result::<McpServer>::http_success(server),
        Err(e) => model::common::Result::<String>::http_response(
            404,
            error::MCP_SERVER_NOT_FOUND.code,
            e,
            String::new(),
        ),
    }
}

/// Delete an MCP server
/// DELETE /v3/console/ai/mcp?namespaceId=xxx&mcpName=xxx
#[delete("")]
async fn delete_server(
    req: HttpRequest,
    data: web::Data<AppState>,
    registry: web::Data<Arc<McpServerRegistry>>,
    query: web::Query<McpDeleteQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/mcp")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    match registry.delete_by_query(&query.into_inner()) {
        Ok(()) => model::common::Result::<bool>::http_success(true),
        Err(e) => model::common::Result::<String>::http_response(
            404,
            error::MCP_SERVER_NOT_FOUND.code,
            e,
            String::new(),
        ),
    }
}

/// Import tools from a running MCP server
/// GET /v3/console/ai/mcp/importToolsFromMcp
#[get("/importToolsFromMcp")]
async fn import_tools_from_mcp(
    req: HttpRequest,
    data: web::Data<AppState>,
    _query: web::Query<crate::api::ai::model::ImportToolsQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/mcp")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    // Stub: returns empty tools list
    let tools: Vec<crate::api::ai::model::McpTool> = vec![];
    model::common::Result::<Vec<crate::api::ai::model::McpTool>>::http_success(tools)
}

/// Validate MCP import content
/// POST /v3/console/ai/mcp/import/validate
#[post("/import/validate")]
async fn import_validate(
    req: HttpRequest,
    data: web::Data<AppState>,
    body: web::Json<McpImportValidateRequest>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/mcp")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let content = &body.content;
    match serde_json::from_str::<HashMap<String, McpServerConfig>>(content) {
        Ok(servers) => {
            let response = McpImportValidateResponse {
                valid: true,
                message: String::new(),
                server_count: servers.len() as u32,
            };
            model::common::Result::<McpImportValidateResponse>::http_success(response)
        }
        Err(e) => {
            let response = McpImportValidateResponse {
                valid: false,
                message: format!("Invalid JSON: {}", e),
                server_count: 0,
            };
            model::common::Result::<McpImportValidateResponse>::http_success(response)
        }
    }
}

/// Execute MCP import
/// POST /v3/console/ai/mcp/import/execute
#[post("/import/execute")]
async fn import_execute(
    req: HttpRequest,
    data: web::Data<AppState>,
    registry: web::Data<Arc<McpServerRegistry>>,
    body: web::Json<McpImportExecuteRequest>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/mcp")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let exec = body.into_inner();
    match serde_json::from_str::<HashMap<String, McpServerConfig>>(&exec.content) {
        Ok(servers) => {
            let import_request = McpServerImportRequest {
                mcp_servers: servers,
                namespace: exec.namespace,
                overwrite: exec.overwrite,
            };
            let result = registry.import(import_request);
            model::common::Result::<BatchRegistrationResponse>::http_success(result)
        }
        Err(e) => model::common::Result::<String>::http_response(
            400,
            400,
            format!("Invalid JSON: {}", e),
            String::new(),
        ),
    }
}

/// Import MCP servers from JSON config (Batata extension)
/// POST /v3/console/ai/mcp/import
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
/// GET /v3/console/ai/mcp/stats
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
    web::scope("/ai/mcp")
        .service(get_stats)
        .service(import_tools_from_mcp)
        .service(import_validate)
        .service(import_execute)
        .service(import_servers)
        .service(list_servers)
        .service(register_server)
        .service(update_server)
        .service(get_server)
        .service(delete_server)
}
