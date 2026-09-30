//! Agent HTTP API handlers — Nacos 3.x compatible
//!
//! Admin: `/v3/admin/ai/agents`
//!
//! Upstream `AgentAdminController` exposes 18 endpoints. Batata implements the
//! ones that do not depend on RAD: `GET /runtime-endpoints` needs the runtime
//! endpoint registry (Naming publications + the publication capacity gate),
//! which Batata does not have. The draft endpoints are not implemented yet
//! either — the agent service creates versions directly as `online`, so there
//! is no draft to create, update or delete.

use std::collections::HashMap;
use std::sync::Arc;

use actix_web::{
    HttpMessage, HttpRequest, HttpResponse, Responder, delete, get, post, put, web,
};
use serde::Deserialize;

use batata_common::{ActionTypes, ApiType, DEFAULT_NAMESPACE_ID, SignType};
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::response::Result;
use batata_server_common::{Secured, secured};

use crate::model::AgentCard;
use crate::service::traits::A2aAgentService;

/// Query and body parameters shared by the agent version endpoints.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AgentVersionForm {
    /// Namespace ID (defaults to the public namespace).
    #[serde(alias = "namespaceId")]
    pub namespace_id: Option<String>,
    /// Agent name.
    #[serde(alias = "agentName")]
    pub agent_name: Option<String>,
    /// Version the action applies to.
    pub version: Option<String>,
    /// New scope for `PUT /scope`.
    pub scope: Option<String>,
}

/// Query parameters for the list endpoint.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AgentListForm {
    /// Namespace ID (defaults to the public namespace).
    #[serde(alias = "namespaceId")]
    pub namespace_id: Option<String>,
    /// Optional name filter.
    #[serde(alias = "agentName")]
    pub agent_name: Option<String>,
    /// `accurate` or fuzzy.
    #[serde(alias = "searchType")]
    pub search_type: Option<String>,
    /// Page number (1-based).
    #[serde(alias = "pageNo")]
    pub page_no: Option<u32>,
    /// Page size.
    #[serde(alias = "pageSize")]
    pub page_size: Option<u32>,
}

/// Query parameters for `POST /draft`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AgentDraftForm {
    /// Namespace ID (defaults to the public namespace).
    #[serde(alias = "namespaceId")]
    pub namespace_id: Option<String>,
    /// Replace the draft currently being edited.
    pub overwrite: Option<bool>,
}

/// Request body for `PUT /labels`.
#[derive(Debug, Clone, Deserialize)]
pub struct AgentLabelsForm {
    /// Label name → version.
    pub labels: HashMap<String, String>,
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
    query: web::Query<AgentDraftForm>,
    body: web::Json<AgentCard>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let ns = normalize_namespace(query.namespace_id.as_deref().unwrap_or_default());
    match service.register_agent(&body.into_inner(), ns, "manual").await {
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
    query: web::Query<AgentDraftForm>,
    body: web::Json<AgentCard>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let ns = normalize_namespace(query.namespace_id.as_deref().unwrap_or_default());
    match service
        .update_agent_card(&body.into_inner(), ns, "manual")
        .await
    {
        Ok(()) => HttpResponse::Ok().json(Result::success("ok")),
        Err(e) => Result::<()>::http_internal_error(e),
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

    match service.delete_agent(ns, name, query.version.as_deref()).await {
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
    query: web::Query<AgentDraftForm>,
    body: web::Json<AgentCard>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let ns = normalize_namespace(query.namespace_id.as_deref().unwrap_or_default());
    match service
        .create_agent_draft(ns, &body.into_inner(), query.overwrite.unwrap_or(false))
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
    query: web::Query<AgentDraftForm>,
    body: web::Json<AgentCard>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    // `AgentCard` carries no namespace, so it comes from the query like every
    // other form field.
    let ns = normalize_namespace(query.namespace_id.as_deref().unwrap_or_default());
    let card = body.into_inner();
    match service.update_agent_draft(ns, &card).await {
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
    query: web::Query<AgentVersionForm>,
    body: web::Json<AgentLabelsForm>,
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
        .update_agent_labels(ns, name, body.into_inner().labels)
        .await
    {
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
