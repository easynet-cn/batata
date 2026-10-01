//! Console Skill management API endpoints.
//!
//! Aligned with Batata V3 Console API contract.
// Mirrors admin endpoints under /v3/console/ai/skills with ConsoleApi security

use std::sync::Arc;

use actix_multipart::Multipart;
use actix_web::{HttpRequest, HttpResponse, Responder, Scope, delete, get, post, put, web};
use futures::StreamExt;

use batata_common::DEFAULT_NAMESPACE_ID;
use batata_common::SkillService;
use batata_common::model::ai::skill::*;
use batata_common::model::ai::skill_zip;
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::response as common_response;
use batata_server_common::secured::Secured;
use batata_server_common::{ActionTypes, ApiType, SignType, secured};

fn normalize_namespace(ns: &str) -> &str {
    if ns.is_empty() {
        DEFAULT_NAMESPACE_ID
    } else {
        ns
    }
}

use super::ai_trace::{self, get_username, trace_write};

/// GET /v3/console/ai/skills — Get skill detail
#[get("")]
async fn get_skill_detail(
    req: HttpRequest,
    data: web::Data<AppState>,
    skill_service: web::Data<Arc<dyn SkillService>>,
    query: web::Query<SkillForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/skills")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);
    let name = match query.skill_name.as_deref() {
        Some(n) if !n.is_empty() => n,
        _ => {
            return common_response::Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_MISSING,
                "skillName is required",
            );
        }
    };

    match skill_service.get_skill_detail(ns, name, Some(get_username(&req).as_str())).await {
        Ok(Some(meta)) => HttpResponse::Ok().json(common_response::Result::success(meta)),
        Ok(None) => common_response::Result::<()>::http_not_found(
            &batata_common::error::SKILL_NOT_FOUND,
            format!("Skill '{}' not found", name),
        ),
        Err(e) => common_response::Result::<()>::http_internal_error(e),
    }
}

/// GET /v3/console/ai/skills/version — Get specific version detail
#[get("/version")]
async fn get_skill_version(
    req: HttpRequest,
    data: web::Data<AppState>,
    skill_service: web::Data<Arc<dyn SkillService>>,
    query: web::Query<SkillForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/skills")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);
    let name = match query.skill_name.as_deref() {
        Some(n) if !n.is_empty() => n,
        _ => {
            return common_response::Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_MISSING,
                "skillName is required",
            );
        }
    };
    let version = match query.version.as_deref() {
        Some(v) if !v.is_empty() => v,
        _ => {
            return common_response::Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_MISSING,
                "version is required",
            );
        }
    };

    match skill_service
        .get_skill_version_detail(ns, name, version, Some(get_username(&req).as_str()))
        .await
    {
        Ok(Some(skill)) => HttpResponse::Ok().json(common_response::Result::success(skill)),
        Ok(None) => common_response::Result::<()>::http_not_found(
            &batata_common::error::SKILL_NOT_FOUND,
            format!("Skill '{}' version '{}' not found", name, version),
        ),
        Err(e) => common_response::Result::<()>::http_internal_error(e),
    }
}

/// GET /v3/console/ai/skills/version/download — Download skill version
#[get("/version/download")]
async fn download_skill_version(
    req: HttpRequest,
    data: web::Data<AppState>,
    skill_service: web::Data<Arc<dyn SkillService>>,
    query: web::Query<SkillForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/skills")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);
    let name = match query.skill_name.as_deref() {
        Some(n) if !n.is_empty() => n,
        _ => {
            return common_response::Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_MISSING,
                "skillName is required",
            );
        }
    };
    let version = match query.version.as_deref() {
        Some(v) if !v.is_empty() => v,
        _ => {
            return common_response::Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_MISSING,
                "version is required",
            );
        }
    };

    match skill_service
        .download_skill_version(ns, name, version, Some(get_username(&req).as_str()))
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
            Err(e) => common_response::Result::<()>::http_internal_error(e),
        },
        Ok(None) => common_response::Result::<()>::http_not_found(
            &batata_common::error::SKILL_NOT_FOUND,
            format!("Skill '{}' version '{}' not found", name, version),
        ),
        Err(e) => common_response::Result::<()>::http_internal_error(e),
    }
}

