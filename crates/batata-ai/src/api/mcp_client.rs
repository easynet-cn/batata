//! MCP Client HTTP API handlers -- Nacos 3.x compatible (F-NAC-CLIENT-MCP).
//!
//! Client: `/v3/client/ai/mcp` (6 endpoints, `@Since("3.3.0")`)
//! - GET    /        - Query one exact or latest serving MCP Version
//! - POST   /        - Release one MCP Version directly online or as a draft
//! - POST   /endpoints          - Register one MCP Runtime Endpoint
//! - DELETE /endpoints          - Deregister one MCP Runtime Endpoint
//! - PUT    /endpoints/heartbeat - Refresh the shared HTTP Client liveness
//! - GET    /search  - Search visible current MCP servers
//!
//! This mirrors [`crate::api::agent_client`]: `query`/`release`/`search` reuse the
//! real MCP domain storage ([`McpServerService`]), while the endpoint family is
//! backed by [`AiEndpointService`], which registers/deregisters real NamingService
//! instances (the same backing Nacos uses for its HTTP-client runtime, where the
//! endpoints are Naming instances under the server's service ref).

use std::sync::Arc;

use actix_web::{HttpRequest, HttpResponse, Responder, Scope, delete, get, post, put, web};
use serde::{Deserialize, Serialize};

use batata_common::{ActionTypes, ApiType, DEFAULT_NAMESPACE_ID, McpServerService, SignType};
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::response::Result;
use batata_server_common::{Secured, secured};

use crate::api::mcp::parse_registration;
use crate::service::endpoint_service::AiEndpointService;

// ============================================================================
// Constants
// ============================================================================

/// HTTP header for client identity (Nacos: `ClientConstants.HTTP_CLIENT_ID_HEADER`).
///
/// Kept aligned with [`crate::api::agent_client`] so a single shared HTTP Client
/// can publish both Agent and MCP endpoints under the same header.
const CLIENT_ID_HEADER: &str = "clientId";

/// HTTP header for request module (Nacos: `HttpHeaderConsts.REQUEST_MODULE`).
const REQUEST_MODULE_HEADER: &str = "requestModule";

/// Default heartbeat interval in milliseconds (5 seconds).
const DEFAULT_HEARTBEAT_INTERVAL_MILLIS: i64 = 5_000;

/// Default unhealthy timeout in milliseconds (15 seconds).
const DEFAULT_UNHEALTHY_TIMEOUT_MILLIS: i64 = 15_000;

/// Default expire timeout in milliseconds (30 seconds).
const DEFAULT_EXPIRE_TIMEOUT_MILLIS: i64 = 30_000;

// ============================================================================
// Helpers
// ============================================================================

fn normalize_namespace(ns: &str) -> &str {
    if ns.is_empty() {
        DEFAULT_NAMESPACE_ID
    } else {
        ns
    }
}

fn get_header(req: &HttpRequest, name: &str) -> String {
    req.headers()
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_default()
}

fn content_type(req: &HttpRequest) -> String {
    use actix_web::http::header::CONTENT_TYPE;
    req.headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// Required parameter that is missing.
fn missing(field: &str) -> HttpResponse {
    Result::<()>::http_bad_request(
        &batata_common::error::PARAMETER_MISSING,
        format!("{field} is required"),
    )
}

/// Parameter that is present but not a value the domain accepts.
fn invalid(message: impl Into<String>) -> HttpResponse {
    Result::<()>::http_bad_request(&batata_common::error::PARAMETER_VALIDATE_ERROR, message)
}

// ============================================================================
// Request Models
// ============================================================================

/// GET / query parameters (Nacos `McpQueryForm`).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpClientQuery {
    #[serde(default)]
    namespace_id: Option<String>,
    /// MCP server name -- required upstream.
    mcp_name: Option<String>,
    /// Optional exact version; absent asks for the latest serving version.
    #[serde(default)]
    version: Option<String>,
}

/// POST / release query switches (Nacos `McpReleaseForm` extras).
#[derive(Deserialize)]
struct McpReleaseQuery {
    #[serde(default)]
    namespace_id: Option<String>,
    /// When true, create a lifecycle draft instead of a direct-online release.
    #[serde(default)]
    create_draft: bool,
}

/// GET /search query parameters (Nacos `McpSearchForm`).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpClientSearchQuery {
    #[serde(default)]
    namespace_id: Option<String>,
    #[serde(default)]
    mcp_name: Option<String>,
    #[serde(default)]
    page_no: Option<u32>,
    #[serde(default)]
    page_size: Option<u32>,
}

