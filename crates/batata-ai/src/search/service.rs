//! Search index service: scheduling and the `base_index` projection.
//!
//! Mirrors upstream `AiResourceIndexMaintenanceService` (scheduling) and
//! `AiResourceIndexServiceImpl.rebuildAiResource` (projection + replace).
//!
//! Vector/embedding storage is deliberately absent: without pgvector the
//! document is written as `enabled` directly, which is what upstream does when
//! `vectorIndex.available()` is false.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use batata_persistence::model::{
    AiResourceInfo, AiResourceSearchChunkInfo, AiResourceSearchDocumentInfo, AiResourceTaskInfo,
    AiResourceVersionInfo,
};
use batata_persistence::PersistenceService;

use crate::model::McpServerStorageInfo;
use crate::repository::{resource_type, search};

use super::task;
use super::{project_mcp, SearchChunk, SearchDocument};

/// Current time in epoch milliseconds.
fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

/// Builds and maintains the AI resource search index.
pub struct AiResourceSearchService {
    persistence: Arc<dyn PersistenceService>,
}

impl AiResourceSearchService {
    /// Creates a service backed by the given persistence.
    pub fn new(persistence: Arc<dyn PersistenceService>) -> Self {
        Self { persistence }
    }

    /// Schedule a `base_index` task for one resource.
    ///
    /// The task key is derived from the resource identity, so scheduling the
    /// same resource again updates the existing row rather than queueing a
    /// duplicate.
    pub async fn schedule(
        &self,
        namespace_id: &str,
        resource_type: &str,
        resource_name: &str,
    ) -> anyhow::Result<()> {
        let payload = task::IndexTaskPayload::new(resource_type, resource_name, false);
        let task = AiResourceTaskInfo {
            task_key: task::task_key(namespace_id, resource_type, resource_name),
            namespace_id: namespace_id.to_string(),
            task_type: search::TASK_TYPE.to_string(),
            task_stage: search::STAGE_BASE_INDEX.to_string(),
            status: search::TASK_STATUS_PENDING.to_string(),
            task_payload: serde_json::to_string(&payload)?,
            task_result: None,
            retry_count: 0,
            revision: 1,
            lease_token: 0,
            next_execute_at: now_millis(),
            lease_expire_at: None,
            last_error: None,
            gmt_create: None,
            gmt_modified: None,
        };
        self.persistence.task_upsert(&task).await
    }

    /// Rebuild the index for the resource's latest published MCP version.
    ///
    /// Mirrors upstream `rebuildLatestAiResource`: only the latest version is
    /// indexed, so documents left over from superseded versions are removed.
    /// Returns `false` when the resource (or its latest version) is gone.
    pub async fn rebuild_latest_mcp(
        &self,
        namespace_id: &str,
        resource_name: &str,
    ) -> anyhow::Result<bool> {
        let Some(resource) = self
            .persistence
            .ai_resource_find(namespace_id, resource_name, resource_type::MCP)
            .await?
        else {
            self.prune_mcp(namespace_id, resource_name, None).await?;
            return Ok(false);
        };

        let latest = resource
            .version_info
            .as_deref()
            .and_then(|json| serde_json::from_str::<crate::model::ResourceVersionInfo>(json).ok())
            .and_then(|vi| vi.latest_version().cloned());

        let Some(version) = latest else {
            self.prune_mcp(namespace_id, resource_name, None).await?;
            return Ok(false);
        };

        let Some(row) = self
            .persistence
            .ai_resource_version_find(namespace_id, resource_name, resource_type::MCP, &version)
            .await?
        else {
            self.prune_mcp(namespace_id, resource_name, None).await?;
            return Ok(false);
        };

        let rebuilt = self.rebuild_mcp_version(&resource, &row).await?;
        self.prune_mcp(namespace_id, resource_name, Some(&version))
            .await?;
        Ok(rebuilt)
    }

    /// Drop documents and chunks for every MCP version except `keep`.
    async fn prune_mcp(
        &self,
        namespace_id: &str,
        resource_name: &str,
        keep: Option<&str>,
    ) -> anyhow::Result<()> {
        let rows = self
            .persistence
            .ai_resource_version_list(namespace_id, resource_name, resource_type::MCP)
            .await?;
        for row in rows {
            if Some(row.version.as_str()) == keep {
                continue;
            }
            self.persistence
                .search_chunk_delete(
                    namespace_id,
                    resource_type::MCP,
                    resource_name,
                    &row.version,
                )
                .await?;
            self.persistence
                .search_document_delete(
                    namespace_id,
                    resource_type::MCP,
                    resource_name,
                    &row.version,
                )
                .await?;
        }
        Ok(())
    }

