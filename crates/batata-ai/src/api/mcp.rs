//! MCP server admin HTTP API handlers — Nacos 3.x compatible.
//!
//! Admin: `/v3/admin/ai/mcp`
//!
//! Create and update keep the SDK's encoding: `serverSpecification`,
//! `toolSpecification` and `endpointSpecification` arrive as JSON *strings*
//! inside a form body — upstream `McpDetailForm`, parsed by
//! `McpRequestUtil.parseMcpServerBasicInfo`. The maintainer client posts that
//! shape, so it is preserved here even though every other endpoint on this
//! scope takes ordinary query or JSON parameters.
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

/// The SDK's create/update encoding: specifications as JSON strings in a form.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)] // Deserialized from an HTTP form; not every field is read.
struct McpAdminForm {
    /// Namespace identifier.
    #[serde(default, alias = "namespaceId")]
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
        if let Some(endpoint) = self.endpoint_specification.as_deref()
            && !endpoint.trim().is_empty()
        {
            let spec: serde_json::Value = serde_json::from_str(endpoint)
                .map_err(|e| format!("Invalid endpointSpecification: {e}"))?;
            // Nacos encodes the endpoint as a McpEndpointSpec: {"type", "data"}.
            // For the "direct" type the address/port live in `data`.
            if let Some(data) = spec.get("data").and_then(|d| d.as_object()) {
                let address = data.get("address").and_then(|v| v.as_str());
                let port = data
                    .get("port")
                    .and_then(|v| v.as_u64())
                    .or_else(|| {
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
            } else if let Ok(parsed) = serde_json::from_value::<McpServerRegistration>(spec.clone()) {
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
    let page = mcp_service
        .list_mcp_servers(
            ns,
            query.mcp_name.as_deref(),
            query.search.as_deref().unwrap_or("blur"),
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
    match mcp_service
        .list_mcp_server_versions(
            ns,
            name,
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
    form: web::Form<McpAdminForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let registration = match form.into_inner().into_registration() {
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
    form: web::Form<McpAdminForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let registration = match form.into_inner().into_registration() {
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
    body: web::Json<McpServerRegistration>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let mut registration = body.into_inner();
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
    body: web::Json<McpServerRegistration>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let registration = body.into_inner();
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
    Fut: std::future::Future<Output = anyhow::Result<batata_common::model::ai::mcp::McpServerVersionDetail>>,
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
    transition(query.into_inner(),"submit a version", |ns, name, version| async move {
        mcp_service.submit_mcp_server_version(&ns, &name, &version).await
    })
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
    transition(query.into_inner(),"publish a version", |ns, name, version| async move {
        mcp_service.publish_mcp_server_version(&ns, &name, &version).await
    })
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
    transition(query.into_inner(),"force publish a version", |ns, name, version| async move {
        mcp_service
            .force_publish_mcp_server_version(&ns, &name, &version)
            .await
    })
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
    transition(query.into_inner(),"redraft a version", |ns, name, version| async move {
        mcp_service.redraft_mcp_server_version(&ns, &name, &version).await
    })
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
    transition(query.into_inner(),"take a version online", |ns, name, version| async move {
        mcp_service.online_mcp_server_version(&ns, &name, &version).await
    })
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
    transition(query.into_inner(),"take a version offline", |ns, name, version| async move {
        mcp_service.offline_mcp_server_version(&ns, &name, &version).await
    })
    .await
}

/// PUT /v3/admin/ai/mcp/labels — Replace the custom version labels
#[put("/labels")]
async fn update_labels(
    req: HttpRequest,
    data: web::Data<AppState>,
    mcp_service: web::Data<Arc<dyn McpServerService>>,
    query: web::Query<McpDetailQuery>,
    body: web::Json<McpLabelsRequest>,
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
    match mcp_service
        .update_mcp_server_labels(ns, name, body.into_inner().labels)
        .await
    {
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
    // Upstream treats anything other than an explicit disable as enable.
    let enabled = !matches!(query.status.as_deref(), Some(s) if s.eq_ignore_ascii_case("disabled"));
    match mcp_service.update_mcp_server_status(ns, name, enabled).await {
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
