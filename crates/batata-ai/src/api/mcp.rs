//! MCP server admin HTTP API handlers — Nacos 3.x compatible.
//!
//! Admin: `/v3/admin/ai/mcp`
//!
//! Write endpoints keep the SDK's encoding: `serverSpecification`,
//! `toolSpecification`, `resourceSpecification` and `endpointSpecification`
//! arrive as JSON *strings* inside a form body — upstream `McpDetailForm` and
//! `McpServerDraftForm`, parsed by `McpRequestUtil`. The maintainer client
//! posts exactly that shape, so it is what these handlers have to accept.
//!
//! Upstream's Spring form binding takes a parameter from the query string or
//! the body alike, so a JSON body is accepted too where a client prefers it;
//! the content type decides how the body is read.
//!
//! The handlers below are backed by the `ai_resource` service, the same one the
//! console layer uses. The earlier admin implementation was backed by the
//! in-memory registry and exposed only a handful of endpoints.

use std::sync::Arc;

use actix_web::{HttpRequest, HttpResponse, Responder, Scope, delete, get, post, put, web};
use batata_common::McpServerService;
use batata_common::model::ai::mcp::{
    McpDeleteQuery, McpDetailQuery, McpDraftCreateQuery, McpLabelsRequest, McpListQuery,
    McpScopeQuery, McpServerRegistration, McpStatusQuery, McpVersionsQuery,
};
use batata_common::{ActionTypes, ApiType, SignType};
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::response::Result;
use batata_server_common::{Secured, secured};
use serde::Deserialize;

pub use crate::registry::mcp::configure;

/// Empty namespaces mean the default one.
fn normalize_namespace(ns: Option<&str>) -> &str {
    match ns {
        Some(ns) if !ns.is_empty() => ns,
        _ => batata_common::DEFAULT_NAMESPACE_ID,
    }
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

/// Whether a request asked to enable or disable a server.
///
/// Upstream's `McpServerStatusForm` carries a required `enabled` Boolean and
/// the maintainer client sends `enabled=false` to disable. The console talks in
/// `status=enable|disable`, so both spellings are understood; `enabled` wins
/// because it is the one with a defined meaning.
fn resolve_enabled(
    enabled: Option<bool>,
    status: Option<&str>,
) -> std::result::Result<bool, String> {
    if let Some(enabled) = enabled {
        return Ok(enabled);
    }
    match status.map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) if s.eq_ignore_ascii_case("enable") || s.eq_ignore_ascii_case("enabled") => {
            Ok(true)
        }
        Some(s) if s.eq_ignore_ascii_case("disable") || s.eq_ignore_ascii_case("disabled") => {
            Ok(false)
        }
        Some(s) => Err(format!("status must be enable or disable, got '{s}'")),
        None => Err("enabled is required".to_string()),
    }
}

/// The version statuses upstream accepts on `GET /versions`.
const VERSION_STATUSES: [&str; 5] = ["draft", "reviewing", "reviewed", "online", "offline"];

/// Normalize the optional status filter, rejecting anything else.
///
/// Upstream lower-cases the value and rejects an unknown one rather than
/// returning an empty page, which is the only way a caller learns it typo'd.
fn resolve_version_status(
    status: Option<&str>,
) -> std::result::Result<Option<String>, HttpResponse> {
    match status.map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(s) => {
            let lowered = s.to_ascii_lowercase();
            if VERSION_STATUSES.contains(&lowered.as_str()) {
                Ok(Some(lowered))
            } else {
                Err(invalid(format!(
                    "Invalid MCP Version status: {s} (expected one of {})",
                    VERSION_STATUSES.join(", ")
                )))
            }
        }
    }
}

/// Read the label map from either upstream's `labels` parameter or a JSON body.
///
/// `McpServerLabelsForm` carries `labels` as a JSON *string* parameter — the
/// maintainer client serializes the map into it — while the console posts the
/// map as a body. An absent value means "no custom labels", not an error.
fn parse_labels(
    from_param: Option<&str>,
    body: &[u8],
) -> std::result::Result<std::collections::HashMap<String, String>, HttpResponse> {
    if let Some(raw) = from_param.map(str::trim).filter(|s| !s.is_empty()) {
        return serde_json::from_str(raw).map_err(|e| invalid(format!("Invalid labels: {e}")));
    }
    let body = String::from_utf8_lossy(body);
    let body = body.trim();
    if body.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    // `{"labels": {...}}` is the documented shape; a bare map is accepted too.
    if let Ok(request) = serde_json::from_str::<McpLabelsRequest>(body) {
        return Ok(request.labels);
    }
    serde_json::from_str(body).map_err(|e| invalid(format!("Invalid labels: {e}")))
}

