//! V2 Aggregate Config API endpoints
//!
//! Provides aggregate configuration (datumId-based) management endpoints.

use actix_web::{HttpResponse, Responder, delete, get, post, web};
use serde::{Deserialize, Serialize};

use batata_config::service::aggr;

use crate::model::common::AppState;

/// Aggregate config request parameters
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AggrConfigRequest {
    /// Configuration data ID
    pub data_id: String,
    /// Configuration group
    #[serde(default = "default_group")]
    pub group: String,
    /// Tenant/Namespace ID
    #[serde(default)]
    pub tenant: String,
    /// Datum ID (unique within aggregate)
    pub datum_id: Option<String>,
    /// Configuration content (for publish)
    pub content: Option<String>,
    /// Application name
    pub app_name: Option<String>,
}

fn default_group() -> String {
    "DEFAULT_GROUP".to_string()
}

/// Aggregate config list request
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AggrListRequest {
    /// Configuration data ID
    pub data_id: String,
    /// Configuration group
    #[serde(default = "default_group")]
    pub group: String,
    /// Tenant/Namespace ID
    #[serde(default)]
    pub tenant: String,
    /// Page number (1-indexed)
    #[serde(default = "default_page")]
    pub page_no: u32,
    /// Page size
    #[serde(default = "default_page_size")]
    pub page_size: u32,
}

fn default_page() -> u32 {
    1
}

fn default_page_size() -> u32 {
    100
}

