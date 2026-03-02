//! V3 Admin core operations endpoints

use actix_web::{HttpRequest, Responder, get, post, put, web};

use crate::{ApiType, model::common::AppState};

use crate::api::shared::core_ops::{
    LogUpdateParam, RaftOpsParam, do_get_ids, do_raft_ops, do_set_log_level,
};

/// POST /v3/admin/core/ops/raft
#[post("raft")]
async fn raft_ops(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<RaftOpsParam>,
) -> impl Responder {
    do_raft_ops(&req, &data, &params, ApiType::AdminApi).await
}

/// GET /v3/admin/core/ops/ids
#[get("ids")]
async fn get_ids(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    do_get_ids(&req, &data, ApiType::AdminApi).await
}

/// PUT /v3/admin/core/ops/log
#[put("log")]
async fn set_log_level(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<LogUpdateParam>,
) -> impl Responder {
    do_set_log_level(&req, &data, &params, ApiType::AdminApi).await
}

pub fn routes() -> actix_web::Scope {
    web::scope("/ops")
        .service(raft_ops)
        .service(get_ids)
        .service(set_log_level)
}
