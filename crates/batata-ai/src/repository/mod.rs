//! AI resource repository.
//!
//! A thin, storage-agnostic facade over the `AiResourcePersistence` operations
//! exposed by [`PersistenceService`], plus the value vocabularies that upstream
//! Nacos defines for the AI resource tables.
//!
//! The vocabularies below are **not** invented here — they are copied from the
//! upstream sources so that Batata and Nacos agree on the persisted values:
//!
//! - resource types: `constant/AiResourceConstants.java`, `constant/Constants.java`
//! - meta / version status: `constant/AiResourceConstants.java`
//! - search index constants: `service/search/AiResourceSearchConstants.java`
//! - task type / stages: `model/search/AiResourceIndexTask.java`
//!
//! Persistence itself already lives in `batata-persistence`, which mirrors
//! upstream's `AiResourcePersistService` / `AiResourceVersionPersistService`
//! with SQL, embedded and distributed implementations. This module only
//! centralizes access and the shared constants.

use batata_persistence::model::AiResourceListFilter;
use batata_persistence::{AiResourceInfo, AiResourceVersionInfo, Page, PersistenceService};

/// AI resource type discriminators, stored in `ai_resource.type`.
pub mod resource_type {
    /// MCP server resource.
    pub const MCP: &str = "mcp";
    /// A2A agent resource.
    pub const AGENT: &str = "agent";
    /// Skill resource.
    pub const SKILL: &str = "skill";
    /// Prompt resource.
    pub const PROMPT: &str = "prompt";
    /// Agent specification resource.
    pub const AGENT_SPEC: &str = "agentspec";
}

/// Metadata status values, stored in `ai_resource.status`.
pub mod meta_status {
    /// Resource is enabled.
    pub const ENABLE: &str = "enable";
    /// Resource is disabled.
    pub const DISABLE: &str = "disable";
}

/// Version status values, stored in `ai_resource_version.status`.
///
/// Lifecycle: `draft` -> `reviewing` -> `reviewed` -> `online` / `offline`.
pub mod version_status {
    /// Version is being edited and not yet submitted.
    pub const DRAFT: &str = "draft";
    /// Version is under review (pipeline running).
    pub const REVIEWING: &str = "reviewing";
    /// Review finished, awaiting the next action.
    pub const REVIEWED: &str = "reviewed";
    /// Version is published and active.
    pub const ONLINE: &str = "online";
    /// Version has been taken offline.
    pub const OFFLINE: &str = "offline";
}

/// Visibility scopes, stored in `ai_resource.scope`.
pub mod scope {
    /// Visible to everyone.
    pub const PUBLIC: &str = "PUBLIC";
    /// Visible only to the owner.
    pub const PRIVATE: &str = "PRIVATE";
}

/// Constants for the AI resource search index.
pub mod search {
    /// Search document is queryable.
    pub const STATUS_ENABLED: &str = "enabled";
    /// Search document is awaiting index building.
    pub const STATUS_PENDING: &str = "pending";
    /// Documents are generated automatically from the resource content.
    pub const GENERATE_MODE_AUTO: &str = "auto";

    /// Chunk holding the resource description.
    pub const CHUNK_TYPE_DESCRIPTION: &str = "description";
    /// Chunk holding a declared capability.
    pub const CHUNK_TYPE_CAPABILITY: &str = "capability";
    /// Chunk holding a representative query.
    pub const CHUNK_TYPE_REPRESENTATIVE_QUERY: &str = "representative_query";
    /// Chunk holding a tag.
    pub const CHUNK_TYPE_TAG: &str = "tag";
    /// Chunk holding input/output metadata.
    pub const CHUNK_TYPE_METADATA_IO: &str = "metadata_io";
    /// Chunk holding risk metadata.
    pub const CHUNK_TYPE_METADATA_RISK: &str = "metadata_risk";
    /// Chunk describing what the resource must not be used for.
    pub const CHUNK_TYPE_NOT_FOR: &str = "not_for";
    /// Chunk holding skill content.
    pub const CHUNK_TYPE_SKILL_CONTENT: &str = "skill_content";
    /// Chunk holding prompt content.
    pub const CHUNK_TYPE_PROMPT_CONTENT: &str = "prompt_content";
    /// Chunk holding MCP content.
    pub const CHUNK_TYPE_MCP_CONTENT: &str = "mcp_content";
    /// Chunk holding agent content.
    pub const CHUNK_TYPE_AGENT_CONTENT: &str = "agent_content";
    /// Chunk holding agentspec content.
    pub const CHUNK_TYPE_AGENTSPEC_CONTENT: &str = "agentspec_content";
    /// Chunk holding an LLM-generated summary.
    pub const CHUNK_TYPE_AI_SUMMARY: &str = "ai_summary";
    /// Chunk describing the search intent.
    pub const CHUNK_TYPE_SEARCH_INTENT: &str = "search_intent";
    /// Chunk holding a search term.
    pub const CHUNK_TYPE_SEARCH_TERM: &str = "search_term";

