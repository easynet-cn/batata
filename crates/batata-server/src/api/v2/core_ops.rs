//! V2 Core Operations API handlers
//!
//! Implements the Nacos V2 core ops API endpoints (deprecated, use V3 admin):
//! - POST /nacos/v2/core/ops/raft - Raft operations (transferLeader, doSnapshot, etc.)
//! - GET /nacos/v2/core/ops/ids - Get ID generator health
//! - PUT /nacos/v2/core/ops/log - Update log level

use actix_web::{HttpRequest, Responder, get, post, put, web};

use crate::{ApiType, model::common::AppState};

use super::super::shared::core_ops::{
    LogUpdateParam, RaftOpsParam, do_get_ids, do_raft_ops, do_set_log_level,
};

/// POST /nacos/v2/core/ops/raft
#[post("raft")]
pub async fn raft_ops(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<RaftOpsParam>,
) -> impl Responder {
    do_raft_ops(&req, &data, &params, ApiType::OpenApi).await
}

/// GET /nacos/v2/core/ops/ids
#[get("ids")]
pub async fn get_ids(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    do_get_ids(&req, &data, ApiType::OpenApi).await
}

/// PUT /nacos/v2/core/ops/log
#[put("log")]
pub async fn set_log_level(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<LogUpdateParam>,
) -> impl Responder {
    do_set_log_level(&req, &data, &params, ApiType::OpenApi).await
}

pub fn routes() -> actix_web::Scope {
    web::scope("/ops")
        .service(raft_ops)
        .service(get_ids)
        .service(set_log_level)
}
