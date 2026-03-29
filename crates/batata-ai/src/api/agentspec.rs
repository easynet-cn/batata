//! AgentSpec HTTP API handlers — Nacos 3.x compatible
//!
//! Admin: `/v3/admin/ai/agentspecs` (15 endpoints)
//! Client: `/v3/client/ai/agentspecs` (2 endpoints)

use std::sync::Arc;

use actix_web::{HttpMessage, HttpRequest, HttpResponse, Responder, delete, get, post, put, web};

use batata_common::{ActionTypes, ApiType, DEFAULT_NAMESPACE_ID, SignType};
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::response::Result;
use batata_server_common::{Secured, secured};

use crate::model::agentspec::*;
use crate::service::agentspec_service::AgentSpecOperationService;

fn normalize_namespace(ns: &str) -> &str {
    if ns.is_empty() {
        DEFAULT_NAMESPACE_ID
    } else {
        ns
    }
}

fn get_username(req: &HttpRequest) -> String {
    req.extensions()
        .get::<batata_common::IdentityContext>()
        .map(|ctx| ctx.username.clone())
        .unwrap_or_default()
}

// ============================================================================
// Admin handlers — `/v3/admin/ai/agentspecs`
// ============================================================================

/// GET /v3/admin/ai/agentspecs — Get agentspec detail (governance + all versions)
#[get("")]
async fn get_agentspec_detail(
    req: HttpRequest,
    data: web::Data<AppState>,
    agentspec_service: web::Data<Arc<AgentSpecOperationService>>,
    query: web::Query<AgentSpecForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);
    let name = match query.agent_spec_name.as_deref() {
        Some(n) if !n.is_empty() => n,
        _ => {
            return Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_MISSING,
                "agentSpecName is required",
            );
        }
    };

    match agentspec_service.get_detail(ns, name).await {
        Ok(Some(meta)) => HttpResponse::Ok().json(Result::success(meta)),
        Ok(None) => Result::<()>::http_not_found(
            &batata_common::error::RESOURCE_NOT_FOUND,
            format!("AgentSpec '{}' not found", name),
        ),
        Err(e) => Result::<()>::http_internal_error(e),
    }
}

/// GET /v3/admin/ai/agentspecs/version — Get specific version detail
#[get("version")]
async fn get_agentspec_version(
    req: HttpRequest,
    data: web::Data<AppState>,
    agentspec_service: web::Data<Arc<AgentSpecOperationService>>,
    query: web::Query<AgentSpecForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);
    let name = match query.agent_spec_name.as_deref() {
        Some(n) if !n.is_empty() => n,
        _ => {
            return Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_MISSING,
                "agentSpecName is required",
            );
        }
    };
    let version = match query.version.as_deref() {
        Some(v) if !v.is_empty() => v,
        _ => {
            return Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_MISSING,
                "version is required",
            );
        }
    };

    match agentspec_service
        .get_version_detail(ns, name, version)
        .await
    {
        Ok(Some(spec)) => HttpResponse::Ok().json(Result::success(spec)),
        Ok(None) => Result::<()>::http_not_found(
            &batata_common::error::RESOURCE_NOT_FOUND,
            format!("AgentSpec '{}' version '{}' not found", name, version),
        ),
        Err(e) => Result::<()>::http_internal_error(e),
    }
}

/// GET /v3/admin/ai/agentspecs/version/download — Download agentspec version as JSON
#[get("version/download")]
async fn download_agentspec_version(
    req: HttpRequest,
    data: web::Data<AppState>,
    agentspec_service: web::Data<Arc<AgentSpecOperationService>>,
    query: web::Query<AgentSpecForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);
    let name = match query.agent_spec_name.as_deref() {
        Some(n) if !n.is_empty() => n,
        _ => {
            return Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_MISSING,
                "agentSpecName is required",
            );
        }
    };
    let version = match query.version.as_deref() {
        Some(v) if !v.is_empty() => v,
        _ => {
            return Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_MISSING,
                "version is required",
            );
        }
    };

    match agentspec_service
        .get_version_detail(ns, name, version)
        .await
    {
        Ok(Some(spec)) => match serde_json::to_vec_pretty(&spec) {
            Ok(json_bytes) => HttpResponse::Ok()
                .content_type("application/json")
                .insert_header((
                    "Content-Disposition",
                    format!("attachment; filename=\"{}-{}.json\"", name, version),
                ))
                .body(json_bytes),
            Err(e) => Result::<()>::http_internal_error(anyhow::anyhow!(
                "JSON serialization error: {}",
                e
            )),
        },
        Ok(None) => Result::<()>::http_not_found(
            &batata_common::error::RESOURCE_NOT_FOUND,
            format!("AgentSpec '{}' version '{}' not found", name, version),
        ),
        Err(e) => Result::<()>::http_internal_error(e),
    }
}