/// The request's content type, lower-cased.
fn content_type(req: &HttpRequest) -> String {
    use actix_web::http::header::CONTENT_TYPE;
    req.headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// Read a registration from either a form body or a JSON body.
///
/// Upstream posts the specifications as JSON strings in a form; the content
/// type decides, with a JSON-looking body as the fallback for clients that
/// never send one.
fn parse_registration(
    content_type: &str,
    body: &[u8],
) -> std::result::Result<McpServerRegistration, String> {
    let body = body.trim_ascii();
    if body.is_empty() {
        return Err("a server specification is required".to_string());
    }
    if content_type.contains("json") || body.first() == Some(&b'{') {
        return serde_json::from_slice::<McpServerRegistration>(body)
            .map_err(|e| format!("Invalid server specification: {e}"));
    }
    let form: McpAdminForm =
        serde_urlencoded::from_bytes(body).map_err(|e| format!("Invalid form body: {e}"))?;
    form.into_registration()
}

/// `PUT /labels` parameters.
///
/// Upstream's `McpServerLabelsForm` carries `labels` as a JSON *string*
/// parameter, not a body, so the maintainer client's call has to land.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpLabelsQuery {
    /// Namespace identifier.
    #[serde(default)]
    namespace_id: Option<String>,
    /// Server name.
    #[serde(default, alias = "mcpName")]
    mcp_name: Option<String>,
    /// Label name -> version, as JSON.
    #[serde(default)]
    labels: Option<String>,
}

/// The SDK's create/update encoding: specifications as JSON strings in a form.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)] // Deserialized from an HTTP form; not every field is read.
struct McpAdminForm {
    /// Namespace identifier.
    #[serde(default)]
    namespace_id: Option<String>,
    /// Server name, used when `serverSpecification` carries none.
    #[serde(default, alias = "mcpName")]
    mcp_name: Option<String>,
    /// Server specification, as JSON.
    #[serde(default, alias = "serverSpecification")]
    server_specification: Option<String>,
    /// Tool specification, as JSON.
    #[serde(default, alias = "toolSpecification")]
    tool_specification: Option<String>,
    /// Resource specification, as JSON. `McpServerDraftForm` carries it;
    /// `McpDetailForm` does not.
    #[serde(default, alias = "resourceSpecification")]
    resource_specification: Option<String>,
    /// Endpoint specification, as JSON.
    #[serde(default, alias = "endpointSpecification")]
    endpoint_specification: Option<String>,
    /// Version to create.
    #[serde(default)]
    version: Option<String>,
}

impl McpAdminForm {
    /// Build a registration, as upstream `McpRequestUtil` does: the server
    /// specification is the base object and the other specifications are folded
    /// into it.
    fn into_registration(self) -> std::result::Result<McpServerRegistration, String> {
        let server_json = self
            .server_specification
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "{}".to_string());
        let mut reg: McpServerRegistration = serde_json::from_str(&server_json)
            .map_err(|e| format!("Invalid serverSpecification: {e}"))?;

        // Upstream falls back to the form-level `mcpName`.
        if reg.name.is_empty()
            && let Some(name) = self.mcp_name.as_deref()
            && !name.is_empty()
        {
            reg.name = name.to_string();
        }

        if let Some(ns) = self.namespace_id.as_deref()
            && !ns.is_empty()
        {
            reg.namespace = ns.to_string();
        }
        if let Some(version) = self.version.as_deref()
            && !version.is_empty()
        {
            reg.version = version.to_string();
        }

