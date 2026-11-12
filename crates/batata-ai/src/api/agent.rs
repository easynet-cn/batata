//! Agent HTTP API handlers — Nacos 3.x compatible
//!
//! Admin: `/v3/admin/ai/agents`
//!
//! Upstream `AgentAdminController` (Nacos 3.3.0 GA) exposes 18 endpoints and
//! Batata mounts 19. Paths are aligned, but GA's ARD `AgentVersionDetail` /
//! `AgentCallInterface` model differs from the A2A `AgentCard` batata stores:
//! the four write endpoints accept both GA's flat admin form and batata's
//! nested JSON card, and reject `callInterfaces` instead of dropping it.
//! `GET /runtime-endpoints` reads live Naming publications through
//! `AiEndpointService` and answers GA's flat `RuntimeEndpointSnapshot`; the
//! console endpoint keeps batata's `ConsoleRuntimeEndpointView` wrapper.

use std::sync::Arc;

use actix_web::{HttpMessage, HttpRequest, HttpResponse, Responder, delete, get, post, put, web};
use serde::Deserialize;

use batata_common::{ActionTypes, ApiType, DEFAULT_NAMESPACE_ID, SignType};
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::response::Result;
use batata_server_common::{Secured, secured};

use crate::api::admin_form::{AdminForm, AdminParams};
use crate::model::AgentCard;
use crate::service::traits::A2aAgentService;

/// Query and body parameters shared by the agent version endpoints.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentVersionForm {
    /// Namespace ID (defaults to the public namespace).
    pub namespace_id: Option<String>,
    /// Agent name.
    pub agent_name: Option<String>,
    /// Version the action applies to.
    pub version: Option<String>,
    /// New scope for `PUT /scope`.
    pub scope: Option<String>,
}

/// Query parameters for the list endpoint.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentListForm {
    /// Namespace ID (defaults to the public namespace).
    pub namespace_id: Option<String>,
    /// Optional name filter.
    pub agent_name: Option<String>,
    /// `accurate` or fuzzy.
    pub search_type: Option<String>,
    /// Page number (1-based).
    pub page_no: Option<u32>,
    /// Page size.
    pub page_size: Option<u32>,
}

/// Query parameters for `GET /runtime-endpoints`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeEndpointForm {
    /// Namespace ID (defaults to the public namespace).
    pub namespace_id: Option<String>,
    /// Agent name.
    pub agent_name: Option<String>,
    /// Optional version filter (GA `AgentRuntimeEndpointForm.version`).
    pub version: Option<String>,
    /// Protocol the caller wants the snapshot for, e.g. `JSONRPC`.
    /// GA validates this as required.
    #[serde(default)]
    pub protocol: Option<String>,
}

/// Flat GA ADMIN write parameters for the four Agent write endpoints.
///
/// Nacos 3.3.0 GA `AgentAdminController` binds `AgentDraftCreateForm`,
/// `AgentDraftUpdateForm` and `AgentUpdateForm` from an urlencoded form body
/// (`ClientHttpProxy.postForm` + `setParamValue`). Batata's own SDK instead
/// posts a nested `AgentCard` JSON object. Both encodings are resolved here.
/// GA's ARD-only `callInterfaces` has no `AgentCard` counterpart and is
/// rejected rather than silently discarded; see the compatibility inventory.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentWriteForm {
    namespace_id: Option<String>,
    agent_name: Option<String>,
    version: Option<String>,
    display_name: Option<String>,
    description: Option<String>,
    icon_url: Option<String>,
    provider: Option<String>,
    tags: Option<String>,
    extensions: Option<String>,
    call_interfaces: Option<String>,
    author: Option<String>,
    change_description: Option<String>,
    based_on_version: Option<String>,
    status: Option<String>,
    overwrite: Option<bool>,
}

/// GA `AgentProvider` uses `name` while the A2A `AgentCard.provider` uses
/// `organization`; the flat form carries the GA shape as a JSON string.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GaAgentProvider {
    #[serde(default)]
    name: String,
    #[serde(default)]
    url: String,
}

