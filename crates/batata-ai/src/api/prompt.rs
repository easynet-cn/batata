//! Prompt HTTP API handlers — Nacos 3.2 compatible
//!
//! Admin: `/v3/admin/ai/prompt`
//! Client: `/v3/client/ai/prompt`

use std::sync::Arc;

use actix_web::{HttpMessage, HttpRequest, HttpResponse, Responder, delete, get, post, put, web};
use serde::Deserialize;

use batata_common::{ActionTypes, ApiType, DEFAULT_NAMESPACE_ID, SignType};
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::response::Result;
use batata_server_common::{Secured, secured};

use crate::model::prompt::PromptVariable;
use crate::service::prompt::PromptOperationService;

// ============================================================================
// Request forms
// ============================================================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Request body for publishing a new prompt version.
pub struct PromptPublishForm {
    /// Namespace identifier.
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
    /// Prompt key (identifier).
    #[serde(alias = "promptKey")]
    pub prompt_key: String,
    /// Prompt version in `major.minor.patch` format.
    pub version: String,
    /// Prompt template content.
    #[serde(default)]
    pub template: String,
    /// Optional commit message.
    #[serde(alias = "commitMsg")]
    pub commit_msg: Option<String>,
    /// Optional description.
    pub description: Option<String>,
    /// Comma-separated biz tags
    #[serde(alias = "bizTags")]
    pub biz_tags: Option<String>,
    /// JSON array of PromptVariable
    pub variables: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Query parameters for fetching prompt metadata or version detail.
pub struct PromptQueryForm {
    /// Namespace identifier.
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
    /// Prompt key (identifier).
    #[serde(alias = "promptKey")]
    pub prompt_key: String,
    /// Optional specific version to fetch.
    pub version: Option<String>,
    /// Optional label to resolve to a version.
    pub label: Option<String>,
    /// Optional client MD5 for conditional (Not Modified) responses.
    pub md5: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Query parameters for listing prompts with pagination and filtering.
pub struct PromptListForm {
    /// Namespace identifier.
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
    /// Optional prompt key filter.
    #[serde(alias = "promptKey")]
    pub prompt_key: Option<String>,
    /// Optional free-text search keyword.
    pub search: Option<String>,
    /// Optional comma-separated biz tags filter.
    #[serde(alias = "bizTags")]
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
/// Query parameters for listing prompt versions (history).
pub struct PromptHistoryForm {
    /// Namespace identifier.
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
    /// Prompt key (identifier).
    #[serde(alias = "promptKey")]
    pub prompt_key: String,
    /// Page number (1-based).
    #[serde(default = "default_page_no", alias = "pageNo")]
    pub page_no: u64,
    /// Page size.
    #[serde(default = "default_page_size", alias = "pageSize")]
    pub page_size: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Request body for binding a label to a prompt version.
pub struct PromptLabelBindForm {
    /// Namespace identifier.
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
    /// Prompt key (identifier).
    #[serde(alias = "promptKey")]
    pub prompt_key: String,
    /// Label to bind.
    pub label: String,
    /// Target version for the label.
    pub version: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Query parameters for unbinding a prompt label.
pub struct PromptLabelForm {
    /// Namespace identifier.
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
    /// Prompt key (identifier).
    #[serde(alias = "promptKey")]
    pub prompt_key: String,
    /// Label to unbind.
    pub label: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Request body for updating prompt metadata.
pub struct PromptMetadataForm {
    /// Namespace identifier.
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
    /// Prompt key (identifier).
    #[serde(alias = "promptKey")]
    pub prompt_key: String,
    /// Optional new description.
    pub description: Option<String>,
    /// Comma-separated biz tags
    #[serde(alias = "bizTags")]
    pub biz_tags: Option<String>,
}

fn default_page_no() -> u64 {
    1
}
fn default_page_size() -> u64 {
    10
}

fn normalize_namespace(ns: &str) -> &str {
    if ns.is_empty() {
        DEFAULT_NAMESPACE_ID
    } else {
        ns
    }
}

fn parse_biz_tags(tags: Option<&str>) -> Vec<String> {
    tags.map(|t| {
        t.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    })
    .unwrap_or_default()
}

// ============================================================================
// Admin handlers — `/v3/admin/ai/prompt`
// ============================================================================

/// POST /v3/admin/ai/prompt — Publish a new prompt version
#[post("")]
async fn publish_prompt(
    req: HttpRequest,
    data: web::Data<AppState>,
    prompt_service: web::Data<Arc<PromptOperationService>>,
    body: web::Form<PromptPublishForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let form = body.into_inner();
    let ns = normalize_namespace(&form.namespace_id);
    let src_user = req
        .extensions()
        .get::<batata_common::IdentityContext>()
        .map(|ctx| ctx.username.clone())
        .unwrap_or_default();
    let src_ip = req
        .connection_info()
        .realip_remote_addr()
        .unwrap_or_default()
        .to_owned();

    let biz_tags = parse_biz_tags(form.biz_tags.as_deref());
    let variables: Option<Vec<PromptVariable>> = form
        .variables
        .as_deref()
        .and_then(|v| serde_json::from_str(v).ok());

    match prompt_service
        .publish_version(
            ns,
            &form.prompt_key,
            &form.version,
            &form.template,
            form.commit_msg.as_deref(),
            form.description.as_deref(),
            biz_tags,
            variables,
            &src_user,
            &src_ip,
        )
        .await
    {
        Ok(_) => HttpResponse::Ok().json(Result::success(true)),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// GET /v3/admin/ai/prompt/metadata — Get prompt metadata
#[get("metadata")]
async fn get_metadata(
    req: HttpRequest,
    data: web::Data<AppState>,
    prompt_service: web::Data<Arc<PromptOperationService>>,
    query: web::Query<PromptQueryForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);

    match prompt_service.get_meta(ns, &query.prompt_key).await {
        Some(meta) => HttpResponse::Ok().json(Result::success(meta)),
        None => Result::<()>::http_not_found(
            &batata_common::error::RESOURCE_NOT_FOUND,
            format!("Prompt '{}' not found", query.prompt_key),
        ),
    }
}

/// DELETE /v3/admin/ai/prompt — Delete prompt
#[delete("")]
async fn delete_prompt(
    req: HttpRequest,
    data: web::Data<AppState>,
    prompt_service: web::Data<Arc<PromptOperationService>>,
    query: web::Query<PromptQueryForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);
    let src_user = req
        .extensions()
        .get::<batata_common::IdentityContext>()
        .map(|ctx| ctx.username.clone())
        .unwrap_or_default();

    match prompt_service
        .delete_prompt(ns, &query.prompt_key, &src_user)
        .await
    {
        Ok(_) => HttpResponse::Ok().json(Result::success(true)),
        Err(e) => Result::<()>::http_internal_error(e),
    }
}

/// GET /v3/admin/ai/prompt/list — List prompts (paginated)
#[get("list")]
async fn list_prompts(
    req: HttpRequest,
    data: web::Data<AppState>,
    prompt_service: web::Data<Arc<PromptOperationService>>,
    query: web::Query<PromptListForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
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
        Ok(page) => HttpResponse::Ok().json(Result::success(page)),
        Err(e) => Result::<()>::http_internal_error(e),
    }
}

/// GET /v3/admin/ai/prompt/versions — List prompt versions
#[get("versions")]
async fn list_versions(
    req: HttpRequest,
    data: web::Data<AppState>,
    prompt_service: web::Data<Arc<PromptOperationService>>,
    query: web::Query<PromptHistoryForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);

