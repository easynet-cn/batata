//! AI resource persistence trait — storage-agnostic interface for
//! ai_resource, ai_resource_version, and pipeline_execution tables.
//!
//! Aligned with Nacos AiResourcePersistService / AiResourceVersionPersistService.

use async_trait::async_trait;

use crate::model::{AiResourceInfo, AiResourceVersionInfo, Page, PipelineExecutionInfo};

/// Persistence operations for AI resources (skills, agentspecs, etc.)
#[async_trait]
pub trait AiResourcePersistence: Send + Sync {
    // ========================================================================
    // ai_resource operations
    // ========================================================================

    /// Find a resource by namespace, name, and type
    async fn ai_resource_find(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
    ) -> anyhow::Result<Option<AiResourceInfo>>;

    /// Insert a new resource, returns the generated ID
    async fn ai_resource_insert(&self, resource: &AiResourceInfo) -> anyhow::Result<i64>;

    /// Update version_info with optimistic lock (CAS on meta_version).
    /// Returns true if update succeeded (meta_version matched).
    async fn ai_resource_update_version_info_cas(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        expected_meta_version: i64,
        version_info: &str,
        new_meta_version: i64,
    ) -> anyhow::Result<bool>;

    /// Update biz_tags for a resource
    async fn ai_resource_update_biz_tags(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        biz_tags: &str,
    ) -> anyhow::Result<()>;

    /// Update status for a resource
    async fn ai_resource_update_status(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        status: &str,
    ) -> anyhow::Result<()>;

    /// Update scope for a resource
    async fn ai_resource_update_scope(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        scope: &str,
    ) -> anyhow::Result<()>;

    /// Increment download count
    async fn ai_resource_increment_download_count(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        increment: i64,
    ) -> anyhow::Result<()>;

    /// Delete a resource
    async fn ai_resource_delete(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
    ) -> anyhow::Result<u64>;

    /// List resources with optional name filter and pagination
    async fn ai_resource_list(
        &self,
        namespace_id: &str,
        resource_type: &str,
        name_filter: Option<&str>,
        search_accurate: bool,
        order_by_downloads: bool,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<AiResourceInfo>>;

    /// Find all resources of a type in a namespace (no pagination, for filtering)
    async fn ai_resource_find_all(
        &self,
        namespace_id: &str,
        resource_type: &str,
    ) -> anyhow::Result<Vec<AiResourceInfo>>;

    // ========================================================================
    // ai_resource_version operations
    // ========================================================================

    /// Find a specific version
    async fn ai_resource_version_find(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        version: &str,
    ) -> anyhow::Result<Option<AiResourceVersionInfo>>;

    /// Insert a new version, returns the generated ID
    async fn ai_resource_version_insert(
        &self,
        version: &AiResourceVersionInfo,
    ) -> anyhow::Result<i64>;

    /// Update version status
    async fn ai_resource_version_update_status(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        version: &str,
        status: &str,
    ) -> anyhow::Result<()>;

    /// Update version storage and description
    async fn ai_resource_version_update_storage(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        version: &str,
        storage: &str,
        description: Option<&str>,
    ) -> anyhow::Result<()>;

    /// Increment version download count
    async fn ai_resource_version_increment_download_count(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        version: &str,
        increment: i64,
    ) -> anyhow::Result<()>;

    /// List all versions for a resource
    async fn ai_resource_version_list(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
    ) -> anyhow::Result<Vec<AiResourceVersionInfo>>;

    /// Count versions with a specific status
    async fn ai_resource_version_count_by_status(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        status: &str,
    ) -> anyhow::Result<u64>;

    /// Delete a specific version
    async fn ai_resource_version_delete(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        version: &str,
    ) -> anyhow::Result<u64>;

    /// Delete all versions for a resource
    async fn ai_resource_version_delete_all(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
    ) -> anyhow::Result<u64>;

    /// Delete versions by status (e.g., delete all drafts)
    async fn ai_resource_version_delete_by_status(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        status: &str,
    ) -> anyhow::Result<u64>;

    // ========================================================================
    // pipeline_execution operations
    // ========================================================================

    /// Find a pipeline execution by ID
    async fn pipeline_execution_find(
        &self,
        execution_id: &str,
    ) -> anyhow::Result<Option<PipelineExecutionInfo>>;

    /// List pipeline executions with filters and pagination
    async fn pipeline_execution_list(
        &self,
        resource_type: &str,
        resource_name: Option<&str>,
        namespace_id: Option<&str>,
        version: Option<&str>,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<PipelineExecutionInfo>>;
}
