//! Prompt management service — `ai_resource`-backed storage
//!
//! Prompts are stored like every other AI domain, which is also how upstream
//! does it:
//! - `ai_resource` (type `prompt`) — biz tags, plus the resource-level
//!   bookkeeping in `version_info`: label routing, latest version and (because
//!   persistence cannot update the description column) the description.
//! - `ai_resource_version` — one row per version, with the template and its
//!   metadata in `storage`.
//!
//! This replaces the earlier config-backed layout (group `nacos-ai-prompt`,
//! four configs per prompt). The public behaviour is unchanged:
//! `tests/prompt_persistence.rs` pins it and must still pass.

use std::sync::Arc;

use batata_persistence::model::{
    AiResourceInfo, AiResourceListFilter, AiResourceVersionInfo, Page,
};
use batata_persistence::PersistenceService;
use tracing::debug;

use batata_common::model::ai::ResourceVersionInfo;

use crate::model::prompt::*;
use crate::repository::{meta_status, resource_type, scope, version_status};
use crate::service::version_lifecycle;

use md5::Digest;

/// Prompt operation service (admin + client)
pub struct PromptOperationService {
    persistence: Arc<dyn PersistenceService>,
}

impl PromptOperationService {
    /// Creates a new `PromptOperationService` backed by the given persistence.
    pub fn new(persistence: Arc<dyn PersistenceService>) -> Self {
        Self { persistence }
    }

    // ========================================================================
    // Internal helpers — resource access
    // ========================================================================

    async fn find_resource(
        &self,
        namespace_id: &str,
        prompt_key: &str,
    ) -> anyhow::Result<Option<AiResourceInfo>> {
        self.persistence
            .ai_resource_find(namespace_id, prompt_key, resource_type::PROMPT)
            .await
    }

    /// Load the resource-level bookkeeping (labels, latest, description).
    async fn load_meta(
        &self,
        namespace_id: &str,
        prompt_key: &str,
    ) -> anyhow::Result<Option<ResourceVersionInfo>> {
        Ok(self
            .find_resource(namespace_id, prompt_key)
            .await?
            .and_then(|resource| {
                resource
                    .version_info
                    .as_deref()
                    .and_then(|json| serde_json::from_str::<ResourceVersionInfo>(json).ok())
            }))
    }

    /// Load the bookkeeping, requiring the prompt to exist.
    async fn require_meta(
        &self,
        namespace_id: &str,
        prompt_key: &str,
    ) -> anyhow::Result<ResourceVersionInfo> {
        self.load_meta(namespace_id, prompt_key)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Prompt '{}' not found", prompt_key))
    }

    /// Save the bookkeeping under an optimistic lock.
    async fn save_meta(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        meta: &ResourceVersionInfo,
    ) -> anyhow::Result<()> {
        let resource = self
            .find_resource(namespace_id, prompt_key)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Prompt '{}' not found", prompt_key))?;
        let json = serde_json::to_string(meta)?;
        self.persistence
            .ai_resource_update_version_info_cas(
                namespace_id,
                prompt_key,
                resource_type::PROMPT,
                resource.meta_version,
                &json,
                resource.meta_version + 1,
            )
            .await?;
        Ok(())
    }

    /// Load one version's content.
    async fn load_version(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        version: &str,
    ) -> anyhow::Result<Option<PromptVersionInfo>> {
        let Some(row) = self
            .persistence
            .ai_resource_version_find(namespace_id, prompt_key, resource_type::PROMPT, version)
            .await?
        else {
            return Ok(None);
        };
        Ok(Some(Self::version_info(&row)))
    }

    /// Every version of a prompt, newest first.
    async fn all_versions(
        &self,
        namespace_id: &str,
        prompt_key: &str,
    ) -> anyhow::Result<Vec<AiResourceVersionInfo>> {
        let mut rows = self
            .persistence
            .ai_resource_version_list(namespace_id, prompt_key, resource_type::PROMPT)
            .await?;
        rows.sort_by(|a, b| compare_versions(&a.version, &b.version).reverse());
        Ok(rows)
    }

