//! Skill HTTP API handlers — Nacos 3.x compatible
//!
//! Admin: `/v3/admin/ai/skills` (16 endpoints)
//! Client: `/v3/client/ai/skills` (1 endpoint)

use std::sync::Arc;

use actix_multipart::Multipart;
use actix_web::{HttpMessage, HttpRequest, HttpResponse, Responder, delete, get, post, put, web};
use futures::StreamExt;

use batata_common::{ActionTypes, ApiType, DEFAULT_NAMESPACE_ID, SignType};
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::response::Result;
use batata_server_common::{Secured, secured};

use crate::model::skill::*;
use crate::service::traits::SkillService;
use crate::service::skill_zip;

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
// Admin handlers — `/v3/admin/ai/skills`
// ============================================================================

/// GET /v3/admin/ai/skills — Get skill detail (governance + all versions)
#[get("")]
async fn get_skill_detail(
    req: HttpRequest,
    data: web::Data<AppState>,
    skill_service: web::Data<Arc<dyn SkillService>>,
    query: web::Query<SkillForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);
    let name = match query.skill_name.as_deref() {
        Some(n) if !n.is_empty() => n,
        _ => {
            return Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_MISSING,
                "skillName is required",
            );
        }
    };

    match skill_service.get_skill_detail(ns, name).await {
        Ok(Some(meta)) => HttpResponse::Ok().json(Result::success(meta)),
        Ok(None) => Result::<()>::http_not_found(
            &batata_common::error::SKILL_NOT_FOUND,
            format!("Skill '{}' not found", name),
        ),
        Err(e) => Result::<()>::http_internal_error(e),
    }
}

/// GET /v3/admin/ai/skills/version — Get specific version detail
#[get("version")]
async fn get_skill_version(
    req: HttpRequest,
    data: web::Data<AppState>,
    skill_service: web::Data<Arc<dyn SkillService>>,
    query: web::Query<SkillForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);
    let name = match query.skill_name.as_deref() {
        Some(n) if !n.is_empty() => n,
        _ => {
            return Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_MISSING,
                "skillName is required",
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

    match skill_service
        .get_skill_version_detail(ns, name, version)
        .await
    {
        Ok(Some(skill)) => HttpResponse::Ok().json(Result::success(skill)),
        Ok(None) => Result::<()>::http_not_found(
            &batata_common::error::SKILL_NOT_FOUND,
            format!("Skill '{}' version '{}' not found", name, version),
        ),
        Err(e) => Result::<()>::http_internal_error(e),
    }
}

/// GET /v3/admin/ai/skills/version/download — Download skill version as ZIP
#[get("version/download")]
async fn download_skill_version(
    req: HttpRequest,
    data: web::Data<AppState>,
    skill_service: web::Data<Arc<dyn SkillService>>,
    query: web::Query<SkillForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);
    let name = match query.skill_name.as_deref() {
        Some(n) if !n.is_empty() => n,
        _ => {
            return Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_MISSING,
                "skillName is required",
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

    match skill_service
        .download_skill_version(ns, name, version)
        .await
    {
        Ok(Some(skill)) => match skill_zip::skill_to_zip_bytes(&skill) {
            Ok(zip_bytes) => HttpResponse::Ok()
                .content_type("application/zip")
                .insert_header((
                    "Content-Disposition",
                    format!("attachment; filename=\"{}-{}.zip\"", name, version),
                ))
                .body(zip_bytes),
            Err(e) => Result::<()>::http_internal_error(e),
        },
        Ok(None) => Result::<()>::http_not_found(
            &batata_common::error::SKILL_NOT_FOUND,
            format!("Skill '{}' version '{}' not found", name, version),
        ),
        Err(e) => Result::<()>::http_internal_error(e),
    }
}

/// DELETE /v3/admin/ai/skills — Delete skill
#[delete("")]
async fn delete_skill(
    req: HttpRequest,
    data: web::Data<AppState>,
    skill_service: web::Data<Arc<dyn SkillService>>,
    query: web::Query<SkillForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);
    let name = match query.skill_name.as_deref() {
        Some(n) if !n.is_empty() => n,
        _ => {
            return Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_MISSING,
                "skillName is required",
            );
        }
    };

    match skill_service.delete_skill(ns, name).await {
        Ok(()) => HttpResponse::Ok().json(Result::success(true)),
        Err(e) => Result::<()>::http_internal_error(e),
    }
}

