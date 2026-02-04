//! V2 Capacity API endpoints
//!
//! Provides capacity quota management endpoints for configurations.

use actix_web::{HttpResponse, Responder, delete, get, post, web};
use serde::{Deserialize, Serialize};

use batata_config::service::capacity;

use crate::model::common::AppState;

/// Capacity request parameters
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapacityRequest {
    /// Tenant ID (namespace)
    pub tenant: Option<String>,
    /// Group ID
    pub group: Option<String>,
    /// Maximum configs quota
    pub quota: Option<u32>,
    /// Maximum config size in bytes
    pub max_size: Option<u32>,
    /// Maximum aggregate config count
    pub max_aggr_count: Option<u32>,
    /// Maximum aggregate config size
    pub max_aggr_size: Option<u32>,
    /// Maximum history count
    pub max_history_count: Option<u32>,
}

/// Capacity response
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapacityResponse {
    pub code: i32,
    pub message: String,
    pub data: Option<CapacityData>,
}

/// Capacity data
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapacityData {
    pub id: Option<u64>,
    pub tenant: Option<String>,
    pub group: Option<String>,
    pub quota: u32,
    pub usage: u32,
    pub max_size: u32,
    pub max_aggr_count: u32,
    pub max_aggr_size: u32,
    pub max_history_count: u32,
}

impl CapacityResponse {
    fn success(data: CapacityData) -> Self {
        Self {
            code: 0,
            message: "success".to_string(),
            data: Some(data),
        }
    }

    fn success_empty() -> Self {
        Self {
            code: 0,
            message: "success".to_string(),
            data: None,
        }
    }

    fn error(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }
}

/// Get capacity for tenant or group
#[get("")]
pub async fn get_capacity(
    data: web::Data<AppState>,
    query: web::Query<CapacityRequest>,
) -> impl Responder {
    let db = data.db();

    if let Some(ref tenant) = query.tenant {
        match capacity::get_tenant_capacity(db, tenant).await {
            Ok(Some(cap)) => HttpResponse::Ok().json(CapacityResponse::success(CapacityData {
                id: cap.id,
                tenant: Some(cap.identifier),
                group: None,
                quota: cap.quota,
                usage: cap.usage,
                max_size: cap.max_size,
                max_aggr_count: cap.max_aggr_count,
                max_aggr_size: cap.max_aggr_size,
                max_history_count: cap.max_history_count,
            })),
            Ok(None) => {
                // Return default capacity
                let defaults = capacity::CapacityInfo::default();
                HttpResponse::Ok().json(CapacityResponse::success(CapacityData {
                    id: None,
                    tenant: Some(tenant.clone()),
                    group: None,
                    quota: defaults.quota,
                    usage: 0,
                    max_size: defaults.max_size,
                    max_aggr_count: defaults.max_aggr_count,
                    max_aggr_size: defaults.max_aggr_size,
                    max_history_count: defaults.max_history_count,
                }))
            }
            Err(e) => HttpResponse::InternalServerError().json(CapacityResponse::error(
                500,
                format!("Failed to get capacity: {}", e),
            )),
        }
    } else if let Some(ref group) = query.group {
        match capacity::get_group_capacity(db, group).await {
            Ok(Some(cap)) => HttpResponse::Ok().json(CapacityResponse::success(CapacityData {
                id: cap.id,
                tenant: None,
                group: Some(cap.identifier),
                quota: cap.quota,
                usage: cap.usage,
                max_size: cap.max_size,
                max_aggr_count: cap.max_aggr_count,
                max_aggr_size: cap.max_aggr_size,
                max_history_count: cap.max_history_count,
            })),
            Ok(None) => {
                // Return default capacity
                let defaults = capacity::CapacityInfo::default();
                HttpResponse::Ok().json(CapacityResponse::success(CapacityData {
                    id: None,
                    tenant: None,
                    group: Some(group.clone()),
                    quota: defaults.quota,
                    usage: 0,
                    max_size: defaults.max_size,
                    max_aggr_count: defaults.max_aggr_count,
                    max_aggr_size: defaults.max_aggr_size,
                    max_history_count: defaults.max_history_count,
                }))
            }
            Err(e) => HttpResponse::InternalServerError().json(CapacityResponse::error(
                500,
                format!("Failed to get capacity: {}", e),
            )),
        }
    } else {
        HttpResponse::BadRequest().json(CapacityResponse::error(
            400,
            "Either tenant or group must be specified",
        ))
    }
}

