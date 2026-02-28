//! V2 Console Health API handlers
//!
//! Implements the Nacos V2 console health endpoints:
//! - GET /v2/console/health/liveness - Check if server is alive
//! - GET /v2/console/health/readiness - Check if server is ready

use actix_web::{Responder, Scope, get, web};

use batata_server_common::model::AppState;
use batata_server_common::model::response::Result;

/// GET /v2/console/health/liveness
///
/// Returns HTTP 200 if server is alive and not in broken state.
#[get("liveness")]
pub async fn liveness() -> impl Responder {
    Result::<String>::http_success("ok".to_string())
}

/// GET /v2/console/health/readiness
///
/// Checks module health and returns 200 if ready, 500 if not ready.
#[get("readiness")]
pub async fn readiness(data: web::Data<AppState>) -> impl Responder {
    let ds = &data.console_datasource;
    let db_ready = ds.server_readiness().await;

    if db_ready {
        Result::<String>::http_success("ok".to_string())
    } else {
        Result::<String>::http_response(500, 500, "server is not ready".to_string(), String::new())
    }
}

pub fn routes() -> Scope {
    web::scope("/health").service(liveness).service(readiness)
}
