// Console MCP server management API endpoints
// Aligned with Batata V3 Console API contract
// Uses Arc<dyn McpServerService> trait object (wired in batata-server)

use std::collections::HashMap;
use std::sync::Arc;

use actix_web::{HttpRequest, Responder, Scope, delete, get, post, put, web};

use batata_common::McpServerService;
use batata_common::model::Page;
use batata_common::model::ai::a2a::BatchRegistrationResponse;
use batata_common::model::ai::mcp::{
    ImportToolsQuery, McpDeleteQuery, McpDetailQuery, McpImportExecuteRequest,
    McpImportValidateRequest, McpImportValidateResponse, McpListQuery, McpRegistryStats, McpServer,
    McpServerBasicInfo, McpServerConfig, McpServerImportRequest, McpServerRegistration, McpTool,
};
use batata_server_common::error;
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::response as common_response;
use batata_server_common::secured::Secured;
use batata_server_common::{ActionTypes, ApiType, SignType, secured};

/// List MCP servers
/// GET /v3/console/ai/mcp/list
#[get("/list")]
async fn list_servers(
    req: HttpRequest,
    data: web::Data<AppState>,
    svc: web::Data<Arc<dyn McpServerService>>,
    params: web::Query<McpListQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/mcp")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let q = params.into_inner();
    let namespace = q.namespace_id.as_deref().unwrap_or("public");
    let search_type = q.search.as_deref().unwrap_or("blur");
    let page_no = q.page_no.unwrap_or(1);
    let page_size = q.page_size.unwrap_or(20);
    let result = svc.list_mcp_servers(
        namespace,
        q.mcp_name.as_deref(),
        search_type,
        page_no,
        page_size,
    );
    common_response::Result::<Page<McpServerBasicInfo>>::http_success(result)
}

