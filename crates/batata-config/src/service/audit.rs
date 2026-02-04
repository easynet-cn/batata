//! Audit logging service
//!
//! Provides comprehensive operation logging for audit purposes.

use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, Order, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect, Set,
};
use serde::{Deserialize, Serialize};

use batata_persistence::entity::operation_log;

/// Operation type constants
pub mod operation {
    pub const CREATE: &str = "CREATE";
    pub const UPDATE: &str = "UPDATE";
    pub const DELETE: &str = "DELETE";
    pub const QUERY: &str = "QUERY";
    pub const LOGIN: &str = "LOGIN";
    pub const LOGOUT: &str = "LOGOUT";
    pub const PUBLISH: &str = "PUBLISH";
    pub const ROLLBACK: &str = "ROLLBACK";
    pub const IMPORT: &str = "IMPORT";
    pub const EXPORT: &str = "EXPORT";
    pub const CLONE: &str = "CLONE";
}

/// Resource type constants
pub mod resource {
    pub const CONFIG: &str = "CONFIG";
    pub const SERVICE: &str = "SERVICE";
    pub const INSTANCE: &str = "INSTANCE";
    pub const NAMESPACE: &str = "NAMESPACE";
    pub const USER: &str = "USER";
    pub const ROLE: &str = "ROLE";
    pub const PERMISSION: &str = "PERMISSION";
    pub const CAPACITY: &str = "CAPACITY";
    pub const CLUSTER: &str = "CLUSTER";
}

/// Operation result constants
pub mod result {
    pub const SUCCESS: &str = "SUCCESS";
    pub const FAILURE: &str = "FAILURE";
}

/// Audit log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditLogEntry {
    pub id: Option<u64>,
    pub operation: String,
    pub resource_type: String,
    pub resource_id: Option<String>,
    pub tenant_id: Option<String>,
    pub operator: String,
    pub source_ip: Option<String>,
    pub result: String,
    pub error_message: Option<String>,
    pub details: Option<String>,
    pub gmt_create: Option<chrono::NaiveDateTime>,
}

impl Default for AuditLogEntry {
    fn default() -> Self {
        Self {
            id: None,
            operation: String::new(),
            resource_type: String::new(),
            resource_id: None,
            tenant_id: None,
            operator: "anonymous".to_string(),
            source_ip: None,
            result: result::SUCCESS.to_string(),
            error_message: None,
            details: None,
            gmt_create: None,
        }
    }
}

impl AuditLogEntry {
    /// Create a new audit log entry builder
    pub fn builder() -> AuditLogBuilder {
        AuditLogBuilder::new()
    }
}

/// Builder for AuditLogEntry
pub struct AuditLogBuilder {
    entry: AuditLogEntry,
}

impl AuditLogBuilder {
    pub fn new() -> Self {
        Self {
            entry: AuditLogEntry::default(),
        }
    }

    pub fn operation(mut self, op: &str) -> Self {
        self.entry.operation = op.to_string();
        self
    }

    pub fn resource_type(mut self, rt: &str) -> Self {
        self.entry.resource_type = rt.to_string();
        self
    }

    pub fn resource_id(mut self, id: impl Into<String>) -> Self {
        self.entry.resource_id = Some(id.into());
        self
    }

    pub fn tenant_id(mut self, tenant: impl Into<String>) -> Self {
        self.entry.tenant_id = Some(tenant.into());
        self
    }

    pub fn operator(mut self, op: impl Into<String>) -> Self {
        self.entry.operator = op.into();
        self
    }

    pub fn source_ip(mut self, ip: impl Into<String>) -> Self {
        self.entry.source_ip = Some(ip.into());
        self
    }

    pub fn success(mut self) -> Self {
        self.entry.result = result::SUCCESS.to_string();
        self
    }

    pub fn failure(mut self, error: impl Into<String>) -> Self {
        self.entry.result = result::FAILURE.to_string();
        self.entry.error_message = Some(error.into());
        self
    }

    pub fn details(mut self, details: impl Into<String>) -> Self {
        self.entry.details = Some(details.into());
        self
    }

    pub fn details_json<T: Serialize>(mut self, details: &T) -> Self {
        self.entry.details = serde_json::to_string(details).ok();
        self
    }

    pub fn build(self) -> AuditLogEntry {
        self.entry
    }
}

impl Default for AuditLogBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Page info for pagination
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditLogPage {
    pub total_count: u64,
    pub page_number: u32,
    pub pages_available: u64,
    pub page_items: Vec<AuditLogEntry>,
}

/// Search criteria for audit logs
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditLogSearch {
    pub operation: Option<String>,
    pub resource_type: Option<String>,
    pub resource_id: Option<String>,
    pub tenant_id: Option<String>,
    pub operator: Option<String>,
    pub result: Option<String>,
    pub start_time: Option<chrono::NaiveDateTime>,
    pub end_time: Option<chrono::NaiveDateTime>,
}