        if let Some(tools) = self.tool_specification.as_deref()
            && !tools.trim().is_empty()
        {
            reg.tools = serde_json::from_str(tools)
                .map_err(|e| format!("Invalid toolSpecification: {e}"))?;
        }
        if let Some(resources) = self.resource_specification.as_deref()
            && !resources.trim().is_empty()
        {
            // Upstream wraps the list: {"resources": [...], "extensions": {}}.
            let spec: serde_json::Value = serde_json::from_str(resources)
                .map_err(|e| format!("Invalid resourceSpecification: {e}"))?;
            let entries = match spec.get("resources") {
                Some(entries) => entries.clone(),
                None => spec,
            };
            reg.resources = serde_json::from_value(entries)
                .map_err(|e| format!("Invalid resourceSpecification: {e}"))?;
        }
        if let Some(endpoint) = self.endpoint_specification.as_deref()
            && !endpoint.trim().is_empty()
        {
            let spec: serde_json::Value = serde_json::from_str(endpoint)
                .map_err(|e| format!("Invalid endpointSpecification: {e}"))?;
            // Nacos encodes the endpoint as a McpEndpointSpec: {"type", "data"}.
            // For the "direct" type the address/port live in `data`.
            if let Some(data) = spec.get("data").and_then(|d| d.as_object()) {
                let address = data.get("address").and_then(|v| v.as_str());
                let port = data.get("port").and_then(|v| v.as_u64()).or_else(|| {
                    data.get("port")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse::<u64>().ok())
                });
                if let (Some(address), Some(port)) = (address, port) {
                    reg.endpoint = format!("{address}:{port}");
                }
                if let Some(t) = spec.get("type").and_then(|v| v.as_str()) {
                    reg.transport = batata_common::model::ai::mcp::McpTransport {
                        transport_type: t.to_string(),
                        ..Default::default()
                    };
                }
            } else if let Ok(parsed) = serde_json::from_value::<McpServerRegistration>(spec.clone())
            {
                // Backwards-compatible: {endpoint, transport}
                if !parsed.endpoint.is_empty() {
                    reg.endpoint = parsed.endpoint;
                }
                reg.transport = parsed.transport;
            }
        }

        Ok(reg)
    }
}

