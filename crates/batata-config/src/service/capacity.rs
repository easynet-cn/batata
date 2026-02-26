//! Configuration capacity management service
//!
//! Provides quota enforcement and capacity tracking for configurations.

use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};

use batata_persistence::entity::{group_capacity, tenant_capacity};

/// Capacity information for a tenant or group
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapacityInfo {
    /// Unique identifier
    pub id: Option<u64>,
    /// Tenant ID or Group ID
    pub identifier: String,
    /// Maximum number of configs allowed
    pub quota: u32,
    /// Current number of configs
    pub usage: u32,
    /// Maximum size per config in bytes
    pub max_size: u32,
    /// Maximum number of aggregate configs
    pub max_aggr_count: u32,
    /// Maximum size per aggregate config
    pub max_aggr_size: u32,
    /// Maximum number of history records
    pub max_history_count: u32,
}

impl Default for CapacityInfo {
    fn default() -> Self {
        Self {
            id: None,
            identifier: String::new(),
            quota: 200, // Default quota
            usage: 0,
            max_size: 100 * 1024, // 100KB
            max_aggr_count: 10000,
            max_aggr_size: 1024,
            max_history_count: 100,
        }
    }
}

/// Capacity check result
#[derive(Debug, Clone)]
pub struct CapacityCheckResult {
    /// Whether the operation is allowed
    pub allowed: bool,
    /// Error message if not allowed
    pub message: Option<String>,
    /// Current usage
    pub usage: u32,
    /// Quota limit
    pub quota: u32,
}

impl CapacityCheckResult {
    pub fn allowed() -> Self {
        Self {
            allowed: true,
            message: None,
            usage: 0,
            quota: 0,
        }
    }

    pub fn denied(message: impl Into<String>, usage: u32, quota: u32) -> Self {
        Self {
            allowed: false,
            message: Some(message.into()),
            usage,
            quota,
        }
    }
}

// ============================================================================
// Tenant Capacity Operations
// ============================================================================

