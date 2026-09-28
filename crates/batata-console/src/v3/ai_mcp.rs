//! Console MCP server management API endpoints.
//!
//! Aligned with Batata V3 Console API contract.
// Uses Arc<dyn McpServerService> trait object (wired in batata-server)

use std::collections::HashMap;
use std::sync::Arc;

use actix_web::{HttpRequest, HttpResponse, Responder, Scope, delete, get, post, put, web};

use batata_common::McpServerService;
use batata_common::model::Page;
use batata_common::model::ai::mcp::{
    ImportToolsQuery, McpDeleteQuery, McpDetailQuery, McpDraftCreateQuery,
    McpImportValidateRequest, McpImportValidateResponse, McpLabelsRequest, McpListQuery,
    McpRegistryStats, McpScopeQuery, McpSearchQuery, McpServer, McpServerBasicInfo, McpServerConfig,
    McpServerRegistration, McpServerVersionDetail, McpServerVersionSummary, McpStatusQuery, McpTool,
    McpVersionsQuery,
};
use super::ai_trace::{self, get_username, trace_write};
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
    let user = get_username(&req);
    let result = svc
        .list_mcp_servers(
            namespace,
            q.mcp_name.as_deref(),
            search_type,
            page_no,
            page_size,
            Some(&user),
        )
        .await;
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
    let user = get_username(&req);
    match svc
        .get_mcp_server_detail(
            namespace,
            q.mcp_id.as_deref(),
            q.mcp_name.as_deref(),
            q.version.as_deref(),
            Some(&user),
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

/// List the versions of one MCP server
/// GET /v3/console/ai/mcp/versions?namespaceId=xxx&mcpName=xxx
#[get("/versions")]
async fn list_versions(
    req: HttpRequest,
    data: web::Data<AppState>,
    svc: web::Data<Arc<dyn McpServerService>>,
    params: web::Query<McpVersionsQuery>,
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
    let name = match q.mcp_name.as_deref() {
        Some(n) => n,
        None => {
            return common_response::Result::<String>::http_response(
                400,
                error::PARAMETER_MISSING.code,
                "mcpName is required".to_string(),
                String::new(),
            );
        }
    };
    let page_no = q.page_no.unwrap_or(1);
    let page_size = q.page_size.unwrap_or(20);

    match svc
        .list_mcp_server_versions(namespace, name, page_no, page_size)
        .await
    {
        Ok(page) => common_response::Result::<Page<McpServerVersionSummary>>::http_success(page),
        Err(e) => common_response::Result::<String>::http_response(
            500,
            error::SERVER_ERROR.code,
            e.to_string(),
            String::new(),
        ),
    }
}

/// Get one version of an MCP server
/// GET /v3/console/ai/mcp/version?namespaceId=xxx&mcpName=xxx&version=xxx
#[get("/version")]
async fn get_version(
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
    let (name, version) = match (q.mcp_name.as_deref(), q.version.as_deref()) {
        (Some(n), Some(v)) => (n, v),
        _ => {
            return common_response::Result::<String>::http_response(
                400,
                error::PARAMETER_MISSING.code,
                "mcpName and version are required".to_string(),
                String::new(),
            );
        }
    };

    match svc.get_mcp_server_version(namespace, name, version).await {
        Ok(Some(detail)) => common_response::Result::<McpServerVersionDetail>::http_success(detail),
        Ok(None) => common_response::Result::<String>::http_response(
            404,
            error::MCP_SERVER_NOT_FOUND.code,
            "MCP server version not found".to_string(),
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
    let user = get_username(&req);
    match svc.create_mcp_server(&namespace, &reg).await {
        Ok(id) => match svc
            .get_mcp_server_detail(&namespace, Some(&id), None, None, Some(&user))
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

    let user = get_username(&req);
    match svc.update_mcp_server(&namespace, &reg).await {
        Ok(()) => match svc
            .get_mcp_server_detail(&namespace, None, Some(&reg.name), None, Some(&user))
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

/// Build a 400 response for a missing required parameter.
fn bad_request(message: &str) -> HttpResponse {
    common_response::Result::<String>::http_response(
        400,
        error::PARAMETER_MISSING.code,
        message.to_string(),
        String::new(),
    )
}

/// Build a 500 response from a service error.
fn server_error(message: String) -> HttpResponse {
    common_response::Result::<String>::http_response(
        500,
        error::SERVER_ERROR.code,
        message,
        String::new(),
    )
}

/// Render the outcome of a lifecycle operation, tracing it first.
fn version_response(
    req: &HttpRequest,
    operation: &str,
    name: &str,
    version: &str,
    result: anyhow::Result<McpServerVersionDetail>,
) -> HttpResponse {
    trace_write(
        req,
        batata_common::ai_trace::RESOURCE_TYPE_MCP,
        operation,
        Some(name),
        Some(version),
        ai_trace::outcome_of(&result),
    );

    match result {
        Ok(detail) => common_response::Result::<McpServerVersionDetail>::http_success(detail),
        Err(e) => server_error(e.to_string()),
    }
}

/// Extract `(namespace, name, version)` from a version-action query.
fn version_target(q: &McpDetailQuery) -> Option<(&str, &str, &str)> {
    let namespace = q.namespace_id.as_deref().unwrap_or("public");
    match (q.mcp_name.as_deref(), q.version.as_deref()) {
        (Some(name), Some(version)) => Some((namespace, name, version)),
        _ => None,
    }
}

/// Keyword search over the MCP search index.
///
/// GET /v3/console/ai/mcp/search?namespaceId=xxx&query=xxx
///
/// Upstream exposes an equivalent client search at
/// `/v3/client/ai/resources/search`; this console variant is scoped to MCP and
/// uses numbered pages rather than cursors.
#[get("/search")]
async fn search_servers(
    req: HttpRequest,
    data: web::Data<AppState>,
    svc: web::Data<Arc<dyn McpServerService>>,
    params: web::Query<McpSearchQuery>,
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
    let query = match q.query.as_deref().map(str::trim) {
        Some(t) if !t.is_empty() => t,
        _ => return bad_request("query is required"),
    };
    if query.len() > batata_common::model::ai::search::MAX_QUERY_LENGTH {
        return bad_request(&format!(
            "query exceeds {} characters",
            batata_common::model::ai::search::MAX_QUERY_LENGTH
        ));
    }

    let page_no = q.page_no.unwrap_or(1).max(1);
    let page_size = q
        .page_size
        .unwrap_or(batata_common::model::ai::search::DEFAULT_PAGE_SIZE)
        .clamp(1, batata_common::model::ai::search::MAX_PAGE_SIZE);

    match svc
        .search_mcp_servers(namespace, query, page_no, page_size)
        .await
    {
        Ok(page) => common_response::Result::<
            batata_common::model::Page<batata_common::model::ai::search::AiResourceSearchHit>,
        >::http_success(page),
        Err(e) => server_error(e.to_string()),
    }
}

/// Create a draft version of an MCP server
/// POST /v3/console/ai/mcp/draft
#[post("/draft")]
async fn create_draft(
    req: HttpRequest,
    data: web::Data<AppState>,
    svc: web::Data<Arc<dyn McpServerService>>,
    params: web::Query<McpDraftCreateQuery>,
    body: web::Json<McpServerRegistration>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/mcp")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let q = params.into_inner();
    let reg = body.into_inner();
    let namespace = if reg.namespace.is_empty() {
        "public"
    } else {
        reg.namespace.as_str()
    };
    version_response(
        &req,
        batata_common::ai_trace::OP_CREATE_DRAFT,
        &reg.name.clone(),
        &reg.version.clone(),
        svc.create_mcp_server_draft(namespace, &reg, q.overwrite.unwrap_or(false))
            .await,
    )
}

/// Update a draft version of an MCP server
/// PUT /v3/console/ai/mcp/draft
#[put("/draft")]
async fn update_draft(
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
    let namespace = if reg.namespace.is_empty() {
        "public"
    } else {
        reg.namespace.as_str()
    };
    version_response(
        &req,
        batata_common::ai_trace::OP_UPDATE_DRAFT,
        &reg.name.clone(),
        &reg.version.clone(),
        svc.update_mcp_server_draft(namespace, &reg).await,
    )
}

/// Delete a draft version
/// DELETE /v3/console/ai/mcp/draft?namespaceId=xxx&mcpName=xxx&version=xxx
#[delete("/draft")]
async fn delete_draft(
    req: HttpRequest,
    data: web::Data<AppState>,
    svc: web::Data<Arc<dyn McpServerService>>,
    query: web::Query<McpDetailQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/mcp")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let q = query.into_inner();
    let (namespace, name, version) = match version_target(&q) {
        Some(target) => target,
        None => return bad_request("mcpName and version are required"),
    };
    let result = svc.delete_mcp_server_draft(namespace, name, version).await;
    trace_write(
        &req,
        batata_common::ai_trace::RESOURCE_TYPE_MCP,
        batata_common::ai_trace::OP_DELETE_DRAFT,
        Some(name),
        Some(version),
        ai_trace::outcome_of(&result),
    );
    match result {
        Ok(()) => common_response::Result::<String>::http_success("ok".to_string()),
        Err(e) => server_error(e.to_string()),
    }
}

/// Submit a draft version for review
/// POST /v3/console/ai/mcp/submit?namespaceId=xxx&mcpName=xxx&version=xxx
#[post("/submit")]
async fn submit_version(
    req: HttpRequest,
    data: web::Data<AppState>,
    svc: web::Data<Arc<dyn McpServerService>>,
    query: web::Query<McpDetailQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/mcp")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let q = query.into_inner();
    let (namespace, name, version) = match version_target(&q) {
        Some(target) => target,
        None => return bad_request("mcpName and version are required"),
    };
    version_response(
        &req,
        batata_common::ai_trace::OP_SUBMIT_REVIEW,
        name,
        version,
        svc.submit_mcp_server_version(namespace, name, version).await,
    )
}

/// Publish a reviewed version
/// POST /v3/console/ai/mcp/publish?namespaceId=xxx&mcpName=xxx&version=xxx
#[post("/publish")]
async fn publish_version(
    req: HttpRequest,
    data: web::Data<AppState>,
    svc: web::Data<Arc<dyn McpServerService>>,
    query: web::Query<McpDetailQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/mcp")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let q = query.into_inner();
    let (namespace, name, version) = match version_target(&q) {
        Some(target) => target,
        None => return bad_request("mcpName and version are required"),
    };
    version_response(
        &req,
        batata_common::ai_trace::OP_PUBLISH,
        name,
        version,
        svc.publish_mcp_server_version(namespace, name, version).await,
    )
}

/// Publish a version bypassing the review gate
/// POST /v3/console/ai/mcp/force-publish?namespaceId=xxx&mcpName=xxx&version=xxx
#[post("/force-publish")]
async fn force_publish_version(
    req: HttpRequest,
    data: web::Data<AppState>,
    svc: web::Data<Arc<dyn McpServerService>>,
    query: web::Query<McpDetailQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/mcp")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let q = query.into_inner();
    let (namespace, name, version) = match version_target(&q) {
        Some(target) => target,
        None => return bad_request("mcpName and version are required"),
    };
    version_response(
        &req,
        batata_common::ai_trace::OP_FORCE_PUBLISH,
        name,
        version,
        svc.force_publish_mcp_server_version(namespace, name, version)
            .await,
    )
}

/// Move a version back to draft
/// POST /v3/console/ai/mcp/redraft?namespaceId=xxx&mcpName=xxx&version=xxx
#[post("/redraft")]
async fn redraft_version(
    req: HttpRequest,
    data: web::Data<AppState>,
    svc: web::Data<Arc<dyn McpServerService>>,
    query: web::Query<McpDetailQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/mcp")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let q = query.into_inner();
    let (namespace, name, version) = match version_target(&q) {
        Some(target) => target,
        None => return bad_request("mcpName and version are required"),
    };
    version_response(
        &req,
        batata_common::ai_trace::OP_REDRAFT,
        name,
        version,
        svc.redraft_mcp_server_version(namespace, name, version).await,
    )
}

/// Bring an offline version online
/// POST /v3/console/ai/mcp/online?namespaceId=xxx&mcpName=xxx&version=xxx
#[post("/online")]
async fn online_version(
    req: HttpRequest,
    data: web::Data<AppState>,
    svc: web::Data<Arc<dyn McpServerService>>,
    query: web::Query<McpDetailQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/mcp")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let q = query.into_inner();
    let (namespace, name, version) = match version_target(&q) {
        Some(target) => target,
        None => return bad_request("mcpName and version are required"),
    };
    version_response(
        &req,
        batata_common::ai_trace::OP_ONLINE_VERSION,
        name,
        version,
        svc.online_mcp_server_version(namespace, name, version).await,
    )
}

/// Take an online version offline
/// POST /v3/console/ai/mcp/offline?namespaceId=xxx&mcpName=xxx&version=xxx
#[post("/offline")]
async fn offline_version(
    req: HttpRequest,
    data: web::Data<AppState>,
    svc: web::Data<Arc<dyn McpServerService>>,
    query: web::Query<McpDetailQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/mcp")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let q = query.into_inner();
    let (namespace, name, version) = match version_target(&q) {
        Some(target) => target,
        None => return bad_request("mcpName and version are required"),
    };
    version_response(
        &req,
        batata_common::ai_trace::OP_OFFLINE_VERSION,
        name,
        version,
        svc.offline_mcp_server_version(namespace, name, version).await,
    )
}

/// Replace the version labels
/// PUT /v3/console/ai/mcp/labels?namespaceId=xxx&mcpName=xxx
#[put("/labels")]
async fn update_labels(
    req: HttpRequest,
    data: web::Data<AppState>,
    svc: web::Data<Arc<dyn McpServerService>>,
    query: web::Query<McpDetailQuery>,
    body: web::Json<McpLabelsRequest>,
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
    let name = match q.mcp_name.as_deref() {
        Some(n) => n,
        None => return bad_request("mcpName is required"),
    };
    let result = svc
        .update_mcp_server_labels(namespace, name, body.into_inner().labels)
        .await;
    trace_write(
        &req,
        batata_common::ai_trace::RESOURCE_TYPE_MCP,
        batata_common::ai_trace::OP_UPDATE_LABELS,
        Some(name),
        None,
        ai_trace::outcome_of(&result),
    );
    match result {
        Ok(labels) => common_response::Result::<HashMap<String, String>>::http_success(labels),
        Err(e) => server_error(e.to_string()),
    }
}

/// Enable or disable a server
/// PUT /v3/console/ai/mcp/status?namespaceId=xxx&mcpName=xxx&status=enable|disable
#[put("/status")]
async fn update_status(
    req: HttpRequest,
    data: web::Data<AppState>,
    svc: web::Data<Arc<dyn McpServerService>>,
    query: web::Query<McpStatusQuery>,
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
    let name = match q.mcp_name.as_deref() {
        Some(n) => n,
        None => return bad_request("mcpName is required"),
    };
    let status = match q.status.as_deref() {
        Some(s) => s,
        None => return bad_request("status is required"),
    };
    let enabled = match status {
        "enable" => true,
        "disable" => false,
        other => return bad_request(&format!("status must be enable or disable, got '{other}'")),
    };
    let result = svc.update_mcp_server_status(namespace, name, enabled).await;
    let operation = if enabled {
        batata_common::ai_trace::OP_ENABLE
    } else {
        batata_common::ai_trace::OP_DISABLE
    };
    trace_write(
        &req,
        batata_common::ai_trace::RESOURCE_TYPE_MCP,
        operation,
        Some(name),
        None,
        ai_trace::outcome_of(&result),
    );
    match result {
        Ok(()) => common_response::Result::<String>::http_success("ok".to_string()),
        Err(e) => server_error(e.to_string()),
    }
}

/// Change the visibility scope
/// PUT /v3/console/ai/mcp/scope?namespaceId=xxx&mcpName=xxx&scope=PUBLIC|PRIVATE
#[put("/scope")]
async fn update_scope(
    req: HttpRequest,
    data: web::Data<AppState>,
    svc: web::Data<Arc<dyn McpServerService>>,
    query: web::Query<McpScopeQuery>,
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
    let name = match q.mcp_name.as_deref() {
        Some(n) => n,
        None => return bad_request("mcpName is required"),
    };
    let scope = match q.scope.as_deref() {
        Some(s) => s,
        None => return bad_request("scope is required"),
    };
    let result = svc.update_mcp_server_scope(namespace, name, scope).await;
    trace_write(
        &req,
        batata_common::ai_trace::RESOURCE_TYPE_MCP,
        batata_common::ai_trace::OP_UPDATE_SCOPE,
        Some(name),
        None,
        ai_trace::outcome_of(&result),
    );
    match result {
        Ok(()) => common_response::Result::<String>::http_success("ok".to_string()),
        Err(e) => server_error(e.to_string()),
    }
}

/// Register the MCP server management routes under `/ai/mcp`.
pub fn routes() -> Scope {
    web::scope("/ai/mcp")
        .service(get_stats)
        .service(import_tools_from_mcp)
        .service(import_validate)
        .service(list_servers)
        .service(register_server)
        .service(update_server)
        .service(get_server)
        .service(list_versions)
        .service(get_version)
        .service(search_servers)
        .service(create_draft)
        .service(update_draft)
        .service(delete_draft)
        .service(submit_version)
        .service(publish_version)
        .service(force_publish_version)
        .service(redraft_version)
        .service(online_version)
        .service(offline_version)
        .service(update_labels)
        .service(update_status)
        .service(update_scope)
        .service(delete_server)
}
