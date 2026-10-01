//! Console Agent management API endpoints.
//!
//! Mirrors upstream `ConsoleAgentController` under `/v3/console/ai/agents`.
//! This is distinct from the A2A registry endpoints in `ai_a2a.rs`
//! (`ConsoleA2aController`): those address `/ai/a2a`, these address
//! `/ai/agents`.
//!
//! As with the other console modules, reads and deletes take query parameters
//! and every write is form-encoded, which is how the Nacos console UI calls
//! them (see `console-ui-next/src/api/agent.ts`).

use std::sync::Arc;

use actix_web::{HttpRequest, HttpResponse, Responder, Scope, delete, get, post, put, web};
use serde::Deserialize;

use batata_common::DEFAULT_NAMESPACE_ID;
use batata_common::A2aAgentService;
use batata_common::model::ai::a2a::{AgentCapabilities, AgentCard};
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::response as common_response;
use batata_server_common::secured::Secured;
use batata_server_common::{ActionTypes, ApiType, SignType, secured};

use super::ai_trace::{self, get_username, trace_write};

fn normalize_namespace(ns: &str) -> &str {
    if ns.is_empty() {
        DEFAULT_NAMESPACE_ID
    } else {
        ns
    }
}

// ============================================================================
// Request forms
// ============================================================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Query for listing agents.
pub struct AgentListQuery {
    /// Optional agent name filter.
    #[serde(default, alias = "agentName")]
    pub agent_name: Option<String>,
    /// Namespace identifier.
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
    /// `accurate` or `blur` matching.
    #[serde(default)]
    pub search: Option<String>,
    /// Page number (1-based).
    #[serde(default = "default_page_no", alias = "pageNo")]
    pub page_no: u32,
    /// Page size.
    #[serde(default = "default_page_size", alias = "pageSize")]
    pub page_size: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Query identifying one agent.
pub struct AgentKeyQuery {
    /// Agent name (identifier).
    #[serde(alias = "agentName")]
    pub agent_name: String,
    /// Namespace identifier.
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Query for one agent version.
pub struct AgentVersionQuery {
    /// Agent name (identifier).
    #[serde(alias = "agentName")]
    pub agent_name: String,
    /// Version in `major.minor.patch` format.
    #[serde(default)]
    pub version: Option<String>,
    /// Namespace identifier.
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Form for an action on one agent version.
pub struct AgentVersionActionForm {
    /// Agent name (identifier).
    #[serde(alias = "agentName")]
    pub agent_name: String,
    /// Target version.
    #[serde(default)]
    pub version: String,
    /// Namespace identifier.
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Form for reading one version's runtime endpoints.
pub struct AgentRuntimeEndpointForm {
    /// Agent name (identifier).
    #[serde(alias = "agentName")]
    pub agent_name: String,
    /// Target version.
    #[serde(default)]
    pub version: String,
    /// Namespace identifier.
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
    /// Protocol the console wants the snapshot for, e.g. `JSONRPC`.
    #[serde(default)]
    pub protocol: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Form for creating a draft version.
///
/// `AgentCard` carries the A2A interface fields (`additionalInterfaces`,
/// `supportedInterfaces`) and the security block, so nothing sent here is
/// silently dropped.
pub struct AgentDraftCreateForm {
    /// Agent name (identifier).
    #[serde(alias = "agentName")]
    pub agent_name: String,
    /// Version to create.
    #[serde(default)]
    pub version: String,
    /// Optional display name.
    #[serde(default, alias = "displayName")]
    pub display_name: Option<String>,
    /// Optional description.
    #[serde(default)]
    pub description: Option<String>,
    /// Optional icon URL.
    #[serde(default, alias = "iconUrl")]
    pub icon_url: Option<String>,
    /// Optional provider name.
    #[serde(default)]
    pub provider: Option<String>,
    /// Accepted but not stored: `AgentCard` has no such field.
    #[serde(default)]
    pub extensions: Option<String>,
    /// Accepted but not stored: `AgentCard` has no such field.
    #[serde(default, alias = "callInterfaces")]
    pub call_interfaces: Option<String>,
    /// Namespace identifier.
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Form for replacing the label routing.
pub struct AgentLabelsForm {
    /// Agent name (identifier).
    #[serde(alias = "agentName")]
    pub agent_name: String,
    /// JSON object mapping each label to a version.
    #[serde(default)]
    pub labels: Option<String>,
    /// Namespace identifier.
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Form for changing the visibility scope.
pub struct AgentScopeForm {
    /// Agent name (identifier).
    #[serde(alias = "agentName")]
    pub agent_name: String,
    /// `PUBLIC` or `PRIVATE`.
    #[serde(default)]
    pub scope: String,
    /// Namespace identifier.
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
}

fn default_page_no() -> u32 {
    1
}

fn default_page_size() -> u32 {
    10
}

// ============================================================================
// Read handlers
// ============================================================================

/// GET /v3/console/ai/agents/list — List agents
#[get("/list")]
async fn list_agents(
    req: HttpRequest,
    data: web::Data<AppState>,
    agent_service: web::Data<Arc<dyn A2aAgentService>>,
    query: web::Query<AgentListQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/agents")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);
    let search_type = query.search.as_deref().unwrap_or("blur");
    match agent_service
        .list_agents(
            ns,
            query.agent_name.as_deref(),
            search_type,
            query.page_no,
            query.page_size,
            Some(&get_username(&req)),
        )
        .await
    {
        Ok(page) => HttpResponse::Ok().json(common_response::Result::success(page)),
        Err(e) => common_response::Result::<()>::http_internal_error(e),
    }
}

/// GET /v3/console/ai/agents — Get one agent
#[get("")]
async fn get_agent(
    req: HttpRequest,
    data: web::Data<AppState>,
    agent_service: web::Data<Arc<dyn A2aAgentService>>,
    query: web::Query<AgentVersionQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/agents")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);
    match agent_service
        .get_agent_card(
            ns,
            &query.agent_name,
            query.version.as_deref(),
            Some(&get_username(&req)),
        )
        .await
    {
        Ok(Some(agent)) => HttpResponse::Ok().json(common_response::Result::success(agent)),
        Ok(None) => common_response::Result::<()>::http_not_found(
            &batata_common::error::RESOURCE_NOT_FOUND,
            format!("Agent '{}' not found", query.agent_name),
        ),
        Err(e) => common_response::Result::<()>::http_internal_error(e),
    }
}

/// GET /v3/console/ai/agents/versions — List agent versions
#[get("/versions")]
async fn list_versions(
    req: HttpRequest,
    data: web::Data<AppState>,
    agent_service: web::Data<Arc<dyn A2aAgentService>>,
    query: web::Query<AgentKeyQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/agents")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);
    match agent_service.list_versions(ns, &query.agent_name).await {
        Ok(versions) => HttpResponse::Ok().json(common_response::Result::success(versions)),
        Err(e) => common_response::Result::<()>::http_internal_error(e),
    }
}

/// GET /v3/console/ai/agents/version — Get one agent version
#[get("/version")]
async fn get_version(
    req: HttpRequest,
    data: web::Data<AppState>,
    agent_service: web::Data<Arc<dyn A2aAgentService>>,
    query: web::Query<AgentVersionQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/agents")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);
    match agent_service
        .get_agent_card(
            ns,
            &query.agent_name,
            query.version.as_deref(),
            Some(&get_username(&req)),
        )
        .await
    {
        Ok(Some(agent)) => HttpResponse::Ok().json(common_response::Result::success(agent)),
        Ok(None) => common_response::Result::<()>::http_not_found(
            &batata_common::error::RESOURCE_NOT_FOUND,
            format!("Agent '{}' version not found", query.agent_name),
        ),
        Err(e) => common_response::Result::<()>::http_internal_error(e),
    }
}

// ============================================================================
// Draft handlers
// ============================================================================

/// GET /v3/console/ai/agents/runtime-endpoints — Live endpoints of one version
///
/// Required by the console's Agent pages: nacos pins this in
/// `agent-console-source-contract.test.ts`, which asserts the next console
/// exposes `getRuntimeEndpoints` against `${BASE}/runtime-endpoints`.
#[get("/runtime-endpoints")]
async fn get_runtime_endpoints(
    req: HttpRequest,
    data: web::Data<AppState>,
    agent_service: web::Data<Arc<dyn A2aAgentService>>,
    query: web::Query<AgentRuntimeEndpointForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/agents")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let form = query.into_inner();
    let ns = normalize_namespace(&form.namespace_id);
    match agent_service
        .get_runtime_endpoints(ns, &form.agent_name, &form.protocol, &form.version)
        .await
    {
        Ok(view) => HttpResponse::Ok().json(common_response::Result::success(view)),
        Err(e) => common_response::Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// POST /v3/console/ai/agents/draft — Create a draft version
#[post("/draft")]
async fn create_draft(
    req: HttpRequest,
    data: web::Data<AppState>,
    agent_service: web::Data<Arc<dyn A2aAgentService>>,
    body: web::Form<AgentDraftCreateForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/agents")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let form = body.into_inner();
    let ns = normalize_namespace(&form.namespace_id);

    // Fields the console form does not carry are left at their defaults; the
    // draft editor fills them in.
    let card = AgentCard {
        name: form.agent_name.clone(),
        display_name: form
            .display_name
            .clone()
            .unwrap_or_else(|| form.agent_name.clone()),
        description: form.description.clone().unwrap_or_default(),
        version: form.version.clone(),
        url: String::new(),
        protocol_version: String::new(),
        capabilities: AgentCapabilities::default(),
        skills: Vec::new(),
        default_input_modes: Vec::new(),
        default_output_modes: Vec::new(),
        preferred_transport: None,
        provider: None,
        documentation_url: None,
        icon_url: form.icon_url.clone(),
        supports_authenticated_extended_card: None,
        metadata: std::collections::HashMap::new(),
        tags: Vec::new(),
        ..Default::default()
    };

    let result = agent_service.create_agent_draft(ns, &card, false).await;
    trace_write(
        &req,
        batata_common::ai_trace::RESOURCE_TYPE_AGENT,
        batata_common::ai_trace::OP_CREATE_DRAFT,
        Some(&form.agent_name),
        Some(&form.version),
        ai_trace::outcome_of(&result),
    );
    match result {
        Ok(detail) => HttpResponse::Ok().json(common_response::Result::success(detail)),
        Err(e) => common_response::Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// DELETE /v3/console/ai/agents/draft — Discard a draft version
#[delete("/draft")]
async fn delete_draft(
    req: HttpRequest,
    data: web::Data<AppState>,
    agent_service: web::Data<Arc<dyn A2aAgentService>>,
    query: web::Query<AgentVersionActionForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/agents")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);
    let result = agent_service
        .delete_agent_draft(ns, &query.agent_name, &query.version)
        .await;
    trace_write(
        &req,
        batata_common::ai_trace::RESOURCE_TYPE_AGENT,
        batata_common::ai_trace::OP_DELETE_DRAFT,
        Some(&query.agent_name),
        Some(&query.version),
        ai_trace::outcome_of(&result),
    );
    match result {
        Ok(()) => HttpResponse::Ok().json(common_response::Result::success(true)),
        Err(e) => common_response::Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

// ============================================================================
// Version lifecycle handlers
// ============================================================================

/// Build the handler for one version action. Each action differs only in the
/// service call and the trace operation.
macro_rules! version_action {
    ($name:ident, $path:literal, $method:ident, $call:ident, $op:expr) => {
        #[doc = concat!(" ", $path, " — agent version action")]
        #[$method($path)]
        async fn $name(
            req: HttpRequest,
            data: web::Data<AppState>,
            agent_service: web::Data<Arc<dyn A2aAgentService>>,
            body: web::Form<AgentVersionActionForm>,
        ) -> impl Responder {
            secured!(
                Secured::builder(&req, &data, "console/ai/agents")
                    .action(ActionTypes::Write)
                    .sign_type(SignType::Console)
                    .api_type(ApiType::ConsoleApi)
                    .build()
            );

            let form = body.into_inner();
            let ns = normalize_namespace(&form.namespace_id);
            let result = agent_service
                .$call(ns, &form.agent_name, &form.version)
                .await;
            trace_write(
                &req,
                batata_common::ai_trace::RESOURCE_TYPE_AGENT,
                $op,
                Some(&form.agent_name),
                Some(&form.version),
                ai_trace::outcome_of(&result),
            );
            match result {
                Ok(detail) => HttpResponse::Ok().json(common_response::Result::success(detail)),
                Err(e) => common_response::Result::<()>::http_bad_request(
                    &batata_common::error::PARAMETER_VALIDATE_ERROR,
                    e.to_string(),
                ),
            }
        }
    };
}

version_action!(
    submit,
    "/submit",
    post,
    submit_agent_version,
    batata_common::ai_trace::OP_SUBMIT_REVIEW
);
version_action!(
    publish,
    "/publish",
    post,
    publish_agent_version,
    batata_common::ai_trace::OP_PUBLISH
);
version_action!(
    force_publish,
    "/force-publish",
    post,
    force_publish_agent_version,
    batata_common::ai_trace::OP_FORCE_PUBLISH
);
version_action!(
    redraft,
    "/redraft",
    post,
    redraft_agent_version,
    batata_common::ai_trace::OP_REDRAFT
);
version_action!(
    online,
    "/online",
    post,
    online_agent_version,
    batata_common::ai_trace::OP_ONLINE_VERSION
);
version_action!(
    offline,
    "/offline",
    post,
    offline_agent_version,
    batata_common::ai_trace::OP_OFFLINE_VERSION
);

// ============================================================================
// Metadata handlers
// ============================================================================

/// PUT /v3/console/ai/agents/labels — Replace the label routing
#[put("/labels")]
async fn update_labels(
    req: HttpRequest,
    data: web::Data<AppState>,
    agent_service: web::Data<Arc<dyn A2aAgentService>>,
    body: web::Form<AgentLabelsForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/agents")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let form = body.into_inner();
    let ns = normalize_namespace(&form.namespace_id);
    let labels: std::collections::HashMap<String, String> = form
        .labels
        .as_deref()
        .and_then(|l| serde_json::from_str(l).ok())
        .unwrap_or_default();

    let result = agent_service
        .update_agent_labels(ns, &form.agent_name, labels)
        .await;
    trace_write(
        &req,
        batata_common::ai_trace::RESOURCE_TYPE_AGENT,
        batata_common::ai_trace::OP_UPDATE_LABELS,
        Some(&form.agent_name),
        None,
        ai_trace::outcome_of(&result),
    );
    match result {
        Ok(labels) => HttpResponse::Ok().json(common_response::Result::success(labels)),
        Err(e) => common_response::Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// PUT /v3/console/ai/agents/scope — Change the visibility scope
#[put("/scope")]
async fn update_scope(
    req: HttpRequest,
    data: web::Data<AppState>,
    agent_service: web::Data<Arc<dyn A2aAgentService>>,
    body: web::Form<AgentScopeForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/agents")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let form = body.into_inner();
    let ns = normalize_namespace(&form.namespace_id);
    let result = agent_service
        .update_agent_scope(ns, &form.agent_name, &form.scope)
        .await;
    trace_write(
        &req,
        batata_common::ai_trace::RESOURCE_TYPE_AGENT,
        batata_common::ai_trace::OP_UPDATE_SCOPE,
        Some(&form.agent_name),
        None,
        ai_trace::outcome_of(&result),
    );
    match result {
        Ok(()) => HttpResponse::Ok().json(common_response::Result::success(true)),
        Err(e) => common_response::Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// DELETE /v3/console/ai/agents — Delete an agent
#[delete("")]
async fn delete_agent(
    req: HttpRequest,
    data: web::Data<AppState>,
    agent_service: web::Data<Arc<dyn A2aAgentService>>,
    query: web::Query<AgentVersionQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/agents")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);
    let result = agent_service
        .delete_agent(ns, &query.agent_name, query.version.as_deref())
        .await;
    trace_write(
        &req,
        batata_common::ai_trace::RESOURCE_TYPE_AGENT,
        batata_common::ai_trace::OP_DELETE_RESOURCE,
        Some(&query.agent_name),
        query.version.as_deref(),
        ai_trace::outcome_of(&result),
    );
    match result {
        Ok(()) => HttpResponse::Ok().json(common_response::Result::success(true)),
        Err(e) => common_response::Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// Configure console agent routes at `/v3/console/ai/agents`
pub fn routes() -> Scope {
    // Mounted under `/v3/console`, so the `/ai` prefix belongs here.
    web::scope("/ai/agents")
        .service(list_agents)
        .service(get_agent)
        .service(list_versions)
        .service(get_version)
        .service(get_runtime_endpoints)
        .service(create_draft)
        .service(delete_draft)
        .service(submit)
        .service(publish)
        .service(force_publish)
        .service(redraft)
        .service(online)
        .service(offline)
        .service(update_labels)
        .service(update_scope)
        .service(delete_agent)
}
