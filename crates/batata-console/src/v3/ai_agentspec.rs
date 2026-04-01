// Console AgentSpec management API endpoints
// Aligned with Nacos V3 Console API contract
// Mirrors admin endpoints under /v3/console/ai/agentspecs with ConsoleApi security

use std::sync::Arc;

use actix_web::{
    HttpMessage, HttpRequest, HttpResponse, Responder, Scope, delete, get, post, put, web,
};

use batata_common::AgentSpecService;
use batata_common::DEFAULT_NAMESPACE_ID;
use batata_common::model::ai::agentspec::*;
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

fn get_username(req: &HttpRequest) -> String {
    req.extensions()
        .get::<batata_common::IdentityContext>()
        .map(|ctx| ctx.username.clone())
        .unwrap_or_default()
}

/// GET /v3/console/ai/agentspecs — Get agentspec detail
#[get("")]
async fn get_agentspec_detail(
    req: HttpRequest,
    data: web::Data<AppState>,
    agentspec_service: web::Data<Arc<dyn AgentSpecService>>,
    query: web::Query<AgentSpecForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/agentspecs")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);
    let name = match query.agent_spec_name.as_deref() {
        Some(n) if !n.is_empty() => n,
        _ => {
            return common_response::Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_MISSING,
                "agentSpecName is required",
            );
        }
    };

    match agentspec_service.get_detail(ns, name).await {
        Ok(Some(meta)) => HttpResponse::Ok().json(common_response::Result::success(meta)),
        Ok(None) => common_response::Result::<()>::http_not_found(
            &batata_common::error::RESOURCE_NOT_FOUND,
            format!("AgentSpec '{}' not found", name),
        ),
        Err(e) => common_response::Result::<()>::http_internal_error(e),
    }
}