/// Get tenant capacity by tenant ID
pub async fn get_tenant_capacity(
    db: &DatabaseConnection,
    tenant_id: &str,
) -> anyhow::Result<Option<CapacityInfo>> {
    let result = tenant_capacity::Entity::find()
        .filter(tenant_capacity::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?;

    Ok(result.map(|m| CapacityInfo {
        id: Some(m.id),
        identifier: m.tenant_id,
        quota: m.quota,
        usage: m.usage,
        max_size: m.max_size,
        max_aggr_count: m.max_aggr_count,
        max_aggr_size: m.max_aggr_size,
        max_history_count: m.max_history_count,
    }))
}

/// Create or update tenant capacity
pub async fn upsert_tenant_capacity(
    db: &DatabaseConnection,
    tenant_id: &str,
    quota: Option<u32>,
    max_size: Option<u32>,
    max_aggr_count: Option<u32>,
    max_aggr_size: Option<u32>,
    max_history_count: Option<u32>,
) -> anyhow::Result<CapacityInfo> {
    let existing = tenant_capacity::Entity::find()
        .filter(tenant_capacity::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?;

    let now = chrono::Utc::now().naive_utc();

    if let Some(model) = existing {
        // Update existing
        let mut active: tenant_capacity::ActiveModel = model.clone().into();
        if let Some(q) = quota {
            active.quota = Set(q);
        }
        if let Some(s) = max_size {
            active.max_size = Set(s);
        }
        if let Some(c) = max_aggr_count {
            active.max_aggr_count = Set(c);
        }
        if let Some(s) = max_aggr_size {
            active.max_aggr_size = Set(s);
        }
        if let Some(h) = max_history_count {
            active.max_history_count = Set(h);
        }
        active.gmt_modified = Set(now);

        let updated = active.update(db).await?;

        Ok(CapacityInfo {
            id: Some(updated.id),
            identifier: updated.tenant_id,
            quota: updated.quota,
            usage: updated.usage,
            max_size: updated.max_size,
            max_aggr_count: updated.max_aggr_count,
            max_aggr_size: updated.max_aggr_size,
            max_history_count: updated.max_history_count,
        })
    } else {
        // Create new
        let defaults = CapacityInfo::default();
        let active = tenant_capacity::ActiveModel {
            tenant_id: Set(tenant_id.to_string()),
            quota: Set(quota.unwrap_or(defaults.quota)),
            usage: Set(0),
            max_size: Set(max_size.unwrap_or(defaults.max_size)),
            max_aggr_count: Set(max_aggr_count.unwrap_or(defaults.max_aggr_count)),
            max_aggr_size: Set(max_aggr_size.unwrap_or(defaults.max_aggr_size)),
            max_history_count: Set(max_history_count.unwrap_or(defaults.max_history_count)),
            gmt_create: Set(now),
            gmt_modified: Set(now),
            ..Default::default()
        };

        let inserted = active.insert(db).await?;

        Ok(CapacityInfo {
            id: Some(inserted.id),
            identifier: inserted.tenant_id,
            quota: inserted.quota,
            usage: inserted.usage,
            max_size: inserted.max_size,
            max_aggr_count: inserted.max_aggr_count,
            max_aggr_size: inserted.max_aggr_size,
            max_history_count: inserted.max_history_count,
        })
    }
}

/// Increment tenant usage
pub async fn increment_tenant_usage(
    db: &DatabaseConnection,
    tenant_id: &str,
) -> anyhow::Result<()> {
    let existing = tenant_capacity::Entity::find()
        .filter(tenant_capacity::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?;

    let now = chrono::Utc::now().naive_utc();

    if let Some(model) = existing {
        let mut active: tenant_capacity::ActiveModel = model.into();
        active.usage = Set(active.usage.unwrap() + 1);
        active.gmt_modified = Set(now);
        active.update(db).await?;
    } else {
        // Create with usage = 1
        let defaults = CapacityInfo::default();
        let active = tenant_capacity::ActiveModel {
            tenant_id: Set(tenant_id.to_string()),
            quota: Set(defaults.quota),
            usage: Set(1),
            max_size: Set(defaults.max_size),
            max_aggr_count: Set(defaults.max_aggr_count),
            max_aggr_size: Set(defaults.max_aggr_size),
            max_history_count: Set(defaults.max_history_count),
            gmt_create: Set(now),
            gmt_modified: Set(now),
            ..Default::default()
        };
        active.insert(db).await?;
    }

    Ok(())
}

/// Decrement tenant usage
pub async fn decrement_tenant_usage(
    db: &DatabaseConnection,
    tenant_id: &str,
) -> anyhow::Result<()> {
    let existing = tenant_capacity::Entity::find()
        .filter(tenant_capacity::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?;

    if let Some(model) = existing
        && model.usage > 0
    {
        let now = chrono::Utc::now().naive_utc();
        let mut active: tenant_capacity::ActiveModel = model.into();
        active.usage = Set(active.usage.unwrap() - 1);
        active.gmt_modified = Set(now);
        active.update(db).await?;
    }

    Ok(())
}

/// Check if tenant has capacity for new config
pub async fn check_tenant_capacity(
    db: &DatabaseConnection,
    tenant_id: &str,
    content_size: usize,
    capacity_limit_check: bool,
) -> anyhow::Result<CapacityCheckResult> {
    if !capacity_limit_check {
        return Ok(CapacityCheckResult::allowed());
    }

    let capacity = get_tenant_capacity(db, tenant_id).await?;

    match capacity {
        Some(cap) => {
            // Check quota
            if cap.usage >= cap.quota {
                return Ok(CapacityCheckResult::denied(
                    format!(
                        "Tenant '{}' has reached quota limit ({}/{})",
                        tenant_id, cap.usage, cap.quota
                    ),
                    cap.usage,
                    cap.quota,
                ));
            }

            // Check content size
            if content_size > cap.max_size as usize {
                return Ok(CapacityCheckResult::denied(
                    format!(
                        "Content size {} exceeds maximum allowed size {}",
                        content_size, cap.max_size
                    ),
                    cap.usage,
                    cap.quota,
                ));
            }

            Ok(CapacityCheckResult::allowed())
        }
        None => {
            // No capacity configured, use defaults
            let defaults = CapacityInfo::default();
            if content_size > defaults.max_size as usize {
                return Ok(CapacityCheckResult::denied(
                    format!(
                        "Content size {} exceeds maximum allowed size {}",
                        content_size, defaults.max_size
                    ),
                    0,
                    defaults.quota,
                ));
            }
            Ok(CapacityCheckResult::allowed())
        }
    }
}

// ============================================================================
// Group Capacity Operations
// ============================================================================

/// Get group capacity by group ID
pub async fn get_group_capacity(
    db: &DatabaseConnection,
    group_id: &str,
) -> anyhow::Result<Option<CapacityInfo>> {
    let result = group_capacity::Entity::find()
        .filter(group_capacity::Column::GroupId.eq(group_id))
        .one(db)
        .await?;

    Ok(result.map(|m| CapacityInfo {
        id: Some(m.id),
        identifier: m.group_id,
        quota: m.quota,
        usage: m.usage,
        max_size: m.max_size,
        max_aggr_count: m.max_aggr_count,
        max_aggr_size: m.max_aggr_size,
        max_history_count: m.max_history_count,
    }))
}

/// Create or update group capacity
pub async fn upsert_group_capacity(
    db: &DatabaseConnection,
    group_id: &str,
    quota: Option<u32>,
    max_size: Option<u32>,
    max_aggr_count: Option<u32>,
    max_aggr_size: Option<u32>,
    max_history_count: Option<u32>,
) -> anyhow::Result<CapacityInfo> {
    let existing = group_capacity::Entity::find()
        .filter(group_capacity::Column::GroupId.eq(group_id))
        .one(db)
        .await?;

    let now = chrono::Utc::now().naive_utc();

    if let Some(model) = existing {
        // Update existing
        let mut active: group_capacity::ActiveModel = model.clone().into();
        if let Some(q) = quota {
            active.quota = Set(q);
        }
        if let Some(s) = max_size {
            active.max_size = Set(s);
        }
        if let Some(c) = max_aggr_count {
            active.max_aggr_count = Set(c);
        }
        if let Some(s) = max_aggr_size {
            active.max_aggr_size = Set(s);
        }
        if let Some(h) = max_history_count {
            active.max_history_count = Set(h);
        }
        active.gmt_modified = Set(now);

        let updated = active.update(db).await?;

        Ok(CapacityInfo {
            id: Some(updated.id),
            identifier: updated.group_id,
            quota: updated.quota,
            usage: updated.usage,
            max_size: updated.max_size,
            max_aggr_count: updated.max_aggr_count,
            max_aggr_size: updated.max_aggr_size,
            max_history_count: updated.max_history_count,
        })
    } else {
        // Create new
        let defaults = CapacityInfo::default();
        let active = group_capacity::ActiveModel {
            group_id: Set(group_id.to_string()),
            quota: Set(quota.unwrap_or(defaults.quota)),
            usage: Set(0),
            max_size: Set(max_size.unwrap_or(defaults.max_size)),
            max_aggr_count: Set(max_aggr_count.unwrap_or(defaults.max_aggr_count)),
            max_aggr_size: Set(max_aggr_size.unwrap_or(defaults.max_aggr_size)),
            max_history_count: Set(max_history_count.unwrap_or(defaults.max_history_count)),
            gmt_create: Set(now),
            gmt_modified: Set(now),
            ..Default::default()
        };

        let inserted = active.insert(db).await?;

        Ok(CapacityInfo {
            id: Some(inserted.id),
            identifier: inserted.group_id,
            quota: inserted.quota,
            usage: inserted.usage,
            max_size: inserted.max_size,
            max_aggr_count: inserted.max_aggr_count,
            max_aggr_size: inserted.max_aggr_size,
            max_history_count: inserted.max_history_count,
        })
    }
}

/// Increment group usage
pub async fn increment_group_usage(db: &DatabaseConnection, group_id: &str) -> anyhow::Result<()> {
    let existing = group_capacity::Entity::find()
        .filter(group_capacity::Column::GroupId.eq(group_id))
        .one(db)
        .await?;

    let now = chrono::Utc::now().naive_utc();

    if let Some(model) = existing {
        let mut active: group_capacity::ActiveModel = model.into();
        active.usage = Set(active.usage.unwrap() + 1);
        active.gmt_modified = Set(now);
        active.update(db).await?;
    } else {
        // Create with usage = 1
        let defaults = CapacityInfo::default();
        let active = group_capacity::ActiveModel {
            group_id: Set(group_id.to_string()),
            quota: Set(defaults.quota),
            usage: Set(1),
            max_size: Set(defaults.max_size),
            max_aggr_count: Set(defaults.max_aggr_count),
            max_aggr_size: Set(defaults.max_aggr_size),
            max_history_count: Set(defaults.max_history_count),
            gmt_create: Set(now),
            gmt_modified: Set(now),
            ..Default::default()
        };
        active.insert(db).await?;
    }

    Ok(())
}

/// Decrement group usage
pub async fn decrement_group_usage(db: &DatabaseConnection, group_id: &str) -> anyhow::Result<()> {
    let existing = group_capacity::Entity::find()
        .filter(group_capacity::Column::GroupId.eq(group_id))
        .one(db)
        .await?;

    if let Some(model) = existing
        && model.usage > 0
    {
        let now = chrono::Utc::now().naive_utc();
        let mut active: group_capacity::ActiveModel = model.into();
        active.usage = Set(active.usage.unwrap() - 1);
        active.gmt_modified = Set(now);
        active.update(db).await?;
    }

    Ok(())
}

/// Check if group has capacity for new config
pub async fn check_group_capacity(
    db: &DatabaseConnection,
    group_id: &str,
    content_size: usize,
    capacity_limit_check: bool,
) -> anyhow::Result<CapacityCheckResult> {
    if !capacity_limit_check {
        return Ok(CapacityCheckResult::allowed());
    }

    let capacity = get_group_capacity(db, group_id).await?;

    match capacity {
        Some(cap) => {
            // Check quota
            if cap.usage >= cap.quota {
                return Ok(CapacityCheckResult::denied(
                    format!(
                        "Group '{}' has reached quota limit ({}/{})",
                        group_id, cap.usage, cap.quota
                    ),
                    cap.usage,
                    cap.quota,
                ));
            }

            // Check content size
            if content_size > cap.max_size as usize {
                return Ok(CapacityCheckResult::denied(
                    format!(
                        "Content size {} exceeds maximum allowed size {}",
                        content_size, cap.max_size
                    ),
                    cap.usage,
                    cap.quota,
                ));
            }

            Ok(CapacityCheckResult::allowed())
        }
        None => {
            // No capacity configured, use defaults
            let defaults = CapacityInfo::default();
            if content_size > defaults.max_size as usize {
                return Ok(CapacityCheckResult::denied(
                    format!(
                        "Content size {} exceeds maximum allowed size {}",
                        content_size, defaults.max_size
                    ),
                    0,
                    defaults.quota,
                ));
            }
            Ok(CapacityCheckResult::allowed())
        }
    }
}

/// Delete tenant capacity
pub async fn delete_tenant_capacity(
    db: &DatabaseConnection,
    tenant_id: &str,
) -> anyhow::Result<bool> {
    let result = tenant_capacity::Entity::delete_many()
        .filter(tenant_capacity::Column::TenantId.eq(tenant_id))
        .exec(db)
        .await?;

    Ok(result.rows_affected > 0)
}

/// Delete group capacity
pub async fn delete_group_capacity(
    db: &DatabaseConnection,
    group_id: &str,
) -> anyhow::Result<bool> {
    let result = group_capacity::Entity::delete_many()
        .filter(group_capacity::Column::GroupId.eq(group_id))
        .exec(db)
        .await?;

    Ok(result.rows_affected > 0)
}