/// POST /endpoints and DELETE /endpoints body / query (Nacos `McpEndpointForm`).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpClientEndpoint {
    #[serde(default)]
    namespace_id: Option<String>,
    /// MCP server name -- required.
    mcp_name: String,
    /// Endpoint host/IP.
    address: String,
    /// Endpoint port.
    port: i32,
    /// MCP version the endpoint serves.
    #[serde(default)]
    version: Option<String>,
    /// Operation type (`register` / `deregister`); informational only here.
    #[serde(default)]
    #[allow(dead_code)]
    r#type: Option<String>,
}

/// HTTP publisher liveness info (Nacos `ClientLivenessInfo` compatible).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ClientLivenessInfo {
    heartbeat_interval_millis: i64,
    unhealthy_timeout_millis: i64,
    expire_timeout_millis: i64,
}

impl Default for ClientLivenessInfo {
    fn default() -> Self {
        Self {
            heartbeat_interval_millis: DEFAULT_HEARTBEAT_INTERVAL_MILLIS,
            unhealthy_timeout_millis: DEFAULT_UNHEALTHY_TIMEOUT_MILLIS,
            expire_timeout_millis: DEFAULT_EXPIRE_TIMEOUT_MILLIS,
        }
    }
}

impl McpClientEndpoint {
    /// Validate the endpoint address/port, returning an error response on failure.
    fn validate(&self) -> std::result::Result<(String, u16), HttpResponse> {
        if self.mcp_name.trim().is_empty() {
            return Err(missing("mcpName"));
        }
        if self.address.trim().is_empty() {
            return Err(missing("address"));
        }
        if !(1..=65535).contains(&self.port) {
            return Err(invalid(format!(
                "parameter `port` should be in the range 1 ~ 65535, got {}",
                self.port
            )));
        }
        Ok((self.address.trim().to_string(), self.port as u16))
    }
}

// ============================================================================
// Handlers
// ============================================================================

/// GET /v3/client/ai/mcp -- Query one exact or latest serving MCP Version.
#[get("")]
async fn query_server(
    req: HttpRequest,
    data: web::Data<AppState>,
    mcp_service: web::Data<Arc<dyn McpServerService>>,
    query: web::Query<McpClientQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Ai)
            .api_type(ApiType::OpenApi)
            .build()
    );

    let ns = normalize_namespace(query.namespace_id.as_deref().unwrap_or(""));
    let Some(mcp_name) = query.mcp_name.as_deref().filter(|n| !n.is_empty()) else {
        return missing("mcpName");
    };
    match mcp_service
        .get_mcp_server_detail(ns, None, Some(mcp_name), query.version.as_deref(), None)
        .await
    {
        Ok(Some(server)) => HttpResponse::Ok().json(Result::success(server)),
        Ok(None) => Result::<()>::http_not_found(
            &batata_common::error::RESOURCE_NOT_FOUND,
            "MCP server not found",
        ),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// POST /v3/client/ai/mcp -- Release one MCP Version directly online or as a draft.
#[post("")]
async fn release(
    req: HttpRequest,
    data: web::Data<AppState>,
    mcp_service: web::Data<Arc<dyn McpServerService>>,
    query: web::Query<McpReleaseQuery>,
    body: web::Bytes,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::OpenApi)
            .build()
    );

    let ns = normalize_namespace(query.namespace_id.as_deref().unwrap_or(""));
    let registration = match parse_registration(&content_type(&req), &body) {
        Ok(registration) => registration,
        Err(e) => {
            return Result::<String>::http_bad_request(
                &batata_common::error::PARAMETER_VALIDATE_ERROR,
                e,
            );
        }
    };

    if query.create_draft {
        // Upstream: create a standard lifecycle draft (`/draft`) and return the
        // server's stable id. Batata's draft call returns the version detail,
        // which carries the name/version identity of the new draft.
        return match mcp_service
            .create_mcp_server_draft(ns, &registration, true)
            .await
        {
            Ok(detail) => HttpResponse::Ok().json(Result::success(detail)),
            Err(e) => Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_VALIDATE_ERROR,
                e.to_string(),
            ),
        };
    }

    // Direct-online release: create a new version. (Upstream additionally
    // updates an existing version when the version id already exists; that
    // overwrite path is not yet handled here and returns the create error.)
    match mcp_service.create_mcp_server(ns, &registration).await {
        Ok(id) => HttpResponse::Ok().json(Result::success(id)),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// POST /v3/client/ai/mcp/endpoints -- Register one MCP Runtime Endpoint.
#[post("endpoints")]
async fn register_endpoint(
    req: HttpRequest,
    data: web::Data<AppState>,
    endpoint_service: web::Data<Arc<AiEndpointService>>,
    body: web::Bytes,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::OpenApi)
            .build()
    );

    let client_id = get_header(&req, CLIENT_ID_HEADER);
    let _request_module = get_header(&req, REQUEST_MODULE_HEADER);
    if client_id.trim().is_empty() {
        return missing("clientId");
    }

    let form = match parse_endpoint(&content_type(&req), &body) {
        Ok(form) => form,
        Err(e) => return invalid(e),
    };
    let ns = normalize_namespace(form.namespace_id.as_deref().unwrap_or(""));
    let (address, port) = match form.validate() {
        Ok(pair) => pair,
        Err(response) => return response,
    };
    let version = form.version.as_deref().unwrap_or("").to_string();

    endpoint_service.create_mcp_endpoint(ns, &form.mcp_name, &version, &address, port);

    tracing::info!(
        namespace = %ns,
        mcp_name = %form.mcp_name,
        version = %version,
        address = %address,
        port = port,
        "MCP endpoint registered via client API"
    );

    HttpResponse::Ok().json(Result::success(ClientLivenessInfo::default()))
}

