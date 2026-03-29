// Console Pipeline management API endpoints
// Mirrors admin pipeline endpoints under /v3/console/ai/pipelines

use std::sync::Arc;

use actix_web::{HttpRequest, HttpResponse, Responder, Scope, get, web};

use batata_ai::model::pipeline::*;
use batata_ai::service::pipeline_service::PipelineQueryService;
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::response as common_response;
use batata_server_common::secured::Secured;
use batata_server_common::{ActionTypes, ApiType, SignType, secured};

/// GET /v3/console/ai/pipelines/{pipelineId}
#[get("/{pipeline_id}")]
async fn get_pipeline(
    req: HttpRequest,
    data: web::Data<AppState>,
    pipeline_service: web::Data<Arc<PipelineQueryService>>,
    path: web::Path<String>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/pipelines")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let pipeline_id = path.into_inner();

    match pipeline_service.get_pipeline(&pipeline_id).await {
        Ok(Some(execution)) => HttpResponse::Ok().json(common_response::Result::success(execution)),
        Ok(None) => common_response::Result::<()>::http_not_found(
            &batata_common::error::RESOURCE_NOT_FOUND,
            format!("Pipeline execution '{}' not found", pipeline_id),
        ),
        Err(e) => common_response::Result::<()>::http_internal_error(e),
    }
}

/// GET /v3/console/ai/pipelines
#[get("")]
async fn list_pipelines(
    req: HttpRequest,
    data: web::Data<AppState>,
    pipeline_service: web::Data<Arc<PipelineQueryService>>,
    query: web::Query<PipelineListForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/pipelines")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let form = query.into_inner();
    if form.resource_type.is_empty() {
        return common_response::Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_MISSING,
            "resourceType is required",
        );
    }

    match pipeline_service
        .list_pipelines(
            &form.resource_type,
            form.resource_name.as_deref(),
            form.namespace_id.as_deref(),
            form.version.as_deref(),
            form.page_no,
            form.page_size,
        )
        .await
    {
        Ok(page) => HttpResponse::Ok().json(common_response::Result::success(page)),
        Err(e) => common_response::Result::<()>::http_internal_error(e),
    }
}

pub fn routes() -> Scope {
    web::scope("/ai/pipelines")
        .service(list_pipelines)
        .service(get_pipeline)
}