    /// Rebuild the public view of one stored version.
    fn version_info(row: &AiResourceVersionInfo) -> PromptVersionInfo {
        let storage: PromptStorage = row
            .storage
            .as_deref()
            .and_then(|json| serde_json::from_str(json).ok())
            .unwrap_or_default();

        PromptVersionInfo {
            prompt_key: row.name.clone(),
            version: row.version.clone(),
            template: storage.template,
            md5: storage.md5,
            commit_msg: storage.commit_msg,
            src_user: row.author.clone().or(storage.src_user),
            gmt_modified: storage.gmt_modified,
            variables: storage.variables,
        }
    }

    /// Work out which version a request means: explicit version, label, or the
    /// latest one.
    fn resolve_version(
        meta: &ResourceVersionInfo,
        versions: &[String],
        version: Option<&str>,
        label: Option<&str>,
    ) -> Option<String> {
        if let Some(asked) = version.filter(|v| !v.is_empty()) {
            return versions.iter().find(|v| *v == asked).cloned();
        }
        if let Some(name) = label.filter(|l| !l.is_empty()) {
            return meta.labels.get(name).cloned();
        }
        meta.latest_version()
            .cloned()
            .or_else(|| versions.first().cloned())
    }

    /// Parse the comma-separated biz tags stored on the resource.
    fn parse_biz_tags(raw: Option<&str>) -> Vec<String> {
        raw.unwrap_or_default()
            .split(',')
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect()
    }

    // ========================================================================
    // Admin operations
    // ========================================================================

    /// Publish a new prompt version.
    #[allow(clippy::too_many_arguments)]
    pub async fn publish_version(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        version: &str,
        template: &str,
        commit_msg: Option<&str>,
        description: Option<&str>,
        biz_tags: Vec<String>,
        variables: Option<Vec<PromptVariable>>,
        src_user: &str,
        src_ip: &str,
    ) -> anyhow::Result<bool> {
        // Kept in the signature for API compatibility with the console layer.
        let _ = src_ip;

        // Validate version format
        if !is_valid_version(version) {
            anyhow::bail!(
                "Invalid version format '{}', must be major.minor.patch",
                version
            );
        }

        if template.is_empty() {
            anyhow::bail!("Template cannot be empty");
        }

        let now = chrono::Utc::now().timestamp_millis();
        let md5 = const_hex::encode(md5::Md5::digest(template.as_bytes()));

        // A version cannot be republished.
        if self
            .persistence
            .ai_resource_version_find(namespace_id, prompt_key, resource_type::PROMPT, version)
            .await?
            .is_some()
        {
            anyhow::bail!(
                "Version '{}' already exists for prompt '{}'",
                version,
                prompt_key
            );
        }

        // First publish creates the resource.
        let mut meta = match self.find_resource(namespace_id, prompt_key).await? {
            Some(resource) => {
                // The description lives on the resource itself.
                if let Some(d) = description {
                    self.persistence
                        .ai_resource_update_description(
                            namespace_id,
                            prompt_key,
                            resource_type::PROMPT,
                            d,
                        )
                        .await?;
                }
                resource
                    .version_info
                    .as_deref()
                    .and_then(|json| serde_json::from_str::<ResourceVersionInfo>(json).ok())
                    .unwrap_or_default()
            }
            None => {
                let meta = ResourceVersionInfo::default();
                self.persistence
                    .ai_resource_insert(&AiResourceInfo {
                        name: prompt_key.to_string(),
                        resource_type: resource_type::PROMPT.to_string(),
                        namespace_id: namespace_id.to_string(),
                        description: description.map(|d| d.to_string()),
                        status: Some(meta_status::ENABLE.to_string()),
                        version_info: Some(serde_json::to_string(&meta)?),
                        meta_version: 1,
                        scope: scope::PRIVATE.to_string(),
                        owner: src_user.to_string(),
                        download_count: 0,
                        ..Default::default()
                    })
                    .await?;
                meta
            }
        };

        meta.set_latest(version);

        if !biz_tags.is_empty() {
            self.persistence
                .ai_resource_update_biz_tags(
                    namespace_id,
                    prompt_key,
                    resource_type::PROMPT,
                    &biz_tags.join(","),
                )
                .await?;
        }

        // Store the version itself.
        let storage = PromptStorage {
            prompt_key: prompt_key.to_string(),
            template: template.to_string(),
            md5: Some(md5),
            commit_msg: commit_msg.map(|s| s.to_string()),
            src_user: Some(src_user.to_string()),
            gmt_modified: Some(now),
            variables,
        };
        self.persistence
            .ai_resource_version_insert(&AiResourceVersionInfo {
                name: prompt_key.to_string(),
                resource_type: resource_type::PROMPT.to_string(),
                namespace_id: namespace_id.to_string(),
                version: version.to_string(),
                status: version_status::ONLINE.to_string(),
                author: Some(src_user.to_string()),
                description: description.map(|d| d.to_string()),
                storage: Some(serde_json::to_string(&storage)?),
                download_count: 0,
                ..Default::default()
            })
            .await?;

        self.save_meta(namespace_id, prompt_key, &meta).await?;

        debug!(
            "Published prompt '{}' version '{}' in namespace '{}'",
            prompt_key, version, namespace_id
        );

        Ok(true)
    }