/// DELETE /v3/console/ai/skills — Delete skill
#[delete("")]
async fn delete_skill(
    req: HttpRequest,
    data: web::Data<AppState>,
    skill_service: web::Data<Arc<dyn SkillService>>,
    query: web::Query<SkillForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/skills")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);
    let name = match query.skill_name.as_deref() {
        Some(n) if !n.is_empty() => n,
        _ => {
            return common_response::Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_MISSING,
                "skillName is required",
            );
        }
    };

    let result = skill_service
        .delete_skill(ns, name, Some(get_username(&req).as_str()))
        .await;
    trace_write(
        &req,
        batata_common::ai_trace::RESOURCE_TYPE_SKILL,
        batata_common::ai_trace::OP_DELETE_RESOURCE,
        Some(name),
        None,
        ai_trace::outcome_of(&result),
    );
    match result {
        Ok(()) => HttpResponse::Ok().json(common_response::Result::success(true)),
        Err(e) => common_response::Result::<()>::http_internal_error(e),
    }
}

/// GET /v3/console/ai/skills/list — List skills
#[get("/list")]
async fn list_skills(
    req: HttpRequest,
    data: web::Data<AppState>,
    skill_service: web::Data<Arc<dyn SkillService>>,
    query: web::Query<SkillListForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/skills")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
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
            Some(get_username(&req).as_str()),
        )
        .await
    {
        Ok(page) => HttpResponse::Ok().json(common_response::Result::success(page)),
        Err(e) => common_response::Result::<()>::http_internal_error(e),
    }
}

/// Upload query params for console
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleSkillUploadQuery {
    /// Namespace the skill belongs to.
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
    /// Whether to overwrite an existing skill with the same name.
    #[serde(default)]
    pub overwrite: bool,
}

/// Read the `file` field of a multipart upload, enforcing the ZIP size limit.
///
/// Shared by `/upload`, `/upload/precheck` and `/upload/batch`, which upstream
/// all declare as `multipart/form-data`.
async fn read_upload_zip(mut payload: Multipart) -> std::result::Result<Vec<u8>, HttpResponse> {
    while let Some(Ok(mut field)) = payload.next().await {
        let field_name = field
            .content_disposition()
            .and_then(|cd| cd.get_name().map(|s| s.to_string()))
            .unwrap_or_default();
        if field_name != "file" {
            continue;
        }

        let mut bytes = Vec::new();
        while let Some(Ok(chunk)) = field.next().await {
            bytes.extend_from_slice(&chunk);
            if bytes.len() > skill_zip::MAX_UPLOAD_ZIP_BYTES {
                return Err(common_response::Result::<()>::http_bad_request(
                    &batata_common::error::PARAMETER_VALIDATE_ERROR,
                    format!(
                        "File too large (max {} bytes)",
                        skill_zip::MAX_UPLOAD_ZIP_BYTES
                    ),
                ));
            }
        }
        if bytes.is_empty() {
            break;
        }
        return Ok(bytes);
    }

    Err(common_response::Result::<()>::http_bad_request(
        &batata_common::error::PARAMETER_MISSING,
        "file field is required",
    ))
}