/// One resolved Agent write request, independent of its wire encoding.
struct AgentWrite {
    card: AgentCard,
    namespace: String,
    overwrite: bool,
    /// Definition-time call interfaces from the GA flat form (GA
    /// `AgentVersionDetail.callInterfaces`). Absent on the nested `AgentCard`
    /// wire and when the caller omits the field.
    call_interfaces: Option<Vec<crate::model::a2a::AgentCallInterface>>,
}

fn invalid(detail: impl Into<String>) -> HttpResponse {
    Result::<()>::http_bad_request(&batata_common::error::PARAMETER_VALIDATE_ERROR, detail)
}

fn non_empty(raw: Option<&str>) -> Option<String> {
    raw.map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn json_field(raw: Option<&str>) -> Option<&str> {
    raw.map(str::trim)
        .filter(|raw| !raw.is_empty() && *raw != "null")
}

/// A nested `AgentCard` JSON body is batata's own wire. Everything else
/// (urlencoded form, empty body, JSON object of flat parameters) is resolved
/// as flat GA admin parameters.
fn is_json_card(req: &HttpRequest, body: &[u8]) -> bool {
    let content_type = req
        .headers()
        .get(actix_web::http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    content_type.contains("json") && body.trim_ascii().starts_with(b"{")
}

/// Resolve either wire into one `AgentCard` plus the shared admin parameters.
fn resolve_agent_write(
    req: &HttpRequest,
    body: &[u8],
    require_version: bool,
) -> std::result::Result<AgentWrite, HttpResponse> {
    let params = AdminParams::from_request(req, body);
    let form: AgentWriteForm = params.decode().map_err(invalid)?;
    let namespace =
        normalize_namespace(form.namespace_id.as_deref().unwrap_or_default()).to_string();
    let overwrite = form.overwrite.unwrap_or(false);

    let card = if is_json_card(req, body) {
        serde_json::from_slice::<AgentCard>(body)
            .map_err(|e| invalid(format!("Invalid AgentCard: {e}")))?
    } else {
        agent_card_from_form(&form, require_version)?
    };
    let call_interfaces = parse_call_interfaces(form.call_interfaces.as_deref())?;
    Ok(AgentWrite {
        card,
        namespace,
        overwrite,
        call_interfaces,
    })
}

/// Parse the GA `callInterfaces` JSON into the definition-time model.
///
/// GA's first-draft validator (`requireInitialDraftContent`) requires a non-null
/// `callInterfaces`; batata stores whichever value was supplied. An absent or
/// `null` field yields `None`; an explicit `[]` yields `Some(vec![])` so the
/// round-trip preserves the "present but empty" distinction.
fn parse_call_interfaces(
    raw: Option<&str>,
) -> std::result::Result<Option<Vec<crate::model::a2a::AgentCallInterface>>, HttpResponse> {
    let raw = match raw
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "null")
    {
        Some(raw) => raw,
        None => return Ok(None),
    };
    let parsed: Vec<crate::model::a2a::AgentCallInterface> =
        serde_json::from_str(raw).map_err(|e| {
            invalid(format!(
                "callInterfaces must be a JSON array of AgentCallInterface: {e}"
            ))
        })?;
    Ok(Some(parsed))
}

/// Build an `AgentCard` from the GA flat form. Fields with no A2A counterpart
/// are preserved in batata's `metadata` map instead of being dropped silently.
fn agent_card_from_form(
    form: &AgentWriteForm,
    require_version: bool,
) -> std::result::Result<AgentCard, HttpResponse> {
    let name = form
        .agent_name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .ok_or_else(|| {
            Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_MISSING,
                "agentName is required",
            )
        })?;
    let version = form.version.as_deref().map(str::trim).unwrap_or_default();
    if require_version && version.is_empty() {
        return Err(Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_MISSING,
            "version is required",
        ));
    }

    // The GA ARD `callInterfaces` field is parsed at the route boundary (see
    // `parse_call_interfaces`) and stored on the version definition; it is no
    // longer rejected fail-closed.

    let mut card = AgentCard {
        name: name.to_string(),
        display_name: form
            .display_name
            .clone()
            .unwrap_or_else(|| name.to_string()),
        description: form.description.clone().unwrap_or_default(),
        version: version.to_string(),
        icon_url: non_empty(form.icon_url.as_deref()),
        ..Default::default()
    };

    if let Some(raw) = json_field(form.provider.as_deref()) {
        let provider: GaAgentProvider =
            serde_json::from_str(raw).map_err(|e| invalid(format!("Invalid provider: {e}")))?;
        card.provider = Some(crate::model::a2a::AgentProvider {
            organization: provider.name,
            url: provider.url,
        });
    }
    if let Some(raw) = json_field(form.tags.as_deref()) {
        card.tags = serde_json::from_str(raw).map_err(|e| invalid(format!("Invalid tags: {e}")))?;
    }
    if let Some(raw) = json_field(form.extensions.as_deref()) {
        let value: serde_json::Value =
            serde_json::from_str(raw).map_err(|e| invalid(format!("Invalid extensions: {e}")))?;
        let serde_json::Value::Object(object) = value else {
            return Err(invalid("extensions must be a JSON object"));
        };
        for (key, value) in object {
            card.metadata.insert(
                key,
                match value {
                    serde_json::Value::String(text) => text,
                    other => other.to_string(),
                },
            );
        }
    }
    // GA-only governance fields are preserved, not interpreted. These keys
    // are batata-internal metadata, not part of the GA wire contract.
    for (key, value) in [
        ("ga.author", &form.author),
        ("ga.changeDescription", &form.change_description),
        ("ga.basedOnVersion", &form.based_on_version),
        ("ga.status", &form.status),
    ] {
        if let Some(value) = value.as_deref().filter(|value| !value.is_empty()) {
            card.metadata.insert(key.to_string(), value.to_string());
        }
    }
    Ok(card)
}