    match prompt_service
        .list_versions(ns, &query.prompt_key, query.page_no, query.page_size)
        .await
    {
        Ok(page) => HttpResponse::Ok().json(Result::success(page)),
        Err(e) => {
            Result::<()>::http_not_found(&batata_common::error::RESOURCE_NOT_FOUND, e.to_string())
        }
    }
}

/// GET /v3/admin/ai/prompt/detail — Get prompt version detail
#[get("detail")]
async fn query_detail(
    req: HttpRequest,
    data: web::Data<AppState>,
    prompt_service: web::Data<Arc<PromptOperationService>>,
    query: web::Query<PromptQueryForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
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
        Ok(Some(info)) => HttpResponse::Ok().json(Result::success(info)),
        Ok(None) => Result::<()>::http_not_found(
            &batata_common::error::RESOURCE_NOT_FOUND,
            format!("Prompt '{}' not found", query.prompt_key),
        ),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// PUT /v3/admin/ai/prompt/label — Bind label to version
#[put("label")]
async fn bind_label(
    req: HttpRequest,
    data: web::Data<AppState>,
    prompt_service: web::Data<Arc<PromptOperationService>>,
    body: web::Form<PromptLabelBindForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let form = body.into_inner();
    let ns = normalize_namespace(&form.namespace_id);
    let src_user = req
        .extensions()
        .get::<batata_common::IdentityContext>()
        .map(|ctx| ctx.username.clone())
        .unwrap_or_default();
    let src_ip = req
        .connection_info()
        .realip_remote_addr()
        .unwrap_or_default()
        .to_owned();

    match prompt_service
        .bind_label(
            ns,
            &form.prompt_key,
            &form.label,
            &form.version,
            &src_user,
            &src_ip,
        )
        .await
    {
        Ok(_) => HttpResponse::Ok().json(Result::success(true)),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// DELETE /v3/admin/ai/prompt/label — Unbind label
#[delete("label")]
async fn unbind_label(
    req: HttpRequest,
    data: web::Data<AppState>,
    prompt_service: web::Data<Arc<PromptOperationService>>,
    query: web::Query<PromptLabelForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);
    let src_user = req
        .extensions()
        .get::<batata_common::IdentityContext>()
        .map(|ctx| ctx.username.clone())
        .unwrap_or_default();
    let src_ip = req
        .connection_info()
        .realip_remote_addr()
        .unwrap_or_default()
        .to_owned();

    match prompt_service
        .unbind_label(ns, &query.prompt_key, &query.label, &src_user, &src_ip)
        .await
    {
        Ok(_) => HttpResponse::Ok().json(Result::success(true)),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// PUT /v3/admin/ai/prompt/metadata — Update prompt metadata
#[put("metadata")]
async fn update_metadata(
    req: HttpRequest,
    data: web::Data<AppState>,
    prompt_service: web::Data<Arc<PromptOperationService>>,
    body: web::Form<PromptMetadataForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let form = body.into_inner();
    let ns = normalize_namespace(&form.namespace_id);
    let src_user = req
        .extensions()
        .get::<batata_common::IdentityContext>()
        .map(|ctx| ctx.username.clone())
        .unwrap_or_default();
    let src_ip = req
        .connection_info()
        .realip_remote_addr()
        .unwrap_or_default()
        .to_owned();

    let biz_tags = form.biz_tags.as_deref().map(|t| parse_biz_tags(Some(t)));

    match prompt_service
        .update_metadata(
            ns,
            &form.prompt_key,
            form.description.as_deref(),
            biz_tags,
            &src_user,
            &src_ip,
        )
        .await
    {
        Ok(_) => HttpResponse::Ok().json(Result::success(true)),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

// ============================================================================
// Client handler — `/v3/client/ai/prompt`
// ============================================================================

/// GET /v3/client/ai/prompt — Query prompt (with MD5 conditional support)
#[get("")]
async fn client_query_prompt(
    prompt_service: web::Data<Arc<PromptOperationService>>,
    query: web::Query<PromptQueryForm>,
) -> impl Responder {
    let ns = normalize_namespace(&query.namespace_id);

    match prompt_service
        .query_prompt(
            ns,
            &query.prompt_key,
            query.version.as_deref(),
            query.label.as_deref(),
            query.md5.as_deref(),
        )
        .await
    {
        Ok(Some(info)) => HttpResponse::Ok().json(Result::success(info.to_client_prompt())),
        Ok(None) => {
            // NOT_MODIFIED (client already has latest)
            HttpResponse::Ok().json(Result::<()>::new(304, "Not Modified".to_string(), ()))
        }
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

// ============================================================================
// Route configuration
// ============================================================================

/// Configure admin prompt routes at `/v3/admin/ai/prompt`
// ============================================================================
// Lifecycle request forms
// ============================================================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Request body for a lifecycle action on one prompt version.
pub struct PromptVersionActionForm {
    /// Namespace identifier.
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
    /// Prompt key (identifier).
    #[serde(alias = "promptKey")]
    pub prompt_key: String,
    /// Target version.
    pub version: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Request body for creating a prompt draft.
pub struct PromptDraftCreateForm {
    /// Namespace identifier.
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
    /// Prompt key (identifier).
    #[serde(alias = "promptKey")]
    pub prompt_key: String,
    /// Version to create; defaults to `0.0.1` when absent.
    #[serde(default)]
    pub version: Option<String>,
    /// Prompt template content.
    #[serde(default)]
    pub template: String,
    /// Optional description.
    pub description: Option<String>,
    /// Optional commit message.
    #[serde(alias = "commitMsg")]
    pub commit_msg: Option<String>,
    /// JSON array of PromptVariable.
    pub variables: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Request body for updating the prompt draft.
pub struct PromptDraftUpdateForm {
    /// Namespace identifier.
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
    /// Prompt key (identifier).
    #[serde(alias = "promptKey")]
    pub prompt_key: String,
    /// Prompt template content.
    #[serde(default)]
    pub template: String,
    /// Optional commit message.
    #[serde(alias = "commitMsg")]
    pub commit_msg: Option<String>,
    /// JSON array of PromptVariable.
    pub variables: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Query parameters for discarding a prompt draft.
pub struct PromptDraftForm {
    /// Namespace identifier.
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
    /// Prompt key (identifier).
    #[serde(alias = "promptKey")]
    pub prompt_key: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Request body for replacing the label routing.
pub struct PromptLabelsForm {
    /// Namespace identifier.
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
    /// Prompt key (identifier).
    #[serde(alias = "promptKey")]
    pub prompt_key: String,
    /// JSON object mapping each label to a version.
    pub labels: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Request body for updating the prompt description.
pub struct PromptDescriptionForm {
    /// Namespace identifier.
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
    /// Prompt key (identifier).
    #[serde(alias = "promptKey")]
    pub prompt_key: String,
    /// New description.
    pub description: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Request body for updating the prompt biz tags.
pub struct PromptBizTagsForm {
    /// Namespace identifier.
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
    /// Prompt key (identifier).
    #[serde(alias = "promptKey")]
    pub prompt_key: String,
    /// Comma-separated biz tags.
    #[serde(alias = "bizTags")]
    pub biz_tags: Option<String>,
}

// ============================================================================
// Lifecycle handlers
// ============================================================================

/// POST /v3/admin/ai/prompt/draft — Create a draft version
#[post("draft")]
async fn create_draft(
    req: HttpRequest,
    data: web::Data<AppState>,
    prompt_service: web::Data<Arc<PromptOperationService>>,
    body: web::Form<PromptDraftCreateForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let form = body.into_inner();
    let ns = normalize_namespace(&form.namespace_id);
    let src_user = req
        .extensions()
        .get::<batata_common::IdentityContext>()
        .map(|ctx| ctx.username.clone())
        .unwrap_or_default();
    let variables: Option<Vec<PromptVariable>> = form
        .variables
        .as_deref()
        .and_then(|v| serde_json::from_str(v).ok());

    match prompt_service
        .create_draft(
            ns,
            &form.prompt_key,
            form.version.as_deref(),
            &form.template,
            form.description.as_deref(),
            variables,
            &src_user,
        )
        .await
    {
        Ok(version) => HttpResponse::Ok().json(Result::success(version)),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// PUT /v3/admin/ai/prompt/draft — Replace the draft content
#[put("draft")]
async fn update_draft(
    req: HttpRequest,
    data: web::Data<AppState>,
    prompt_service: web::Data<Arc<PromptOperationService>>,
    body: web::Form<PromptDraftUpdateForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let form = body.into_inner();
    let ns = normalize_namespace(&form.namespace_id);
    let src_user = req
        .extensions()
        .get::<batata_common::IdentityContext>()
        .map(|ctx| ctx.username.clone())
        .unwrap_or_default();
    let variables: Option<Vec<PromptVariable>> = form
        .variables
        .as_deref()
        .and_then(|v| serde_json::from_str(v).ok());

    match prompt_service
        .update_draft(
            ns,
            &form.prompt_key,
            &form.template,
            form.commit_msg.as_deref(),
            variables,
            &src_user,
        )
        .await
    {
        Ok(version) => HttpResponse::Ok().json(Result::success(version)),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// DELETE /v3/admin/ai/prompt/draft — Discard the draft version
#[delete("draft")]
async fn delete_draft(
    req: HttpRequest,
    data: web::Data<AppState>,
    prompt_service: web::Data<Arc<PromptOperationService>>,
    query: web::Query<PromptDraftForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);
    match prompt_service.delete_draft(ns, &query.prompt_key).await {
        Ok(()) => HttpResponse::Ok().json(Result::success(true)),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// POST /v3/admin/ai/prompt/submit — Send a draft for review
#[post("submit")]
async fn submit_prompt(
    req: HttpRequest,
    data: web::Data<AppState>,
    prompt_service: web::Data<Arc<PromptOperationService>>,
    body: web::Form<PromptVersionActionForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let form = body.into_inner();
    let ns = normalize_namespace(&form.namespace_id);
    match prompt_service
        .submit(ns, &form.prompt_key, &form.version)
        .await
    {
        Ok(()) => HttpResponse::Ok().json(Result::success(true)),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// POST /v3/admin/ai/prompt/publish — Publish a reviewed version
#[post("publish")]
async fn publish_version(
    req: HttpRequest,
    data: web::Data<AppState>,
    prompt_service: web::Data<Arc<PromptOperationService>>,
    body: web::Form<PromptVersionActionForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let form = body.into_inner();
    let ns = normalize_namespace(&form.namespace_id);
    match prompt_service
        .publish(ns, &form.prompt_key, &form.version)
        .await
    {
        Ok(()) => HttpResponse::Ok().json(Result::success(true)),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// POST /v3/admin/ai/prompt/force-publish — Publish bypassing the review gate
#[post("force-publish")]
async fn force_publish_version(
    req: HttpRequest,
    data: web::Data<AppState>,
    prompt_service: web::Data<Arc<PromptOperationService>>,
    body: web::Form<PromptVersionActionForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let form = body.into_inner();
    let ns = normalize_namespace(&form.namespace_id);
    match prompt_service
        .force_publish(ns, &form.prompt_key, &form.version)
        .await
    {
        Ok(()) => HttpResponse::Ok().json(Result::success(true)),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// POST /v3/admin/ai/prompt/redraft — Move a version back to draft
#[post("redraft")]
async fn redraft_version(
    req: HttpRequest,
    data: web::Data<AppState>,
    prompt_service: web::Data<Arc<PromptOperationService>>,
    body: web::Form<PromptVersionActionForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let form = body.into_inner();
    let ns = normalize_namespace(&form.namespace_id);
    match prompt_service
        .redraft(ns, &form.prompt_key, &form.version)
        .await
    {
        Ok(()) => HttpResponse::Ok().json(Result::success(true)),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// POST /v3/admin/ai/prompt/online — Bring an offline version back online
#[post("online")]
async fn online_version(
    req: HttpRequest,
    data: web::Data<AppState>,
    prompt_service: web::Data<Arc<PromptOperationService>>,
    body: web::Form<PromptVersionActionForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let form = body.into_inner();
    let ns = normalize_namespace(&form.namespace_id);
    match prompt_service
        .online(ns, &form.prompt_key, &form.version)
        .await
    {
        Ok(()) => HttpResponse::Ok().json(Result::success(true)),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// POST /v3/admin/ai/prompt/offline — Take an online version offline
#[post("offline")]
async fn offline_version(
    req: HttpRequest,
    data: web::Data<AppState>,
    prompt_service: web::Data<Arc<PromptOperationService>>,
    body: web::Form<PromptVersionActionForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let form = body.into_inner();
    let ns = normalize_namespace(&form.namespace_id);
    match prompt_service
        .offline(ns, &form.prompt_key, &form.version)
        .await
    {
        Ok(()) => HttpResponse::Ok().json(Result::success(true)),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// PUT /v3/admin/ai/prompt/labels — Replace the label routing
#[put("labels")]
async fn update_labels(
    req: HttpRequest,
    data: web::Data<AppState>,
    prompt_service: web::Data<Arc<PromptOperationService>>,
    body: web::Form<PromptLabelsForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let form = body.into_inner();
    let ns = normalize_namespace(&form.namespace_id);
    let labels: std::collections::HashMap<String, String> = form
        .labels
        .as_deref()
        .and_then(|l| serde_json::from_str(l).ok())
        .unwrap_or_default();

    match prompt_service
        .update_labels(ns, &form.prompt_key, labels)
        .await
    {
        Ok(labels) => HttpResponse::Ok().json(Result::success(labels)),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// PUT /v3/admin/ai/prompt/description — Update the prompt description
#[put("description")]
async fn update_description(
    req: HttpRequest,
    data: web::Data<AppState>,
    prompt_service: web::Data<Arc<PromptOperationService>>,
    body: web::Form<PromptDescriptionForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let form = body.into_inner();
    let ns = normalize_namespace(&form.namespace_id);
    match prompt_service
        .update_metadata(ns, &form.prompt_key, Some(&form.description), None, "", "")
        .await
    {
        Ok(_) => HttpResponse::Ok().json(Result::success(true)),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// PUT /v3/admin/ai/prompt/biz-tags — Update the prompt biz tags
#[put("biz-tags")]
async fn update_biz_tags(
    req: HttpRequest,
    data: web::Data<AppState>,
    prompt_service: web::Data<Arc<PromptOperationService>>,
    body: web::Form<PromptBizTagsForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let form = body.into_inner();
    let ns = normalize_namespace(&form.namespace_id);
    let tags = parse_biz_tags(form.biz_tags.as_deref());
    match prompt_service
        .update_metadata(ns, &form.prompt_key, None, Some(tags), "", "")
        .await
    {
        Ok(_) => HttpResponse::Ok().json(Result::success(true)),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// Configure admin prompt routes at `/v3/admin/ai/prompt`
pub fn admin_routes() -> actix_web::Scope {
    web::scope("/prompt")
        .service(publish_prompt)
        .service(get_metadata)
        .service(list_prompts)
        .service(list_versions)
        .service(query_detail)
        .service(bind_label)
        .service(unbind_label)
        .service(update_metadata)
        .service(delete_prompt)
        .service(create_draft)
        .service(update_draft)
        .service(delete_draft)
        .service(submit_prompt)
        .service(publish_version)
        .service(force_publish_version)
        .service(redraft_version)
        .service(online_version)
        .service(offline_version)
        .service(update_labels)
        .service(update_description)
        .service(update_biz_tags)
}

/// Configure client prompt routes at `/v3/client/ai/prompt`
pub fn client_routes() -> actix_web::Scope {
    web::scope("/prompt").service(client_query_prompt)
}