/// DELETE /v3/admin/ai/agentspecs — Delete agentspec
#[delete("")]
async fn delete_agentspec(
    req: HttpRequest,
    data: web::Data<AppState>,
    agentspec_service: web::Data<Arc<AgentSpecOperationService>>,
    query: web::Query<AgentSpecForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);
    let name = match query.agent_spec_name.as_deref() {
        Some(n) if !n.is_empty() => n,
        _ => {
            return Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_MISSING,
                "agentSpecName is required",
            );
        }
    };

    match agentspec_service.delete(ns, name).await {
        Ok(()) => HttpResponse::Ok().json(Result::success(true)),
        Err(e) => Result::<()>::http_internal_error(e),
    }
}

/// GET /v3/admin/ai/agentspecs/list — List agentspecs with pagination
#[get("list")]
async fn list_agentspecs(
    req: HttpRequest,
    data: web::Data<AppState>,
    agentspec_service: web::Data<Arc<AgentSpecOperationService>>,
    query: web::Query<AgentSpecListForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);

    match agentspec_service
        .list(
            ns,
            query.agent_spec_name.as_deref(),
            query.search.as_deref(),
            query.page_no,
            query.page_size,
        )
        .await
    {
        Ok(page) => HttpResponse::Ok().json(Result::success(page)),
        Err(e) => Result::<()>::http_internal_error(e),
    }
}