fn normalize_namespace(ns: &str) -> &str {
    if ns.is_empty() {
        DEFAULT_NAMESPACE_ID
    } else {
        ns
    }
}

/// Split a version action request into its parts, or produce a 400 response.
fn require_target(
    query: &AgentVersionForm,
) -> std::result::Result<(&str, &str, &str), HttpResponse> {
    let ns = normalize_namespace(query.namespace_id.as_deref().unwrap_or_default());
    match (
        query.agent_name.as_deref().filter(|n| !n.is_empty()),
        query.version.as_deref().filter(|v| !v.is_empty()),
    ) {
        (Some(name), Some(version)) => Ok((ns, name, version)),
        _ => Err(Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_MISSING,
            "agentName and version are required",
        )),
    }
}

// ============================================================================
// Admin handlers — `/v3/admin/ai/agents`
// ============================================================================

/// GET /v3/admin/ai/agents — get agent detail
#[get("")]
async fn get_agent_detail(
    req: HttpRequest,
    data: web::Data<AppState>,
    service: web::Data<Arc<dyn A2aAgentService>>,
    query: web::Query<AgentVersionForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let ns = normalize_namespace(query.namespace_id.as_deref().unwrap_or_default());
    let name = match query.agent_name.as_deref().filter(|n| !n.is_empty()) {
        Some(n) => n,
        None => {
            return Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_MISSING,
                "agentName is required",
            );
        }
    };

    let username = req
        .extensions()
        .get::<batata_common::IdentityContext>()
        .map(|ctx| ctx.username.clone())
        .unwrap_or_default();

    match service
        .get_agent_card(ns, name, query.version.as_deref(), Some(&username))
        .await
    {
        Ok(Some(card)) => HttpResponse::Ok().json(Result::success(card)),
        Ok(None) => Result::<()>::http_not_found(
            &batata_common::error::AGENT_NOT_FOUND,
            format!("Agent '{}' not found", name),
        ),
        Err(e) => Result::<()>::http_internal_error(e),
    }
}

/// GET /v3/admin/ai/agents/list — list agents
#[get("/list")]
async fn list_agents(
    req: HttpRequest,
    data: web::Data<AppState>,
    service: web::Data<Arc<dyn A2aAgentService>>,
    query: web::Query<AgentListForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let ns = normalize_namespace(query.namespace_id.as_deref().unwrap_or_default());
    let username = req
        .extensions()
        .get::<batata_common::IdentityContext>()
        .map(|ctx| ctx.username.clone())
        .unwrap_or_default();

    match service
        .list_agents(
            ns,
            query.agent_name.as_deref(),
            query.search_type.as_deref().unwrap_or("blur"),
            query.page_no.unwrap_or(1),
            query.page_size.unwrap_or(20),
            Some(&username),
        )
        .await
    {
        Ok(page) => HttpResponse::Ok().json(Result::success(page)),
        Err(e) => Result::<()>::http_internal_error(e),
    }
}