/// GET /v3/admin/ai/mcp — Server detail
#[get("")]
async fn get_server(
    req: HttpRequest,
    data: web::Data<AppState>,
    mcp_service: web::Data<Arc<dyn McpServerService>>,
    query: web::Query<McpDetailQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let ns = normalize_namespace(query.namespace_id.as_deref());
    match mcp_service
        .get_mcp_server_detail(
            ns,
            query.mcp_id.as_deref(),
            query.mcp_name.as_deref(),
            query.version.as_deref(),
            None,
        )
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

/// GET /v3/admin/ai/mcp/list — List servers
#[get("/list")]
async fn list_servers(
    req: HttpRequest,
    data: web::Data<AppState>,
    mcp_service: web::Data<Arc<dyn McpServerService>>,
    query: web::Query<McpListQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let ns = normalize_namespace(query.namespace_id.as_deref());
    // Upstream's `McpListForm` defaults to `accurate` and rejects anything
    // else, so an unknown value is a client error rather than a silent blur.
    let search = query.search.as_deref().unwrap_or("accurate");
    if !search.eq_ignore_ascii_case("accurate") && !search.eq_ignore_ascii_case("blur") {
        return invalid("Request parameter `search` should be `accurate` or `blur`.");
    }
    let page = mcp_service
        .list_mcp_servers(
            ns,
            query.mcp_name.as_deref(),
            search,
            query.page_no.unwrap_or(1),
            query.page_size.unwrap_or(20),
            None,
        )
        .await;
    HttpResponse::Ok().json(Result::success(page))
}

/// GET /v3/admin/ai/mcp/versions — List versions of a server
#[get("/versions")]
async fn list_versions(
    req: HttpRequest,
    data: web::Data<AppState>,
    mcp_service: web::Data<Arc<dyn McpServerService>>,
    query: web::Query<McpVersionsQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let ns = normalize_namespace(query.namespace_id.as_deref());
    let Some(name) = query.mcp_name.as_deref().filter(|n| !n.is_empty()) else {
        return missing("mcpName");
    };
    let status = match resolve_version_status(query.status.as_deref()) {
        Ok(status) => status,
        Err(response) => return response,
    };
    match mcp_service
        .list_mcp_server_versions(
            ns,
            name,
            status.as_deref(),
            query.page_no.unwrap_or(1),
            query.page_size.unwrap_or(20),
        )
        .await
    {
        Ok(page) => HttpResponse::Ok().json(Result::success(page)),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// GET /v3/admin/ai/mcp/version — One version of a server
#[get("/version")]
async fn get_version(
    req: HttpRequest,
    data: web::Data<AppState>,
    mcp_service: web::Data<Arc<dyn McpServerService>>,
    query: web::Query<McpDetailQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let ns = normalize_namespace(query.namespace_id.as_deref());
    let Some(name) = query.mcp_name.as_deref().filter(|n| !n.is_empty()) else {
        return missing("mcpName");
    };
    let Some(version) = query.version.as_deref().filter(|v| !v.is_empty()) else {
        return missing("version");
    };
    match mcp_service.get_mcp_server_version(ns, name, version).await {
        Ok(Some(detail)) => HttpResponse::Ok().json(Result::success(detail)),
        Ok(None) => Result::<()>::http_not_found(
            &batata_common::error::RESOURCE_NOT_FOUND,
            format!("MCP server '{name}' version '{version}' not found"),
        ),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// POST /v3/admin/ai/mcp — Create a server (SDK form encoding)
#[post("")]
async fn create_server(
    req: HttpRequest,
    data: web::Data<AppState>,
    mcp_service: web::Data<Arc<dyn McpServerService>>,
    body: web::Bytes,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let registration = match parse_registration(&content_type(&req), &body) {
        Ok(reg) => reg,
        Err(e) => {
            return Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_VALIDATE_ERROR,
                e,
            );
        }
    };
    let ns = normalize_namespace(Some(&registration.namespace));
    match mcp_service.create_mcp_server(ns, &registration).await {
        Ok(id) => HttpResponse::Ok().json(Result::success(id)),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// PUT /v3/admin/ai/mcp — Update a server (SDK form encoding)
#[put("")]
async fn update_server(
    req: HttpRequest,
    data: web::Data<AppState>,
    mcp_service: web::Data<Arc<dyn McpServerService>>,
    body: web::Bytes,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let registration = match parse_registration(&content_type(&req), &body) {
        Ok(reg) => reg,
        Err(e) => {
            return Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_VALIDATE_ERROR,
                e,
            );
        }
    };
    let ns = normalize_namespace(Some(&registration.namespace));
    match mcp_service.update_mcp_server(ns, &registration).await {
        Ok(()) => HttpResponse::Ok().json(Result::success("ok")),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// DELETE /v3/admin/ai/mcp — Delete a server or one of its versions
#[delete("")]
async fn delete_server(
    req: HttpRequest,
    data: web::Data<AppState>,
    mcp_service: web::Data<Arc<dyn McpServerService>>,
    query: web::Query<McpDeleteQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let ns = normalize_namespace(query.namespace_id.as_deref());
    match mcp_service
        .delete_mcp_server(
            ns,
            query.mcp_name.as_deref(),
            query.mcp_id.as_deref(),
            query.version.as_deref(),
        )
        .await
    {
        Ok(()) => HttpResponse::Ok().json(Result::success("ok")),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// POST /v3/admin/ai/mcp/draft — Create a draft version
#[post("/draft")]
async fn create_draft(
    req: HttpRequest,
    data: web::Data<AppState>,
    mcp_service: web::Data<Arc<dyn McpServerService>>,
    query: web::Query<McpDraftCreateQuery>,
    body: web::Bytes,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let mut registration = match parse_registration(&content_type(&req), &body) {
        Ok(reg) => reg,
        Err(e) => {
            return Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_VALIDATE_ERROR,
                e,
            );
        }
    };
    let ns = normalize_namespace(query.namespace_id.as_deref());
    if !ns.is_empty() {
        registration.namespace = ns.to_string();
    }
    match mcp_service
        .create_mcp_server_draft(
            &registration.namespace,
            &registration,
            query.overwrite.unwrap_or(false),
        )
        .await
    {
        Ok(detail) => HttpResponse::Ok().json(Result::success(detail)),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// PUT /v3/admin/ai/mcp/draft — Update a draft version
#[put("/draft")]
async fn update_draft(
    req: HttpRequest,
    data: web::Data<AppState>,
    mcp_service: web::Data<Arc<dyn McpServerService>>,
    body: web::Bytes,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let registration = match parse_registration(&content_type(&req), &body) {
        Ok(reg) => reg,
        Err(e) => {
            return Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_VALIDATE_ERROR,
                e,
            );
        }
    };
    match mcp_service
        .update_mcp_server_draft(&registration.namespace, &registration)
        .await
    {
        Ok(detail) => HttpResponse::Ok().json(Result::success(detail)),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// DELETE /v3/admin/ai/mcp/draft — Delete a draft version
#[delete("/draft")]
async fn delete_draft(
    req: HttpRequest,
    data: web::Data<AppState>,
    mcp_service: web::Data<Arc<dyn McpServerService>>,
    query: web::Query<McpDetailQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let ns = normalize_namespace(query.namespace_id.as_deref());
    let Some(name) = query.mcp_name.as_deref().filter(|n| !n.is_empty()) else {
        return missing("mcpName");
    };
    let Some(version) = query.version.as_deref().filter(|v| !v.is_empty()) else {
        return missing("version");
    };
    match mcp_service.delete_mcp_server_draft(ns, name, version).await {
        Ok(()) => HttpResponse::Ok().json(Result::success("ok")),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// Run a version transition, which all take the same `(name, version)` pair.
async fn transition<F, Fut>(query: McpDetailQuery, what: &str, call: F) -> HttpResponse
where
    F: FnOnce(String, String, String) -> Fut,
    Fut: std::future::Future<
            Output = anyhow::Result<batata_common::model::ai::mcp::McpServerVersionDetail>,
        >,
{
    let ns = normalize_namespace(query.namespace_id.as_deref()).to_string();
    let (Some(name), Some(version)) = (
        query.mcp_name.as_deref().filter(|n| !n.is_empty()),
        query.version.as_deref().filter(|v| !v.is_empty()),
    ) else {
        return Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_MISSING,
            format!("mcpName and version are required to {what}"),
        );
    };
    let name = name.to_string();
    let version = version.to_string();

    match call(ns, name.clone(), version.clone()).await {
        Ok(detail) => HttpResponse::Ok().json(Result::success(detail)),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// POST /v3/admin/ai/mcp/submit — Submit a draft for review
#[post("/submit")]
async fn submit_version(
    req: HttpRequest,
    data: web::Data<AppState>,
    mcp_service: web::Data<Arc<dyn McpServerService>>,
    query: web::Query<McpDetailQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );
    transition(
        query.into_inner(),
        "submit a version",
        |ns, name, version| async move {
            mcp_service
                .submit_mcp_server_version(&ns, &name, &version)
                .await
        },
    )
    .await
}

/// POST /v3/admin/ai/mcp/publish — Publish a reviewed version
#[post("/publish")]
async fn publish_version(
    req: HttpRequest,
    data: web::Data<AppState>,
    mcp_service: web::Data<Arc<dyn McpServerService>>,
    query: web::Query<McpDetailQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );
    transition(
        query.into_inner(),
        "publish a version",
        |ns, name, version| async move {
            mcp_service
                .publish_mcp_server_version(&ns, &name, &version)
                .await
        },
    )
    .await
}

/// POST /v3/admin/ai/mcp/force-publish — Publish bypassing the review gate
#[post("/force-publish")]
async fn force_publish_version(
    req: HttpRequest,
    data: web::Data<AppState>,
    mcp_service: web::Data<Arc<dyn McpServerService>>,
    query: web::Query<McpDetailQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );
    transition(
        query.into_inner(),
        "force publish a version",
        |ns, name, version| async move {
            mcp_service
                .force_publish_mcp_server_version(&ns, &name, &version)
                .await
        },
    )
    .await
}

/// POST /v3/admin/ai/mcp/redraft — Move a version back to draft
#[post("/redraft")]
async fn redraft_version(
    req: HttpRequest,
    data: web::Data<AppState>,
    mcp_service: web::Data<Arc<dyn McpServerService>>,
    query: web::Query<McpDetailQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );
    transition(
        query.into_inner(),
        "redraft a version",
        |ns, name, version| async move {
            mcp_service
                .redraft_mcp_server_version(&ns, &name, &version)
                .await
        },
    )
    .await
}

/// POST /v3/admin/ai/mcp/online — Take a version online
#[post("/online")]
async fn online_version(
    req: HttpRequest,
    data: web::Data<AppState>,
    mcp_service: web::Data<Arc<dyn McpServerService>>,
    query: web::Query<McpDetailQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );
    transition(
        query.into_inner(),
        "take a version online",
        |ns, name, version| async move {
            mcp_service
                .online_mcp_server_version(&ns, &name, &version)
                .await
        },
    )
    .await
}

/// POST /v3/admin/ai/mcp/offline — Take a version offline
#[post("/offline")]
async fn offline_version(
    req: HttpRequest,
    data: web::Data<AppState>,
    mcp_service: web::Data<Arc<dyn McpServerService>>,
    query: web::Query<McpDetailQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );
    transition(
        query.into_inner(),
        "take a version offline",
        |ns, name, version| async move {
            mcp_service
                .offline_mcp_server_version(&ns, &name, &version)
                .await
        },
    )
    .await
}

/// PUT /v3/admin/ai/mcp/labels — Replace the custom version labels
#[put("/labels")]
async fn update_labels(
    req: HttpRequest,
    data: web::Data<AppState>,
    mcp_service: web::Data<Arc<dyn McpServerService>>,
    query: web::Query<McpLabelsQuery>,
    body: web::Bytes,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let ns = normalize_namespace(query.namespace_id.as_deref());
    let Some(name) = query.mcp_name.as_deref().filter(|n| !n.is_empty()) else {
        return missing("mcpName");
    };
    let labels = match parse_labels(query.labels.as_deref(), &body) {
        Ok(labels) => labels,
        Err(response) => return response,
    };
    match mcp_service.update_mcp_server_labels(ns, name, labels).await {
        Ok(labels) => HttpResponse::Ok().json(Result::success(labels)),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// PUT /v3/admin/ai/mcp/status — Enable or disable a server
#[put("/status")]
async fn update_status(
    req: HttpRequest,
    data: web::Data<AppState>,
    mcp_service: web::Data<Arc<dyn McpServerService>>,
    query: web::Query<McpStatusQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let ns = normalize_namespace(query.namespace_id.as_deref());
    let Some(name) = query.mcp_name.as_deref().filter(|n| !n.is_empty()) else {
        return missing("mcpName");
    };
    // Upstream requires `enabled`; the console's `status=enable|disable` is
    // understood as well. Reading `status` alone would have turned the
    // maintainer client's `enabled=false` into an *enable*.
    let enabled = match resolve_enabled(query.enabled, query.status.as_deref()) {
        Ok(enabled) => enabled,
        Err(e) => return invalid(e),
    };
    match mcp_service
        .update_mcp_server_status(ns, name, enabled)
        .await
    {
        Ok(()) => HttpResponse::Ok().json(Result::success("ok")),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// PUT /v3/admin/ai/mcp/scope — Change the visibility scope
#[put("/scope")]
async fn update_scope(
    req: HttpRequest,
    data: web::Data<AppState>,
    mcp_service: web::Data<Arc<dyn McpServerService>>,
    query: web::Query<McpScopeQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let ns = normalize_namespace(query.namespace_id.as_deref());
    let (Some(name), Some(scope)) = (
        query.mcp_name.as_deref().filter(|n| !n.is_empty()),
        query.scope.as_deref().filter(|s| !s.is_empty()),
    ) else {
        return missing("mcpName and scope");
    };
    match mcp_service.update_mcp_server_scope(ns, name, scope).await {
        Ok(()) => HttpResponse::Ok().json(Result::success("ok")),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// Configure admin MCP routes at `/v3/admin/ai/mcp`.
pub fn admin_routes() -> Scope {
    web::scope("/mcp")
        .service(get_server)
        .service(list_servers)
        .service(list_versions)
        .service(get_version)
        .service(create_server)
        .service(update_server)
        .service(delete_server)
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
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Upstream's maintainer client posts the specifications as JSON strings
    /// in a form; that is what `createMcpServer` and `createMcpServerDraft`
    /// send, so it is what has to parse.
    #[test]
    fn registration_is_read_from_the_sdk_form_encoding() {
        let body =
            br#"serverSpecification={"name":"svc","version":"2.0.0"}&namespaceId=ns1&mcpName=other"#;
        let reg = parse_registration("application/x-www-form-urlencoded", body).expect("parse");
        assert_eq!(reg.name, "svc", "the specification wins over mcpName");
        assert_eq!(reg.version, "2.0.0");
        assert_eq!(reg.namespace, "ns1");
    }

    /// `McpForm.validate` only requires one of `mcpId` / `mcpName`, and
    /// `McpRequestUtil` falls back to the form-level name.
    #[test]
    fn registration_falls_back_to_the_form_level_name() {
        let body = br#"serverSpecification={}&mcpName=from-form"#;
        let reg = parse_registration("application/x-www-form-urlencoded", body).expect("parse");
        assert_eq!(reg.name, "from-form");
    }

    /// `McpResourceSpecification` wraps the list; the list alone is accepted.
    #[test]
    fn resource_specification_is_unwrapped_like_upstream() {
        let wrapped = br#"serverSpecification={"name":"svc"}&resourceSpecification={"resources":[{"uri":"file:///a","name":"a"}]}"#;
        let reg = parse_registration("application/x-www-form-urlencoded", wrapped).expect("parse");
        assert_eq!(reg.resources.len(), 1);
        assert_eq!(reg.resources[0].name, "a");

        let bare = br#"serverSpecification={"name":"svc"}&resourceSpecification=[{"uri":"file:///b","name":"b"}]"#;
        let reg = parse_registration("application/x-www-form-urlencoded", bare).expect("parse");
        assert_eq!(reg.resources.len(), 1);
        assert_eq!(reg.resources[0].name, "b");
    }

    /// A JSON body is what the console posts; the content type decides.
    #[test]
    fn registration_is_read_from_a_json_body() {
        let body = br#"{"name":"json-svc","namespace":"ns2"}"#;
        let reg = parse_registration("application/json", body).expect("parse");
        assert_eq!(reg.name, "json-svc");
        assert_eq!(reg.namespace, "ns2");
    }

    #[test]
    fn a_malformed_or_empty_body_is_refused() {
        assert!(parse_registration("application/x-www-form-urlencoded", b"").is_err());
        assert!(
            parse_registration(
                "application/x-www-form-urlencoded",
                b"serverSpecification={not json"
            )
            .is_err()
        );
    }

    /// `McpServerLabelsForm` carries `labels` as a JSON string parameter.
    #[test]
    fn labels_are_read_from_the_upstream_parameter() {
        let labels = parse_labels(Some(r#"{"stable":"1.0.0"}"#), b"").expect("parse");
        assert_eq!(labels.get("stable").map(String::as_str), Some("1.0.0"));
    }

    /// The console posts the map as a body instead.
    #[test]
    fn labels_are_read_from_a_json_body() {
        let labels = parse_labels(None, br#"{"labels":{"beta":"2.0.0"}}"#).expect("parse");
        assert_eq!(labels.get("beta").map(String::as_str), Some("2.0.0"));
    }

    #[test]
    fn absent_labels_mean_no_custom_labels() {
        assert!(parse_labels(None, b"").expect("parse").is_empty());
    }

    /// `enabled=false` is how the maintainer client disables a server. Reading
    /// a `status` parameter instead used to turn that into an *enable*.
    #[test]
    fn enabled_false_disables() {
        assert!(!resolve_enabled(Some(false), None).expect("resolve"));
        assert!(!resolve_enabled(None, Some("disable")).expect("resolve"));
        assert!(!resolve_enabled(None, Some("DISABLED")).expect("resolve"));
        assert!(resolve_enabled(Some(true), None).expect("resolve"));
        assert!(resolve_enabled(None, Some("enable")).expect("resolve"));
    }

    /// `enabled` is the parameter with a defined meaning, so it wins.
    #[test]
    fn enabled_wins_over_the_console_spelling() {
        assert!(!resolve_enabled(Some(false), Some("enable")).expect("resolve"));
    }

    #[test]
    fn an_unusable_status_is_refused() {
        assert!(resolve_enabled(None, Some("maybe")).is_err());
        assert!(resolve_enabled(None, None).is_err(), "enabled is required");
    }

    #[test]
    fn version_status_is_normalized_and_validated() {
        assert_eq!(
            resolve_version_status(Some("ONLINE")).expect("resolve"),
            Some("online".to_string())
        );
        assert_eq!(resolve_version_status(None).expect("resolve"), None);
        assert!(resolve_version_status(Some("bogus")).is_err());
    }
}