/// GET /v3/admin/ai/skills/list — List skills with pagination
#[get("list")]
async fn list_skills(
    req: HttpRequest,
    data: web::Data<AppState>,
    skill_service: web::Data<Arc<dyn SkillService>>,
    query: web::Query<SkillListForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);

    match skill_service
        .list_skills(
            ns,
            query.skill_name.as_deref(),
            query.search.as_deref(),
            query.order_by.as_deref(),
            query.page_no,
            query.page_size,
        )
        .await
    {
        Ok(page) => HttpResponse::Ok().json(Result::success(page)),
        Err(e) => Result::<()>::http_internal_error(e),
    }
}

/// POST /v3/admin/ai/skills/upload — Upload skill from ZIP file (multipart/form-data)
/// Params: namespaceId (query, optional), overwrite (query, optional, default=false), file (multipart)
#[post("upload")]
async fn upload_skill(
    req: HttpRequest,
    data: web::Data<AppState>,
    skill_service: web::Data<Arc<dyn SkillService>>,
    query: web::Query<SkillUploadQuery>,
    mut payload: Multipart,
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

    // Read ZIP file from multipart
    let mut zip_bytes: Option<Vec<u8>> = None;
    while let Some(Ok(mut field)) = payload.next().await {
        let field_name = field
            .content_disposition()
            .and_then(|cd| cd.get_name().map(|s| s.to_string()))
            .unwrap_or_default();
        if field_name == "file" {
            let mut bytes = Vec::new();
            while let Some(Ok(chunk)) = field.next().await {
                bytes.extend_from_slice(&chunk);
                if bytes.len() > skill_zip::MAX_UPLOAD_ZIP_BYTES {
                    return Result::<()>::http_bad_request(
                        &batata_common::error::PARAMETER_VALIDATE_ERROR,
                        format!(
                            "File too large (max {} bytes)",
                            skill_zip::MAX_UPLOAD_ZIP_BYTES
                        ),
                    );
                }
            }
            zip_bytes = Some(bytes);
            break;
        }
    }

    let zip_bytes = match zip_bytes {
        Some(b) if !b.is_empty() => b,
        _ => {
            return Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_MISSING,
                "file field is required",
            );
        }
    };

    // Parse skill from ZIP
    let skill = match skill_zip::parse_skill_from_zip(&zip_bytes, ns) {
        Ok(s) => s,
        Err(e) => {
            return Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_VALIDATE_ERROR,
                format!("Failed to parse ZIP: {}", e),
            );
        }
    };

    let name = skill.name.clone();

    match skill_service
        .upload_skill(ns, &name, &skill, &author, overwrite)
        .await
    {
        Ok(skill_name) => HttpResponse::Ok().json(Result::success(skill_name)),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// POST /v3/admin/ai/skills/draft — Create draft
#[post("draft")]
async fn create_draft(
    req: HttpRequest,
    data: web::Data<AppState>,
    skill_service: web::Data<Arc<dyn SkillService>>,
    body: web::Form<SkillDraftCreateForm>,
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

    let initial_content: Option<Skill> = form
        .skill_card
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok());

    // Resolve skill name: form param takes priority, then from skill_card JSON
    let skill_name = form
        .skill_name
        .as_deref()
        .filter(|n| !n.is_empty())
        .or_else(|| initial_content.as_ref().map(|s| s.name.as_str()))
        .unwrap_or("");

    if skill_name.is_empty() {
        return Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_MISSING,
            "skillName or skillCard with name is required",
        );
    }

    match skill_service
        .create_draft(
            ns,
            skill_name,
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

/// PUT /v3/admin/ai/skills/draft — Update draft
#[put("draft")]
async fn update_draft(
    req: HttpRequest,
    data: web::Data<AppState>,
    skill_service: web::Data<Arc<dyn SkillService>>,
    body: web::Form<SkillUpdateForm>,
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

    let skill: Skill = match form
        .skill_card
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
    {
        Ok(Some(s)) => s,
        Ok(None) => {
            return Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_MISSING,
                "skillCard is required",
            );
        }
        Err(e) => {
            return Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_VALIDATE_ERROR,
                format!("Invalid skillCard JSON: {}", e),
            );
        }
    };

    // Resolve skill name: form param takes priority, then from skillCard JSON content
    let skill_name = form
        .skill_name
        .as_deref()
        .filter(|n| !n.is_empty())
        .or_else(|| {
            if !skill.name.is_empty() {
                Some(skill.name.as_str())
            } else {
                None
            }
        })
        .unwrap_or("");

    if skill_name.is_empty() {
        return Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_MISSING,
            "skillName or skillCard with name is required",
        );
    }

    match skill_service.update_draft(ns, skill_name, &skill).await {
        Ok(()) => HttpResponse::Ok().json(Result::success(true)),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// DELETE /v3/admin/ai/skills/draft — Delete draft
#[delete("draft")]
async fn delete_draft(
    req: HttpRequest,
    data: web::Data<AppState>,
    skill_service: web::Data<Arc<dyn SkillService>>,
    query: web::Query<SkillForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);
    let name = match query.skill_name.as_deref() {
        Some(n) if !n.is_empty() => n,
        _ => {
            return Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_MISSING,
                "skillName is required",
            );
        }
    };

    match skill_service.delete_draft(ns, name).await {
        Ok(()) => HttpResponse::Ok().json(Result::success(true)),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// POST /v3/admin/ai/skills/submit — Submit version for review
#[post("submit")]
async fn submit_skill(
    req: HttpRequest,
    data: web::Data<AppState>,
    skill_service: web::Data<Arc<dyn SkillService>>,
    body: web::Form<SkillSubmitForm>,
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

    match skill_service
        .submit(ns, &form.skill_name, &form.version)
        .await
    {
        Ok(version) => HttpResponse::Ok().json(Result::success(version)),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// POST /v3/admin/ai/skills/publish — Publish approved version
#[post("publish")]
async fn publish_skill(
    req: HttpRequest,
    data: web::Data<AppState>,
    skill_service: web::Data<Arc<dyn SkillService>>,
    body: web::Form<SkillPublishForm>,
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

    match skill_service
        .publish(
            ns,
            &form.skill_name,
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

/// PUT /v3/admin/ai/skills/labels — Update label→version routing
#[put("labels")]
async fn update_labels(
    req: HttpRequest,
    data: web::Data<AppState>,
    skill_service: web::Data<Arc<dyn SkillService>>,
    body: web::Form<SkillLabelsUpdateForm>,
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

    match skill_service
        .update_labels(ns, &form.skill_name, labels)
        .await
    {
        Ok(()) => HttpResponse::Ok().json(Result::success(true)),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// PUT /v3/admin/ai/skills/biz-tags — Update business tags
#[put("biz-tags")]
async fn update_biz_tags(
    req: HttpRequest,
    data: web::Data<AppState>,
    skill_service: web::Data<Arc<dyn SkillService>>,
    body: web::Form<SkillBizTagsUpdateForm>,
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

    match skill_service
        .update_biz_tags(ns, &form.skill_name, &form.biz_tags)
        .await
    {
        Ok(()) => HttpResponse::Ok().json(Result::success(true)),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// POST /v3/admin/ai/skills/online — Online operation
#[post("online")]
async fn online_skill(
    req: HttpRequest,
    data: web::Data<AppState>,
    skill_service: web::Data<Arc<dyn SkillService>>,
    body: web::Form<SkillOnlineForm>,
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

    match skill_service
        .change_online_status(
            ns,
            &form.skill_name,
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

/// POST /v3/admin/ai/skills/offline — Offline operation
#[post("offline")]
async fn offline_skill(
    req: HttpRequest,
    data: web::Data<AppState>,
    skill_service: web::Data<Arc<dyn SkillService>>,
    body: web::Form<SkillOnlineForm>,
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

    match skill_service
        .change_online_status(
            ns,
            &form.skill_name,
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

/// PUT /v3/admin/ai/skills/scope — Update visibility scope
#[put("scope")]
async fn update_scope(
    req: HttpRequest,
    data: web::Data<AppState>,
    skill_service: web::Data<Arc<dyn SkillService>>,
    body: web::Form<SkillScopeForm>,
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

    match skill_service
        .update_scope(ns, &form.skill_name, &form.scope)
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
// Client handlers — `/v3/client/ai/skills`
// ============================================================================

/// GET /v3/client/ai/skills — Query skill by label/version/latest (runtime download)
/// Nacos: OPEN_API + ALLOW_ANONYMOUS
#[get("")]
async fn client_query_skill(
    req: HttpRequest,
    data: web::Data<AppState>,
    skill_service: web::Data<Arc<dyn SkillService>>,
    query: web::Query<SkillQueryForm>,
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

    match skill_service
        .query_skill(
            ns,
            &query.name,
            query.version.as_deref(),
            query.label.as_deref(),
        )
        .await
    {
        Ok(Some(skill)) => {
            // Return as ZIP (matching Nacos: ResponseEntity<byte[]>)
            match skill_zip::skill_to_zip_bytes(&skill) {
                Ok(zip_bytes) => HttpResponse::Ok()
                    .content_type("application/zip")
                    .body(zip_bytes),
                Err(e) => Result::<()>::http_internal_error(e),
            }
        }
        Ok(None) => Result::<()>::http_not_found(
            &batata_common::error::SKILL_NOT_FOUND,
            format!("Skill '{}' not found or not online", query.name),
        ),
        Err(e) => Result::<()>::http_internal_error(e),
    }
}

/// GET /v3/client/ai/skills/search — Search skills for discovery (client)
/// Returns only enabled skills with online versions
#[get("search")]
async fn client_search_skills(
    req: HttpRequest,
    data: web::Data<AppState>,
    skill_service: web::Data<Arc<dyn SkillService>>,
    query: web::Query<SkillSearchForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Ai)
            .api_type(ApiType::OpenApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);

    match skill_service
        .search_skills(ns, query.keyword.as_deref(), query.page_no, query.page_size)
        .await
    {
        Ok(page) => HttpResponse::Ok().json(Result::success(page)),
        Err(e) => Result::<()>::http_internal_error(e),
    }
}

// ============================================================================
// Upload query params (multipart upload uses query params + file body)
// ============================================================================

/// Upload query params for ZIP upload
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillUploadQuery {
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
    #[serde(default)]
    pub overwrite: bool,
}

// ============================================================================
// Route configuration
// ============================================================================

/// Configure admin skill routes at `/v3/admin/ai/skills`
pub fn admin_routes() -> actix_web::Scope {
    web::scope("/skills")
        .service(list_skills)
        .service(download_skill_version)
        .service(get_skill_version)
        .service(upload_skill)
        .service(create_draft)
        .service(update_draft)
        .service(delete_draft)
        .service(submit_skill)
        .service(publish_skill)
        .service(update_labels)
        .service(update_biz_tags)
        .service(online_skill)
        .service(offline_skill)
        .service(update_scope)
        .service(get_skill_detail)
        .service(delete_skill)
}

/// Configure client skill routes at `/v3/client/ai/skills`
pub fn client_routes() -> actix_web::Scope {
    web::scope("/skills")
        .service(client_search_skills)
        .service(client_query_skill)
}