    /// Get prompt metadata (resource + version bookkeeping composed)
    pub async fn get_meta(&self, namespace_id: &str, prompt_key: &str) -> Option<PromptMetaInfo> {
        let resource = self.find_resource(namespace_id, prompt_key).await.ok()??;
        let meta: ResourceVersionInfo = resource
            .version_info
            .as_deref()
            .and_then(|json| serde_json::from_str(json).ok())
            .unwrap_or_default();
        let rows = self.all_versions(namespace_id, prompt_key).await.ok()?;

        Some(PromptMetaInfo {
            schema_version: 1,
            prompt_key: prompt_key.to_string(),
            description: resource.description.clone(),
            biz_tags: Self::parse_biz_tags(resource.biz_tags.as_deref()),
            biz_tags_str: resource.biz_tags.clone(),
            latest_version: meta.latest_version().cloned(),
            gmt_modified: rows.first().and_then(|row| {
                row.storage
                    .as_deref()
                    .and_then(|json| serde_json::from_str::<PromptStorage>(json).ok())
                    .and_then(|storage| storage.gmt_modified)
            }),
            editing_version: meta.editing_version.clone(),
            reviewing_version: meta.reviewing_version.clone(),
            online_cnt: meta.online_cnt,
            download_count: Some(resource.download_count),
            versions: rows.iter().map(|row| row.version.clone()).collect(),
            version_details: Self::version_summaries(&rows),
            labels: meta.labels.clone(),
        })
    }

    /// Build version summaries, newest first, each carrying its lifecycle
    /// status — the console governance view renders them for that reason.
    fn version_summaries(rows: &[AiResourceVersionInfo]) -> Vec<PromptVersionSummary> {
        rows.iter()
            .map(|row| {
                let info = Self::version_info(row);
                PromptVersionSummary {
                    prompt_key: info.prompt_key,
                    version: info.version,
                    status: row.status.clone(),
                    commit_msg: info.commit_msg,
                    src_user: info.src_user,
                    gmt_modified: info.gmt_modified,
                    publish_pipeline_info: None,
                    download_count: Some(row.download_count),
                }
            })
            .collect()
    }

    /// Delete a prompt and all its versions
    pub async fn delete_prompt(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        src_user: &str,
    ) -> anyhow::Result<bool> {
        let _ = src_user;

        self.persistence
            .ai_resource_version_delete_all(namespace_id, prompt_key, resource_type::PROMPT)
            .await?;
        self.persistence
            .ai_resource_delete(namespace_id, prompt_key, resource_type::PROMPT)
            .await?;

        debug!(
            "Deleted prompt '{}' in namespace '{}'",
            prompt_key, namespace_id
        );
        Ok(true)
    }

    /// Bind a label to a specific version
    pub async fn bind_label(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        label: &str,
        version: &str,
        src_user: &str,
        src_ip: &str,
    ) -> anyhow::Result<bool> {
        let _ = (src_user, src_ip);

        let mut meta = self.require_meta(namespace_id, prompt_key).await?;

        let known = self
            .persistence
            .ai_resource_version_find(namespace_id, prompt_key, resource_type::PROMPT, version)
            .await?
            .is_some();
        if !known {
            anyhow::bail!(
                "Version '{}' not found for prompt '{}'",
                version,
                prompt_key
            );
        }

        meta.labels.insert(label.to_string(), version.to_string());
        self.save_meta(namespace_id, prompt_key, &meta).await?;
        Ok(true)
    }

