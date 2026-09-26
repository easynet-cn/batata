//! V3 Console health endpoints.
//!
//! Provides liveness/readiness probing and component health reporting.

// actix-web route macros (`#[get]`, `#[post]`, `#[delete]`, ...) expand to a
// struct that cannot carry a doc comment, which trips `missing_docs`. The
// generated struct is an internal implementation detail, so the lint is allowed
// for this module.
#![allow(missing_docs)]

use actix_web::{HttpResponse, Responder, Scope, get, web};
use serde::Serialize;

use batata_server_common::error;
use batata_server_common::model::AppState;
use batata_server_common::model::response::Result;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Overall health status of the server and its components.
pub struct HealthStatus {
    /// Overall health status (`UP` or `DOWN`).
    pub status: String,
    /// Health status of the database component.
    pub database: ComponentStatus,
    /// Health status of the cluster component.
    pub cluster: ClusterStatus,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Health status of a single component.
pub struct ComponentStatus {
    /// Status of the component (`UP` or `DOWN`).
    pub status: String,
    /// Optional human-readable detail message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Health status of the cluster component.
pub struct ClusterStatus {
    /// Status of the cluster (`UP` or `DOWN`).
    pub status: String,
    /// Number of members in the cluster.
    pub member_count: usize,
    /// Whether this node is the cluster leader.
    pub is_leader: bool,
}

impl ComponentStatus {
    /// Create an `UP` component status with no message.
    pub fn up() -> Self {
        Self {
            status: "UP".to_string(),
            message: None,
        }
    }

    /// Create a `DOWN` component status with the given error message.
    pub fn down(message: String) -> Self {
        Self {
            status: "DOWN".to_string(),
            message: Some(message),
        }
    }
}

#[get("/liveness")]
async fn liveness() -> web::Json<Result<String>> {
    web::Json(Result::<String>::success("ok".to_string()))
}

#[get("/readiness")]
async fn readiness(data: web::Data<AppState>) -> impl Responder {
    if !data.server_status.is_up() {
        let status = data.server_status.status().to_string();
        return Result::<String>::http_response(
            503,
            error::SERVER_ERROR.code,
            format!("server is {} now, please try again later!", status),
            String::new(),
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
            "not ready".to_string(),
            String::new(),
        )
    }
}

#[get("/detailed")]
async fn health_detailed(data: web::Data<AppState>) -> HttpResponse {
    let mut components = serde_json::Map::new();

    // Database health
    let db_status = if let Some(ref persistence) = data.persistence {
        let start = std::time::Instant::now();
        match persistence.health_check().await {
            Ok(()) => {
                let latency = start.elapsed().as_millis();
                serde_json::json!({
                    "status": "UP",
                    "latencyMs": latency
                })
            }
            Err(e) => serde_json::json!({
                "status": "DOWN",
                "error": e.to_string()
            }),
        }
    } else {
        serde_json::json!({ "status": "UP", "type": "embedded" })
    };
    components.insert("database".to_string(), db_status);

    // Raft status
    let raft_status = if let Some(ref raft) = data.raft_node {
        let metrics = raft.metrics();
        serde_json::json!({
            "status": "UP",
            "role": format!("{:?}", metrics.state),
            "term": metrics.current_term,
            "lastLogIndex": metrics.last_log_index.unwrap_or(0),
            "lastApplied": metrics.last_applied.map(|l| l.index).unwrap_or(0),
        })
    } else {
        serde_json::json!({ "status": "N/A", "mode": "standalone" })
    };
    components.insert("raft".to_string(), raft_status);

    // Server status
    let server_state = data.server_status.status().to_string();

    // Cluster info
    let cluster_status = if let Some(ref cm) = data.cluster_manager {
        serde_json::json!({
            "status": "UP",
            "members": cm.member_count(),
            "standalone": cm.is_standalone(),
        })
    } else {
        serde_json::json!({ "status": "N/A" })
    };
    components.insert("cluster".to_string(), cluster_status);

    let overall_status = if server_state == "UP" {
        "UP"
    } else {
        &server_state
    };

    HttpResponse::Ok().json(serde_json::json!({
        "status": overall_status,
        "serverState": server_state,
        "components": components,
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

#[get("")]
async fn health_check(data: web::Data<AppState>) -> impl Responder {
    let ds = &data.console_datasource;
    let db_ready = ds.server_readiness().await;

    let db_status = if db_ready {
        ComponentStatus::up()
    } else {
        ComponentStatus::down("Database check failed".to_string())
    };

    let cluster_status = ClusterStatus {
        status: "UP".to_string(),
        member_count: ds.cluster_member_count(),
        is_leader: ds.cluster_is_leader(),
    };

    let overall_status = if db_status.status == "UP" && cluster_status.status == "UP" {
        "UP"
    } else {
        "DOWN"
    };

    let health_status = HealthStatus {
        status: overall_status.to_string(),
        database: db_status,
        cluster: cluster_status,
    };

    if overall_status == "UP" {
        HttpResponse::Ok().json(health_status)
    } else {
        HttpResponse::ServiceUnavailable().json(health_status)
    }
}

/// Register the health check routes under `/health`.
pub fn routes() -> Scope {
    web::scope("/health")
        .service(health_check)
        .service(health_detailed)
        .service(liveness)
        .service(readiness)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_component_status_up() {
        let status = ComponentStatus::up();
        assert_eq!(status.status, "UP");
        assert!(status.message.is_none());
    }

    #[test]
    fn test_component_status_down() {
        let status = ComponentStatus::down("connection refused".to_string());
        assert_eq!(status.status, "DOWN");
        assert_eq!(status.message, Some("connection refused".to_string()));
    }

    #[test]
    fn test_health_status_serialization() {
        let health = HealthStatus {
            status: "UP".to_string(),
            database: ComponentStatus::up(),
            cluster: ClusterStatus {
                status: "UP".to_string(),
                member_count: 3,
                is_leader: true,
            },
        };

        let json = serde_json::to_string(&health).unwrap();
        assert!(json.contains("\"status\":\"UP\""));
        assert!(json.contains("\"memberCount\":3"));
        assert!(json.contains("\"isLeader\":true"));
    }

    #[test]
    fn test_cluster_status_serialization() {
        let cluster = ClusterStatus {
            status: "DOWN".to_string(),
            member_count: 0,
            is_leader: false,
        };

        let json = serde_json::to_string(&cluster).unwrap();
        assert!(json.contains("\"status\":\"DOWN\""));
        assert!(json.contains("\"memberCount\":0"));
        assert!(json.contains("\"isLeader\":false"));
    }
}
