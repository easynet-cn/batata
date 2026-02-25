//! V3 Admin capacity management endpoints

use actix_web::{HttpResponse, Responder, get, post, web};
use serde::{Deserialize, Serialize};

use crate::model::common::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapacityRequest {
    pub tenant: Option<String>,
    pub group: Option<String>,
    pub quota: Option<u32>,
    pub max_size: Option<u32>,
    pub max_aggr_count: Option<u32>,
    pub max_aggr_size: Option<u32>,
    pub max_history_count: Option<u32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapacityResponse {
    pub code: i32,
    pub message: String,
    pub data: Option<CapacityData>,
}

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

/// GET /v3/admin/cs/capacity
#[get("")]
pub async fn get_capacity(
    data: web::Data<AppState>,
    query: web::Query<CapacityRequest>,
) -> impl Responder {
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

/// POST /v3/admin/cs/capacity
#[post("")]
pub async fn set_capacity(
    data: web::Data<AppState>,
    query: web::Query<CapacityRequest>,
) -> impl Responder {
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
                format!("Failed to set capacity: {}", e),
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
                format!("Failed to set capacity: {}", e),
            )),
        }
    } else {
        HttpResponse::BadRequest().json(CapacityResponse::error(
            400,
            "Either tenant or group must be specified",
        ))
    }
}

pub fn routes() -> actix_web::Scope {
    web::scope("/capacity")
        .service(get_capacity)
        .service(set_capacity)
}
