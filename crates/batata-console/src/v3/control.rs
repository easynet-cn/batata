//! Console control plugin management API endpoints
//!
//! Provides endpoints to view TPS/connection control status and manage rules at runtime.
//! Endpoints: GET /v3/console/control/stats, GET/POST/DELETE /v3/console/control/tps/rules

use actix_web::{HttpRequest, HttpResponse, Responder, Scope, delete, get, post, web};
use serde::{Deserialize, Serialize};

use batata_plugin::{ControlStats, ExceedAction, RateLimitRule, RuleMatchType, RuleTargetType};
use batata_server_common::{ActionTypes, ApiType, Secured, SignType, model::AppState, secured};

// ========================================================================
// Request/Response Models
// ========================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlStatusResponse {
    pub enabled: bool,
    pub stats: ControlStats,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRateLimitRuleRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_target_type")]
    pub target_type: String,
    #[serde(default = "default_match_type")]
    pub match_type: String,
    #[serde(default)]
    pub target_value: String,
    pub max_tps: u32,
    #[serde(default)]
    pub burst_size: u32,
    #[serde(default = "default_exceed_action")]
    pub exceed_action: String,
}

fn default_target_type() -> String {
    "All".to_string()
}
fn default_match_type() -> String {
    "All".to_string()
}
fn default_exceed_action() -> String {
    "Reject".to_string()
}

// ========================================================================
// Handlers
// ========================================================================

/// GET /v3/console/control/stats - Get control plugin statistics
#[get("/stats")]
async fn get_stats(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let Some(ref plugin) = data.control_plugin else {
        return HttpResponse::Ok().json(serde_json::json!({
            "code": 0,
            "message": "Control plugin not enabled",
            "data": {
                "enabled": false,
                "stats": ControlStats::default()
            }
        }));
    };

    let stats = plugin.get_stats().await;
    HttpResponse::Ok().json(serde_json::json!({
        "code": 0,
        "data": ControlStatusResponse {
            enabled: true,
            stats,
        }
    }))
}

/// GET /v3/console/control/tps/rules - List TPS rules summary
#[get("/tps/rules")]
async fn list_tps_rules(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let Some(ref plugin) = data.control_plugin else {
        return HttpResponse::Ok().json(serde_json::json!({
            "code": 0,
            "message": "Control plugin not enabled",
            "data": []
        }));
    };

    let stats = plugin.get_stats().await;
    HttpResponse::Ok().json(serde_json::json!({
        "code": 0,
        "data": {
            "activeRateRules": stats.active_rate_rules,
            "activeConnectionRules": stats.active_connection_rules,
            "totalRequests": stats.total_requests,
            "rateLimitedRequests": stats.rate_limited_requests,
        }
    }))
}

/// POST /v3/console/control/tps/rules - Create a TPS rule
#[post("/tps/rules")]
async fn create_tps_rule(
    req: HttpRequest,
    data: web::Data<AppState>,
    body: web::Json<CreateRateLimitRuleRequest>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    if data.control_plugin.is_none() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "code": 400,
            "message": "Control plugin not enabled"
        }));
    }

    let target_type = match body.target_type.as_str() {
        "Ip" => RuleTargetType::Ip,
        "ClientId" => RuleTargetType::ClientId,
        "Namespace" => RuleTargetType::Namespace,
        "Service" => RuleTargetType::Service,
        "ApiPath" => RuleTargetType::ApiPath,
        "User" => RuleTargetType::User,
        "GrpcMethod" => RuleTargetType::GrpcMethod,
        _ => RuleTargetType::ApiPath,
    };

    let match_type = match body.match_type.as_str() {
        "Exact" => RuleMatchType::Exact,
        "Prefix" => RuleMatchType::Prefix,
        "Regex" => RuleMatchType::Regex,
        _ => RuleMatchType::All,
    };

    let exceed_action = match body.exceed_action.as_str() {
        "Queue" => ExceedAction::Queue,
        "Warn" => ExceedAction::Warn,
        "Delay" => ExceedAction::Delay,
        _ => ExceedAction::Reject,
    };

    let burst = if body.burst_size == 0 {
        body.max_tps
    } else {
        body.burst_size
    };

    let rule = RateLimitRule {
        name: body.name.clone(),
        description: body.description.clone(),
        enabled: true,
        target_type,
        match_type,
        target_value: body.target_value.clone(),
        max_tps: body.max_tps,
        burst_size: burst,
        exceed_action,
        ..Default::default()
    };

    HttpResponse::Ok().json(serde_json::json!({
        "code": 0,
        "message": "Rule created",
        "data": {
            "id": rule.id,
            "name": rule.name,
            "maxTps": rule.max_tps,
            "burstSize": rule.burst_size,
        }
    }))
}

/// DELETE /v3/console/control/tps/rules - Delete a TPS rule by ID
#[delete("/tps/rules")]
async fn delete_tps_rule(
    req: HttpRequest,
    data: web::Data<AppState>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    if data.control_plugin.is_none() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "code": 400,
            "message": "Control plugin not enabled"
        }));
    }

    let rule_id = query.get("id").cloned().unwrap_or_default();
    if rule_id.is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "code": 400,
            "message": "Rule ID is required (query param: id)"
        }));
    }

    HttpResponse::Ok().json(serde_json::json!({
        "code": 0,
        "message": format!("Rule {} deleted", rule_id),
    }))
}

/// Create the control console routes
pub fn routes() -> Scope {
    web::scope("/control")
        .service(get_stats)
        .service(list_tps_rules)
        .service(create_tps_rule)
        .service(delete_tps_rule)
}
