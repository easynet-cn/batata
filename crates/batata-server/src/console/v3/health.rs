use actix_web::{HttpResponse, Responder, Scope, get, web};
use sea_orm::DatabaseConnection;
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

async fn check_database(db: &DatabaseConnection) -> ComponentStatus {
    use sea_orm::ConnectionTrait;
    match db.execute_unprepared("SELECT 1").await {
        Ok(_) => ComponentStatus::up(),
        Err(e) => ComponentStatus::down(e.to_string()),
    }
}

#[get("/liveness")]
async fn liveness() -> web::Json<Result<String>> {
    web::Json(Result::<String>::success("ok".to_string()))
}

#[get("/readiness")]
async fn readiness(data: web::Data<AppState>) -> impl Responder {
    let db_status = check_database(data.db()).await;

    let cluster_status = ClusterStatus {
        status: "UP".to_string(),
        member_count: data.member_manager().all_members().len(),
        is_leader: true, // Simplified - in production would check Raft leader status
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
    let db_status = check_database(data.db()).await;

    let cluster_status = ClusterStatus {
        status: "UP".to_string(),
        member_count: data.member_manager().all_members().len(),
        is_leader: true, // Simplified - in production would check Raft leader status
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
