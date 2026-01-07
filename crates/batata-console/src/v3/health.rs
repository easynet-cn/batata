//! Health check console endpoints

use std::sync::Arc;

use actix_web::{HttpResponse, Responder, Scope, get, web};
use sea_orm::{ConnectionTrait, DatabaseConnection};
use serde::Serialize;

use crate::datasource::ConsoleDataSource;

use super::namespace::ApiResult;

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
    match db.execute_unprepared("SELECT 1").await {
        Ok(_) => ComponentStatus::up(),
        Err(e) => ComponentStatus::down(e.to_string()),
    }
}

#[get("/liveness")]
pub async fn liveness() -> web::Json<ApiResult<String>> {
    web::Json(ApiResult::<String>::success("ok".to_string()))
}

#[get("/readiness")]
pub async fn readiness(datasource: web::Data<Arc<dyn ConsoleDataSource>>) -> impl Responder {
    let db_status = if let Some(db) = datasource.database() {
        check_database(db).await
    } else {
        // Remote mode - assume database is ok if we can reach the server
        ComponentStatus::up()
    };

    let cluster_status = ClusterStatus {
        status: "UP".to_string(),
        member_count: datasource.cluster_member_count(),
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
pub async fn health_check(datasource: web::Data<Arc<dyn ConsoleDataSource>>) -> impl Responder {
    let db_status = if let Some(db) = datasource.database() {
        check_database(db).await
    } else {
        ComponentStatus::up()
    };

    let cluster_status = ClusterStatus {
        status: "UP".to_string(),
        member_count: datasource.cluster_member_count(),
        is_leader: true,
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
