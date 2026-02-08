use actix_web::{HttpResponse, Responder, Scope, get, web};
use serde::Serialize;

use crate::model::common::{AppState, Result};

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
