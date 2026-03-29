//! Console HTTP API endpoints for Copilot
//!
//! All endpoints under `/v3/console/copilot`
//! SSE streaming for LLM interactions, JSON for config management.

use std::sync::Arc;

use actix_web::{HttpRequest, HttpResponse, Responder, get, post, web};
use futures::StreamExt;

use batata_common::{ActionTypes, ApiType, SignType};
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::response::Result;
use batata_server_common::{Secured, secured};

use crate::agent::CopilotAgentManager;
use crate::config::CopilotConfig;
use crate::model::*;
use crate::service::{
    PromptDebugService, PromptOptimizationService, SkillGenerationService, SkillOptimizationService,
};

// ============================================================================
// SSE Streaming Endpoints
// ============================================================================

/// POST /v3/console/copilot/skill/optimize — Optimize skill via LLM (SSE)
#[post("skill/optimize")]
async fn optimize_skill_stream(
    req: HttpRequest,
    data: web::Data<AppState>,
    service: web::Data<Arc<SkillOptimizationService>>,
    body: web::Json<SkillOptimizationRequest>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/copilot")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let request = body.into_inner();
    match service.optimize_stream(request).await {
        Ok(stream) => build_sse_response(stream),
        Err(e) => build_sse_error_response(&e.to_string()),
    }
}

/// POST /v3/console/copilot/skill/generate — Generate skill via LLM (SSE)
#[post("skill/generate")]
async fn generate_skill_stream(
    req: HttpRequest,
    data: web::Data<AppState>,
    service: web::Data<Arc<SkillGenerationService>>,
    body: web::Json<SkillGenerationRequest>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/copilot")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let request = body.into_inner();
    match service.generate_stream(request).await {
        Ok(stream) => build_sse_response(stream),
        Err(e) => build_sse_error_response(&e.to_string()),
    }
}

/// POST /v3/console/copilot/prompt/optimize — Optimize prompt via LLM (SSE)
#[post("prompt/optimize")]
async fn optimize_prompt_stream(
    req: HttpRequest,
    data: web::Data<AppState>,
    service: web::Data<Arc<PromptOptimizationService>>,
    body: web::Json<PromptOptimizationRequest>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/copilot")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let request = body.into_inner();
    match service.optimize_stream(request).await {
        Ok(stream) => build_sse_response(stream),
        Err(e) => build_sse_error_response(&e.to_string()),
    }
}

/// POST /v3/console/copilot/prompt/debug — Debug prompt via LLM (SSE)
#[post("prompt/debug")]
async fn debug_prompt_stream(
    req: HttpRequest,
    data: web::Data<AppState>,
    service: web::Data<Arc<PromptDebugService>>,
    body: web::Json<PromptDebugRequest>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/copilot")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let request = body.into_inner();
    match service.debug_stream(request).await {
        Ok(stream) => build_sse_response(stream),
        Err(e) => build_sse_error_response(&e.to_string()),
    }
}

// ============================================================================
// Config Endpoints
// ============================================================================

/// Simplified config response (only expose safe fields)
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CopilotConfigResponse {
    api_key: Option<String>,
    model: String,
    base_url: Option<String>,
    studio_url: Option<String>,
    studio_project: String,
}

/// Config update request
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct CopilotConfigUpdateRequest {
    api_key: Option<String>,
    model: Option<String>,
    base_url: Option<String>,
    studio_url: Option<String>,
    studio_project: Option<String>,
}

/// GET /v3/console/copilot/config — Get copilot configuration
#[get("config")]
async fn get_config(
    req: HttpRequest,
    data: web::Data<AppState>,
    agent_manager: web::Data<Arc<CopilotAgentManager>>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/copilot")
            .action(ActionTypes::Read)
            .sign_type(SignType::Ai)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let config = agent_manager.get_config().await;

    // Mask API key for security (show only last 4 chars)
    let masked_key = config.api_key.as_ref().map(|k| {
        if k.len() > 4 {
            format!("{}...{}", &k[..3], &k[k.len() - 4..])
        } else {
            "****".to_string()
        }
    });

    HttpResponse::Ok().json(Result::success(CopilotConfigResponse {
        api_key: masked_key,
        model: config.model,
        base_url: config.base_url,
        studio_url: config.studio_url,
        studio_project: config.studio_project,
    }))
}

/// POST /v3/console/copilot/config — Save copilot configuration
#[post("config")]
async fn save_config(
    req: HttpRequest,
    data: web::Data<AppState>,
    agent_manager: web::Data<Arc<CopilotAgentManager>>,
    body: web::Json<CopilotConfigUpdateRequest>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/copilot")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let update = body.into_inner();
    let config = CopilotConfig {
        enabled: true,
        api_key: update.api_key,
        model: update.model.unwrap_or_default(),
        base_url: update.base_url,
        studio_url: update.studio_url,
        studio_project: update.studio_project.unwrap_or_default(),
        default_namespace: String::new(),
    };

    match agent_manager.update_config(config).await {
        Ok(()) => HttpResponse::Ok().json(Result::success(true)),
        Err(e) => Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            e.to_string(),
        ),
    }
}

// ============================================================================
// SSE Response Helpers
// ============================================================================

fn build_sse_response(
    stream: std::pin::Pin<Box<dyn futures::Stream<Item = StreamChunk> + Send>>,
) -> HttpResponse {
    let body_stream = stream.map(|chunk| {
        let event = chunk.to_sse_event();
        Ok::<_, actix_web::Error>(actix_web::web::Bytes::from(event))
    });

    HttpResponse::Ok()
        .content_type("text/event-stream")
        .insert_header(("Cache-Control", "no-cache"))
        .insert_header(("Connection", "keep-alive"))
        .streaming(body_stream)
}

fn build_sse_error_response(message: &str) -> HttpResponse {
    let error_chunk = StreamChunk::error(message);
    HttpResponse::Ok()
        .content_type("text/event-stream")
        .insert_header(("Cache-Control", "no-cache"))
        .body(error_chunk.to_sse_event())
}

// ============================================================================
// Route Configuration
// ============================================================================

/// Console routes at `/v3/console/copilot`
pub fn console_routes() -> actix_web::Scope {
    web::scope("/copilot")
        .service(optimize_skill_stream)
        .service(generate_skill_stream)
        .service(optimize_prompt_stream)
        .service(debug_prompt_stream)
        .service(get_config)
        .service(save_config)
}
