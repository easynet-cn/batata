//! V3 Admin server state endpoints

use std::collections::HashMap;
use std::sync::Arc;

use actix_web::{Responder, get, web};

use crate::{error, model::common::AppState, model::response::Result};

/// GET /v3/admin/core/state
///
/// Returns server state information as a key-value map.
/// No authentication required - matches Nacos ServerStateController behavior (no @Secured).
#[get("")]
async fn get_state(data: web::Data<AppState>) -> impl Responder {
    let mut state = HashMap::new();
    state.extend(data.env_state());
    state.extend(data.config_state());
    state.extend(data.auth_state(true));
    state.extend(data.plugin_state());

    Result::<HashMap<String, Option<String>>>::http_success(state)
}

/// GET /v3/admin/core/state/liveness
///
/// Kubernetes-compatible liveness probe. Returns "ok" if the server process is alive.
#[get("liveness")]
async fn liveness() -> impl Responder {
    Result::<String>::http_success("ok".to_string())
}

/// GET /v3/admin/core/state/readiness
///
/// Kubernetes-compatible readiness probe. Checks that the server is ready to accept requests.
#[get("readiness")]
async fn readiness(data: web::Data<AppState>) -> impl Responder {
    if !data.server_status.is_up() {
        let status = data.server_status.status().to_string();
        return Result::<String>::http_response(
            503,
            error::SERVER_ERROR.code,
            format!("server is {} now, please try again later!", status),
            "not ready".to_string(),
        );
    }

    let ds = &data.console_datasource;
    let db_ready = ds.server_readiness().await;

    if db_ready {
        Result::<String>::http_success("ok".to_string())
    } else {
        Result::<String>::http_response(
            500,
            error::SERVER_ERROR.code,
            "Server is not ready".to_string(),
            "not ready".to_string(),
        )
    }
}

/// GET /v3/admin/core/state/servers
///
/// Returns per-server health status for all registered server components
/// (SDK gRPC, Cluster gRPC, Raft, Main HTTP, Console HTTP, etc.).
#[get("servers")]
async fn servers(registry: web::Data<Arc<batata_core::ServerRegistry>>) -> impl Responder {
    let health = registry.health();
    Result::<Vec<batata_core::ServerHealthInfo>>::http_success(health)
}

pub fn routes() -> actix_web::Scope {
    web::scope("/state")
        .service(get_state)
        .service(liveness)
        .service(readiness)
        .service(servers)
}
