//! V2 Audit Log API endpoints
//!
//! Provides audit log query endpoints for operation tracking.

use actix_web::{HttpMessage, HttpRequest, HttpResponse, Responder, get, web};
use serde::{Deserialize, Serialize};

use batata_config::service::audit;

use crate::model::common::AppState;
use crate::{ActionTypes, ApiType, Secured, SignType, secured};

/// Audit log search parameters
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditLogQuery {
    /// Operation type filter
    pub operation: Option<String>,
    /// Resource type filter
    pub resource_type: Option<String>,
    /// Resource ID filter (partial match)
    pub resource_id: Option<String>,
    /// Tenant ID filter
    pub tenant_id: Option<String>,
    /// Operator filter
    pub operator: Option<String>,
    /// Result filter (SUCCESS or FAILURE)
    pub result: Option<String>,
    /// Start time (ISO 8601 format)
    pub start_time: Option<String>,
    /// End time (ISO 8601 format)
    pub end_time: Option<String>,
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

/// Audit log response
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditLogResponse {
    pub code: i32,
    pub message: String,
    pub data: Option<AuditLogPageData>,
}

/// Audit log page data
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditLogPageData {
    pub total_count: u64,
    pub page_number: u32,
    pub pages_available: u64,
    pub page_items: Vec<AuditLogItem>,
}

/// Audit log item
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditLogItem {
    pub id: u64,
    pub operation: String,
    pub resource_type: String,
    pub resource_id: Option<String>,
    pub tenant_id: Option<String>,
    pub operator: String,
    pub source_ip: Option<String>,
    pub result: String,
    pub error_message: Option<String>,
    pub details: Option<String>,
    pub gmt_create: String,
}

impl AuditLogResponse {
    fn success(data: AuditLogPageData) -> Self {
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

/// Get audit log list with pagination and filtering
#[get("/list")]
pub async fn get_audit_logs(
    req: HttpRequest,
    data: web::Data<AppState>,
    query: web::Query<AuditLogQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/audit")
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::OpenApi)
            .build()
    );

    let db = data.db();

    // Parse time filters
    let start_time = query.start_time.as_ref().and_then(|s| {
        chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S")
            .or_else(|_| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S"))
            .ok()
    });
    let end_time = query.end_time.as_ref().and_then(|s| {
        chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S")
            .or_else(|_| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S"))
            .ok()
    });

    let search = audit::AuditLogSearch {
        operation: query.operation.clone(),
        resource_type: query.resource_type.clone(),
        resource_id: query.resource_id.clone(),
        tenant_id: query.tenant_id.clone(),
        operator: query.operator.clone(),
        result: query.result.clone(),
        start_time,
        end_time,
    };

    match audit::search_logs(db, &search, query.page_no, query.page_size).await {
        Ok(page) => {
            let page_items: Vec<AuditLogItem> = page
                .page_items
                .into_iter()
                .map(|e| AuditLogItem {
                    id: e.id.unwrap_or(0),
                    operation: e.operation,
                    resource_type: e.resource_type,
                    resource_id: e.resource_id,
                    tenant_id: e.tenant_id,
                    operator: e.operator,
                    source_ip: e.source_ip,
                    result: e.result,
                    error_message: e.error_message,
                    details: e.details,
                    gmt_create: e
                        .gmt_create
                        .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_default(),
                })
                .collect();

            HttpResponse::Ok().json(AuditLogResponse::success(AuditLogPageData {
                total_count: page.total_count,
                page_number: page.page_number,
                pages_available: page.pages_available,
                page_items,
            }))
        }
        Err(e) => HttpResponse::InternalServerError().json(AuditLogResponse::error(
            500,
            format!("Failed to search audit logs: {}", e),
        )),
    }
}

/// Get a single audit log entry by ID
#[get("")]
pub async fn get_audit_log(
    req: HttpRequest,
    data: web::Data<AppState>,
    query: web::Query<IdQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/audit")
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::OpenApi)
            .build()
    );

    let db = data.db();

    match audit::get_log(db, query.id).await {
        Ok(Some(entry)) => {
            let item = AuditLogItem {
                id: entry.id.unwrap_or(0),
                operation: entry.operation,
                resource_type: entry.resource_type,
                resource_id: entry.resource_id,
                tenant_id: entry.tenant_id,
                operator: entry.operator,
                source_ip: entry.source_ip,
                result: entry.result,
                error_message: entry.error_message,
                details: entry.details,
                gmt_create: entry
                    .gmt_create
                    .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_default(),
            };

            HttpResponse::Ok().json(serde_json::json!({
                "code": 0,
                "message": "success",
                "data": item
            }))
        }
        Ok(None) => {
            HttpResponse::NotFound().json(AuditLogResponse::error(404, "Audit log not found"))
        }
        Err(e) => HttpResponse::InternalServerError().json(AuditLogResponse::error(
            500,
            format!("Failed to get audit log: {}", e),
        )),
    }
}

#[derive(Debug, Deserialize)]
pub struct IdQuery {
    pub id: u64,
}

/// Get operation statistics
#[get("/stats")]
pub async fn get_audit_stats(
    req: HttpRequest,
    data: web::Data<AppState>,
    query: web::Query<StatsQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/audit")
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::OpenApi)
            .build()
    );

    let db = data.db();

    let start_time = query.start_time.as_ref().and_then(|s| {
        chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S")
            .or_else(|_| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S"))
            .ok()
    });
    let end_time = query.end_time.as_ref().and_then(|s| {
        chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S")
            .or_else(|_| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S"))
            .ok()
    });

    match audit::count_by_operation(db, query.tenant_id.as_deref(), start_time, end_time).await {
        Ok(counts) => HttpResponse::Ok().json(serde_json::json!({
            "code": 0,
            "message": "success",
            "data": counts
        })),
        Err(e) => HttpResponse::InternalServerError().json(AuditLogResponse::error(
            500,
            format!("Failed to get audit stats: {}", e),
        )),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsQuery {
    pub tenant_id: Option<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
}