    /// Rebuild the index for one MCP resource version.
    ///
    /// Returns `false` when the stored document already carries the same
    /// `source_digest`, which is the incremental-rebuild short circuit upstream
    /// describes. Returns `true` when the index was (re)written.
    pub async fn rebuild_mcp_version(
        &self,
        resource: &AiResourceInfo,
        row: &AiResourceVersionInfo,
    ) -> anyhow::Result<bool> {
        let (document, chunks) = project_mcp_from_rows(resource, row);

        if let Some(existing) = self
            .persistence
            .search_document_find(
                &document.namespace_id,
                &document.resource_type,
                &document.resource_name,
                &document.resource_version,
            )
            .await?
            && existing.source_digest == document.source_digest
        {
            return Ok(false);
        }

        self.persistence
            .search_document_upsert(&to_document_info(&document))
            .await?;
        self.persistence
            .search_chunk_replace(
                &document.namespace_id,
                &document.resource_type,
                &document.resource_name,
                &document.resource_version,
                &chunks
                    .iter()
                    .map(|c| to_chunk_info(&document, c))
                    .collect::<Vec<_>>(),
            )
            .await?;

        Ok(true)
    }
}

/// Project an MCP resource row pair into a document and chunks.
fn project_mcp_from_rows(
    resource: &AiResourceInfo,
    row: &AiResourceVersionInfo,
) -> (SearchDocument, Vec<SearchChunk>) {
    let stored = row
        .storage
        .as_deref()
        .and_then(|json| serde_json::from_str::<McpServerStorageInfo>(json).ok())
        .and_then(|info| info.server_data);

    let description = stored
        .as_ref()
        .map(|reg| reg.description.clone())
        .filter(|d| !d.is_empty())
        .or_else(|| resource.description.clone());

    let capabilities: Vec<String> = stored
        .as_ref()
        .map(|reg| {
            reg.capabilities.iter().map(capability_name).collect()
        })
        .unwrap_or_default();

    let server_id = resource
        .ext
        .as_deref()
        .and_then(|json| serde_json::from_str::<crate::model::McpResourceExt>(json).ok())
        .map(|e| e.mcp_id);

    let protocol = stored.as_ref().map(|reg| protocol_name(&reg.server_type));

    let enabled = resource.status.as_deref() != Some(crate::repository::meta_status::DISABLE);

    // The tool specification is the second indexed source document.
    let tools_json = row
        .storage
        .as_deref()
        .and_then(|json| serde_json::from_str::<McpServerStorageInfo>(json).ok())
        .and_then(|info| info.server_data)
        .map(|reg| serde_json::to_string(&reg.tools).unwrap_or_default());

    project_mcp(
        &resource.namespace_id,
        &resource.name,
        &row.version,
        description.as_deref(),
        capabilities,
        server_id.as_deref(),
        protocol.as_deref(),
        enabled,
        Some("ACTIVE"),
        tools_json.as_deref(),
    )
}

/// Render a capability value as its plain name.
fn capability_name(capability: &crate::model::McpCapability) -> String {
    serde_json::to_value(capability)
        .ok()
        .map(|v| match v {
            serde_json::Value::String(s) => s,
            other => other.to_string(),
        })
        .unwrap_or_default()
}

/// Render a server type as its protocol name.
fn protocol_name(server_type: &crate::model::McpServerType) -> String {
    format!("{server_type:?}").to_lowercase()
}

/// Convert a projected document into its persistence representation.
fn to_document_info(document: &SearchDocument) -> AiResourceSearchDocumentInfo {
    AiResourceSearchDocumentInfo {
        id: 0,
        namespace_id: document.namespace_id.clone(),
        resource_type: document.resource_type.clone(),
        resource_name: document.resource_name.clone(),
        resource_version: document.resource_version.clone(),
        display_name: document.display_name.clone(),
        description: document.description.clone(),
        tags: Some(serde_json::to_string(&document.tags).unwrap_or_default()),
        capabilities: Some(serde_json::to_string(&document.capabilities).unwrap_or_default()),
        representative_queries: Some(
            serde_json::to_string(&document.representative_queries).unwrap_or_default(),
        ),
        metadata: Some(serde_json::Value::Object(document.metadata.clone()).to_string()),
        source_digest: document.source_digest.clone(),
        // No vector index: the document is queryable as soon as it is written.
        status: search::STATUS_ENABLED.to_string(),
        generate_mode: search::GENERATE_MODE_AUTO.to_string(),
        gmt_create: None,
        gmt_modified: None,
    }
}

/// Convert a projected chunk into its persistence representation.
fn to_chunk_info(document: &SearchDocument, chunk: &SearchChunk) -> AiResourceSearchChunkInfo {
    AiResourceSearchChunkInfo {
        id: 0,
        document_id: 0,
        namespace_id: document.namespace_id.clone(),
        resource_type: document.resource_type.clone(),
        resource_name: document.resource_name.clone(),
        resource_version: document.resource_version.clone(),
        chunk_type: chunk.chunk_type.clone(),
        chunk_text: chunk.chunk_text.clone(),
        canonical_text: chunk.canonical_text.clone(),
        language: Some(chunk.language.clone()),
        chunk_hash: chunk.chunk_hash.clone(),
        metadata: chunk.metadata.clone(),
        status: search::STATUS_ENABLED.to_string(),
        gmt_create: None,
        gmt_modified: None,
    }
}

/// Resource type this service currently indexes.
pub const INDEXED_RESOURCE_TYPE: &str = resource_type::MCP;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_millis_is_sane() {
        assert!(now_millis() > 1_700_000_000_000);
    }
}