/// Log an operation to the audit log
pub async fn log_operation(db: &DatabaseConnection, entry: AuditLogEntry) -> anyhow::Result<u64> {
    let now = chrono::Utc::now().naive_utc();

    let active = operation_log::ActiveModel {
        operation: Set(entry.operation),
        resource_type: Set(entry.resource_type),
        resource_id: Set(entry.resource_id),
        tenant_id: Set(entry.tenant_id),
        operator: Set(entry.operator),
        source_ip: Set(entry.source_ip),
        result: Set(entry.result),
        error_message: Set(entry.error_message),
        details: Set(entry.details),
        gmt_create: Set(now),
        ..Default::default()
    };

    let inserted = active.insert(db).await?;
    Ok(inserted.id)
}

/// Search audit logs with pagination
pub async fn search_logs(
    db: &DatabaseConnection,
    search: &AuditLogSearch,
    page_number: u32,
    page_size: u32,
) -> anyhow::Result<AuditLogPage> {
    let mut query = operation_log::Entity::find();

    // Apply filters
    if let Some(ref op) = search.operation {
        query = query.filter(operation_log::Column::Operation.eq(op));
    }
    if let Some(ref rt) = search.resource_type {
        query = query.filter(operation_log::Column::ResourceType.eq(rt));
    }
    if let Some(ref rid) = search.resource_id {
        query = query.filter(operation_log::Column::ResourceId.like(format!("%{}%", rid)));
    }
    if let Some(ref tenant) = search.tenant_id {
        query = query.filter(operation_log::Column::TenantId.eq(tenant));
    }
    if let Some(ref operator) = search.operator {
        query = query.filter(operation_log::Column::Operator.eq(operator));
    }
    if let Some(ref result) = search.result {
        query = query.filter(operation_log::Column::Result.eq(result));
    }
    if let Some(start) = search.start_time {
        query = query.filter(operation_log::Column::GmtCreate.gte(start));
    }
    if let Some(end) = search.end_time {
        query = query.filter(operation_log::Column::GmtCreate.lte(end));
    }

    // Order by creation time descending (newest first)
    query = query.order_by(operation_log::Column::GmtCreate, Order::Desc);

    // Get total count
    let total_count = query.clone().count(db).await?;

    // Calculate pagination
    let pages_available = (total_count + page_size as u64 - 1) / page_size as u64;
    let offset = (page_number.saturating_sub(1)) * page_size;

    // Get page items
    let models = query
        .offset(offset as u64)
        .limit(page_size as u64)
        .all(db)
        .await?;

    let page_items: Vec<AuditLogEntry> = models
        .into_iter()
        .map(|m| AuditLogEntry {
            id: Some(m.id),
            operation: m.operation,
            resource_type: m.resource_type,
            resource_id: m.resource_id,
            tenant_id: m.tenant_id,
            operator: m.operator,
            source_ip: m.source_ip,
            result: m.result,
            error_message: m.error_message,
            details: m.details,
            gmt_create: Some(m.gmt_create),
        })
        .collect();

    Ok(AuditLogPage {
        total_count,
        page_number,
        pages_available,
        page_items,
    })
}

/// Get a single audit log entry by ID
pub async fn get_log(db: &DatabaseConnection, id: u64) -> anyhow::Result<Option<AuditLogEntry>> {
    let model = operation_log::Entity::find_by_id(id).one(db).await?;

    Ok(model.map(|m| AuditLogEntry {
        id: Some(m.id),
        operation: m.operation,
        resource_type: m.resource_type,
        resource_id: m.resource_id,
        tenant_id: m.tenant_id,
        operator: m.operator,
        source_ip: m.source_ip,
        result: m.result,
        error_message: m.error_message,
        details: m.details,
        gmt_create: Some(m.gmt_create),
    }))
}

/// Delete old audit logs (retention policy)
pub async fn cleanup_old_logs(db: &DatabaseConnection, retention_days: u32) -> anyhow::Result<u64> {
    let cutoff = chrono::Utc::now().naive_utc() - chrono::Duration::days(retention_days as i64);

    let result = operation_log::Entity::delete_many()
        .filter(operation_log::Column::GmtCreate.lt(cutoff))
        .exec(db)
        .await?;

    Ok(result.rows_affected)
}

/// Count operations by type for statistics
pub async fn count_by_operation(
    db: &DatabaseConnection,
    tenant_id: Option<&str>,
    start_time: Option<chrono::NaiveDateTime>,
    end_time: Option<chrono::NaiveDateTime>,
) -> anyhow::Result<std::collections::HashMap<String, u64>> {
    let mut query = operation_log::Entity::find();

    if let Some(tenant) = tenant_id {
        query = query.filter(operation_log::Column::TenantId.eq(tenant));
    }
    if let Some(start) = start_time {
        query = query.filter(operation_log::Column::GmtCreate.gte(start));
    }
    if let Some(end) = end_time {
        query = query.filter(operation_log::Column::GmtCreate.lte(end));
    }

    let models = query.all(db).await?;

    let mut counts = std::collections::HashMap::new();
    for m in models {
        *counts.entry(m.operation).or_insert(0) += 1;
    }

    Ok(counts)
}