    /// Unbind a label
    pub async fn unbind_label(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        label: &str,
        src_user: &str,
        src_ip: &str,
    ) -> anyhow::Result<bool> {
        let _ = (src_user, src_ip);

        let mut meta = self.require_meta(namespace_id, prompt_key).await?;
        meta.labels.remove(label);
        self.save_meta(namespace_id, prompt_key, &meta).await?;
        Ok(true)
    }

    /// Update prompt metadata (description, bizTags) without changing versions
    pub async fn update_metadata(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        description: Option<&str>,
        biz_tags: Option<Vec<String>>,
        src_user: &str,
        src_ip: &str,
    ) -> anyhow::Result<bool> {
        let _ = (src_user, src_ip);

        // Updating metadata for a prompt that does not exist is an error.
        self.require_meta(namespace_id, prompt_key).await?;

        if let Some(d) = description {
            self.persistence
                .ai_resource_update_description(namespace_id, prompt_key, resource_type::PROMPT, d)
                .await?;
        }

        if let Some(tags) = biz_tags {
            self.persistence
                .ai_resource_update_biz_tags(
                    namespace_id,
                    prompt_key,
                    resource_type::PROMPT,
                    &tags.join(","),
                )
                .await?;
        }
        Ok(true)
    }

    /// Query a specific prompt version with version/label/latest resolution
    pub async fn query_detail(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        version: Option<&str>,
        label: Option<&str>,
    ) -> anyhow::Result<Option<PromptVersionInfo>> {
        let Some(meta) = self.load_meta(namespace_id, prompt_key).await? else {
            return Ok(None);
        };
        let rows = self.all_versions(namespace_id, prompt_key).await?;
        let versions: Vec<String> = rows.iter().map(|row| row.version.clone()).collect();

        match Self::resolve_version(&meta, &versions, version, label) {
            Some(resolved) => self.load_version(namespace_id, prompt_key, &resolved).await,
            None => {
                if version.is_some() {
                    anyhow::bail!("Version not found for prompt '{}'", prompt_key);
                }
                if label.is_some() {
                    anyhow::bail!("Label not found for prompt '{}'", prompt_key);
                }
                Ok(None)
            }
        }
    }

    // ========================================================================
    // Version lifecycle
    //
    // The state machine is shared with every other AI resource type
    // (`version_lifecycle`); only the resource type differs.
    // ========================================================================

    /// Create a draft version, which is not served until it is published.
    pub async fn create_draft(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        target_version: Option<&str>,
        template: &str,
        description: Option<&str>,
        variables: Option<Vec<PromptVariable>>,
        src_user: &str,
    ) -> anyhow::Result<String> {
        if template.is_empty() {
            anyhow::bail!("Template cannot be empty");
        }

        let version = match target_version {
            Some(v) => {
                if !is_valid_version(v) {
                    anyhow::bail!(
                        "Invalid version format '{}', must be major.minor.patch",
                        v
                    );
                }
                v.to_string()
            }
            None => PROMPT_DEFAULT_VERSION.to_string(),
        };

        if self
            .persistence
            .ai_resource_version_find(namespace_id, prompt_key, resource_type::PROMPT, &version)
            .await?
            .is_some()
        {
            anyhow::bail!(
                "Version '{}' already exists for prompt '{}'",
                version,
                prompt_key
            );
        }

        // Ensure the prompt exists and load its bookkeeping.
        if self.find_resource(namespace_id, prompt_key).await?.is_none() {
            self.persistence
                .ai_resource_insert(&AiResourceInfo {
                    name: prompt_key.to_string(),
                    resource_type: resource_type::PROMPT.to_string(),
                    namespace_id: namespace_id.to_string(),
                    description: description.map(|d| d.to_string()),
                    status: Some(meta_status::ENABLE.to_string()),
                    version_info: Some(serde_json::to_string(&ResourceVersionInfo::default())?),
                    meta_version: 1,
                    scope: scope::PRIVATE.to_string(),
                    owner: src_user.to_string(),
                    download_count: 0,
                    ..Default::default()
                })
                .await?;
        }
        let mut meta = self.require_meta(namespace_id, prompt_key).await?;

        // Only one version may be edited at a time.
        if meta.editing_version.is_some() {
            anyhow::bail!("Prompt '{}' already has a draft version", prompt_key);
        }

        let now = chrono::Utc::now().timestamp_millis();
        let storage = PromptStorage {
            prompt_key: prompt_key.to_string(),
            template: template.to_string(),
            md5: Some(const_hex::encode(md5::Md5::digest(template.as_bytes()))),
            commit_msg: None,
            src_user: Some(src_user.to_string()),
            gmt_modified: Some(now),
            variables,
        };
        self.persistence
            .ai_resource_version_insert(&AiResourceVersionInfo {
                name: prompt_key.to_string(),
                resource_type: resource_type::PROMPT.to_string(),
                namespace_id: namespace_id.to_string(),
                version: version.clone(),
                status: version_status::DRAFT.to_string(),
                author: Some(src_user.to_string()),
                description: description.map(|d| d.to_string()),
                storage: Some(serde_json::to_string(&storage)?),
                download_count: 0,
                ..Default::default()
            })
            .await?;

        meta.editing_version = Some(version.clone());
        self.save_meta(namespace_id, prompt_key, &meta).await?;

        Ok(version)
    }