/// Get MCP server by query params
/// GET /v3/console/ai/mcp?namespaceId=xxx&mcpName=xxx
#[get("")]
async fn get_server(
    req: HttpRequest,
    data: web::Data<AppState>,
    svc: web::Data<Arc<dyn McpServerService>>,
    query: web::Query<McpDetailQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/mcp")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let q = query.into_inner();
    let namespace = q.namespace_id.as_deref().unwrap_or("public");
    match svc
        .get_mcp_server_detail(
            namespace,
            q.mcp_id.as_deref(),
            q.mcp_name.as_deref(),
            q.version.as_deref(),
        )
        .await
    {
        Ok(Some(server)) => common_response::Result::<McpServer>::http_success(server),
        Ok(None) => common_response::Result::<String>::http_response(
            404,
            error::MCP_SERVER_NOT_FOUND.code,
            "MCP server not found".to_string(),
            String::new(),
        ),
        Err(e) => common_response::Result::<String>::http_response(
            500,
            error::SERVER_ERROR.code,
            e.to_string(),
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
    svc: web::Data<Arc<dyn McpServerService>>,
    body: web::Json<McpServerRegistration>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/mcp")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let reg = body.into_inner();
    let namespace = reg.namespace.clone();
    match svc.create_mcp_server(&namespace, &reg).await {
        Ok(id) => match svc
            .get_mcp_server_detail(&namespace, Some(&id), None, None)
            .await
        {
            Ok(Some(server)) => common_response::Result::<McpServer>::http_success(server),
            _ => common_response::Result::<String>::http_success(id),
        },
        Err(e) => common_response::Result::<String>::http_response(
            400,
            error::PARAMETER_VALIDATE_ERROR.code,
            e.to_string(),
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
    svc: web::Data<Arc<dyn McpServerService>>,
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
    let mut reg = body.into_inner();
    if let Some(ref name) = q.mcp_name {
        reg.name = name.clone();
    }
    reg.namespace = namespace.clone();

    match svc.update_mcp_server(&namespace, &reg).await {
        Ok(()) => match svc
            .get_mcp_server_detail(&namespace, None, Some(&reg.name), None)
            .await
        {
            Ok(Some(server)) => common_response::Result::<McpServer>::http_success(server),
            _ => common_response::Result::<bool>::http_success(true),
        },
        Err(e) => common_response::Result::<String>::http_response(
            404,
            error::MCP_SERVER_NOT_FOUND.code,
            e.to_string(),
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
    svc: web::Data<Arc<dyn McpServerService>>,
    query: web::Query<McpDeleteQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/mcp")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let q = query.into_inner();
    let namespace = q.namespace_id.as_deref().unwrap_or("public");
    match svc
        .delete_mcp_server(
            namespace,
            q.mcp_name.as_deref(),
            q.mcp_id.as_deref(),
            q.version.as_deref(),
        )
        .await
    {
        Ok(()) => common_response::Result::<bool>::http_success(true),
        Err(e) => common_response::Result::<String>::http_response(
            404,
            error::MCP_SERVER_NOT_FOUND.code,
            e.to_string(),
            String::new(),
        ),
    }
}

/// Import tools from a running MCP server via SSE transport.
/// GET /v3/console/ai/mcp/importToolsFromMcp
#[get("/importToolsFromMcp")]
async fn import_tools_from_mcp(
    req: HttpRequest,
    data: web::Data<AppState>,
    svc: web::Data<Arc<dyn McpServerService>>,
    query: web::Query<ImportToolsQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/mcp")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let q = query.into_inner();
    if q.transport_type != "mcp-sse" {
        return common_response::Result::<String>::http_response(
            500,
            error::SERVER_ERROR.code,
            format!("Unsupported transport type: {}", q.transport_type),
            String::new(),
        );
    }

    let timeout = std::time::Duration::from_secs(10);
    match svc
        .import_tools_from_mcp(&q.base_url, &q.endpoint, q.auth_token.as_deref(), timeout)
        .await
    {
        Ok(tools) => common_response::Result::<Vec<McpTool>>::http_success(tools),
        Err(e) => common_response::Result::<String>::http_response(
            500,
            error::SERVER_ERROR.code,
            format!("Failed to import tools from MCP server: {}", e),
            String::new(),
        ),
    }
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
            common_response::Result::<McpImportValidateResponse>::http_success(response)
        }
        Err(e) => {
            let response = McpImportValidateResponse {
                valid: false,
                message: format!("Invalid JSON: {}", e),
                server_count: 0,
            };
            common_response::Result::<McpImportValidateResponse>::http_success(response)
        }
    }
}

/// Execute MCP import
/// POST /v3/console/ai/mcp/import/execute
#[post("/import/execute")]
async fn import_execute(
    req: HttpRequest,
    data: web::Data<AppState>,
    svc: web::Data<Arc<dyn McpServerService>>,
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
            match svc.import_mcp_servers(import_request).await {
                Ok(result) => {
                    common_response::Result::<BatchRegistrationResponse>::http_success(result)
                }
                Err(e) => common_response::Result::<String>::http_response(
                    500,
                    error::SERVER_ERROR.code,
                    e.to_string(),
                    String::new(),
                ),
            }
        }
        Err(e) => common_response::Result::<String>::http_response(
            400,
            error::PARAMETER_VALIDATE_ERROR.code,
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
    svc: web::Data<Arc<dyn McpServerService>>,
    body: web::Json<McpServerImportRequest>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/mcp")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    match svc.import_mcp_servers(body.into_inner()).await {
        Ok(result) => common_response::Result::<BatchRegistrationResponse>::http_success(result),
        Err(e) => common_response::Result::<String>::http_response(
            500,
            error::SERVER_ERROR.code,
            e.to_string(),
            String::new(),
        ),
    }
}

/// Get MCP registry statistics
/// GET /v3/console/ai/mcp/stats
#[get("/stats")]
async fn get_stats(
    req: HttpRequest,
    data: web::Data<AppState>,
    svc: web::Data<Arc<dyn McpServerService>>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/mcp")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    match svc.mcp_stats().await {
        Ok(stats) => common_response::Result::<McpRegistryStats>::http_success(stats),
        Err(e) => common_response::Result::<String>::http_response(
            500,
            error::SERVER_ERROR.code,
            e.to_string(),
            String::new(),
        ),
    }
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
