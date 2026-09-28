//! AI resource persistence trait — storage-agnostic interface for
//! ai_resource, ai_resource_version, and pipeline_execution tables.
//!
//! Aligned with Nacos AiResourcePersistService / AiResourceVersionPersistService.

use async_trait::async_trait;

use crate::model::{
    AiResourceInfo, AiResourceListFilter, AiResourceSearchChunkInfo, AiResourceSearchDocumentInfo,
    AiResourceSearchHitInfo, AiResourceTaskInfo, AiResourceVersionInfo, Page, PipelineExecutionInfo,
};

/// Error returned by backends that do not implement the AI search index yet.
const SEARCH_INDEX_UNSUPPORTED: &str =
    "AI resource search index is not supported by this storage backend";

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

    /// List resources with optional name filter, visibility filters, and pagination.
    async fn ai_resource_list(
        &self,
        namespace_id: &str,
        resource_type: &str,
        filter: &AiResourceListFilter<'_>,
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

    // ========================================================================
    // ai_resource_search_document operations
    // ========================================================================

    /// Find the search document for one resource version.
    async fn search_document_find(
        &self,
        namespace_id: &str,
        resource_type: &str,
        resource_name: &str,
        resource_version: &str,
    ) -> anyhow::Result<Option<AiResourceSearchDocumentInfo>> {
        let _ = (namespace_id, resource_type, resource_name, resource_version);
        anyhow::bail!(SEARCH_INDEX_UNSUPPORTED)
    }

    /// Insert or update the search document for one resource version.
    ///
    /// Returns the document ID.
    async fn search_document_upsert(
        &self,
        document: &AiResourceSearchDocumentInfo,
    ) -> anyhow::Result<i64> {
        let _ = document;
        anyhow::bail!(SEARCH_INDEX_UNSUPPORTED)
    }

    /// Delete the search document for one resource version.
    async fn search_document_delete(
        &self,
        namespace_id: &str,
        resource_type: &str,
        resource_name: &str,
        resource_version: &str,
    ) -> anyhow::Result<u64> {
        let _ = (namespace_id, resource_type, resource_name, resource_version);
        anyhow::bail!(SEARCH_INDEX_UNSUPPORTED)
    }

    // ========================================================================
    // ai_resource_search_chunk operations
    // ========================================================================

    /// Replace every chunk of one resource version with `chunks`.
    ///
    /// Returns the number of chunks written.
    async fn search_chunk_replace(
        &self,
        namespace_id: &str,
        resource_type: &str,
        resource_name: &str,
        resource_version: &str,
        chunks: &[AiResourceSearchChunkInfo],
    ) -> anyhow::Result<u64> {
        let _ = (
            namespace_id,
            resource_type,
            resource_name,
            resource_version,
            chunks,
        );
        anyhow::bail!(SEARCH_INDEX_UNSUPPORTED)
    }

    /// List every chunk of one resource version.
    async fn search_chunk_list(
        &self,
        namespace_id: &str,
        resource_type: &str,
        resource_name: &str,
        resource_version: &str,
    ) -> anyhow::Result<Vec<AiResourceSearchChunkInfo>> {
        let _ = (namespace_id, resource_type, resource_name, resource_version);
        anyhow::bail!(SEARCH_INDEX_UNSUPPORTED)
    }

    /// Delete every chunk of one resource version.
    async fn search_chunk_delete(
        &self,
        namespace_id: &str,
        resource_type: &str,
        resource_name: &str,
        resource_version: &str,
    ) -> anyhow::Result<u64> {
        let _ = (namespace_id, resource_type, resource_name, resource_version);
        anyhow::bail!(SEARCH_INDEX_UNSUPPORTED)
    }

    // ========================================================================
    // ai_resource_search_chunk querying
    // ========================================================================

    /// Keyword search over enabled chunks of one namespace.
    ///
    /// Mirrors upstream `AiResourceSearchRepository.searchChunks`: matches
    /// `text` as a case-insensitive substring of `canonical_text` or
    /// `chunk_text`, scoring canonical matches higher. `resource_types`
    /// restricts the search when non-empty. Results are ordered by descending
    /// score and capped at `limit`.
    async fn search_chunk_search(
        &self,
        namespace_id: &str,
        text: &str,
        resource_types: &[&str],
        limit: u64,
    ) -> anyhow::Result<Vec<AiResourceSearchHitInfo>> {
        let _ = (namespace_id, text, resource_types, limit);
        anyhow::bail!(SEARCH_INDEX_UNSUPPORTED)
    }

    // ========================================================================
    // ai_resource_task operations
    // ========================================================================

    /// Insert or update a task by its key.
    async fn task_upsert(&self, task: &AiResourceTaskInfo) -> anyhow::Result<()> {
        let _ = task;
        anyhow::bail!(SEARCH_INDEX_UNSUPPORTED)
    }

    /// Find a task by key.
    async fn task_find(&self, task_key: &str) -> anyhow::Result<Option<AiResourceTaskInfo>> {
        let _ = task_key;
        anyhow::bail!(SEARCH_INDEX_UNSUPPORTED)
    }

    /// Find tasks of `task_type` that are due for execution and not currently
    /// leased, ordered by `next_execute_at` ascending.
    ///
    /// `now_millis` is the current time in Unix epoch milliseconds.
    async fn task_find_due(
        &self,
        task_type: &str,
        now_millis: i64,
        limit: u64,
    ) -> anyhow::Result<Vec<AiResourceTaskInfo>> {
        let _ = (task_type, now_millis, limit);
        anyhow::bail!(SEARCH_INDEX_UNSUPPORTED)
    }

    /// Delete a task by key.
    async fn task_delete(&self, task_key: &str) -> anyhow::Result<u64> {
        let _ = task_key;
        anyhow::bail!(SEARCH_INDEX_UNSUPPORTED)
    }
}
