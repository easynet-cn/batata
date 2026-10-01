//! Console Prompt management API endpoints.
//!
//! Mirrors upstream `ConsolePromptController` under `/v3/console/ai/prompt`.
//! The Nacos console UI reads with query parameters and writes with
//! form-encoded bodies — including every lifecycle action — so the handlers
//! follow that split rather than using JSON throughout.

use std::sync::Arc;

use actix_web::{HttpRequest, HttpResponse, Responder, Scope, delete, get, post, put, web};
use serde::Deserialize;

use batata_common::DEFAULT_NAMESPACE_ID;
use batata_common::PromptService;
use batata_common::model::ai::prompt::*;
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
/// Query for listing prompts.
pub struct PromptListQuery {
    /// Optional prompt key filter.
    #[serde(default, alias = "promptKey")]
    pub prompt_key: Option<String>,
    /// Namespace identifier.
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
    /// `accurate` or `blur` matching.
    #[serde(default)]
    pub search: Option<String>,
    /// Comma-separated biz tags filter.
    #[serde(default, alias = "bizTags")]
    pub biz_tags: Option<String>,
    /// Page number (1-based).
    #[serde(default = "default_page_no", alias = "pageNo")]
    pub page_no: u64,
    /// Page size.
    #[serde(default = "default_page_size", alias = "pageSize")]
    pub page_size: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Query identifying one prompt.
pub struct PromptKeyQuery {
    /// Prompt key (identifier).
    #[serde(alias = "promptKey")]
    pub prompt_key: String,
    /// Namespace identifier.
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Query for one version, addressed by version or by label.
pub struct PromptVersionQuery {
    /// Prompt key (identifier).
    #[serde(alias = "promptKey")]
    pub prompt_key: String,
    /// Version in `major.minor.patch` format.
    #[serde(default)]
    pub version: Option<String>,
    /// Label resolving to a version.
    #[serde(default)]
    pub label: Option<String>,
    /// Namespace identifier.
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Query for the version history.
pub struct PromptVersionsQuery {
    /// Prompt key (identifier).
    #[serde(alias = "promptKey")]
    pub prompt_key: String,
    /// Namespace identifier.
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
    /// Page number (1-based).
    #[serde(default = "default_page_no", alias = "pageNo")]
    pub page_no: u64,
    /// Page size.
    #[serde(default = "default_page_size", alias = "pageSize")]
    pub page_size: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Form for creating a draft.
pub struct PromptDraftCreateForm {
    /// Prompt key (identifier).
    #[serde(alias = "promptKey")]
    pub prompt_key: String,
    /// Prompt template content.
    #[serde(default)]
    pub template: String,
    /// JSON array of PromptVariable.
    #[serde(default)]
    pub variables: Option<String>,
    /// Optional commit message.
    #[serde(default, alias = "commitMsg")]
    pub commit_msg: Option<String>,
    /// Optional description.
    #[serde(default)]
    pub description: Option<String>,
    /// Comma-separated biz tags.
    #[serde(default, alias = "bizTags")]
    pub biz_tags: Option<String>,
    /// Version to create; defaults to `0.0.1` when absent.
    #[serde(default, alias = "targetVersion")]
    pub target_version: Option<String>,
    /// Namespace identifier.
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Form for updating the draft.
pub struct PromptDraftUpdateForm {
    /// Prompt key (identifier).
    #[serde(alias = "promptKey")]
    pub prompt_key: String,
    /// Prompt template content.
    #[serde(default)]
    pub template: String,
    /// JSON array of PromptVariable.
    #[serde(default)]
    pub variables: Option<String>,
    /// Optional commit message.
    #[serde(default, alias = "commitMsg")]
    pub commit_msg: Option<String>,
    /// Namespace identifier.
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Form for submitting a version for review. The version is optional: omitting
/// it submits the draft, which is how upstream's submit behaves.
pub struct PromptSubmitForm {
    /// Prompt key (identifier).
    #[serde(alias = "promptKey")]
    pub prompt_key: String,
    /// Version to submit; the draft when absent.
    #[serde(default)]
    pub version: Option<String>,
    /// Namespace identifier.
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Form for an action on one version.
pub struct PromptVersionActionForm {
    /// Prompt key (identifier).
    #[serde(alias = "promptKey")]
    pub prompt_key: String,
    /// Target version.
    #[serde(default)]
    pub version: String,
    /// Namespace identifier.
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Form for replacing the label routing. `labels` carries a JSON object.
pub struct PromptLabelsForm {
    /// Prompt key (identifier).
    #[serde(alias = "promptKey")]
    pub prompt_key: String,
    /// JSON object mapping each label to a version.
    #[serde(default)]
    pub labels: Option<String>,
    /// Namespace identifier.
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Form for updating the description.
pub struct PromptDescriptionForm {
    /// Prompt key (identifier).
    #[serde(alias = "promptKey")]
    pub prompt_key: String,
    /// New description.
    #[serde(default)]
    pub description: String,
    /// Namespace identifier.
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Form for updating the biz tags.
pub struct PromptBizTagsForm {
    /// Prompt key (identifier).
    #[serde(alias = "promptKey")]
    pub prompt_key: String,
    /// Comma-separated biz tags.
    #[serde(default, alias = "bizTags")]
    pub biz_tags: Option<String>,
    /// Namespace identifier.
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
}

fn default_page_no() -> u64 {
    1
}

fn default_page_size() -> u64 {
    10
}

fn parse_biz_tags(raw: Option<&str>) -> Vec<String> {
    raw.unwrap_or_default()
        .split(',')
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect()
}

// ============================================================================
// Read handlers
// ============================================================================

/// GET /v3/console/ai/prompt/list — List prompts
#[get("/list")]
async fn list_prompts(
    req: HttpRequest,
    data: web::Data<AppState>,
    prompt_service: web::Data<Arc<dyn PromptService>>,
    query: web::Query<PromptListQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/prompt")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);
    match prompt_service
        .list_prompts(
            ns,
            query.prompt_key.as_deref(),
            query.search.as_deref(),
            query.biz_tags.as_deref(),
            query.page_no,
            query.page_size,
        )
        .await
    {
        Ok(page) => HttpResponse::Ok().json(common_response::Result::success(page)),
        Err(e) => common_response::Result::<()>::http_internal_error(e),
    }
}

/// GET /v3/console/ai/prompt/governance — Governance detail
#[get("/governance")]
async fn get_governance(
    req: HttpRequest,
    data: web::Data<AppState>,
    prompt_service: web::Data<Arc<dyn PromptService>>,
    query: web::Query<PromptKeyQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/prompt")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);
    match prompt_service.get_governance(ns, &query.prompt_key).await {
        Ok(Some(meta)) => HttpResponse::Ok().json(common_response::Result::success(meta)),
        Ok(None) => common_response::Result::<()>::http_not_found(
            &batata_common::error::RESOURCE_NOT_FOUND,
            format!("Prompt '{}' not found", query.prompt_key),
        ),
        Err(e) => common_response::Result::<()>::http_internal_error(e),
    }
}

/// GET /v3/console/ai/prompt/version — One version's content
#[get("/version")]
async fn get_version(
    req: HttpRequest,
    data: web::Data<AppState>,
    prompt_service: web::Data<Arc<dyn PromptService>>,
    query: web::Query<PromptVersionQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/prompt")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);
    match prompt_service
        .query_detail(
            ns,
            &query.prompt_key,
            query.version.as_deref(),
            query.label.as_deref(),
        )
        .await
    {
        Ok(Some(info)) => HttpResponse::Ok().json(common_response::Result::success(info)),
        Ok(None) => common_response::Result::<()>::http_not_found(
            &batata_common::error::RESOURCE_NOT_FOUND,
            format!("Prompt '{}' version not found", query.prompt_key),
        ),
        Err(e) => common_response::Result::<()>::http_internal_error(e),
    }
}

/// GET /v3/console/ai/prompt/versions — Version history
#[get("/versions")]
async fn list_versions(
    req: HttpRequest,
    data: web::Data<AppState>,
    prompt_service: web::Data<Arc<dyn PromptService>>,
    query: web::Query<PromptVersionsQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/prompt")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);
    match prompt_service
        .list_versions(ns, &query.prompt_key, query.page_no, query.page_size)
        .await
    {
        Ok(page) => HttpResponse::Ok().json(common_response::Result::success(page)),
        Err(e) => common_response::Result::<()>::http_internal_error(e),
    }
}

/// GET /v3/console/ai/prompt/version/download — Download a version as Markdown
#[get("/version/download")]
async fn download_version(
    req: HttpRequest,
    data: web::Data<AppState>,
    prompt_service: web::Data<Arc<dyn PromptService>>,
    query: web::Query<PromptVersionQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/prompt")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);
    match prompt_service
        .query_detail(
            ns,
            &query.prompt_key,
            query.version.as_deref(),
            query.label.as_deref(),
        )
        .await
    {
        Ok(Some(info)) => HttpResponse::Ok()
            .content_type("text/markdown; charset=utf-8")
            .insert_header((
                "content-disposition",
                format!(
                    "attachment; filename=\"{}-{}.md\"",
                    info.prompt_key, info.version
                ),
            ))
            .body(info.template),
        Ok(None) => common_response::Result::<()>::http_not_found(
            &batata_common::error::RESOURCE_NOT_FOUND,
            format!("Prompt '{}' version not found", query.prompt_key),
        ),
        Err(e) => common_response::Result::<()>::http_internal_error(e),
    }
}

// ============================================================================
// Lifecycle handlers
// ============================================================================

/// POST /v3/console/ai/prompt/draft — Create a draft
#[post("/draft")]
async fn create_draft(
    req: HttpRequest,
    data: web::Data<AppState>,
    prompt_service: web::Data<Arc<dyn PromptService>>,
    body: web::Form<PromptDraftCreateForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/prompt")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let form = body.into_inner();
    let ns = normalize_namespace(&form.namespace_id);
    let variables: Option<Vec<PromptVariable>> = form
        .variables
        .as_deref()
        .and_then(|v| serde_json::from_str(v).ok());

    let result = prompt_service
        .create_draft(
            ns,
            &form.prompt_key,
            form.target_version.as_deref(),
            &form.template,
            form.description.as_deref(),
            variables,
            &get_username(&req),
        )
        .await;
    trace_write(
        &req,
        batata_common::ai_trace::RESOURCE_TYPE_PROMPT,
        batata_common::ai_trace::OP_CREATE_DRAFT,
        Some(&form.prompt_key),
        form.target_version.as_deref(),
        ai_trace::outcome_of(&result),
    );
    match result {
        Ok(version) => HttpResponse::Ok().json(common_response::Result::success(version)),
        Err(e) => common_response::Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// PUT /v3/console/ai/prompt/draft — Replace the draft content
#[put("/draft")]
async fn update_draft(
    req: HttpRequest,
    data: web::Data<AppState>,
    prompt_service: web::Data<Arc<dyn PromptService>>,
    body: web::Form<PromptDraftUpdateForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/prompt")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let form = body.into_inner();
    let ns = normalize_namespace(&form.namespace_id);
    let variables: Option<Vec<PromptVariable>> = form
        .variables
        .as_deref()
        .and_then(|v| serde_json::from_str(v).ok());

    let result = prompt_service
        .update_draft(
            ns,
            &form.prompt_key,
            &form.template,
            form.commit_msg.as_deref(),
            variables,
            &get_username(&req),
        )
        .await;
    trace_write(
        &req,
        batata_common::ai_trace::RESOURCE_TYPE_PROMPT,
        batata_common::ai_trace::OP_UPDATE_DRAFT,
        Some(&form.prompt_key),
        None,
        ai_trace::outcome_of(&result),
    );
    match result {
        Ok(version) => HttpResponse::Ok().json(common_response::Result::success(version)),
        Err(e) => common_response::Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// DELETE /v3/console/ai/prompt/draft — Discard the draft
#[delete("/draft")]
async fn delete_draft(
    req: HttpRequest,
    data: web::Data<AppState>,
    prompt_service: web::Data<Arc<dyn PromptService>>,
    query: web::Query<PromptKeyQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/prompt")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);
    let result = prompt_service.delete_draft(ns, &query.prompt_key).await;
    trace_write(
        &req,
        batata_common::ai_trace::RESOURCE_TYPE_PROMPT,
        batata_common::ai_trace::OP_DELETE_DRAFT,
        Some(&query.prompt_key),
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

/// POST /v3/console/ai/prompt/submit — Submit for review.
/// Omitting the version submits the draft, as upstream allows.
#[post("/submit")]
async fn submit(
    req: HttpRequest,
    data: web::Data<AppState>,
    prompt_service: web::Data<Arc<dyn PromptService>>,
    body: web::Form<PromptSubmitForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/prompt")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let form = body.into_inner();
    let ns = normalize_namespace(&form.namespace_id);

    let result = match form.version.as_deref().filter(|v| !v.is_empty()) {
        Some(version) => prompt_service
            .submit(ns, &form.prompt_key, version)
            .await
            .map(|()| version.to_string()),
        None => prompt_service.submit_draft(ns, &form.prompt_key).await,
    };
    trace_write(
        &req,
        batata_common::ai_trace::RESOURCE_TYPE_PROMPT,
        batata_common::ai_trace::OP_SUBMIT_REVIEW,
        Some(&form.prompt_key),
        form.version.as_deref(),
        ai_trace::outcome_of(&result),
    );
    match result {
        Ok(version) => HttpResponse::Ok().json(common_response::Result::success(version)),
        Err(e) => common_response::Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// POST /v3/console/ai/prompt/publish — Publish a reviewed version
#[post("/publish")]
async fn publish(
    req: HttpRequest,
    data: web::Data<AppState>,
    prompt_service: web::Data<Arc<dyn PromptService>>,
    body: web::Form<PromptVersionActionForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/prompt")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let form = body.into_inner();
    let ns = normalize_namespace(&form.namespace_id);
    let result = prompt_service
        .publish(ns, &form.prompt_key, &form.version)
        .await;
    trace_write(
        &req,
        batata_common::ai_trace::RESOURCE_TYPE_PROMPT,
        batata_common::ai_trace::OP_PUBLISH,
        Some(&form.prompt_key),
        Some(&form.version),
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

/// POST /v3/console/ai/prompt/force-publish — Publish bypassing the review gate
#[post("/force-publish")]
async fn force_publish(
    req: HttpRequest,
    data: web::Data<AppState>,
    prompt_service: web::Data<Arc<dyn PromptService>>,
    body: web::Form<PromptVersionActionForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/prompt")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let form = body.into_inner();
    let ns = normalize_namespace(&form.namespace_id);
    let result = prompt_service
        .force_publish(ns, &form.prompt_key, &form.version)
        .await;
    trace_write(
        &req,
        batata_common::ai_trace::RESOURCE_TYPE_PROMPT,
        batata_common::ai_trace::OP_FORCE_PUBLISH,
        Some(&form.prompt_key),
        Some(&form.version),
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

/// POST /v3/console/ai/prompt/redraft — Move a version back to draft
#[post("/redraft")]
async fn redraft(
    req: HttpRequest,
    data: web::Data<AppState>,
    prompt_service: web::Data<Arc<dyn PromptService>>,
    body: web::Form<PromptVersionActionForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/prompt")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let form = body.into_inner();
    let ns = normalize_namespace(&form.namespace_id);
    let result = prompt_service
        .redraft(ns, &form.prompt_key, &form.version)
        .await;
    trace_write(
        &req,
        batata_common::ai_trace::RESOURCE_TYPE_PROMPT,
        batata_common::ai_trace::OP_REDRAFT,
        Some(&form.prompt_key),
        Some(&form.version),
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

/// POST /v3/console/ai/prompt/online — Bring a version online
#[post("/online")]
async fn online(
    req: HttpRequest,
    data: web::Data<AppState>,
    prompt_service: web::Data<Arc<dyn PromptService>>,
    body: web::Form<PromptVersionActionForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/prompt")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let form = body.into_inner();
    let ns = normalize_namespace(&form.namespace_id);
    let result = prompt_service
        .online(ns, &form.prompt_key, &form.version)
        .await;
    trace_write(
        &req,
        batata_common::ai_trace::RESOURCE_TYPE_PROMPT,
        batata_common::ai_trace::OP_ONLINE_VERSION,
        Some(&form.prompt_key),
        Some(&form.version),
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

/// POST /v3/console/ai/prompt/offline — Take a version offline
#[post("/offline")]
async fn offline(
    req: HttpRequest,
    data: web::Data<AppState>,
    prompt_service: web::Data<Arc<dyn PromptService>>,
    body: web::Form<PromptVersionActionForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/prompt")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let form = body.into_inner();
    let ns = normalize_namespace(&form.namespace_id);
    let result = prompt_service
        .offline(ns, &form.prompt_key, &form.version)
        .await;
    trace_write(
        &req,
        batata_common::ai_trace::RESOURCE_TYPE_PROMPT,
        batata_common::ai_trace::OP_OFFLINE_VERSION,
        Some(&form.prompt_key),
        Some(&form.version),
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
// Metadata handlers
// ============================================================================

/// PUT /v3/console/ai/prompt/labels — Replace the label routing
#[put("/labels")]
async fn update_labels(
    req: HttpRequest,
    data: web::Data<AppState>,
    prompt_service: web::Data<Arc<dyn PromptService>>,
    body: web::Form<PromptLabelsForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/prompt")
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

    let result = prompt_service
        .update_labels(ns, &form.prompt_key, labels)
        .await;
    trace_write(
        &req,
        batata_common::ai_trace::RESOURCE_TYPE_PROMPT,
        batata_common::ai_trace::OP_UPDATE_LABELS,
        Some(&form.prompt_key),
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

/// PUT /v3/console/ai/prompt/description — Update the description
#[put("/description")]
async fn update_description(
    req: HttpRequest,
    data: web::Data<AppState>,
    prompt_service: web::Data<Arc<dyn PromptService>>,
    body: web::Form<PromptDescriptionForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/prompt")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let form = body.into_inner();
    let ns = normalize_namespace(&form.namespace_id);
    let result = prompt_service
        .update_metadata(ns, &form.prompt_key, Some(&form.description), None)
        .await;
    trace_write(
        &req,
        batata_common::ai_trace::RESOURCE_TYPE_PROMPT,
        batata_common::ai_trace::OP_UPDATE_DESCRIPTION,
        Some(&form.prompt_key),
        None,
        ai_trace::outcome_of(&result),
    );
    match result {
        Ok(_) => HttpResponse::Ok().json(common_response::Result::success(true)),
        Err(e) => common_response::Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// PUT /v3/console/ai/prompt/biz-tags — Update the biz tags
#[put("/biz-tags")]
async fn update_biz_tags(
    req: HttpRequest,
    data: web::Data<AppState>,
    prompt_service: web::Data<Arc<dyn PromptService>>,
    body: web::Form<PromptBizTagsForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/prompt")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let form = body.into_inner();
    let ns = normalize_namespace(&form.namespace_id);
    let tags = parse_biz_tags(form.biz_tags.as_deref());
    let result = prompt_service
        .update_metadata(ns, &form.prompt_key, None, Some(tags))
        .await;
    trace_write(
        &req,
        batata_common::ai_trace::RESOURCE_TYPE_PROMPT,
        batata_common::ai_trace::OP_UPDATE_BIZ_TAGS,
        Some(&form.prompt_key),
        None,
        ai_trace::outcome_of(&result),
    );
    match result {
        Ok(_) => HttpResponse::Ok().json(common_response::Result::success(true)),
        Err(e) => common_response::Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// DELETE /v3/console/ai/prompt — Delete a prompt and all its versions
#[delete("")]
async fn delete_prompt(
    req: HttpRequest,
    data: web::Data<AppState>,
    prompt_service: web::Data<Arc<dyn PromptService>>,
    query: web::Query<PromptKeyQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/prompt")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);
    let result = prompt_service
        .delete_prompt(ns, &query.prompt_key, &get_username(&req))
        .await;
    trace_write(
        &req,
        batata_common::ai_trace::RESOURCE_TYPE_PROMPT,
        batata_common::ai_trace::OP_DELETE_RESOURCE,
        Some(&query.prompt_key),
        None,
        ai_trace::outcome_of(&result),
    );
    match result {
        Ok(_) => HttpResponse::Ok().json(common_response::Result::success(true)),
        Err(e) => common_response::Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// Configure console prompt routes at `/v3/console/ai/prompt`
pub fn routes() -> Scope {
    // Mounted under `/v3/console`, so the `/ai` prefix belongs here.
    web::scope("/ai/prompt")
        .service(list_prompts)
        .service(get_governance)
        .service(get_version)
        .service(list_versions)
        .service(download_version)
        .service(create_draft)
        .service(update_draft)
        .service(delete_draft)
        .service(submit)
        .service(publish)
        .service(force_publish)
        .service(redraft)
        .service(online)
        .service(offline)
        .service(update_labels)
        .service(update_description)
        .service(update_biz_tags)
        .service(delete_prompt)
}