/// GET /v3/admin/ai/agents/versions — list versions of one agent
#[get("/versions")]
async fn list_agent_versions(
    req: HttpRequest,
    data: web::Data<AppState>,
    service: web::Data<Arc<dyn A2aAgentService>>,
    query: web::Query<AgentVersionForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let ns = normalize_namespace(query.namespace_id.as_deref().unwrap_or_default());
    let name = match query.agent_name.as_deref().filter(|n| !n.is_empty()) {
        Some(n) => n,
        None => {
            return Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_MISSING,
                "agentName is required",
            );
        }
    };

    match service.list_versions(ns, name).await {
        Ok(versions) => HttpResponse::Ok().json(Result::success(versions)),
        Err(e) => Result::<()>::http_internal_error(e),
    }
}

/// POST /v3/admin/ai/agents — register an agent
#[post("")]
async fn create_agent(
    req: HttpRequest,
    data: web::Data<AppState>,
    service: web::Data<Arc<dyn A2aAgentService>>,
    body: web::Bytes,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let write = match resolve_agent_write(&req, &body, false) {
        Ok(write) => write,
        Err(response) => return response,
    };
    match service
        .register_agent(&write.card, &write.namespace, "manual")
        .await
    {
        Ok(id) => HttpResponse::Ok().json(Result::success(id)),
        Err(e) => Result::<()>::http_internal_error(e),
    }
}

/// PUT /v3/admin/ai/agents — update an agent
#[put("")]
async fn update_agent(
    req: HttpRequest,
    data: web::Data<AppState>,
    service: web::Data<Arc<dyn A2aAgentService>>,
    body: web::Bytes,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let write = match resolve_agent_write(&req, &body, false) {
        Ok(write) => write,
        Err(response) => return response,
    };
    match service
        .update_agent_card(&write.card, &write.namespace, "manual")
        .await
    {
        Ok(()) => HttpResponse::Ok().json(Result::success("ok")),
        Err(e) => Result::<()>::http_internal_error(e),
    }
}

/// GET /v3/admin/ai/agents/version — get one exact agent version definition
#[get("/version")]
async fn get_version(
    req: HttpRequest,
    data: web::Data<AppState>,
    service: web::Data<Arc<dyn A2aAgentService>>,
    query: web::Query<AgentVersionForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let ns = normalize_namespace(query.namespace_id.as_deref().unwrap_or_default());
    let name = match query.agent_name.as_deref().filter(|n| !n.is_empty()) {
        Some(n) => n,
        None => {
            return Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_MISSING,
                "agentName is required",
            );
        }
    };
    let version = match query.version.as_deref().filter(|v| !v.is_empty()) {
        Some(v) => v,
        None => {
            return Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_MISSING,
                "version is required",
            );
        }
    };

    let username = req
        .extensions()
        .get::<batata_common::IdentityContext>()
        .map(|ctx| ctx.username.clone())
        .unwrap_or_default();

    // `get_agent_card` with an explicit version resolves the exact version
    // definition, which is exactly what `/version` asks for.
    match service
        .get_agent_card(ns, name, Some(version), Some(&username))
        .await
    {
        Ok(Some(card)) => HttpResponse::Ok().json(Result::success(card)),
        Ok(None) => Result::<()>::http_not_found(
            &batata_common::error::AGENT_NOT_FOUND,
            format!("Agent '{}' version '{}' not found", name, version),
        ),
        Err(e) => Result::<()>::http_internal_error(e),
    }
}