/// DELETE /v3/client/ai/mcp/endpoints -- Deregister one MCP Runtime Endpoint.
#[delete("endpoints")]
async fn deregister_endpoint(
    req: HttpRequest,
    data: web::Data<AppState>,
    endpoint_service: web::Data<Arc<AiEndpointService>>,
    query: web::Query<McpClientEndpoint>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::OpenApi)
            .build()
    );

    let client_id = get_header(&req, CLIENT_ID_HEADER);
    if client_id.trim().is_empty() {
        return missing("clientId");
    }

    let form = query.into_inner();
    let ns = normalize_namespace(form.namespace_id.as_deref().unwrap_or(""));
    let (address, port) = match form.validate() {
        Ok(pair) => pair,
        Err(response) => return response,
    };
    let version = form.version.as_deref().unwrap_or("").to_string();

    endpoint_service.delete_mcp_endpoint(ns, &form.mcp_name, &version, &address, port);

    tracing::info!(
        namespace = %ns,
        mcp_name = %form.mcp_name,
        version = %version,
        address = %address,
        port = port,
        "MCP endpoint deregistered via client API"
    );

    HttpResponse::Ok().json(Result::success(()))
}

/// PUT /v3/client/ai/mcp/endpoints/heartbeat -- Refresh shared HTTP Client liveness.
#[put("endpoints/heartbeat")]
async fn heartbeat(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::OpenApi)
            .build()
    );

    let client_id = get_header(&req, CLIENT_ID_HEADER);
    let _request_module = get_header(&req, REQUEST_MODULE_HEADER);

    tracing::debug!(
        client_id = %client_id,
        "MCP client heartbeat received"
    );

    HttpResponse::Ok().json(Result::success(ClientLivenessInfo::default()))
}

/// GET /v3/client/ai/mcp/search -- Search visible current MCP servers.
#[get("search")]
async fn search(
    req: HttpRequest,
    data: web::Data<AppState>,
    mcp_service: web::Data<Arc<dyn McpServerService>>,
    query: web::Query<McpClientSearchQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Ai)
            .api_type(ApiType::OpenApi)
            .build()
    );

    let ns = normalize_namespace(query.namespace_id.as_deref().unwrap_or(""));
    let page = mcp_service
        .list_mcp_servers(
            ns,
            query.mcp_name.as_deref(),
            "accurate",
            query.page_no.unwrap_or(1),
            query.page_size.unwrap_or(20),
            None,
        )
        .await;
    HttpResponse::Ok().json(Result::success(page))
}

// ============================================================================
// Endpoint form parsing
// ============================================================================

/// Parse an endpoint form from a form body or a JSON body.
fn parse_endpoint(
    content_type: &str,
    body: &[u8],
) -> std::result::Result<McpClientEndpoint, String> {
    let body = body.trim_ascii();
    if body.is_empty() {
        return Err("an endpoint registration is required".to_string());
    }
    if content_type.contains("json") || body.first() == Some(&b'{') {
        return serde_json::from_slice::<McpClientEndpoint>(body)
            .map_err(|e| format!("Invalid endpoint registration: {e}"));
    }
    serde_urlencoded::from_bytes::<McpClientEndpoint>(body)
        .map_err(|e| format!("Invalid form body: {e}"))
}

// ============================================================================
// Route configuration
// ============================================================================

/// Configure MCP client routes at `/v3/client/ai/mcp`.
///
/// Registers 6 endpoints matching Nacos `McpClientController`:
/// - `GET    /`                   -> query
/// - `POST   /`                   -> release
/// - `POST   /endpoints`          -> register_endpoint
/// - `DELETE /endpoints`          -> deregister_endpoint
/// - `PUT    /endpoints/heartbeat` -> heartbeat
/// - `GET    /search`             -> search
pub fn client_routes() -> Scope {
    web::scope("/mcp")
        .service(query_server)
        .service(release)
        .service(register_endpoint)
        .service(deregister_endpoint)
        .service(heartbeat)
        .service(search)
}