/// Aggregate config response
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AggrConfigResponse {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

impl AggrConfigResponse {
    fn success<T: Serialize>(data: T) -> Self {
        Self {
            code: 0,
            message: "success".to_string(),
            data: Some(serde_json::to_value(data).unwrap_or(serde_json::Value::Null)),
        }
    }

    fn success_bool(result: bool) -> Self {
        Self {
            code: 0,
            message: "success".to_string(),
            data: Some(serde_json::Value::Bool(result)),
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

/// Publish aggregate configuration
#[post("")]
pub async fn publish_aggr_config(
    data: web::Data<AppState>,
    query: web::Query<AggrConfigRequest>,
) -> impl Responder {
    let db = data.db();

    let datum_id = match &query.datum_id {
        Some(id) => id.as_str(),
        None => {
            return HttpResponse::BadRequest()
                .json(AggrConfigResponse::error(400, "datumId is required"));
        }
    };

    let content = match &query.content {
        Some(c) => c.as_str(),
        None => {
            return HttpResponse::BadRequest()
                .json(AggrConfigResponse::error(400, "content is required"));
        }
    };

    // Check capacity limits
    if data.configuration.is_capacity_limit_check() {
        let count = match aggr::count_aggr(db, &query.data_id, &query.group, &query.tenant).await {
            Ok(c) => c,
            Err(e) => {
                return HttpResponse::InternalServerError().json(AggrConfigResponse::error(
                    500,
                    format!("Failed to count aggregate configs: {}", e),
                ));
            }
        };

        let max_aggr_count = data.configuration.default_max_aggr_count() as u64;
        if count >= max_aggr_count {
            return HttpResponse::BadRequest().json(AggrConfigResponse::error(
                400,
                format!(
                    "Aggregate config count {} exceeds limit {}",
                    count, max_aggr_count
                ),
            ));
        }

        let max_aggr_size = data.configuration.default_max_aggr_size() as usize;
        if content.len() > max_aggr_size {
            return HttpResponse::BadRequest().json(AggrConfigResponse::error(
                400,
                format!(
                    "Content size {} exceeds maximum aggregate size {}",
                    content.len(),
                    max_aggr_size
                ),
            ));
        }
    }

    match aggr::publish_aggr(
        db,
        &query.data_id,
        &query.group,
        &query.tenant,
        datum_id,
        content,
        query.app_name.as_deref(),
    )
    .await
    {
        Ok(result) => HttpResponse::Ok().json(AggrConfigResponse::success_bool(result)),
        Err(e) => HttpResponse::InternalServerError().json(AggrConfigResponse::error(
            500,
            format!("Failed to publish aggregate config: {}", e),
        )),
    }
}

/// Delete aggregate configuration
#[delete("")]
pub async fn delete_aggr_config(
    data: web::Data<AppState>,
    query: web::Query<AggrConfigRequest>,
) -> impl Responder {
    let db = data.db();

    if let Some(ref datum_id) = query.datum_id {
        // Delete single datum
        match aggr::remove_aggr(db, &query.data_id, &query.group, &query.tenant, datum_id).await {
            Ok(deleted) => HttpResponse::Ok().json(AggrConfigResponse::success_bool(deleted)),
            Err(e) => HttpResponse::InternalServerError().json(AggrConfigResponse::error(
                500,
                format!("Failed to delete aggregate config: {}", e),
            )),
        }
    } else {
        // Delete all datums for the parent config
        match aggr::remove_all_aggr(db, &query.data_id, &query.group, &query.tenant).await {
            Ok(count) => HttpResponse::Ok().json(AggrConfigResponse::success(serde_json::json!({
                "deleted": count
            }))),
            Err(e) => HttpResponse::InternalServerError().json(AggrConfigResponse::error(
                500,
                format!("Failed to delete aggregate configs: {}", e),
            )),
        }
    }
}

/// Get aggregate configuration
#[get("")]
pub async fn get_aggr_config(
    data: web::Data<AppState>,
    query: web::Query<AggrConfigRequest>,
) -> impl Responder {
    let db = data.db();

    if let Some(ref datum_id) = query.datum_id {
        // Get single datum
        match aggr::get_aggr(db, &query.data_id, &query.group, &query.tenant, datum_id).await {
            Ok(Some(item)) => HttpResponse::Ok().json(AggrConfigResponse::success(item)),
            Ok(None) => {
                HttpResponse::NotFound().json(AggrConfigResponse::error(404, "Config not found"))
            }
            Err(e) => HttpResponse::InternalServerError().json(AggrConfigResponse::error(
                500,
                format!("Failed to get aggregate config: {}", e),
            )),
        }
    } else {
        // Get merged content
        match aggr::get_merged_aggr(db, &query.data_id, &query.group, &query.tenant).await {
            Ok(Some(content)) => {
                HttpResponse::Ok().json(AggrConfigResponse::success(serde_json::json!({
                    "dataId": query.data_id,
                    "group": query.group,
                    "tenant": query.tenant,
                    "content": content
                })))
            }
            Ok(None) => {
                HttpResponse::NotFound().json(AggrConfigResponse::error(404, "Config not found"))
            }
            Err(e) => HttpResponse::InternalServerError().json(AggrConfigResponse::error(
                500,
                format!("Failed to get aggregate config: {}", e),
            )),
        }
    }
}

/// List aggregate configuration items
#[get("/list")]
pub async fn list_aggr_config(
    data: web::Data<AppState>,
    query: web::Query<AggrListRequest>,
) -> impl Responder {
    let db = data.db();

    match aggr::list_aggr_page(
        db,
        &query.data_id,
        &query.group,
        &query.tenant,
        query.page_no,
        query.page_size,
    )
    .await
    {
        Ok(page) => HttpResponse::Ok().json(AggrConfigResponse::success(page)),
        Err(e) => HttpResponse::InternalServerError().json(AggrConfigResponse::error(
            500,
            format!("Failed to list aggregate configs: {}", e),
        )),
    }
}

/// List all datum IDs for a parent config
#[get("/datumIds")]
pub async fn list_datum_ids(
    data: web::Data<AppState>,
    query: web::Query<AggrListRequest>,
) -> impl Responder {
    let db = data.db();

    match aggr::list_datum_ids(db, &query.data_id, &query.group, &query.tenant).await {
        Ok(ids) => HttpResponse::Ok().json(AggrConfigResponse::success(ids)),
        Err(e) => HttpResponse::InternalServerError().json(AggrConfigResponse::error(
            500,
            format!("Failed to list datum IDs: {}", e),
        )),
    }
}

/// Count aggregate configuration items
#[get("/count")]
pub async fn count_aggr_config(
    data: web::Data<AppState>,
    query: web::Query<AggrListRequest>,
) -> impl Responder {
    let db = data.db();

    match aggr::count_aggr(db, &query.data_id, &query.group, &query.tenant).await {
        Ok(count) => HttpResponse::Ok().json(AggrConfigResponse::success(serde_json::json!({
            "count": count
        }))),
        Err(e) => HttpResponse::InternalServerError().json(AggrConfigResponse::error(
            500,
            format!("Failed to count aggregate configs: {}", e),
        )),
    }
}