/// GET /v3/admin/ai/agents/runtime-endpoints — GA runtime endpoint snapshot.
///
/// GA binds `namespaceId` (default `public`), requires `agentName` and
/// `protocol`, and treats `version` as optional. An unknown agent or an
/// unwired Naming backend yields a successful empty snapshot.
#[get("/runtime-endpoints")]
async fn get_runtime_endpoints(
    req: HttpRequest,
    data: web::Data<AppState>,
    service: web::Data<Arc<dyn A2aAgentService>>,
    query: web::Query<AgentRuntimeEndpointForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let ns = normalize_namespace(query.namespace_id.as_deref().unwrap_or_default());
    let name = match query.agent_name.as_deref().filter(|n| !n.is_empty()) {
        Some(n) => n,
        None => {
            return Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_VALIDATE_ERROR,
                "agentName is required",
            );
        }
    };
    let protocol = match query.protocol.as_deref().filter(|p| !p.is_empty()) {
        Some(p) => p,
        None => {
            return Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_VALIDATE_ERROR,
                "protocol is required",
            );
        }
    };
    // GA's `AgentRuntimeEndpointForm.validate()` only checks version when it
    // is present; absent means "all runtime versions".
    let version = query.version.as_deref().filter(|v| !v.is_empty());

    match service
        .get_runtime_endpoint_snapshot(ns, name, protocol, version)
        .await
    {
        Ok(snapshot) => HttpResponse::Ok().json(Result::success(snapshot)),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// DELETE /v3/admin/ai/agents — delete an agent or one of its versions
#[delete("")]
async fn delete_agent(
    req: HttpRequest,
    data: web::Data<AppState>,
    service: web::Data<Arc<dyn A2aAgentService>>,
    query: web::Query<AgentVersionForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let ns = normalize_namespace(query.namespace_id.as_deref().unwrap_or_default());
    let name = match query.agent_name.as_deref().filter(|n| !n.is_empty()) {
        Some(n) => n,
        None => {
            return Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_MISSING,
                "agentName is required",
            );
        }
    };

    match service
        .delete_agent(ns, name, query.version.as_deref())
        .await
    {
        Ok(()) => HttpResponse::Ok().json(Result::success("ok")),
        Err(e) => Result::<()>::http_internal_error(e),
    }
}

/// POST /v3/admin/ai/agents/draft — create a draft version
#[post("/draft")]
async fn create_draft(
    req: HttpRequest,
    data: web::Data<AppState>,
    service: web::Data<Arc<dyn A2aAgentService>>,
    body: web::Bytes,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let write = match resolve_agent_write(&req, &body, true) {
        Ok(write) => write,
        Err(response) => return response,
    };
    match service
        .create_agent_draft(
            &write.namespace,
            &write.card,
            write.overwrite,
            write.call_interfaces,
        )
        .await
    {
        Ok(detail) => HttpResponse::Ok().json(Result::success(detail)),
        Err(e) => Result::<()>::http_internal_error(e),
    }
}

/// PUT /v3/admin/ai/agents/draft — update the draft being edited
#[put("/draft")]
async fn update_draft(
    req: HttpRequest,
    data: web::Data<AppState>,
    service: web::Data<Arc<dyn A2aAgentService>>,
    body: web::Bytes,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    // `AgentCard` carries no namespace, so it comes from the merged admin
    // parameters (query or form body) like every other form field.
    let write = match resolve_agent_write(&req, &body, true) {
        Ok(write) => write,
        Err(response) => return response,
    };
    match service
        .update_agent_draft(&write.namespace, &write.card, write.call_interfaces)
        .await
    {
        Ok(detail) => HttpResponse::Ok().json(Result::success(detail)),
        Err(e) => Result::<()>::http_internal_error(e),
    }
}

/// DELETE /v3/admin/ai/agents/draft — delete a draft version
#[delete("/draft")]
async fn delete_draft(
    req: HttpRequest,
    data: web::Data<AppState>,
    service: web::Data<Arc<dyn A2aAgentService>>,
    query: web::Query<AgentVersionForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let (ns, name, version) = match require_target(&query) {
        Ok(target) => target,
        Err(response) => return response,
    };
    match service.delete_agent_draft(ns, name, version).await {
        Ok(()) => HttpResponse::Ok().json(Result::success("ok")),
        Err(e) => Result::<()>::http_internal_error(e),
    }
}

/// POST /v3/admin/ai/agents/submit — submit a draft for review
#[post("/submit")]
async fn submit_agent(
    req: HttpRequest,
    data: web::Data<AppState>,
    service: web::Data<Arc<dyn A2aAgentService>>,
    query: AdminForm<AgentVersionForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let (ns, name, version) = match require_target(&query) {
        Ok(target) => target,
        Err(response) => return response,
    };
    match service.submit_agent_version(ns, name, version).await {
        Ok(detail) => HttpResponse::Ok().json(Result::success(detail)),
        Err(e) => Result::<()>::http_internal_error(e),
    }
}