/// GET /v3/console/ai/agentspecs/version — Get specific version detail
#[get("/version")]
async fn get_agentspec_version(
    req: HttpRequest,
    data: web::Data<AppState>,
    agentspec_service: web::Data<Arc<dyn AgentSpecService>>,
    query: web::Query<AgentSpecForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/agentspecs")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);
    let name = match query.agent_spec_name.as_deref() {
        Some(n) if !n.is_empty() => n,
        _ => {
            return common_response::Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_MISSING,
                "agentSpecName is required",
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

    match agentspec_service
        .get_version_detail(ns, name, version)
        .await
    {
        Ok(Some(spec)) => HttpResponse::Ok().json(common_response::Result::success(spec)),
        Ok(None) => common_response::Result::<()>::http_not_found(
            &batata_common::error::RESOURCE_NOT_FOUND,
            format!("AgentSpec '{}' version '{}' not found", name, version),
        ),
        Err(e) => common_response::Result::<()>::http_internal_error(e),
    }
}

/// GET /v3/console/ai/agentspecs/version/download — Download agentspec version (JSON)
#[get("/version/download")]
async fn download_agentspec_version(
    req: HttpRequest,
    data: web::Data<AppState>,
    agentspec_service: web::Data<Arc<dyn AgentSpecService>>,
    query: web::Query<AgentSpecForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/agentspecs")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);
    let name = match query.agent_spec_name.as_deref() {
        Some(n) if !n.is_empty() => n,
        _ => {
            return common_response::Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_MISSING,
                "agentSpecName is required",
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

    match agentspec_service
        .get_version_detail(ns, name, version)
        .await
    {
        Ok(Some(spec)) => HttpResponse::Ok().json(common_response::Result::success(spec)),
        Ok(None) => common_response::Result::<()>::http_not_found(
            &batata_common::error::RESOURCE_NOT_FOUND,
            format!("AgentSpec '{}' version '{}' not found", name, version),
        ),
        Err(e) => common_response::Result::<()>::http_internal_error(e),
    }
}

/// DELETE /v3/console/ai/agentspecs — Delete agentspec
#[delete("")]
async fn delete_agentspec(
    req: HttpRequest,
    data: web::Data<AppState>,
    agentspec_service: web::Data<Arc<dyn AgentSpecService>>,
    query: web::Query<AgentSpecForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/agentspecs")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);
    let name = match query.agent_spec_name.as_deref() {
        Some(n) if !n.is_empty() => n,
        _ => {
            return common_response::Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_MISSING,
                "agentSpecName is required",
            );
        }
    };

    match agentspec_service.delete(ns, name).await {
        Ok(()) => HttpResponse::Ok().json(common_response::Result::success(true)),
        Err(e) => common_response::Result::<()>::http_internal_error(e),
    }
}

/// GET /v3/console/ai/agentspecs/list — List agentspecs
#[get("/list")]
async fn list_agentspecs(
    req: HttpRequest,
    data: web::Data<AppState>,
    agentspec_service: web::Data<Arc<dyn AgentSpecService>>,
    query: web::Query<AgentSpecListForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/agentspecs")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
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
        Ok(page) => HttpResponse::Ok().json(common_response::Result::success(page)),
        Err(e) => common_response::Result::<()>::http_internal_error(e),
    }
}

/// Upload form for console (JSON body, not multipart)
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleAgentSpecUploadForm {
    #[serde(default)]
    pub namespace_id: String,
    pub agent_spec_name: String,
    pub agent_spec_card: Option<String>,
    #[serde(default)]
    pub overwrite: bool,
}

/// POST /v3/console/ai/agentspecs/upload — Upload agentspec from JSON body
#[post("/upload")]
async fn upload_agentspec(
    req: HttpRequest,
    data: web::Data<AppState>,
    agentspec_service: web::Data<Arc<dyn AgentSpecService>>,
    body: web::Json<ConsoleAgentSpecUploadForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/agentspecs")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let form = body.into_inner();
    let ns = normalize_namespace(&form.namespace_id);
    let overwrite = form.overwrite;
    let author = get_username(&req);

    let spec: AgentSpec = match form
        .agent_spec_card
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
    {
        Ok(Some(s)) => s,
        Ok(None) => {
            return common_response::Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_MISSING,
                "agentSpecCard is required",
            );
        }
        Err(e) => {
            return common_response::Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_VALIDATE_ERROR,
                format!("Invalid agentSpecCard JSON: {}", e),
            );
        }
    };

    let name = form.agent_spec_name.clone();

    match agentspec_service
        .upload(ns, &name, &spec, &author, overwrite)
        .await
    {
        Ok(spec_name) => HttpResponse::Ok().json(common_response::Result::success(spec_name)),
        Err(e) => common_response::Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// POST /v3/console/ai/agentspecs/draft — Create draft
#[post("/draft")]
async fn create_draft(
    req: HttpRequest,
    data: web::Data<AppState>,
    agentspec_service: web::Data<Arc<dyn AgentSpecService>>,
    body: web::Json<AgentSpecDraftCreateForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/agentspecs")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let form = body.into_inner();
    let ns = normalize_namespace(&form.namespace_id);
    let author = get_username(&req);

    let initial_content: Option<AgentSpec> = form
        .agent_spec_card
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok());

    // Resolve name: form param takes priority, then from agentSpecCard JSON
    let agent_spec_name = form
        .agent_spec_name
        .as_deref()
        .filter(|n| !n.is_empty())
        .or_else(|| initial_content.as_ref().map(|s| s.name.as_str()))
        .unwrap_or("");

    if agent_spec_name.is_empty() {
        return common_response::Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_MISSING,
            "agentSpecName or agentSpecCard with name is required",
        );
    }

    match agentspec_service
        .create_draft(
            ns,
            agent_spec_name,
            form.based_on_version.as_deref(),
            form.target_version.as_deref(),
            initial_content.as_ref(),
            &author,
        )
        .await
    {
        Ok(version) => HttpResponse::Ok().json(common_response::Result::success(version)),
        Err(e) => common_response::Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// PUT /v3/console/ai/agentspecs/draft — Update draft
#[put("/draft")]
async fn update_draft(
    req: HttpRequest,
    data: web::Data<AppState>,
    agentspec_service: web::Data<Arc<dyn AgentSpecService>>,
    body: web::Json<AgentSpecUpdateForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/agentspecs")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
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
            return common_response::Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_MISSING,
                "agentSpecCard is required",
            );
        }
        Err(e) => {
            return common_response::Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_VALIDATE_ERROR,
                format!("Invalid agentSpecCard JSON: {}", e),
            );
        }
    };

    // Resolve name: form param takes priority, then from agentSpecCard JSON content
    let agent_spec_name = form
        .agent_spec_name
        .as_deref()
        .filter(|n| !n.is_empty())
        .or_else(|| {
            if !spec.name.is_empty() {
                Some(spec.name.as_str())
            } else {
                None
            }
        })
        .unwrap_or("");

    if agent_spec_name.is_empty() {
        return common_response::Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_MISSING,
            "agentSpecName or agentSpecCard with name is required",
        );
    }

    match agentspec_service
        .update_draft(ns, agent_spec_name, &spec)
        .await
    {
        Ok(()) => HttpResponse::Ok().json(common_response::Result::success(true)),
        Err(e) => common_response::Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// DELETE /v3/console/ai/agentspecs/draft — Delete draft
#[delete("/draft")]
async fn delete_draft(
    req: HttpRequest,
    data: web::Data<AppState>,
    agentspec_service: web::Data<Arc<dyn AgentSpecService>>,
    query: web::Query<AgentSpecForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/agentspecs")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let ns = normalize_namespace(&query.namespace_id);
    let name = match query.agent_spec_name.as_deref() {
        Some(n) if !n.is_empty() => n,
        _ => {
            return common_response::Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_MISSING,
                "agentSpecName is required",
            );
        }
    };

    match agentspec_service.delete_draft(ns, name).await {
        Ok(()) => HttpResponse::Ok().json(common_response::Result::success(true)),
        Err(e) => common_response::Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// POST /v3/console/ai/agentspecs/submit — Submit for review
#[post("/submit")]
async fn submit_agentspec(
    req: HttpRequest,
    data: web::Data<AppState>,
    agentspec_service: web::Data<Arc<dyn AgentSpecService>>,
    body: web::Json<AgentSpecSubmitForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/agentspecs")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let form = body.into_inner();
    let ns = normalize_namespace(&form.namespace_id);

    match agentspec_service
        .submit(ns, &form.agent_spec_name, &form.version)
        .await
    {
        Ok(version) => HttpResponse::Ok().json(common_response::Result::success(version)),
        Err(e) => common_response::Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// POST /v3/console/ai/agentspecs/publish — Publish version
#[post("/publish")]
async fn publish_agentspec(
    req: HttpRequest,
    data: web::Data<AppState>,
    agentspec_service: web::Data<Arc<dyn AgentSpecService>>,
    body: web::Json<AgentSpecPublishForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/agentspecs")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
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
        Ok(()) => HttpResponse::Ok().json(common_response::Result::success(true)),
        Err(e) => common_response::Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// PUT /v3/console/ai/agentspecs/labels — Update labels
#[put("/labels")]
async fn update_labels(
    req: HttpRequest,
    data: web::Data<AppState>,
    agentspec_service: web::Data<Arc<dyn AgentSpecService>>,
    body: web::Json<AgentSpecLabelsUpdateForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/agentspecs")
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

    match agentspec_service
        .update_labels(ns, &form.agent_spec_name, labels)
        .await
    {
        Ok(()) => HttpResponse::Ok().json(common_response::Result::success(true)),
        Err(e) => common_response::Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// PUT /v3/console/ai/agentspecs/biz-tags — Update business tags
#[put("/biz-tags")]
async fn update_biz_tags(
    req: HttpRequest,
    data: web::Data<AppState>,
    agentspec_service: web::Data<Arc<dyn AgentSpecService>>,
    body: web::Json<AgentSpecBizTagsUpdateForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/agentspecs")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let form = body.into_inner();
    let ns = normalize_namespace(&form.namespace_id);

    match agentspec_service
        .update_biz_tags(ns, &form.agent_spec_name, &form.biz_tags)
        .await
    {
        Ok(()) => HttpResponse::Ok().json(common_response::Result::success(true)),
        Err(e) => common_response::Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// POST /v3/console/ai/agentspecs/online — Online operation
#[post("/online")]
async fn online_agentspec(
    req: HttpRequest,
    data: web::Data<AppState>,
    agentspec_service: web::Data<Arc<dyn AgentSpecService>>,
    body: web::Json<AgentSpecOnlineForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/agentspecs")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
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
        Ok(()) => HttpResponse::Ok().json(common_response::Result::success(true)),
        Err(e) => common_response::Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// POST /v3/console/ai/agentspecs/offline — Offline operation
#[post("/offline")]
async fn offline_agentspec(
    req: HttpRequest,
    data: web::Data<AppState>,
    agentspec_service: web::Data<Arc<dyn AgentSpecService>>,
    body: web::Json<AgentSpecOnlineForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/agentspecs")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
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
        Ok(()) => HttpResponse::Ok().json(common_response::Result::success(true)),
        Err(e) => common_response::Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

/// PUT /v3/console/ai/agentspecs/scope — Update scope
#[put("/scope")]
async fn update_scope(
    req: HttpRequest,
    data: web::Data<AppState>,
    agentspec_service: web::Data<Arc<dyn AgentSpecService>>,
    body: web::Json<AgentSpecScopeForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/agentspecs")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let form = body.into_inner();
    let ns = normalize_namespace(&form.namespace_id);

    match agentspec_service
        .update_scope(ns, &form.agent_spec_name, &form.scope)
        .await
    {
        Ok(()) => HttpResponse::Ok().json(common_response::Result::success(true)),
        Err(e) => common_response::Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

pub fn routes() -> Scope {
    web::scope("/ai/agentspecs")
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