    /// Replace the content of the version currently being edited.
    pub async fn update_draft(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        template: &str,
        commit_msg: Option<&str>,
        variables: Option<Vec<PromptVariable>>,
        src_user: &str,
    ) -> anyhow::Result<String> {
        if template.is_empty() {
            anyhow::bail!("Template cannot be empty");
        }

        let meta = self.require_meta(namespace_id, prompt_key).await?;
        let version = meta
            .editing_version
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Prompt '{}' has no draft version", prompt_key))?;

        let storage = PromptStorage {
            prompt_key: prompt_key.to_string(),
            template: template.to_string(),
            md5: Some(const_hex::encode(md5::Md5::digest(template.as_bytes()))),
            commit_msg: commit_msg.map(|s| s.to_string()),
            src_user: Some(src_user.to_string()),
            gmt_modified: Some(chrono::Utc::now().timestamp_millis()),
            variables,
        };
        self.persistence
            .ai_resource_version_update_storage(
                namespace_id,
                prompt_key,
                resource_type::PROMPT,
                &version,
                &serde_json::to_string(&storage)?,
                None,
            )
            .await?;

        Ok(version)
    }

    /// Discard the version currently being edited.
    pub async fn delete_draft(
        &self,
        namespace_id: &str,
        prompt_key: &str,
    ) -> anyhow::Result<()> {
        let mut meta = self.require_meta(namespace_id, prompt_key).await?;
        let version = meta
            .editing_version
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Prompt '{}' has no draft version", prompt_key))?;

        self.persistence
            .ai_resource_version_delete(namespace_id, prompt_key, resource_type::PROMPT, &version)
            .await?;

        meta.editing_version = None;
        self.save_meta(namespace_id, prompt_key, &meta).await?;
        Ok(())
    }

    /// Submit a draft for review (draft → reviewing).
    pub async fn submit(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        version: &str,
    ) -> anyhow::Result<()> {
        version_lifecycle::submit(
            self.persistence.as_ref(),
            namespace_id,
            prompt_key,
            resource_type::PROMPT,
            version,
        )
        .await
    }

    /// Publish a reviewed version (reviewing / reviewed / online → online).
    pub async fn publish(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        version: &str,
    ) -> anyhow::Result<()> {
        version_lifecycle::publish(
            self.persistence.as_ref(),
            namespace_id,
            prompt_key,
            resource_type::PROMPT,
            version,
        )
        .await
    }

    /// Publish a version bypassing the review gate.
    pub async fn force_publish(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        version: &str,
    ) -> anyhow::Result<()> {
        version_lifecycle::force_publish(
            self.persistence.as_ref(),
            namespace_id,
            prompt_key,
            resource_type::PROMPT,
            version,
        )
        .await
    }

    /// Move a version back to draft so it can be edited again.
    pub async fn redraft(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        version: &str,
    ) -> anyhow::Result<()> {
        version_lifecycle::redraft(
            self.persistence.as_ref(),
            namespace_id,
            prompt_key,
            resource_type::PROMPT,
            version,
        )
        .await
    }

    /// Bring an offline version back online.
    pub async fn online(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        version: &str,
    ) -> anyhow::Result<()> {
        version_lifecycle::online(
            self.persistence.as_ref(),
            namespace_id,
            prompt_key,
            resource_type::PROMPT,
            version,
        )
        .await
    }

    /// Take an online version offline.
    pub async fn offline(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        version: &str,
    ) -> anyhow::Result<()> {
        version_lifecycle::offline(
            self.persistence.as_ref(),
            namespace_id,
            prompt_key,
            resource_type::PROMPT,
            version,
        )
        .await
    }

