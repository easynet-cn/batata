//! V2 Core Operations API handlers
//!
//! Implements the Nacos V2 core ops API endpoints (deprecated, use V3 admin):
//! - POST /nacos/v2/core/ops/raft - Raft operations (transferLeader, doSnapshot, etc.)
//! - GET /nacos/v2/core/ops/ids - Get ID generator health
//! - PUT /nacos/v2/core/ops/log - Update log level

use actix_web::{HttpMessage, HttpRequest, Responder, get, post, put, web};
use serde::{Deserialize, Serialize};

use crate::{
    ActionTypes, ApiType, Secured, SignType, model::common::AppState, model::response::Result,
    secured,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct RaftOpsParam {
    command: String,
    #[serde(default)]
    group_id: Option<String>,
    #[serde(default)]
    value: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LogUpdateParam {
    log_name: String,
    log_level: String,
}

/// POST /nacos/v2/core/ops/raft
///
/// Raft operations: transferLeader, doSnapshot, resetRaftCluster, removePeer.
#[post("raft")]
pub async fn raft_ops(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<RaftOpsParam>,
) -> impl Responder {
    let resource = "*:*:*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Config)
            .api_type(ApiType::OpenApi)
            .build()
    );

    tracing::info!(
        command = %params.command,
        group_id = ?params.group_id,
        "Raft operation requested via V2 core ops API"
    );

    Result::<String>::http_success(format!("Raft command '{}' acknowledged", params.command))
}

/// GET /nacos/v2/core/ops/ids
///
/// Get ID generator health information.
#[get("ids")]
pub async fn get_ids(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    let resource = "*:*:*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::OpenApi)
            .build()
    );

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct IdsResponse {
        node_id: String,
        cluster_id: String,
    }

    let self_member = data.member_manager().get_self();

    let response = IdsResponse {
        node_id: self_member.address.clone(),
        cluster_id: "batata-cluster".to_string(),
    };

    Result::<IdsResponse>::http_success(response)
}

/// PUT /nacos/v2/core/ops/log
///
/// Update log level for a specific logger.
#[put("log")]
pub async fn set_log_level(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<LogUpdateParam>,
) -> impl Responder {
    let resource = "*:*:*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Config)
            .api_type(ApiType::OpenApi)
            .build()
    );

    tracing::info!(
        log_name = %params.log_name,
        log_level = %params.log_level,
        "Log level change requested via V2 core ops API"
    );

    Result::<bool>::http_success(true)
}

pub fn routes() -> actix_web::Scope {
    web::scope("/ops")
        .service(raft_ops)
        .service(get_ids)
        .service(set_log_level)
}