/// POST /v3/console/ai/skills/upload — Upload skill from ZIP file
#[post("/upload")]
async fn upload_skill(
    req: HttpRequest,
    data: web::Data<AppState>,
    skill_service: web::Data<Arc<dyn SkillService>>,
    query: web::Query<ConsoleSkillUploadQuery>,
    payload: Multipart,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/skills")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);
    let overwrite = query.overwrite;
    let author = get_username(&req);

    let zip_bytes = match read_upload_zip(payload).await {
        Ok(bytes) => bytes,
        Err(response) => return response,
    };

    let skill = match skill_zip::parse_skill_from_zip(&zip_bytes, ns) {
        Ok(s) => s,
        Err(e) => {
            return common_response::Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_VALIDATE_ERROR,
                format!("Failed to parse ZIP: {}", e),
            );
        }
    };

    let name = skill.name.clone();

    let result = skill_service
        .upload_skill(ns, &name, &skill, &author, overwrite)
        .await;
    trace_write(
        &req,
        batata_common::ai_trace::RESOURCE_TYPE_SKILL,
        batata_common::ai_trace::OP_UPLOAD,
        Some(&name),
        None,
        ai_trace::outcome_of(&result),
    );
    match result {
        Ok(skill_name) => HttpResponse::Ok().json(common_response::Result::success(skill_name)),
        Err(e) => common_response::Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// POST /v3/console/ai/skills/draft — Create draft
#[post("/draft")]
async fn create_draft(
    req: HttpRequest,
    data: web::Data<AppState>,
    skill_service: web::Data<Arc<dyn SkillService>>,
    body: web::Json<SkillDraftCreateForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/skills")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
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
        return common_response::Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_MISSING,
            "skillName or skillCard with name is required",
        );
    }

    let result = skill_service
        .create_draft(
            ns,
            skill_name,
            form.based_on_version.as_deref(),
            form.target_version.as_deref(),
            initial_content.as_ref(),
            &author,
        )
        .await;
    trace_write(
        &req,
        batata_common::ai_trace::RESOURCE_TYPE_SKILL,
        batata_common::ai_trace::OP_CREATE_DRAFT,
        Some(skill_name),
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

/// PUT /v3/console/ai/skills/draft — Update draft
#[put("/draft")]
async fn update_draft(
    req: HttpRequest,
    data: web::Data<AppState>,
    skill_service: web::Data<Arc<dyn SkillService>>,
    body: web::Json<SkillUpdateForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/skills")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
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
            return common_response::Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_MISSING,
                "skillCard is required",
            );
        }
        Err(e) => {
            return common_response::Result::<()>::http_bad_request(
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
        .or({
            if !skill.name.is_empty() {
                Some(skill.name.as_str())
            } else {
                None
            }
        })
        .unwrap_or("");

    if skill_name.is_empty() {
        return common_response::Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_MISSING,
            "skillName or skillCard with name is required",
        );
    }

    let result = skill_service
        .update_draft(ns, skill_name, &skill, Some(get_username(&req).as_str()))
        .await;
    trace_write(
        &req,
        batata_common::ai_trace::RESOURCE_TYPE_SKILL,
        batata_common::ai_trace::OP_UPDATE_DRAFT,
        Some(skill_name),
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

/// DELETE /v3/console/ai/skills/draft — Delete draft
#[delete("/draft")]
async fn delete_draft(
    req: HttpRequest,
    data: web::Data<AppState>,
    skill_service: web::Data<Arc<dyn SkillService>>,
    query: web::Query<SkillForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/skills")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);
    let name = match query.skill_name.as_deref() {
        Some(n) if !n.is_empty() => n,
        _ => {
            return common_response::Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_MISSING,
                "skillName is required",
            );
        }
    };

    let result = skill_service
        .delete_draft(ns, name, Some(get_username(&req).as_str()))
        .await;
    trace_write(
        &req,
        batata_common::ai_trace::RESOURCE_TYPE_SKILL,
        batata_common::ai_trace::OP_DELETE_DRAFT,
        Some(name),
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

/// POST /v3/console/ai/skills/submit — Submit for review
#[post("/submit")]
async fn submit_skill(
    req: HttpRequest,
    data: web::Data<AppState>,
    skill_service: web::Data<Arc<dyn SkillService>>,
    body: web::Json<SkillSubmitForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/skills")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let form = body.into_inner();
    let ns = normalize_namespace(&form.namespace_id);

    let result = skill_service
        .submit(ns, &form.skill_name, &form.version, Some(get_username(&req).as_str()))
        .await;
    trace_write(
        &req,
        batata_common::ai_trace::RESOURCE_TYPE_SKILL,
        batata_common::ai_trace::OP_SUBMIT_REVIEW,
        Some(&form.skill_name),
        Some(&form.version),
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

/// POST /v3/console/ai/skills/publish — Publish version
#[post("/publish")]
async fn publish_skill(
    req: HttpRequest,
    data: web::Data<AppState>,
    skill_service: web::Data<Arc<dyn SkillService>>,
    body: web::Json<SkillPublishForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/skills")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let form = body.into_inner();
    let ns = normalize_namespace(&form.namespace_id);

    let result = skill_service
        .publish(
            ns,
            &form.skill_name,
            &form.version,
            Some(get_username(&req).as_str()),
        )
        .await;
    trace_write(
        &req,
        batata_common::ai_trace::RESOURCE_TYPE_SKILL,
        batata_common::ai_trace::OP_PUBLISH,
        Some(&form.skill_name),
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

/// POST /v3/console/ai/skills/force-publish — Publish bypassing the review gate
#[post("/force-publish")]
async fn force_publish_skill(
    req: HttpRequest,
    data: web::Data<AppState>,
    skill_service: web::Data<Arc<dyn SkillService>>,
    body: web::Json<SkillPublishForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/skills")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let form = body.into_inner();
    let ns = normalize_namespace(&form.namespace_id);

    let result = skill_service
        .force_publish(
            ns,
            &form.skill_name,
            &form.version,
            Some(get_username(&req).as_str()),
        )
        .await;
    trace_write(
        &req,
        batata_common::ai_trace::RESOURCE_TYPE_SKILL,
        batata_common::ai_trace::OP_FORCE_PUBLISH,
        Some(&form.skill_name),
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

/// POST /v3/console/ai/skills/redraft — Move a version back to draft
#[post("/redraft")]
async fn redraft_skill(
    req: HttpRequest,
    data: web::Data<AppState>,
    skill_service: web::Data<Arc<dyn SkillService>>,
    body: web::Json<SkillPublishForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/skills")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let form = body.into_inner();
    let ns = normalize_namespace(&form.namespace_id);

    let result = skill_service
        .redraft(
            ns,
            &form.skill_name,
            &form.version,
            Some(get_username(&req).as_str()),
        )
        .await;
    trace_write(
        &req,
        batata_common::ai_trace::RESOURCE_TYPE_SKILL,
        batata_common::ai_trace::OP_REDRAFT,
        Some(&form.skill_name),
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

/// POST /v3/console/ai/skills/upload/precheck — Report what an upload would do,
/// without persisting anything
#[post("/upload/precheck")]
async fn precheck_upload_skill(
    req: HttpRequest,
    data: web::Data<AppState>,
    skill_service: web::Data<Arc<dyn SkillService>>,
    query: web::Query<ConsoleSkillUploadQuery>,
    payload: Multipart,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/skills")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);
    let zip_bytes = match read_upload_zip(payload).await {
        Ok(bytes) => bytes,
        Err(response) => return response,
    };

    let result = skill_service
        .precheck_upload_from_zip(ns, &zip_bytes, Some(get_username(&req).as_str()))
        .await;
    match result {
        Ok(results) => HttpResponse::Ok().json(common_response::Result::success(results)),
        Err(e) => common_response::Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// POST /v3/console/ai/skills/upload/batch — Upload every skill in a ZIP
#[post("/upload/batch")]
async fn batch_upload_skills(
    req: HttpRequest,
    data: web::Data<AppState>,
    skill_service: web::Data<Arc<dyn SkillService>>,
    query: web::Query<ConsoleSkillUploadQuery>,
    payload: Multipart,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/skills")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);
    let zip_bytes = match read_upload_zip(payload).await {
        Ok(bytes) => bytes,
        Err(response) => return response,
    };

    let result = skill_service
        .batch_upload_from_zip(
            ns,
            &zip_bytes,
            query.overwrite,
            Some(get_username(&req).as_str()),
        )
        .await;
    match result {
        Ok(result) => HttpResponse::Ok().json(common_response::Result::success(result)),
        Err(e) => common_response::Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// PUT /v3/console/ai/skills/labels — Update labels
#[put("/labels")]
async fn update_labels(
    req: HttpRequest,
    data: web::Data<AppState>,
    skill_service: web::Data<Arc<dyn SkillService>>,
    body: web::Json<SkillLabelsUpdateForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/skills")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let form = body.into_inner();
    let ns = normalize_namespace(&form.namespace_id);

    let labels: std::collections::HashMap<String, String> = match serde_json::from_str(&form.labels)
    {
        Ok(l) => l,
        Err(e) => {
            return common_response::Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_VALIDATE_ERROR,
                format!("Invalid labels JSON: {}", e),
            );
        }
    };

    let result = skill_service
        .update_labels(ns, &form.skill_name, labels, Some(get_username(&req).as_str()))
        .await;
    trace_write(
        &req,
        batata_common::ai_trace::RESOURCE_TYPE_SKILL,
        batata_common::ai_trace::OP_UPDATE_LABELS,
        Some(&form.skill_name),
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

/// PUT /v3/console/ai/skills/biz-tags — Update business tags
#[put("/biz-tags")]
async fn update_biz_tags(
    req: HttpRequest,
    data: web::Data<AppState>,
    skill_service: web::Data<Arc<dyn SkillService>>,
    body: web::Json<SkillBizTagsUpdateForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/skills")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let form = body.into_inner();
    let ns = normalize_namespace(&form.namespace_id);

    let result = skill_service
        .update_biz_tags(ns, &form.skill_name, &form.biz_tags, Some(get_username(&req).as_str()))
        .await;
    trace_write(
        &req,
        batata_common::ai_trace::RESOURCE_TYPE_SKILL,
        batata_common::ai_trace::OP_UPDATE_BIZ_TAGS,
        Some(&form.skill_name),
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

/// POST /v3/console/ai/skills/online — Online operation
#[post("/online")]
async fn online_skill(
    req: HttpRequest,
    data: web::Data<AppState>,
    skill_service: web::Data<Arc<dyn SkillService>>,
    body: web::Json<SkillOnlineForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/skills")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let form = body.into_inner();
    let ns = normalize_namespace(&form.namespace_id);

    let result = skill_service
        .change_online_status(
            ns,
            &form.skill_name,
            form.scope.as_deref(),
            form.version.as_deref(),
            true,
            Some(get_username(&req).as_str()),
        )
        .await;
    trace_write(
        &req,
        batata_common::ai_trace::RESOURCE_TYPE_SKILL,
        batata_common::ai_trace::OP_ONLINE_VERSION,
        Some(&form.skill_name),
        form.version.as_deref(),
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

/// POST /v3/console/ai/skills/offline — Offline operation
#[post("/offline")]
async fn offline_skill(
    req: HttpRequest,
    data: web::Data<AppState>,
    skill_service: web::Data<Arc<dyn SkillService>>,
    body: web::Json<SkillOnlineForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/skills")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let form = body.into_inner();
    let ns = normalize_namespace(&form.namespace_id);

    let result = skill_service
        .change_online_status(
            ns,
            &form.skill_name,
            form.scope.as_deref(),
            form.version.as_deref(),
            false,
            Some(get_username(&req).as_str()),
        )
        .await;
    trace_write(
        &req,
        batata_common::ai_trace::RESOURCE_TYPE_SKILL,
        batata_common::ai_trace::OP_OFFLINE_VERSION,
        Some(&form.skill_name),
        form.version.as_deref(),
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

/// PUT /v3/console/ai/skills/scope — Update scope
#[put("/scope")]
async fn update_scope(
    req: HttpRequest,
    data: web::Data<AppState>,
    skill_service: web::Data<Arc<dyn SkillService>>,
    body: web::Json<SkillScopeForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/skills")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let form = body.into_inner();
    let ns = normalize_namespace(&form.namespace_id);

    let result = skill_service
        .update_scope(ns, &form.skill_name, &form.scope, Some(get_username(&req).as_str()))
        .await;
    trace_write(
        &req,
        batata_common::ai_trace::RESOURCE_TYPE_SKILL,
        batata_common::ai_trace::OP_UPDATE_SCOPE,
        Some(&form.skill_name),
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

/// Register the skill management routes under `/ai/skills`.
pub fn routes() -> Scope {
    web::scope("/ai/skills")
        .service(list_skills)
        .service(download_skill_version)
        .service(get_skill_version)
        .service(upload_skill)
        .service(precheck_upload_skill)
        .service(batch_upload_skills)
        .service(create_draft)
        .service(update_draft)
        .service(delete_draft)
        .service(submit_skill)
        .service(publish_skill)
        .service(force_publish_skill)
        .service(redraft_skill)
        .service(update_labels)
        .service(update_biz_tags)
        .service(online_skill)
        .service(offline_skill)
        .service(update_scope)
        .service(get_skill_detail)
        .service(delete_skill)
}
