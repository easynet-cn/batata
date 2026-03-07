use actix_web::{HttpResponse, Responder, Scope, get, web};
use serde::Serialize;

use batata_server_common::error;
use batata_server_common::model::AppState;
use batata_server_common::model::response::Result;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthStatus {
    pub status: String,
    pub database: ComponentStatus,
    pub cluster: ClusterStatus,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentStatus {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClusterStatus {
    pub status: String,
    pub member_count: usize,
    pub is_leader: bool,
}

impl ComponentStatus {
    pub fn up() -> Self {
        Self {
            status: "UP".to_string(),
            message: None,
        }
    }

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

pub fn routes() -> Scope {
    web::scope("/health")
        .service(health_check)
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
