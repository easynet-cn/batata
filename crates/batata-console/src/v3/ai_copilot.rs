// Console Copilot API endpoints
// Provides AI-assisted skill/prompt optimization via SSE streaming
// Endpoints under /v3/console/copilot

use actix_web::{HttpRequest, HttpResponse, Responder, Scope, get, post, web};
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::response as common_response;
use batata_server_common::secured::Secured;
use batata_server_common::{ActionTypes, ApiType, SignType, secured};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Model structs
// ---------------------------------------------------------------------------

/// Copilot configuration (API key, model, etc.)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopilotConfig {
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub studio_url: Option<String>,
    pub studio_project: Option<String>,
}

/// Request body for skill optimization
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillOptimizationForm {
    pub skill: Option<serde_json::Value>,
    pub optimization_goal: Option<String>,
    pub target_file_name: Option<String>,
}

/// Request body for skill generation
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillGenerateForm {
    pub background_info: Option<String>,
}

/// Request body for prompt optimization
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptOptimizeForm {
    pub prompt: Option<String>,
    pub optimization_goal: Option<String>,
}

/// Request body for prompt debugging
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptDebugForm {
    pub prompt: Option<String>,
    pub user_input: Option<String>,
}

// ---------------------------------------------------------------------------
// SSE helper
// ---------------------------------------------------------------------------

/// Build a stub SSE response indicating copilot is not configured.
fn stub_sse_response() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/event-stream")
        .insert_header(("Cache-Control", "no-cache"))
        .insert_header(("Connection", "keep-alive"))
        .body("data: {\"done\":true,\"explanation\":\"Copilot is not configured. Please set API key in Copilot Config.\"}\n\n")
}

// ---------------------------------------------------------------------------
// SSE streaming endpoints
// ---------------------------------------------------------------------------

/// POST /v3/console/copilot/skill/optimize — Optimize a skill via SSE
#[post("/skill/optimize")]
async fn skill_optimize(
    req: HttpRequest,
    data: web::Data<AppState>,
    _body: web::Json<SkillOptimizationForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/copilot")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    stub_sse_response()
}

/// POST /v3/console/copilot/skill/generate — Generate a skill via SSE
#[post("/skill/generate")]
async fn skill_generate(
    req: HttpRequest,
    data: web::Data<AppState>,
    _body: web::Json<SkillGenerateForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/copilot")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    stub_sse_response()
}

/// POST /v3/console/copilot/prompt/optimize — Optimize a prompt via SSE
#[post("/prompt/optimize")]
async fn prompt_optimize(
    req: HttpRequest,
    data: web::Data<AppState>,
    _body: web::Json<PromptOptimizeForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/copilot")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    stub_sse_response()
}

/// POST /v3/console/copilot/prompt/debug — Debug a prompt via SSE
#[post("/prompt/debug")]
async fn prompt_debug(
    req: HttpRequest,
    data: web::Data<AppState>,
    _body: web::Json<PromptDebugForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/copilot")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    stub_sse_response()
}

// ---------------------------------------------------------------------------
// Config endpoints
// ---------------------------------------------------------------------------

/// GET /v3/console/copilot/config — Get copilot configuration
#[get("/config")]
async fn get_config(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/copilot")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    HttpResponse::Ok().json(common_response::Result::success(CopilotConfig::default()))
}

/// POST /v3/console/copilot/config — Save copilot configuration
#[post("/config")]
async fn save_config(
    req: HttpRequest,
    data: web::Data<AppState>,
    _body: web::Json<CopilotConfig>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/copilot")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    // Stub: config is not persisted yet
    HttpResponse::Ok().json(common_response::Result::success(true))
}

// ---------------------------------------------------------------------------
// Route registration
// ---------------------------------------------------------------------------

pub fn routes() -> Scope {
    web::scope("/copilot")
        .service(skill_optimize)
        .service(skill_generate)
        .service(prompt_optimize)
        .service(prompt_debug)
        .service(get_config)
        .service(save_config)
}