/// POST /v3/admin/ai/agents/publish — publish a reviewed version
#[post("/publish")]
async fn publish_agent(
    req: HttpRequest,
    data: web::Data<AppState>,
    service: web::Data<Arc<dyn A2aAgentService>>,
    query: AdminForm<AgentVersionForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let (ns, name, version) = match require_target(&query) {
        Ok(target) => target,
        Err(response) => return response,
    };
    match service.publish_agent_version(ns, name, version).await {
        Ok(detail) => HttpResponse::Ok().json(Result::success(detail)),
        Err(e) => Result::<()>::http_internal_error(e),
    }
}

/// POST /v3/admin/ai/agents/force-publish — publish bypassing review
#[post("/force-publish")]
async fn force_publish_agent(
    req: HttpRequest,
    data: web::Data<AppState>,
    service: web::Data<Arc<dyn A2aAgentService>>,
    query: AdminForm<AgentVersionForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let (ns, name, version) = match require_target(&query) {
        Ok(target) => target,
        Err(response) => return response,
    };
    match service.force_publish_agent_version(ns, name, version).await {
        Ok(detail) => HttpResponse::Ok().json(Result::success(detail)),
        Err(e) => Result::<()>::http_internal_error(e),
    }
}

/// POST /v3/admin/ai/agents/redraft — move a version back to draft
#[post("/redraft")]
async fn redraft_agent(
    req: HttpRequest,
    data: web::Data<AppState>,
    service: web::Data<Arc<dyn A2aAgentService>>,
    query: AdminForm<AgentVersionForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let (ns, name, version) = match require_target(&query) {
        Ok(target) => target,
        Err(response) => return response,
    };
    match service.redraft_agent_version(ns, name, version).await {
        Ok(detail) => HttpResponse::Ok().json(Result::success(detail)),
        Err(e) => Result::<()>::http_internal_error(e),
    }
}

/// POST /v3/admin/ai/agents/online — bring an offline version online
#[post("/online")]
async fn online_agent(
    req: HttpRequest,
    data: web::Data<AppState>,
    service: web::Data<Arc<dyn A2aAgentService>>,
    query: AdminForm<AgentVersionForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let (ns, name, version) = match require_target(&query) {
        Ok(target) => target,
        Err(response) => return response,
    };
    match service.online_agent_version(ns, name, version).await {
        Ok(detail) => HttpResponse::Ok().json(Result::success(detail)),
        Err(e) => Result::<()>::http_internal_error(e),
    }
}

/// POST /v3/admin/ai/agents/offline — take an online version offline
#[post("/offline")]
async fn offline_agent(
    req: HttpRequest,
    data: web::Data<AppState>,
    service: web::Data<Arc<dyn A2aAgentService>>,
    query: AdminForm<AgentVersionForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let (ns, name, version) = match require_target(&query) {
        Ok(target) => target,
        Err(response) => return response,
    };
    match service.offline_agent_version(ns, name, version).await {
        Ok(detail) => HttpResponse::Ok().json(Result::success(detail)),
        Err(e) => Result::<()>::http_internal_error(e),
    }
}

/// PUT /v3/admin/ai/agents/labels — replace the custom version labels
#[put("/labels")]
async fn update_labels(
    req: HttpRequest,
    data: web::Data<AppState>,
    service: web::Data<Arc<dyn A2aAgentService>>,
    body: web::Bytes,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let params = AdminParams::from_request(&req, &body);
    let query: AgentVersionForm = match params.decode() {
        Ok(query) => query,
        Err(e) => {
            return Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_VALIDATE_ERROR,
                e,
            );
        }
    };
    let ns = normalize_namespace(query.namespace_id.as_deref().unwrap_or_default());
    let name = match query.agent_name.as_deref().filter(|n| !n.is_empty()) {
        Some(n) => n,
        None => {
            return Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_MISSING,
                "agentName is required",
            );
        }
    };
    let labels = match params.labels(&body) {
        Ok(labels) => labels,
        Err(e) => {
            return Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_VALIDATE_ERROR,
                e,
            );
        }
    };

    match service.update_agent_labels(ns, name, labels).await {
        Ok(labels) => HttpResponse::Ok().json(Result::success(labels)),
        Err(e) => Result::<()>::http_internal_error(e),
    }
}