    /// Task type for durable search index maintenance.
    pub const TASK_TYPE: &str = "search_index";
    /// Stage that builds the relational document and chunks.
    pub const STAGE_BASE_INDEX: &str = "base_index";
    /// Stage that performs LLM enrichment over the built index.
    pub const STAGE_LLM_ENHANCEMENT: &str = "llm_enhancement";

    /// Task is waiting to be picked up.
    pub const TASK_STATUS_PENDING: &str = "pending";
    /// Task is currently leased by a worker.
    pub const TASK_STATUS_PROCESSING: &str = "processing";
    /// Task finished all of its stages.
    pub const TASK_STATUS_COMPLETED: &str = "completed";
}

/// Repository for AI resources.
///
/// Wraps a [`PersistenceService`] so AI services do not need to touch the
/// persistence trait directly, and so the persisted vocabularies above stay in
/// one place.
pub struct AiResourceRepository<'a> {
    persistence: &'a dyn PersistenceService,
}

impl<'a> AiResourceRepository<'a> {
    /// Create a repository bound to the given persistence service.
    pub fn new(persistence: &'a dyn PersistenceService) -> Self {
        Self { persistence }
    }

    /// Find a resource by namespace, name and type.
    pub async fn find(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
    ) -> anyhow::Result<Option<AiResourceInfo>> {
        self.persistence
            .ai_resource_find(namespace_id, name, resource_type)
            .await
    }

    /// Insert a resource, returning the generated ID.
    pub async fn insert(&self, resource: &AiResourceInfo) -> anyhow::Result<i64> {
        self.persistence.ai_resource_insert(resource).await
    }

    /// List resources with filters and pagination.
    pub async fn list(
        &self,
        namespace_id: &str,
        resource_type: &str,
        filter: &AiResourceListFilter<'_>,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<AiResourceInfo>> {
        self.persistence
            .ai_resource_list(namespace_id, resource_type, filter, page_no, page_size)
            .await
    }

    /// Find all resources of one type in a namespace, without pagination.
    pub async fn find_all(
        &self,
        namespace_id: &str,
        resource_type: &str,
    ) -> anyhow::Result<Vec<AiResourceInfo>> {
        self.persistence
            .ai_resource_find_all(namespace_id, resource_type)
            .await
    }

    /// Update `version_info` with an optimistic lock on `meta_version`.
    pub async fn update_version_info_cas(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        expected_meta_version: i64,
        version_info: &str,
        new_meta_version: i64,
    ) -> anyhow::Result<bool> {
        self.persistence
            .ai_resource_update_version_info_cas(
                namespace_id,
                name,
                resource_type,
                expected_meta_version,
                version_info,
                new_meta_version,
            )
            .await
    }

    /// Update the business tags of a resource.
    pub async fn update_biz_tags(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        biz_tags: &str,
    ) -> anyhow::Result<()> {
        self.persistence
            .ai_resource_update_biz_tags(namespace_id, name, resource_type, biz_tags)
            .await
    }

    /// Update the metadata status of a resource.
    pub async fn update_status(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        status: &str,
    ) -> anyhow::Result<()> {
        self.persistence
            .ai_resource_update_status(namespace_id, name, resource_type, status)
            .await
    }

    /// Update the visibility scope of a resource.
    pub async fn update_scope(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        scope: &str,
    ) -> anyhow::Result<()> {
        self.persistence
            .ai_resource_update_scope(namespace_id, name, resource_type, scope)
            .await
    }

    /// Increment the download counter of a resource.
    pub async fn increment_download_count(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        increment: i64,
    ) -> anyhow::Result<()> {
        self.persistence
            .ai_resource_increment_download_count(namespace_id, name, resource_type, increment)
            .await
    }

    /// Delete a resource.
    pub async fn delete(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
    ) -> anyhow::Result<u64> {
        self.persistence
            .ai_resource_delete(namespace_id, name, resource_type)
            .await
    }

    /// Find one version of a resource.
    pub async fn version_find(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        version: &str,
    ) -> anyhow::Result<Option<AiResourceVersionInfo>> {
        self.persistence
            .ai_resource_version_find(namespace_id, name, resource_type, version)
            .await
    }

    /// Insert a version, returning the generated ID.
    pub async fn version_insert(
        &self,
        version: &AiResourceVersionInfo,
    ) -> anyhow::Result<i64> {
        self.persistence.ai_resource_version_insert(version).await
    }

    /// Update the status of a version.
    pub async fn version_update_status(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        version: &str,
        status: &str,
    ) -> anyhow::Result<()> {
        self.persistence
            .ai_resource_version_update_status(namespace_id, name, resource_type, version, status)
            .await
    }

    /// List every version of a resource.
    pub async fn version_list(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
    ) -> anyhow::Result<Vec<AiResourceVersionInfo>> {
        self.persistence
            .ai_resource_version_list(namespace_id, name, resource_type)
            .await
    }

    /// Delete one version of a resource.
    pub async fn version_delete(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        version: &str,
    ) -> anyhow::Result<u64> {
        self.persistence
            .ai_resource_version_delete(namespace_id, name, resource_type, version)
            .await
    }
}
