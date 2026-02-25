//! V2 Capacity API endpoints
//!
//! Provides capacity quota management endpoints for configurations.

use actix_web::{HttpMessage, HttpRequest, HttpResponse, Responder, delete, get, post, web};
use serde::{Deserialize, Serialize};

use crate::model::common::AppState;
use crate::{ActionTypes, ApiType, Secured, SignType, secured};

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

fn cap_to_data(cap: &batata_persistence::CapacityInfo, is_tenant: bool) -> CapacityData {
    CapacityData {
        id: cap.id,
        tenant: if is_tenant {
            Some(cap.identifier.clone())
        } else {
            None
        },
        group: if is_tenant {
            None
        } else {
            Some(cap.identifier.clone())
        },
        quota: cap.quota,
        usage: cap.usage,
        max_size: cap.max_size,
        max_aggr_count: cap.max_aggr_count,
        max_aggr_size: cap.max_aggr_size,
        max_history_count: cap.max_history_count,
    }
}

/// Get capacity for tenant or group
#[get("")]
pub async fn get_capacity(
    req: HttpRequest,
    data: web::Data<AppState>,
    query: web::Query<CapacityRequest>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/capacity")
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::OpenApi)
            .build()
    );

    let persistence = data.persistence();

    if let Some(ref tenant) = query.tenant {
        match persistence.capacity_get_tenant(tenant).await {
            Ok(Some(cap)) => {
                HttpResponse::Ok().json(CapacityResponse::success(cap_to_data(&cap, true)))
            }
            Ok(None) => {
                let defaults = batata_persistence::CapacityInfo::default();
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
        match persistence.capacity_get_group(group).await {
            Ok(Some(cap)) => {
                HttpResponse::Ok().json(CapacityResponse::success(cap_to_data(&cap, false)))
            }
            Ok(None) => {
                let defaults = batata_persistence::CapacityInfo::default();
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
    req: HttpRequest,
    data: web::Data<AppState>,
    query: web::Query<CapacityRequest>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/capacity")
            .action(ActionTypes::Write)
            .sign_type(SignType::Config)
            .api_type(ApiType::OpenApi)
            .build()
    );

    let persistence = data.persistence();

    if let Some(ref tenant) = query.tenant {
        match persistence
            .capacity_upsert_tenant(
                tenant,
                query.quota,
                query.max_size,
                query.max_aggr_count,
                query.max_aggr_size,
                query.max_history_count,
            )
            .await
        {
            Ok(cap) => HttpResponse::Ok().json(CapacityResponse::success(cap_to_data(&cap, true))),
            Err(e) => HttpResponse::InternalServerError().json(CapacityResponse::error(
                500,
                format!("Failed to update capacity: {}", e),
            )),
        }
    } else if let Some(ref group) = query.group {
        match persistence
            .capacity_upsert_group(
                group,
                query.quota,
                query.max_size,
                query.max_aggr_count,
                query.max_aggr_size,
                query.max_history_count,
            )
            .await
        {
            Ok(cap) => HttpResponse::Ok().json(CapacityResponse::success(cap_to_data(&cap, false))),
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
    req: HttpRequest,
    data: web::Data<AppState>,
    query: web::Query<CapacityRequest>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/capacity")
            .action(ActionTypes::Write)
            .sign_type(SignType::Config)
            .api_type(ApiType::OpenApi)
            .build()
    );

    let persistence = data.persistence();

    if let Some(ref tenant) = query.tenant {
        match persistence.capacity_delete_tenant(tenant).await {
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
        match persistence.capacity_delete_group(group).await {
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
