//! Capacity persistence trait
//!
//! Defines the interface for capacity quota storage operations.

use async_trait::async_trait;

use crate::model::CapacityInfo;

/// Capacity persistence operations for tenant and group quotas
#[async_trait]
pub trait CapacityPersistence: Send + Sync {
    /// Get tenant capacity by tenant ID
    async fn capacity_get_tenant(&self, tenant_id: &str) -> anyhow::Result<Option<CapacityInfo>>;

    /// Create or update tenant capacity
    #[allow(clippy::too_many_arguments)]
    async fn capacity_upsert_tenant(
        &self,
        tenant_id: &str,
        quota: Option<u32>,
        max_size: Option<u32>,
        max_aggr_count: Option<u32>,
        max_aggr_size: Option<u32>,
        max_history_count: Option<u32>,
    ) -> anyhow::Result<CapacityInfo>;

    /// Delete tenant capacity
    async fn capacity_delete_tenant(&self, tenant_id: &str) -> anyhow::Result<bool>;

    /// Get group capacity by group ID
    async fn capacity_get_group(&self, group_id: &str) -> anyhow::Result<Option<CapacityInfo>>;

    /// Create or update group capacity
    #[allow(clippy::too_many_arguments)]
    async fn capacity_upsert_group(
        &self,
        group_id: &str,
        quota: Option<u32>,
        max_size: Option<u32>,
        max_aggr_count: Option<u32>,
        max_aggr_size: Option<u32>,
        max_history_count: Option<u32>,
    ) -> anyhow::Result<CapacityInfo>;

    /// Delete group capacity
    async fn capacity_delete_group(&self, group_id: &str) -> anyhow::Result<bool>;
}
