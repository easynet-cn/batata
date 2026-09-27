//! Pipeline HTTP API handlers — Nacos 3.x compatible
//!
//! Admin: `/v3/admin/ai/pipelines`
//! Console: registered separately in batata-console
//!
//! Only the canonical endpoints are implemented:
//! - `GET /list` (`@Since("3.2.0")`)
//! - `GET /detail?pipelineId=` (`@Since("3.2.1")`)
//!
//! Upstream additionally exposes `GET /{pipelineId}` and `GET` (base path), but
//! both are marked `@Deprecated(since = "3.2.1", forRemoval = true)` and exist
//! only for legacy clients. Batata is unreleased and therefore does not carry
//! them.

use std::sync::Arc;

use actix_web::{HttpRequest, HttpResponse, Responder, get, web};

use batata_common::{ActionTypes, ApiType, SignType};
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::response::Result;
use batata_server_common::{Secured, secured};

use crate::model::pipeline::*;
use crate::service::traits::PipelineService;

/// GET /v3/admin/ai/pipelines/list — List pipeline executions
#[get("/list")]
async fn list_pipelines(
    req: HttpRequest,
    data: web::Data<AppState>,
    pipeline_service: web::Data<Arc<dyn PipelineService>>,
    query: web::Query<PipelineListForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let form = query.into_inner();
    if form.resource_type.is_empty() {
        return Result::<()>::http_bad_request(
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
        Ok(page) => HttpResponse::Ok().json(Result::success(page)),
        Err(e) => Result::<()>::http_internal_error(e),
    }
}

/// GET /v3/admin/ai/pipelines/detail?pipelineId= — Get pipeline execution by ID
#[get("/detail")]
async fn get_pipeline_detail(
    req: HttpRequest,
    data: web::Data<AppState>,
    pipeline_service: web::Data<Arc<dyn PipelineService>>,
    query: web::Query<PipelineDetailForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let form = query.into_inner();
    // Mirrors upstream `PipelineDetailForm.validate()`: a blank id is a
    // parameter validation error, not a "not found".
    if form.pipeline_id.is_empty() {
        return Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_MISSING,
            "Required parameter 'pipelineId' is missing",
        );
    }

    match pipeline_service.get_pipeline(&form.pipeline_id).await {
        Ok(Some(execution)) => HttpResponse::Ok().json(Result::success(execution)),
        Ok(None) => Result::<()>::http_not_found(
            &batata_common::error::RESOURCE_NOT_FOUND,
            format!("Pipeline execution '{}' not found", form.pipeline_id),
        ),
        Err(e) => Result::<()>::http_internal_error(e),
    }
}

/// Admin routes at `/v3/admin/ai/pipelines`
pub fn admin_routes() -> actix_web::Scope {
    web::scope("/pipelines")
        .service(list_pipelines)
        .service(get_pipeline_detail)
}
