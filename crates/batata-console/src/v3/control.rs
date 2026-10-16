//! Console control plugin management API endpoints
//!
//! Provides endpoints to view TPS/connection control status and manage rules at runtime.
//! Endpoints: GET /v3/console/control/stats, GET/POST/DELETE /v3/console/control/tps/rules

use actix_web::{HttpRequest, HttpResponse, Responder, Scope, delete, get, post, web};
use serde::{Deserialize, Serialize};

use batata_common::model::plugin::control::{
    ControlStats, ExceedAction, RateLimitRule, RuleMatchType, RuleTargetType,
};
use batata_server_common::{ActionTypes, ApiType, Secured, SignType, model::AppState, secured};

// ========================================================================
// Request/Response Models
// ========================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Response describing the current control plugin status.
pub struct ControlStatusResponse {
    /// Whether the control plugin is enabled.
    pub enabled: bool,
    /// Current control statistics.
    pub stats: ControlStats,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Request body for creating a rate limit rule.
pub struct CreateRateLimitRuleRequest {
    /// Name of the rate limit rule.
    pub name: String,
    /// Optional description of the rule.
    #[serde(default)]
    pub description: String,
    /// Target type the rule applies to (e.g. `Ip`, `Service`, `ApiPath`).
    #[serde(default = "default_target_type")]
    pub target_type: String,
    /// Match type used to evaluate the target (e.g. `Exact`, `Prefix`, `Regex`).
    #[serde(default = "default_match_type")]
    pub match_type: String,
    /// Target value to match against.
    #[serde(default)]
    pub target_value: String,
    /// Maximum allowed transactions per second.
    pub max_tps: u32,
    /// Maximum burst size allowed above `max_tps`.
    #[serde(default)]
    pub burst_size: u32,
    /// Action taken when the limit is exceeded (e.g. `Reject`, `Queue`, `Warn`).
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

/// Console-facing projection of a rate limit rule.
///
/// Uses camelCase like the rest of the console API and renders the enum
/// fields with their variant names, which is exactly what
/// `CreateRateLimitRuleRequest` accepts.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitRuleView {
    /// Unique rule ID.
    pub id: String,
    /// Rule name.
    pub name: String,
    /// Rule description.
    pub description: String,
    /// Whether the rule is enabled.
    pub enabled: bool,
    /// Rule priority (higher wins).
    pub priority: i32,
    /// Target type of the rule.
    pub target_type: String,
    /// Match type of the rule.
    pub match_type: String,
    /// Target value to match against.
    pub target_value: String,
    /// Maximum allowed transactions per second.
    pub max_tps: u32,
    /// Maximum burst size.
    pub burst_size: u32,
    /// Time window in seconds.
    pub window_seconds: u64,
    /// Action taken when the limit is exceeded.
    pub exceed_action: String,
    /// Creation timestamp (seconds since epoch).
    pub created_at: i64,
    /// Last update timestamp (seconds since epoch).
    pub updated_at: i64,
}

impl From<&RateLimitRule> for RateLimitRuleView {
    fn from(rule: &RateLimitRule) -> Self {
        Self {
            id: rule.id.clone(),
            name: rule.name.clone(),
            description: rule.description.clone(),
            enabled: rule.enabled,
            priority: rule.priority,
            target_type: format!("{:?}", rule.target_type),
            match_type: format!("{:?}", rule.match_type),
            target_value: rule.target_value.clone(),
            max_tps: rule.max_tps,
            burst_size: rule.burst_size,
            window_seconds: rule.window_seconds,
            exceed_action: format!("{:?}", rule.exceed_action),
            created_at: rule.created_at,
            updated_at: rule.updated_at,
        }
    }
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
    let rules = plugin.list_rate_rules().await.unwrap_or_default();

    HttpResponse::Ok().json(serde_json::json!({
        "code": 0,
        "data": {
            "rules": rules.iter().map(RateLimitRuleView::from).collect::<Vec<_>>(),
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

    let Some(ref plugin) = data.control_plugin else {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "code": 400,
            "message": "Control plugin not enabled"
        }));
    };

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

    // Persist through the plugin so the rule survives restarts and reaches
    // the in-memory cache used by the request path.
    let rule_id = match plugin.save_rate_rule(rule).await {
        Ok(id) => id,
        Err(e) => {
            tracing::warn!("Failed to save rate limit rule: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "code": 500,
                "message": format!("Failed to save rule: {}", e)
            }));
        }
    };

    HttpResponse::Ok().json(serde_json::json!({
        "code": 0,
        "message": "Rule created",
        "data": {
            "id": rule_id,
            "name": body.name,
            "maxTps": body.max_tps,
            "burstSize": burst,
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

    let Some(ref plugin) = data.control_plugin else {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "code": 400,
            "message": "Control plugin not enabled"
        }));
    };

    let rule_id = query.get("id").cloned().unwrap_or_default();
    if rule_id.is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "code": 400,
            "message": "Rule ID is required (query param: id)"
        }));
    }

    match plugin.delete_rate_rule(&rule_id).await {
        Ok(true) => HttpResponse::Ok().json(serde_json::json!({
            "code": 0,
            "message": format!("Rule {} deleted", rule_id),
        })),
        Ok(false) => HttpResponse::NotFound().json(serde_json::json!({
            "code": 404,
            "message": format!("Rule {} not found", rule_id),
        })),
        Err(e) => {
            tracing::warn!("Failed to delete rate limit rule {}: {}", rule_id, e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "code": 500,
                "message": format!("Failed to delete rule: {}", e)
            }))
        }
    }
}

/// Create the control console routes
pub fn routes() -> Scope {
    web::scope("/control")
        .service(get_stats)
        .service(list_tps_rules)
        .service(create_tps_rule)
        .service(delete_tps_rule)
}
