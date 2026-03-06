//! CapacityPersistence implementation for ExternalDbPersistService

use async_trait::async_trait;
use sea_orm::*;

use crate::entity::{group_capacity, tenant_capacity};
use crate::model::*;
use crate::traits::*;

use super::ExternalDbPersistService;

const DEFAULT_QUOTA: u32 = 200;
const DEFAULT_MAX_SIZE: u32 = 102400; // 100KB
const DEFAULT_MAX_AGGR_COUNT: u32 = 10000;
const DEFAULT_MAX_AGGR_SIZE: u32 = 2097152; // 2MB
const DEFAULT_MAX_HISTORY_COUNT: u32 = 24;

#[async_trait]
impl CapacityPersistence for ExternalDbPersistService {
    async fn capacity_get_tenant(&self, tenant_id: &str) -> anyhow::Result<Option<CapacityInfo>> {
        let result = tenant_capacity::Entity::find()
            .filter(tenant_capacity::Column::TenantId.eq(tenant_id))
            .one(&self.db)
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

    #[allow(clippy::too_many_arguments)]
    async fn capacity_upsert_tenant(
        &self,
        tenant_id: &str,
        quota: Option<u32>,
        max_size: Option<u32>,
        max_aggr_count: Option<u32>,
        max_aggr_size: Option<u32>,
        max_history_count: Option<u32>,
    ) -> anyhow::Result<CapacityInfo> {
        let existing = tenant_capacity::Entity::find()
            .filter(tenant_capacity::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?;

        let now = chrono::Utc::now().naive_utc();

        if let Some(model) = existing {
            // Update existing record
            let mut active: tenant_capacity::ActiveModel = model.into();
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

            let updated = active.update(&self.db).await?;

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
            // Create new record with defaults overridden by provided values
            let active = tenant_capacity::ActiveModel {
                tenant_id: Set(tenant_id.to_string()),
                quota: Set(quota.unwrap_or(DEFAULT_QUOTA)),
                usage: Set(0),
                max_size: Set(max_size.unwrap_or(DEFAULT_MAX_SIZE)),
                max_aggr_count: Set(max_aggr_count.unwrap_or(DEFAULT_MAX_AGGR_COUNT)),
                max_aggr_size: Set(max_aggr_size.unwrap_or(DEFAULT_MAX_AGGR_SIZE)),
                max_history_count: Set(max_history_count.unwrap_or(DEFAULT_MAX_HISTORY_COUNT)),
                gmt_create: Set(now),
                gmt_modified: Set(now),
                ..Default::default()
            };

            let inserted = active.insert(&self.db).await?;

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

    async fn capacity_delete_tenant(&self, tenant_id: &str) -> anyhow::Result<bool> {
        let result = tenant_capacity::Entity::delete_many()
            .filter(tenant_capacity::Column::TenantId.eq(tenant_id))
            .exec(&self.db)
            .await?;

        Ok(result.rows_affected > 0)
    }

    async fn capacity_get_group(&self, group_id: &str) -> anyhow::Result<Option<CapacityInfo>> {
        let result = group_capacity::Entity::find()
            .filter(group_capacity::Column::GroupId.eq(group_id))
            .one(&self.db)
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

    #[allow(clippy::too_many_arguments)]
    async fn capacity_upsert_group(
        &self,
        group_id: &str,
        quota: Option<u32>,
        max_size: Option<u32>,
        max_aggr_count: Option<u32>,
        max_aggr_size: Option<u32>,
        max_history_count: Option<u32>,
    ) -> anyhow::Result<CapacityInfo> {
        let existing = group_capacity::Entity::find()
            .filter(group_capacity::Column::GroupId.eq(group_id))
            .one(&self.db)
            .await?;

        let now = chrono::Utc::now().naive_utc();

        if let Some(model) = existing {
            // Update existing record
            let mut active: group_capacity::ActiveModel = model.into();
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

            let updated = active.update(&self.db).await?;

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
            // Create new record with defaults overridden by provided values
            let active = group_capacity::ActiveModel {
                group_id: Set(group_id.to_string()),
                quota: Set(quota.unwrap_or(DEFAULT_QUOTA)),
                usage: Set(0),
                max_size: Set(max_size.unwrap_or(DEFAULT_MAX_SIZE)),
                max_aggr_count: Set(max_aggr_count.unwrap_or(DEFAULT_MAX_AGGR_COUNT)),
                max_aggr_size: Set(max_aggr_size.unwrap_or(DEFAULT_MAX_AGGR_SIZE)),
                max_history_count: Set(max_history_count.unwrap_or(DEFAULT_MAX_HISTORY_COUNT)),
                gmt_create: Set(now),
                gmt_modified: Set(now),
                ..Default::default()
            };

            let inserted = active.insert(&self.db).await?;

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

    async fn capacity_delete_group(&self, group_id: &str) -> anyhow::Result<bool> {
        let result = group_capacity::Entity::delete_many()
            .filter(group_capacity::Column::GroupId.eq(group_id))
            .exec(&self.db)
            .await?;

        Ok(result.rows_affected > 0)
    }
}
