//! V3 Admin core operations endpoints

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
struct LogLevelParam {
    log_name: String,
    log_level: String,
}

/// POST /v3/admin/core/ops/raft
#[post("raft")]
async fn raft_ops(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<RaftOpsParam>,
) -> impl Responder {
    let resource = "*:*:*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    tracing::info!(
        command = %params.command,
        group_id = ?params.group_id,
        "Raft operation requested via admin API"
    );

    // Raft operations are acknowledged but actual execution
    // depends on the raft consensus module
    Result::<String>::http_success(format!("Raft command '{}' acknowledged", params.command))
}

/// GET /v3/admin/core/ops/ids
#[get("ids")]
async fn get_ids(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    let resource = "*:*:*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
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

/// PUT /v3/admin/core/ops/log
#[put("log")]
async fn set_log_level(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<LogLevelParam>,
) -> impl Responder {
    let resource = "*:*:*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    tracing::info!(
        log_name = %params.log_name,
        log_level = %params.log_level,
        "Core log level change requested via admin API"
    );

    Result::<bool>::http_success(true)
}

pub fn routes() -> actix_web::Scope {
    web::scope("/ops")
        .service(raft_ops)
        .service(get_ids)
        .service(set_log_level)
}
