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
pub struct PromptPublishForm {
    #[serde(default)]
    pub namespace_id: String,
    pub prompt_key: String,
    pub version: String,
    #[serde(default)]
    pub template: String,
    pub commit_msg: Option<String>,
    pub description: Option<String>,
    /// Comma-separated biz tags
    pub biz_tags: Option<String>,
    /// JSON array of PromptVariable
    pub variables: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptQueryForm {
    #[serde(default)]
    pub namespace_id: String,
    pub prompt_key: String,
    pub version: Option<String>,
    pub label: Option<String>,
    pub md5: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptListForm {
    #[serde(default)]
    pub namespace_id: String,
    pub prompt_key: Option<String>,
    pub search: Option<String>,
    pub biz_tags: Option<String>,
    #[serde(default = "default_page_no")]
    pub page_no: u64,
    #[serde(default = "default_page_size")]
    pub page_size: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptHistoryForm {
    #[serde(default)]
    pub namespace_id: String,
    pub prompt_key: String,
    #[serde(default = "default_page_no")]
    pub page_no: u64,
    #[serde(default = "default_page_size")]
    pub page_size: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptLabelBindForm {
    #[serde(default)]
    pub namespace_id: String,
    pub prompt_key: String,
    pub label: String,
    pub version: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptLabelForm {
    #[serde(default)]
    pub namespace_id: String,
    pub prompt_key: String,
    pub label: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptMetadataForm {
    #[serde(default)]
    pub namespace_id: String,
    pub prompt_key: String,
    pub description: Option<String>,
    /// Comma-separated biz tags
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
    body: web::Json<PromptPublishForm>,
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
    _prompt_service: web::Data<Arc<PromptOperationService>>,
    _query: web::Query<PromptListForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    // TODO: Implement list with search/filter via config search
    // Requires iterating descriptor configs to build summaries
    HttpResponse::Ok().json(Result::success(batata_persistence::model::Page::<
        crate::model::prompt::PromptMetaSummary,
    > {
        total_count: 0,
        page_number: 1,
        pages_available: 0,
        page_items: vec![],
    }))
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
    body: web::Json<PromptLabelBindForm>,
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
    body: web::Json<PromptMetadataForm>,
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
}

/// Configure client prompt routes at `/v3/client/ai/prompt`
pub fn client_routes() -> actix_web::Scope {
    web::scope("/prompt").service(client_query_prompt)
}
