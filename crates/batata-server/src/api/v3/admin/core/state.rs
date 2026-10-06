//! V3 Admin server state endpoints

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use actix_web::{HttpRequest, Responder, get, put, web};
use serde::Deserialize;

use crate::{
    ActionTypes, ApiType, Secured, error, model::common::AppState, model::response::Result, secured,
};
use batata_common::server_status::ServerStatus;

/// Request body for [`put_state`].
///
/// Mirrors Nacos `SwitchDomain.overriddenServerStatus`: the field name is identical and the
/// value is a `ServerStatus` enum name (`UP`, `DOWN`, `STARTING`, `DRAINING`, `READ_ONLY`,
/// `WRITE_ONLY`, `PAUSED`). `null`, an empty string, or the literal `"null"` clears the
/// override — Nacos uses `update(overriddenServerStatus, "null")` for the same effect.
#[derive(Debug, Deserialize)]
struct OverrideStateBody {
    #[serde(rename = "overriddenServerStatus")]
    overridden_server_status: Option<String>,
}

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

    // Lifecycle status (Nacos `ServerStatusManager` parity). Nacos does not publish the
    // status through `/state` — it only returns configuration KV — but operators and
    // readiness tooling need it, and it is the same value the 5s refresher derives from
    // db + raft + distro readiness. Published additively, so Nacos clients that read
    // `/state` are unaffected.
    let status = data.server_status.status();
    state.insert("serverStatus".to_string(), Some(status.to_string()));

    // Startup phase. Unlike Nacos (which runs separate core / web / console / ai-registry
    // Spring contexts), batata is a single process, so the phase tracks subsystem
    // readiness: STARTING until the refresher marks the node UP, DRAINING while
    // shutting down.
    state.insert("startupPhase".to_string(), Some(status.to_string()));

    if let Some(reason) = data.server_status.error_msg().await {
        state.insert("serverStatusMessage".to_string(), Some(reason));
    }

    // Operator override (Nacos `switchDomain.overriddenServerStatus` parity). Nacos
    // publishes this through `/operator/switches`; we surface it here so operators and
    // status tooling see the same value the traffic gates use. Empty string when unset,
    // matching Nacos's `""` representation of a cleared override.
    let overridden = data.server_status.overridden();
    state.insert(
        "overriddenServerStatus".to_string(),
        Some(overridden.map(|s| s.to_string()).unwrap_or_default()),
    );

    Result::<HashMap<String, Option<String>>>::http_success(state)
}

/// PUT /v3/admin/core/state
///
/// Override the server status (Nacos `switchDomain.overriddenServerStatus` parity).
///
/// Requires admin authority — mirrors Nacos `@Secured(action = WRITE, apiType = ADMIN_API)`
/// on the operator switches endpoint. The body field is `overriddenServerStatus`:
/// a `ServerStatus` enum name to force, or `null`/empty/`"null"` to clear and let the
/// 5s refresher derive UP/DOWN from subsystem readiness again.
///
/// Unlike `GET`, this endpoint is authenticated because it mutates the traffic-gating
/// status that every SDK and peer request depends on.
#[put("")]
async fn put_state(
    req: HttpRequest,
    data: web::Data<AppState>,
    body: web::Json<OverrideStateBody>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/core/state")
            .action(ActionTypes::Write)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let value = body.into_inner().overridden_server_status;
    match value {
        // Clear the override (Nacos: `update(overriddenServerStatus, "null")`).
        None => {
            data.server_status.clear_overridden();
            Result::<String>::http_success("ok".to_string())
        }
        Some(v) => {
            if v.is_empty() || v.eq_ignore_ascii_case("null") {
                data.server_status.clear_overridden();
                Result::<String>::http_success("ok".to_string())
            } else {
                match ServerStatus::from_str(v.trim()) {
                    Ok(status) => {
                        data.server_status.set_overridden(status);
                        Result::<String>::http_success("ok".to_string())
                    }
                    Err(e) => {
                        Result::<String>::http_bad_request(&error::PARAMETER_VALIDATE_ERROR, e)
                    }
                }
            }
        }
    }
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
        // 503 (not 500): the server itself is healthy but not yet able to serve traffic,
        // which is exactly what a readiness probe must report. Matches the status used
        // for the "not UP" branch above.
        Result::<String>::http_response(
            503,
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

/// `routes` function.
///
/// # Returns
/// `actix_web :: Scope`.
pub fn routes() -> actix_web::Scope {
    web::scope("/state")
        .service(get_state)
        .service(put_state)
        .service(liveness)
        .service(readiness)
        .service(servers)
}
