//! Audit log endpoints for the V3 Console API
//!
//! Provides audit trail of operations (config changes, auth changes, etc.)
//! by querying the config history via the console data source.

use actix_web::{HttpRequest, Responder, Scope, get, web};
use serde::{Deserialize, Serialize};

use batata_api::model::Page;
use batata_server_common::{
    ActionTypes, ApiType, Secured, SignType,
    model::{AppState, response::Result},
    secured,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditListQuery {
    #[serde(default = "default_page_no", alias = "pageNo")]
    pub page_no: u64,
    #[serde(default = "default_page_size", alias = "pageSize")]
    pub page_size: u64,
    #[serde(default, alias = "operationType")]
    pub operation_type: Option<String>,
    #[serde(default)]
    pub operator: Option<String>,
    #[serde(default)]
    pub resource: Option<String>,
    #[serde(default, alias = "startTime")]
    pub start_time: Option<String>,
    #[serde(default, alias = "endTime")]
    pub end_time: Option<String>,
}

fn default_page_no() -> u64 {
    1
}
fn default_page_size() -> u64 {
    20
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditLogEntry {
    pub id: i64,
    pub operation_type: String,
    pub resource_type: String,
    pub resource_name: String,
    pub operator: String,
    pub operator_ip: String,
    pub result: String,
    pub detail: String,
    pub created_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditStats {
    pub total_operations: u64,
    pub today_operations: u64,
    pub config_changes: u64,
    pub auth_changes: u64,
}

/// Map op_type code to human-readable operation type
fn map_op_type(op_type: &str) -> &str {
    match op_type {
        "I" => "CREATE",
        "U" => "UPDATE",
        "D" => "DELETE",
        other => other,
    }
}

/// Map query operation type back to op_type code for filtering.
/// Accepts human-readable names (CREATE/UPDATE/DELETE) or raw codes (I/U/D).
fn map_query_op_type(operation_type: &str) -> Option<&'static str> {
    let upper = operation_type.trim().to_uppercase();
    match upper.as_str() {
        "CREATE" | "I" => Some("I"),
        "UPDATE" | "U" => Some("U"),
        "DELETE" | "D" => Some("D"),
        "" => None,
        _ => None,
    }
}

/// Format timestamp (millis since epoch) to a display string
fn format_timestamp(ts: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ts)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_default()
}

/// Parse a datetime string to millis since epoch
fn parse_time_string(s: &str) -> Option<i64> {
    // Try ISO 8601 / common datetime formats
    chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
        .ok()
        .map(|ndt| ndt.and_utc().timestamp_millis())
        .or_else(|| s.parse::<i64>().ok())
}

/// GET /v3/console/audit/logs
///
/// List audit log entries with pagination and optional filters.
#[get("/logs")]
pub async fn list_audit_logs(
    req: HttpRequest,
    data: web::Data<AppState>,
    query: web::Query<AuditListQuery>,
) -> impl Responder {
    let resource = "*:*:*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    // Map operation_type filter to persistence op_type code
    let op_type = query.operation_type.as_deref().and_then(map_query_op_type);

    let src_user = query.operator.as_deref().filter(|s| !s.is_empty());
    let start_time = query.start_time.as_deref().and_then(parse_time_string);
    let end_time = query.end_time.as_deref().and_then(parse_time_string);

    match data
        .console_datasource
        .history_search_with_filters(
            "", // all data_ids
            "", // all groups
            "", // all namespaces
            op_type,
            src_user,
            start_time,
            end_time,
            query.page_no,
            query.page_size,
        )
        .await
    {
        Ok(page) => {
            let entries: Vec<AuditLogEntry> = page
                .page_items
                .iter()
                .map(|h| {
                    let basic = &h.config_basic_info;
                    AuditLogEntry {
                        id: basic.id,
                        operation_type: map_op_type(&h.op_type).to_string(),
                        resource_type: "config".to_string(),
                        resource_name: format!("{}/{}", basic.group_name, basic.data_id),
                        operator: h.src_user.clone(),
                        operator_ip: h.src_ip.clone(),
                        result: "SUCCESS".to_string(),
                        detail: format!(
                            "Config {} in namespace '{}', publish_type: {}",
                            map_op_type(&h.op_type),
                            basic.namespace_id,
                            h.publish_type
                        ),
                        created_time: format_timestamp(basic.create_time),
                    }
                })
                .collect();

            let result_page = Page {
                total_count: page.total_count,
                page_number: page.page_number,
                pages_available: page.pages_available,
                page_items: entries,
            };
            Result::<Page<AuditLogEntry>>::http_success(result_page)
        }
        Err(e) => {
            tracing::error!("Failed to query audit logs: {}", e);
            Result::<String>::http_response(
                500,
                500,
                format!("Failed to query audit logs: {}", e),
                String::new(),
            )
        }
    }
}

/// GET /v3/console/audit/logs/{id}
///
/// Get a specific audit log entry by its ID.
#[get("/logs/{id}")]
pub async fn get_audit_log(
    req: HttpRequest,
    data: web::Data<AppState>,
    path: web::Path<u64>,
) -> impl Responder {
    let resource = "*:*:*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let nid = path.into_inner();

    match data.console_datasource.history_find_by_id(nid).await {
        Ok(Some(detail)) => {
            let basic = &detail.config_history_basic_info.config_basic_info;
            let h = &detail.config_history_basic_info;
            let entry = AuditLogEntry {
                id: basic.id,
                operation_type: map_op_type(&h.op_type).to_string(),
                resource_type: "config".to_string(),
                resource_name: format!("{}/{}", basic.group_name, basic.data_id),
                operator: h.src_user.clone(),
                operator_ip: h.src_ip.clone(),
                result: "SUCCESS".to_string(),
                detail: format!("Content MD5: {}", basic.md5),
                created_time: format_timestamp(basic.create_time),
            };
            Result::<AuditLogEntry>::http_success(entry)
        }
        Ok(None) => Result::<String>::http_response(
            404,
            404,
            "Audit log entry not found".to_string(),
            String::new(),
        ),
        Err(e) => {
            tracing::error!("Failed to query audit log {}: {}", nid, e);
            Result::<String>::http_response(
                500,
                500,
                format!("Failed to query audit log: {}", e),
                String::new(),
            )
        }
    }
}

/// GET /v3/console/audit/stats
///
/// Get audit log statistics (total operations, today's operations, etc.).
#[get("/stats")]
pub async fn get_audit_stats(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    let resource = "*:*:*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    // Get total count by searching with no filters, page_size=1
    let total = data
        .console_datasource
        .history_search_with_filters("", "", "", None, None, None, None, 1, 1)
        .await
        .map(|p| p.total_count)
        .unwrap_or(0);

    // Get today's count by filtering from start of today (UTC)
    let today_start = chrono::Utc::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .map(|ndt| ndt.and_utc().timestamp_millis());

    let today_count = if let Some(start_ts) = today_start {
        data.console_datasource
            .history_search_with_filters("", "", "", None, None, Some(start_ts), None, 1, 1)
            .await
            .map(|p| p.total_count)
            .unwrap_or(0)
    } else {
        0
    };

    let stats = AuditStats {
        total_operations: total,
        today_operations: today_count,
        config_changes: total,
        auth_changes: 0,
    };

    Result::<AuditStats>::http_success(stats)
}

/// Create the audit routes scope
pub fn routes() -> Scope {
    web::scope("/audit")
        .service(get_audit_stats)
        .service(list_audit_logs)
        .service(get_audit_log)
}