    /// Replace the label routing, preserving the server-managed `latest` label.
    pub async fn update_labels(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        labels: std::collections::HashMap<String, String>,
    ) -> anyhow::Result<std::collections::HashMap<String, String>> {
        version_lifecycle::update_labels(
            self.persistence.as_ref(),
            namespace_id,
            prompt_key,
            resource_type::PROMPT,
            labels,
        )
        .await
    }

    /// Governance view.
    ///
    /// Upstream answers `GET /governance` with the same `PromptMetaInfo` as the
    /// metadata endpoint, so this is deliberately not a second shape.
    pub async fn get_governance(
        &self,
        namespace_id: &str,
        prompt_key: &str,
    ) -> Option<PromptMetaInfo> {
        self.get_meta(namespace_id, prompt_key).await
    }

    /// Submit the version currently being edited.
    ///
    /// The console's submit action carries no version — it acts on the draft —
    /// so the draft is resolved here.
    pub async fn submit_draft(
        &self,
        namespace_id: &str,
        prompt_key: &str,
    ) -> anyhow::Result<String> {
        let meta = self.require_meta(namespace_id, prompt_key).await?;
        let version = meta
            .editing_version
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Prompt '{}' has no draft version", prompt_key))?;
        self.submit(namespace_id, prompt_key, &version).await?;
        Ok(version)
    }

    // ========================================================================
    // Client operations
    // ========================================================================

    /// Query prompt with MD5-based conditional support.
    /// Returns None if client md5 matches (NOT_MODIFIED).
    pub async fn query_prompt(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        version: Option<&str>,
        label: Option<&str>,
        client_md5: Option<&str>,
    ) -> anyhow::Result<Option<PromptVersionInfo>> {
        let info = self
            .query_detail(namespace_id, prompt_key, version, label)
            .await?;

        if let Some(ref info) = info {
            // If client already has this version (MD5 match), return None (NOT_MODIFIED)
            if let Some(client) = client_md5
                && !client.is_empty()
                && let Some(ref server_md5) = info.md5
                && client == server_md5
            {
                return Ok(None);
            }
        }

        Ok(info)
    }