/// PUT /v3/admin/ai/agents/scope — change the visibility scope
#[put("/scope")]
async fn update_scope(
    req: HttpRequest,
    data: web::Data<AppState>,
    service: web::Data<Arc<dyn A2aAgentService>>,
    query: AdminForm<AgentVersionForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let ns = normalize_namespace(query.namespace_id.as_deref().unwrap_or_default());
    let name = match query.agent_name.as_deref().filter(|n| !n.is_empty()) {
        Some(n) => n,
        None => {
            return Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_MISSING,
                "agentName is required",
            );
        }
    };
    let scope = match query.scope.as_deref().filter(|s| !s.is_empty()) {
        Some(s) => s,
        None => {
            return Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_MISSING,
                "scope is required",
            );
        }
    };

    match service.update_agent_scope(ns, name, scope).await {
        Ok(()) => HttpResponse::Ok().json(Result::success("ok")),
        Err(e) => Result::<()>::http_internal_error(e),
    }
}

/// Admin agent routes at `/v3/admin/ai/agents`.
pub fn admin_routes() -> actix_web::Scope {
    web::scope("/agents")
        .service(get_agent_detail)
        .service(list_agents)
        .service(list_agent_versions)
        .service(get_version)
        .service(get_runtime_endpoints)
        .service(create_agent)
        .service(update_agent)
        .service(delete_agent)
        .service(create_draft)
        .service(update_draft)
        .service(delete_draft)
        .service(submit_agent)
        .service(publish_agent)
        .service(force_publish_agent)
        .service(redraft_agent)
        .service(online_agent)
        .service(offline_agent)
        .service(update_labels)
        .service(update_scope)
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::test::TestRequest;

    fn ga_form() -> AgentWriteForm {
        AgentWriteForm {
            namespace_id: Some("ns-1".to_string()),
            agent_name: Some("ga-agent".to_string()),
            version: Some("2.1.0".to_string()),
            display_name: Some("GA Agent".to_string()),
            description: Some("created from a GA form".to_string()),
            icon_url: Some("https://example.com/icon.png".to_string()),
            provider: Some(r#"{"name":"acme","url":"https://acme.example"}"#.to_string()),
            tags: Some(r#"["coding","writing"]"#.to_string()),
            extensions: Some(r#"{"team":"ai","seats":3}"#.to_string()),
            author: Some("alice".to_string()),
            change_description: Some("initial draft".to_string()),
            based_on_version: Some("2.0.0".to_string()),
            status: Some("ONLINE".to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn ga_flat_form_maps_every_supported_field() {
        let card = agent_card_from_form(&ga_form(), true).expect("flat form must map");
        assert_eq!(card.name, "ga-agent");
        assert_eq!(card.version, "2.1.0");
        assert_eq!(card.display_name, "GA Agent");
        assert_eq!(card.description, "created from a GA form");
        assert_eq!(
            card.icon_url.as_deref(),
            Some("https://example.com/icon.png")
        );
        let provider = card.provider.expect("provider must map");
        assert_eq!(provider.organization, "acme");
        assert_eq!(provider.url, "https://acme.example");
        assert_eq!(card.tags, vec!["coding", "writing"]);
        assert_eq!(card.metadata.get("team").map(String::as_str), Some("ai"));
        assert_eq!(card.metadata.get("seats").map(String::as_str), Some("3"));
        assert_eq!(
            card.metadata.get("ga.author").map(String::as_str),
            Some("alice")
        );
        assert_eq!(
            card.metadata
                .get("ga.changeDescription")
                .map(String::as_str),
            Some("initial draft")
        );
        assert_eq!(
            card.metadata.get("ga.basedOnVersion").map(String::as_str),
            Some("2.0.0")
        );
        assert_eq!(
            card.metadata.get("ga.status").map(String::as_str),
            Some("ONLINE")
        );
    }

    #[test]
    fn ga_call_interfaces_are_accepted_and_parsed() {
        // GA's `requireInitialDraftContent` needs a non-null `callInterfaces`; it
        // must be accepted (not rejected fail-closed) and parsed onto the write.
        let parsed = parse_call_interfaces(Some(r#"[{"protocol":"JSONRPC"}]"#)).unwrap();
        let cis = parsed.expect("one interface");
        assert_eq!(cis.len(), 1);
        assert_eq!(cis[0].protocol, "JSONRPC");

        // An explicit empty array is preserved as `Some(vec![])` so the
        // round-trip keeps the "present but empty" distinction.
        assert_eq!(parse_call_interfaces(Some("[]")).unwrap(), Some(vec![]));

        // Absent or `null` yields `None`.
        assert_eq!(parse_call_interfaces(Some("null")).unwrap(), None);
        assert_eq!(parse_call_interfaces(None).unwrap(), None);

        // Malformed JSON is rejected.
        assert!(parse_call_interfaces(Some(r#"{"protocol":"JSONRPC"}"#)).is_err());
    }

    #[test]
    fn ga_call_interfaces_round_trip_through_write() {
        let req =
            TestRequest::with_uri("/v3/admin/ai/agents/draft?namespaceId=ns-1&overwrite=true")
                .insert_header(("content-type", "application/x-www-form-urlencoded"))
                .to_http_request();
        // callInterfaces = [{"protocol":"JSONRPC"}]
        let body =
            b"agentName=ci-agent&version=1.0.0&callInterfaces=%5B%7B%22protocol%22%3A%22JSONRPC%22%7D%5D";
        let write = resolve_agent_write(&req, body, true).expect("write");
        let cis = write.call_interfaces.expect("call interfaces parsed");
        assert_eq!(cis.len(), 1);
        assert_eq!(cis[0].protocol, "JSONRPC");
    }

    #[test]
    fn flat_form_identity_and_version_are_validated() {
        let mut form = ga_form();
        form.agent_name = Some("  ".to_string());
        assert!(agent_card_from_form(&form, true).is_err());
        form.agent_name = Some("ga-agent".to_string());
        form.version = None;
        assert!(agent_card_from_form(&form, true).is_err());
        // `PUT /agents` has no version in GA, so it must still work.
        assert!(agent_card_from_form(&form, false).is_ok());
    }

    #[test]
    fn malformed_ga_json_fields_are_rejected() {
        let mut form = ga_form();
        form.tags = Some("not json".to_string());
        assert!(agent_card_from_form(&form, true).is_err());
        form.tags = Some(r#"["coding"]"#.to_string());
        form.extensions = Some(r#"["not","an","object"]"#.to_string());
        assert!(agent_card_from_form(&form, true).is_err());
    }

    #[test]
    fn ga_form_body_is_merged_with_the_query_which_wins() {
        let req =
            TestRequest::with_uri("/v3/admin/ai/agents/draft?namespaceId=ns-query&overwrite=true")
                .insert_header(("content-type", "application/x-www-form-urlencoded"))
                .to_http_request();
        let body = b"namespaceId=ns-body&agentName=ga-agent&version=3.0.0&overwrite=false";
        let write = resolve_agent_write(&req, body, true).expect("form write");
        assert_eq!(write.namespace, "ns-query");
        assert!(write.overwrite, "the query value must win");
        assert_eq!(write.card.name, "ga-agent");
        assert_eq!(write.card.version, "3.0.0");
    }

    #[test]
    fn nested_json_agent_card_wire_is_unchanged() {
        let req =
            TestRequest::with_uri("/v3/admin/ai/agents/draft?namespaceId=ns-1&overwrite=true")
                .insert_header(("content-type", "application/json"))
                .to_http_request();
        let card = AgentCard {
            name: "json-agent".to_string(),
            version: "4.0.0".to_string(),
            description: "nested wire".to_string(),
            ..Default::default()
        };
        let body = serde_json::to_vec(&card).expect("serialize card");
        let write = resolve_agent_write(&req, &body, true).expect("json write");
        assert_eq!(write.namespace, "ns-1");
        assert!(write.overwrite);
        assert_eq!(write.card.name, "json-agent");
        assert_eq!(write.card.version, "4.0.0");
        assert_eq!(write.card.description, "nested wire");
    }
}