/// POST /v3/admin/ai/agentspecs/upload — Upload agentspec from JSON body
/// Params: namespaceId (query, optional), overwrite (query, optional, default=false)
/// Body: JSON with agentSpecName, agentSpecCard fields
#[post("upload")]
async fn upload_agentspec(
    req: HttpRequest,
    data: web::Data<AppState>,
    agentspec_service: web::Data<Arc<AgentSpecOperationService>>,
    query: web::Query<AgentSpecUploadQuery>,
    body: web::Json<AgentSpecUploadForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);
    let overwrite = query.overwrite;
    let author = get_username(&req);

    let form = body.into_inner();
    let name = match form.agent_spec_name.as_deref() {
        Some(n) if !n.is_empty() => n.to_string(),
        _ => {
            return Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_MISSING,
                "agentSpecName is required",
            );
        }
    };

    let spec: AgentSpec = match form
        .agent_spec_card
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
    {
        Ok(Some(s)) => s,
        Ok(None) => {
            return Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_MISSING,
                "agentSpecCard is required",
            );
        }
        Err(e) => {
            return Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_VALIDATE_ERROR,
                format!("Invalid agentSpecCard JSON: {}", e),
            );
        }
    };

    match agentspec_service
        .upload(ns, &name, &spec, &author, overwrite)
        .await
    {
        Ok(spec_name) => HttpResponse::Ok().json(Result::success(spec_name)),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// POST /v3/admin/ai/agentspecs/draft — Create draft
#[post("draft")]
async fn create_draft(
    req: HttpRequest,
    data: web::Data<AppState>,
    agentspec_service: web::Data<Arc<AgentSpecOperationService>>,
    body: web::Json<AgentSpecDraftCreateForm>,
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
    let author = get_username(&req);

    let initial_content: Option<AgentSpec> = form
        .agent_spec_card
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok());

    match agentspec_service
        .create_draft(
            ns,
            &form.agent_spec_name,
            form.based_on_version.as_deref(),
            form.target_version.as_deref(),
            initial_content.as_ref(),
            &author,
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

/// PUT /v3/admin/ai/agentspecs/draft — Update draft
#[put("draft")]
async fn update_draft(
    req: HttpRequest,
    data: web::Data<AppState>,
    agentspec_service: web::Data<Arc<AgentSpecOperationService>>,
    body: web::Json<AgentSpecUpdateForm>,
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

    let spec: AgentSpec = match form
        .agent_spec_card
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
    {
        Ok(Some(s)) => s,
        Ok(None) => {
            return Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_MISSING,
                "agentSpecCard is required",
            );
        }
        Err(e) => {
            return Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_VALIDATE_ERROR,
                format!("Invalid agentSpecCard JSON: {}", e),
            );
        }
    };

    match agentspec_service
        .update_draft(ns, &form.agent_spec_name, &spec)
        .await
    {
        Ok(()) => HttpResponse::Ok().json(Result::success(true)),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// DELETE /v3/admin/ai/agentspecs/draft — Delete draft
#[delete("draft")]
async fn delete_draft(
    req: HttpRequest,
    data: web::Data<AppState>,
    agentspec_service: web::Data<Arc<AgentSpecOperationService>>,
    query: web::Query<AgentSpecForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);
    let name = match query.agent_spec_name.as_deref() {
        Some(n) if !n.is_empty() => n,
        _ => {
            return Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_MISSING,
                "agentSpecName is required",
            );
        }
    };

    match agentspec_service.delete_draft(ns, name).await {
        Ok(()) => HttpResponse::Ok().json(Result::success(true)),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// POST /v3/admin/ai/agentspecs/submit — Submit version for review
#[post("submit")]
async fn submit_agentspec(
    req: HttpRequest,
    data: web::Data<AppState>,
    agentspec_service: web::Data<Arc<AgentSpecOperationService>>,
    body: web::Json<AgentSpecSubmitForm>,
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

    match agentspec_service
        .submit(ns, &form.agent_spec_name, &form.version)
        .await
    {
        Ok(version) => HttpResponse::Ok().json(Result::success(version)),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// POST /v3/admin/ai/agentspecs/publish — Publish approved version
#[post("publish")]
async fn publish_agentspec(
    req: HttpRequest,
    data: web::Data<AppState>,
    agentspec_service: web::Data<Arc<AgentSpecOperationService>>,
    body: web::Json<AgentSpecPublishForm>,
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

    match agentspec_service
        .publish(
            ns,
            &form.agent_spec_name,
            &form.version,
            form.update_latest_label,
        )
        .await
    {
        Ok(()) => HttpResponse::Ok().json(Result::success(true)),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// PUT /v3/admin/ai/agentspecs/labels — Update label->version routing
#[put("labels")]
async fn update_labels(
    req: HttpRequest,
    data: web::Data<AppState>,
    agentspec_service: web::Data<Arc<AgentSpecOperationService>>,
    body: web::Json<AgentSpecLabelsUpdateForm>,
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

    let labels: std::collections::HashMap<String, String> = match serde_json::from_str(&form.labels)
    {
        Ok(l) => l,
        Err(e) => {
            return Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_VALIDATE_ERROR,
                format!("Invalid labels JSON: {}", e),
            );
        }
    };

    match agentspec_service
        .update_labels(ns, &form.agent_spec_name, labels)
        .await
    {
        Ok(()) => HttpResponse::Ok().json(Result::success(true)),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// PUT /v3/admin/ai/agentspecs/biz-tags — Update business tags
#[put("biz-tags")]
async fn update_biz_tags(
    req: HttpRequest,
    data: web::Data<AppState>,
    agentspec_service: web::Data<Arc<AgentSpecOperationService>>,
    body: web::Json<AgentSpecBizTagsUpdateForm>,
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

    match agentspec_service
        .update_biz_tags(ns, &form.agent_spec_name, &form.biz_tags)
        .await
    {
        Ok(()) => HttpResponse::Ok().json(Result::success(true)),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// POST /v3/admin/ai/agentspecs/online — Online operation
#[post("online")]
async fn online_agentspec(
    req: HttpRequest,
    data: web::Data<AppState>,
    agentspec_service: web::Data<Arc<AgentSpecOperationService>>,
    body: web::Json<AgentSpecOnlineForm>,
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

    match agentspec_service
        .change_online_status(
            ns,
            &form.agent_spec_name,
            form.scope.as_deref(),
            form.version.as_deref(),
            true,
        )
        .await
    {
        Ok(()) => HttpResponse::Ok().json(Result::success(true)),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// POST /v3/admin/ai/agentspecs/offline — Offline operation
#[post("offline")]
async fn offline_agentspec(
    req: HttpRequest,
    data: web::Data<AppState>,
    agentspec_service: web::Data<Arc<AgentSpecOperationService>>,
    body: web::Json<AgentSpecOnlineForm>,
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

    match agentspec_service
        .change_online_status(
            ns,
            &form.agent_spec_name,
            form.scope.as_deref(),
            form.version.as_deref(),
            false,
        )
        .await
    {
        Ok(()) => HttpResponse::Ok().json(Result::success(true)),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// PUT /v3/admin/ai/agentspecs/scope — Update visibility scope
#[put("scope")]
async fn update_scope(
    req: HttpRequest,
    data: web::Data<AppState>,
    agentspec_service: web::Data<Arc<AgentSpecOperationService>>,
    body: web::Json<AgentSpecScopeForm>,
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

    match agentspec_service
        .update_scope(ns, &form.agent_spec_name, &form.scope)
        .await
    {
        Ok(()) => HttpResponse::Ok().json(Result::success(true)),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

// ============================================================================
// Client handlers — `/v3/client/ai/agentspecs`
// ============================================================================

/// GET /v3/client/ai/agentspecs — Query agentspec by label/version/latest (runtime)
/// Returns JSON (not ZIP) since AgentSpec uses manifest.json format
/// Nacos: OPEN_API + ALLOW_ANONYMOUS
#[get("")]
async fn client_query_agentspec(
    req: HttpRequest,
    data: web::Data<AppState>,
    agentspec_service: web::Data<Arc<AgentSpecOperationService>>,
    query: web::Query<AgentSpecQueryForm>,
) -> impl Responder {
    // Nacos uses OPEN_API + ALLOW_ANONYMOUS for client SDK access
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Ai)
            .api_type(ApiType::OpenApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);

    match agentspec_service
        .query(
            ns,
            &query.name,
            query.version.as_deref(),
            query.label.as_deref(),
        )
        .await
    {
        Ok(Some(spec)) => HttpResponse::Ok().json(Result::success(spec)),
        Ok(None) => Result::<()>::http_not_found(
            &batata_common::error::RESOURCE_NOT_FOUND,
            format!("AgentSpec '{}' not found or not online", query.name),
        ),
        Err(e) => Result::<()>::http_internal_error(e),
    }
}

/// GET /v3/client/ai/agentspecs/search — Search agentspecs for discovery (client)
/// Returns only enabled agentspecs with online versions
#[get("search")]
async fn client_search_agentspecs(
    req: HttpRequest,
    data: web::Data<AppState>,
    agentspec_service: web::Data<Arc<AgentSpecOperationService>>,
    query: web::Query<AgentSpecSearchForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Ai)
            .api_type(ApiType::OpenApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);

    match agentspec_service
        .search(ns, query.keyword.as_deref(), query.page_no, query.page_size)
        .await
    {
        Ok(page) => HttpResponse::Ok().json(Result::success(page)),
        Err(e) => Result::<()>::http_internal_error(e),
    }
}

// ============================================================================
// Upload form (JSON body for upload endpoint)
// ============================================================================

/// JSON body for agentspec upload
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSpecUploadForm {
    pub agent_spec_name: Option<String>,
    pub agent_spec_card: Option<String>,
}

// ============================================================================
// Route configuration
// ============================================================================

/// Configure admin agentspec routes at `/v3/admin/ai/agentspecs`
pub fn admin_routes() -> actix_web::Scope {
    web::scope("/agentspecs")
        .service(list_agentspecs)
        .service(download_agentspec_version)
        .service(get_agentspec_version)
        .service(upload_agentspec)
        .service(create_draft)
        .service(update_draft)
        .service(delete_draft)
        .service(submit_agentspec)
        .service(publish_agentspec)
        .service(update_labels)
        .service(update_biz_tags)
        .service(online_agentspec)
        .service(offline_agentspec)
        .service(update_scope)
        .service(get_agentspec_detail)
        .service(delete_agentspec)
}

/// Configure client agentspec routes at `/v3/client/ai/agentspecs`
pub fn client_routes() -> actix_web::Scope {
    web::scope("/agentspecs")
        .service(client_search_agentspecs)
        .service(client_query_agentspec)
}