    /// List prompt versions for a prompt key (sorted by version descending)
    pub async fn list_versions(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<PromptVersionSummary>> {
        if self.find_resource(namespace_id, prompt_key).await?.is_none() {
            anyhow::bail!("Prompt '{}' not found", prompt_key);
        }

        let rows = self.all_versions(namespace_id, prompt_key).await?;
        let total = rows.len() as u64;
        let start = ((page_no.saturating_sub(1)) * page_size) as usize;

        let summaries = Self::version_summaries(&rows)
            .into_iter()
            .skip(start)
            .take(page_size as usize)
            .collect();

        Ok(Page {
            total_count: total,
            page_number: page_no,
            pages_available: total.div_ceil(page_size),
            page_items: summaries,
        })
    }

    /// List prompts with search/filter and pagination.
    pub async fn list_prompts(
        &self,
        namespace_id: &str,
        prompt_key: Option<&str>,
        search: Option<&str>,
        biz_tags: Option<&str>,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<PromptMetaSummary>> {
        let accurate = search == Some("accurate");
        let filter = AiResourceListFilter::new()
            .with_name_filter(prompt_key.filter(|key| !key.is_empty()), accurate);

        let page = self
            .persistence
            .ai_resource_list(namespace_id, resource_type::PROMPT, &filter, page_no, page_size)
            .await?;

        let wanted: Vec<String> = biz_tags
            .map(|t| {
                t.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        let mut summaries = Vec::with_capacity(page.page_items.len());
        for resource in &page.page_items {
            let meta: ResourceVersionInfo = resource
                .version_info
                .as_deref()
                .and_then(|json| serde_json::from_str(json).ok())
                .unwrap_or_default();

            let tags = Self::parse_biz_tags(resource.biz_tags.as_deref());
            if !wanted.is_empty() && !wanted.iter().any(|t| tags.contains(t)) {
                continue;
            }

            summaries.push(PromptMetaSummary {
                schema_version: 1,
                prompt_key: resource.name.clone(),
                description: resource.description.clone(),
                biz_tags: tags,
                biz_tags_str: resource.biz_tags.clone(),
                latest_version: meta.latest_version().cloned(),
                gmt_modified: None,
                editing_version: meta.editing_version.clone(),
                reviewing_version: meta.reviewing_version.clone(),
                online_cnt: meta.online_cnt,
                labels: meta.labels.clone(),
                download_count: Some(resource.download_count),
            });
        }

        Ok(Page {
            total_count: page.total_count,
            page_number: page.page_number,
            pages_available: page.pages_available,
            page_items: summaries,
        })
    }
}

/// Console-facing surface.
///
/// The console lives in `batata-console`, which depends on `batata-common`
/// only, so it reaches prompts through the [`PromptService`] trait rather than
/// this concrete type.
#[async_trait::async_trait]
impl batata_common::PromptService for PromptOperationService {
    async fn list_prompts(
        &self,
        namespace_id: &str,
        prompt_key: Option<&str>,
        search: Option<&str>,
        biz_tags: Option<&str>,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<PromptMetaSummary>> {
        self.list_prompts(namespace_id, prompt_key, search, biz_tags, page_no, page_size)
            .await
    }

    async fn get_governance(
        &self,
        namespace_id: &str,
        prompt_key: &str,
    ) -> anyhow::Result<Option<PromptMetaInfo>> {
        Ok(self.get_governance(namespace_id, prompt_key).await)
    }

    async fn query_detail(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        version: Option<&str>,
        label: Option<&str>,
    ) -> anyhow::Result<Option<PromptVersionInfo>> {
        self.query_detail(namespace_id, prompt_key, version, label)
            .await
    }

    async fn list_versions(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<PromptVersionSummary>> {
        self.list_versions(namespace_id, prompt_key, page_no, page_size)
            .await
    }

    async fn create_draft(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        target_version: Option<&str>,
        template: &str,
        description: Option<&str>,
        variables: Option<Vec<PromptVariable>>,
        src_user: &str,
    ) -> anyhow::Result<String> {
        self.create_draft(
            namespace_id,
            prompt_key,
            target_version,
            template,
            description,
            variables,
            src_user,
        )
        .await
    }

    async fn update_draft(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        template: &str,
        commit_msg: Option<&str>,
        variables: Option<Vec<PromptVariable>>,
        src_user: &str,
    ) -> anyhow::Result<String> {
        self.update_draft(
            namespace_id,
            prompt_key,
            template,
            commit_msg,
            variables,
            src_user,
        )
        .await
    }

    async fn delete_draft(&self, namespace_id: &str, prompt_key: &str) -> anyhow::Result<()> {
        self.delete_draft(namespace_id, prompt_key).await
    }

    async fn submit(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        version: &str,
    ) -> anyhow::Result<()> {
        self.submit(namespace_id, prompt_key, version).await
    }

    async fn submit_draft(&self, namespace_id: &str, prompt_key: &str) -> anyhow::Result<String> {
        self.submit_draft(namespace_id, prompt_key).await
    }

    async fn publish(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        version: &str,
    ) -> anyhow::Result<()> {
        self.publish(namespace_id, prompt_key, version).await
    }

    async fn force_publish(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        version: &str,
    ) -> anyhow::Result<()> {
        self.force_publish(namespace_id, prompt_key, version).await
    }

    async fn redraft(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        version: &str,
    ) -> anyhow::Result<()> {
        self.redraft(namespace_id, prompt_key, version).await
    }

    async fn online(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        version: &str,
    ) -> anyhow::Result<()> {
        self.online(namespace_id, prompt_key, version).await
    }

    async fn offline(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        version: &str,
    ) -> anyhow::Result<()> {
        self.offline(namespace_id, prompt_key, version).await
    }

    async fn update_labels(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        labels: std::collections::HashMap<String, String>,
    ) -> anyhow::Result<std::collections::HashMap<String, String>> {
        self.update_labels(namespace_id, prompt_key, labels).await
    }

    async fn update_metadata(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        description: Option<&str>,
        biz_tags: Option<Vec<String>>,
    ) -> anyhow::Result<bool> {
        self.update_metadata(namespace_id, prompt_key, description, biz_tags, "", "")
            .await
    }

    async fn delete_prompt(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        src_user: &str,
    ) -> anyhow::Result<bool> {
        self.delete_prompt(namespace_id, prompt_key, src_user).await
    }
}