/// Create or update capacity
#[post("")]
pub async fn update_capacity(
    data: web::Data<AppState>,
    query: web::Query<CapacityRequest>,
) -> impl Responder {
    let db = data.db();

    if let Some(ref tenant) = query.tenant {
        match capacity::upsert_tenant_capacity(
            db,
            tenant,
            query.quota,
            query.max_size,
            query.max_aggr_count,
            query.max_aggr_size,
            query.max_history_count,
        )
        .await
        {
            Ok(cap) => HttpResponse::Ok().json(CapacityResponse::success(CapacityData {
                id: cap.id,
                tenant: Some(cap.identifier),
                group: None,
                quota: cap.quota,
                usage: cap.usage,
                max_size: cap.max_size,
                max_aggr_count: cap.max_aggr_count,
                max_aggr_size: cap.max_aggr_size,
                max_history_count: cap.max_history_count,
            })),
            Err(e) => HttpResponse::InternalServerError().json(CapacityResponse::error(
                500,
                format!("Failed to update capacity: {}", e),
            )),
        }
    } else if let Some(ref group) = query.group {
        match capacity::upsert_group_capacity(
            db,
            group,
            query.quota,
            query.max_size,
            query.max_aggr_count,
            query.max_aggr_size,
            query.max_history_count,
        )
        .await
        {
            Ok(cap) => HttpResponse::Ok().json(CapacityResponse::success(CapacityData {
                id: cap.id,
                tenant: None,
                group: Some(cap.identifier),
                quota: cap.quota,
                usage: cap.usage,
                max_size: cap.max_size,
                max_aggr_count: cap.max_aggr_count,
                max_aggr_size: cap.max_aggr_size,
                max_history_count: cap.max_history_count,
            })),
            Err(e) => HttpResponse::InternalServerError().json(CapacityResponse::error(
                500,
                format!("Failed to update capacity: {}", e),
            )),
        }
    } else {
        HttpResponse::BadRequest().json(CapacityResponse::error(
            400,
            "Either tenant or group must be specified",
        ))
    }
}

/// Delete capacity
#[delete("")]
pub async fn delete_capacity(
    data: web::Data<AppState>,
    query: web::Query<CapacityRequest>,
) -> impl Responder {
    let db = data.db();

    if let Some(ref tenant) = query.tenant {
        match capacity::delete_tenant_capacity(db, tenant).await {
            Ok(deleted) => {
                if deleted {
                    HttpResponse::Ok().json(CapacityResponse::success_empty())
                } else {
                    HttpResponse::NotFound()
                        .json(CapacityResponse::error(404, "Tenant capacity not found"))
                }
            }
            Err(e) => HttpResponse::InternalServerError().json(CapacityResponse::error(
                500,
                format!("Failed to delete capacity: {}", e),
            )),
        }
    } else if let Some(ref group) = query.group {
        match capacity::delete_group_capacity(db, group).await {
            Ok(deleted) => {
                if deleted {
                    HttpResponse::Ok().json(CapacityResponse::success_empty())
                } else {
                    HttpResponse::NotFound()
                        .json(CapacityResponse::error(404, "Group capacity not found"))
                }
            }
            Err(e) => HttpResponse::InternalServerError().json(CapacityResponse::error(
                500,
                format!("Failed to delete capacity: {}", e),
            )),
        }
    } else {
        HttpResponse::BadRequest().json(CapacityResponse::error(
            400,
            "Either tenant or group must be specified",
        ))
    }
}
