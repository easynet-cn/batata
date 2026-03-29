//! Skill operation service — persistence-backed storage via ai_resource / ai_resource_version tables
//!
//! Skills use ai_resource (type="skill") for governance metadata and
//! ai_resource_version for version-specific content with draft→reviewing→online lifecycle.

use std::collections::HashMap;
use std::sync::Arc;

use batata_persistence::PersistenceService;
use batata_persistence::model::{AiResourceInfo, AiResourceVersionInfo, Page};
use chrono::Utc;
use tracing::debug;

use crate::model::skill::*;

/// Skill operation service backed by ai_resource/ai_resource_version tables
pub struct SkillOperationService {
    persistence: Arc<dyn PersistenceService>,
}

impl SkillOperationService {
    pub fn new(persistence: Arc<dyn PersistenceService>) -> Self {
        Self { persistence }
    }

    // ========================================================================
    // Internal DB helpers
    // ========================================================================

    async fn find_resource(
        &self,
        namespace_id: &str,
        name: &str,
    ) -> anyhow::Result<Option<AiResourceInfo>> {
        self.persistence
            .ai_resource_find(namespace_id, name, SKILL_TYPE)
            .await
    }

    async fn find_version(
        &self,
        namespace_id: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<Option<AiResourceVersionInfo>> {
        self.persistence
            .ai_resource_version_find(namespace_id, name, SKILL_TYPE, version)
            .await
    }

    async fn list_versions_for_skill(
        &self,
        namespace_id: &str,
        name: &str,
    ) -> anyhow::Result<Vec<AiResourceVersionInfo>> {
        self.persistence
            .ai_resource_version_list(namespace_id, name, SKILL_TYPE)
            .await
    }

    fn parse_version_info(resource: &AiResourceInfo) -> SkillVersionInfo {
        resource
            .version_info
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default()
    }

    fn resource_to_summary(resource: &AiResourceInfo) -> SkillSummary {
        let vi = Self::parse_version_info(resource);
        SkillSummary {
            namespace_id: resource.namespace_id.clone(),
            name: resource.name.clone(),
            description: resource.description.clone(),
            update_time: resource.gmt_modified.clone(),
            enable: resource.status.as_deref() == Some(RESOURCE_STATUS_ENABLE),
            biz_tags: resource.biz_tags.clone(),
            from: Some(resource.from.clone()),
            scope: Some(resource.scope.clone()),
            labels: vi.labels,
            editing_version: vi.editing_version,
            reviewing_version: vi.reviewing_version,
            online_cnt: vi.online_cnt,
            download_count: resource.download_count,
        }
    }

    fn version_to_summary(v: &AiResourceVersionInfo) -> SkillVersionSummary {
        SkillVersionSummary {
            version: v.version.clone(),
            status: v.status.clone(),
            author: v.author.clone(),
            description: v.description.clone(),
            create_time: v.gmt_create.clone(),
            update_time: v.gmt_modified.clone(),
            publish_pipeline_info: v.publish_pipeline_info.clone(),
            download_count: v.download_count,
        }
    }

    /// Maximum CAS retry attempts (matches Nacos 3-retry pattern)
    const CAS_MAX_RETRIES: u32 = 3;

    /// Update ai_resource.version_info JSON with optimistic lock (meta_version CAS).
    /// Retries up to CAS_MAX_RETRIES times on conflict, re-reading the resource each time.
    async fn update_version_info_cas(
        &self,
        namespace_id: &str,
        name: &str,
        expected_meta_version: i64,
        version_info: &SkillVersionInfo,
    ) -> anyhow::Result<bool> {
        let vi_json = serde_json::to_string(version_info)?;

        let ok = self
            .persistence
            .ai_resource_update_version_info_cas(
                namespace_id,
                name,
                SKILL_TYPE,
                expected_meta_version,
                &vi_json,
                expected_meta_version + 1,
            )
            .await?;

        if ok {
            return Ok(true);
        }

        // CAS failed — retry with fresh meta_version
        for _ in 1..Self::CAS_MAX_RETRIES {
            let resource = match self.find_resource(namespace_id, name).await? {
                Some(r) => r,
                None => return Ok(false),
            };
            let ok = self
                .persistence
                .ai_resource_update_version_info_cas(
                    namespace_id,
                    name,
                    SKILL_TYPE,
                    resource.meta_version,
                    &vi_json,
                    resource.meta_version + 1,
                )
                .await?;
            if ok {
                return Ok(true);
            }
        }

        anyhow::bail!(
            "CAS conflict updating version_info for skill '{}' after {} retries",
            name,
            Self::CAS_MAX_RETRIES
        )
    }

    // ========================================================================
    // Admin operations
    // ========================================================================

    /// Get skill detail (governance metadata + all version summaries)
    pub async fn get_skill_detail(
        &self,
        namespace_id: &str,
        name: &str,
    ) -> anyhow::Result<Option<SkillMeta>> {
        let resource = match self.find_resource(namespace_id, name).await? {
            Some(r) => r,
            None => return Ok(None),
        };

        let versions = self.list_versions_for_skill(namespace_id, name).await?;
        let vi = Self::parse_version_info(&resource);

        Ok(Some(SkillMeta {
            namespace_id: resource.namespace_id.clone(),
            name: resource.name.clone(),
            description: resource.description.clone(),
            update_time: resource.gmt_modified.clone(),
            enable: resource.status.as_deref() == Some(RESOURCE_STATUS_ENABLE),
            biz_tags: resource.biz_tags.clone(),
            from: Some(resource.from.clone()),
            scope: Some(resource.scope.clone()),
            labels: vi.labels,
            editing_version: vi.editing_version,
            reviewing_version: vi.reviewing_version,
            online_cnt: vi.online_cnt,
            download_count: resource.download_count,
            versions: versions.iter().map(Self::version_to_summary).collect(),
        }))
    }

    /// Get a specific version's full content
    pub async fn get_skill_version_detail(
        &self,
        namespace_id: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<Option<Skill>> {
        let ver = match self.find_version(namespace_id, name, version).await? {
            Some(v) => v,
            None => return Ok(None),
        };

        let storage: SkillStorage = ver
            .storage
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();

        let mut resource = HashMap::new();
        let mut skill_md = None;

        for file in &storage.files {
            if file.name == "SKILL.md" {
                skill_md = file.content.clone();
            } else {
                let sr = SkillResource {
                    name: file.name.clone(),
                    resource_type: file.file_type.clone(),
                    content: file.content.clone(),
                    metadata: HashMap::new(),
                };
                resource.insert(sr.resource_identifier(), sr);
            }
        }

        Ok(Some(Skill {
            namespace_id: namespace_id.to_string(),
            name: name.to_string(),
            description: ver.description.clone(),
            skill_md,
            resource,
        }))
    }

    /// Download skill version (same as detail but increments download count)
    pub async fn download_skill_version(
        &self,
        namespace_id: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<Option<Skill>> {
        let skill = self
            .get_skill_version_detail(namespace_id, name, version)
            .await?;

        if skill.is_some() {
            // Increment download count on version
            let _ = self
                .persistence
                .ai_resource_version_increment_download_count(
                    namespace_id,
                    name,
                    SKILL_TYPE,
                    version,
                    1,
                )
                .await;

            // Increment download count on resource
            let _ = self
                .persistence
                .ai_resource_increment_download_count(namespace_id, name, SKILL_TYPE, 1)
                .await;
        }

        Ok(skill)
    }

    /// Delete a skill and all its versions.
    /// Order: meta resource first (cuts off discovery), then versions (cleanup).
    pub async fn delete_skill(&self, namespace_id: &str, name: &str) -> anyhow::Result<()> {
        // 1. Delete the resource meta (immediately removes from list/query results)
        self.persistence
            .ai_resource_delete(namespace_id, name, SKILL_TYPE)
            .await?;

        // 2. Delete all version records
        self.persistence
            .ai_resource_version_delete_all(namespace_id, name, SKILL_TYPE)
            .await?;

        debug!("Deleted skill '{}' in namespace '{}'", name, namespace_id);
        Ok(())
    }

    /// List skills with pagination and optional search
    pub async fn list_skills(
        &self,
        namespace_id: &str,
        skill_name: Option<&str>,
        search: Option<&str>,
        order_by: Option<&str>,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<SkillSummary>> {
        let search_accurate = search == Some("accurate");
        let order_by_downloads = order_by == Some("download_count");

        let name_filter = skill_name.filter(|n| !n.is_empty());

        let page = self
            .persistence
            .ai_resource_list(
                namespace_id,
                SKILL_TYPE,
                name_filter,
                search_accurate,
                order_by_downloads,
                page_no,
                page_size,
            )
            .await?;

        let items: Vec<SkillSummary> = page
            .page_items
            .iter()
            .map(Self::resource_to_summary)
            .collect();

        Ok(Page::new(page.total_count, page_no, page_size, items))
    }

    /// Upload a skill from parsed content (creates resource + draft version)
    pub async fn upload_skill(
        &self,
        namespace_id: &str,
        name: &str,
        skill: &Skill,
        author: &str,
        overwrite: bool,
    ) -> anyhow::Result<String> {
        let existing = self.find_resource(namespace_id, name).await?;

        if let Some(ref res) = existing {
            let vi = Self::parse_version_info(res);
            if !overwrite && (vi.editing_version.is_some() || vi.reviewing_version.is_some()) {
                anyhow::bail!(
                    "Skill '{}' has an editing or reviewing version, set overwrite=true",
                    name
                );
            }
        }

        // Determine version
        let version = self
            .resolve_upload_version(namespace_id, name, skill)
            .await?;

        // Build storage
        let storage = self.skill_to_storage(skill);
        let storage_json = serde_json::to_string(&storage)?;

        let now = Utc::now().naive_utc().to_string();

        if let Some(res) = existing {
            let mut vi = Self::parse_version_info(&res);
            vi.editing_version = Some(version.clone());
            self.update_version_info_cas(namespace_id, name, res.meta_version, &vi)
                .await?;
        } else {
            // Create resource
            let vi = SkillVersionInfo {
                editing_version: Some(version.clone()),
                ..Default::default()
            };
            let vi_json = serde_json::to_string(&vi)?;

            let info = AiResourceInfo {
                id: 0,
                name: name.to_string(),
                resource_type: SKILL_TYPE.to_string(),
                description: skill.description.clone(),
                status: Some(RESOURCE_STATUS_ENABLE.to_string()),
                namespace_id: namespace_id.to_string(),
                biz_tags: None,
                ext: None,
                from: SKILL_DEFAULT_FROM.to_string(),
                version_info: Some(vi_json),
                meta_version: 1,
                scope: SCOPE_PRIVATE.to_string(),
                owner: author.to_string(),
                download_count: 0,
                gmt_create: Some(now.clone()),
                gmt_modified: Some(now.clone()),
            };
            self.persistence.ai_resource_insert(&info).await?;
        }

        // Delete existing draft if overwriting
        if overwrite {
            self.persistence
                .ai_resource_version_delete_by_status(
                    namespace_id,
                    name,
                    SKILL_TYPE,
                    VERSION_STATUS_DRAFT,
                )
                .await?;
        }

        // Create draft version
        let ver_info = AiResourceVersionInfo {
            id: 0,
            resource_type: SKILL_TYPE.to_string(),
            author: Some(author.to_string()),
            name: name.to_string(),
            description: skill.description.clone(),
            status: VERSION_STATUS_DRAFT.to_string(),
            version: version.clone(),
            namespace_id: namespace_id.to_string(),
            storage: Some(storage_json),
            publish_pipeline_info: None,
            download_count: 0,
            gmt_create: Some(now.clone()),
            gmt_modified: Some(now),
        };
        self.persistence
            .ai_resource_version_insert(&ver_info)
            .await?;

        debug!(
            "Uploaded skill '{}' version '{}' in namespace '{}'",
            name, version, namespace_id
        );
        Ok(name.to_string())
    }

    /// Create a draft version
    pub async fn create_draft(
        &self,
        namespace_id: &str,
        name: &str,
        based_on_version: Option<&str>,
        target_version: Option<&str>,
        initial_content: Option<&Skill>,
        author: &str,
    ) -> anyhow::Result<String> {
        let existing = self.find_resource(namespace_id, name).await?;

        // Determine version
        let version = if let Some(tv) = target_version {
            tv.to_string()
        } else if let Some(bv) = based_on_version {
            next_patch_version(bv)
        } else {
            SKILL_DEFAULT_VERSION.to_string()
        };

        // Check version doesn't exist
        if self
            .find_version(namespace_id, name, &version)
            .await?
            .is_some()
        {
            anyhow::bail!("Version '{}' already exists for skill '{}'", version, name);
        }

        let now = Utc::now().naive_utc().to_string();

        // Build storage from initial content or fork from base version
        let storage_json = if let Some(bv) = based_on_version {
            // Fork: copy storage from base version
            let base = self
                .find_version(namespace_id, name, bv)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Base version '{}' not found", bv))?;
            base.storage.unwrap_or_default()
        } else if let Some(skill) = initial_content {
            let storage = self.skill_to_storage(skill);
            serde_json::to_string(&storage)?
        } else {
            anyhow::bail!("Either basedOnVersion or skillCard is required");
        };

        if let Some(res) = existing {
            let mut vi = Self::parse_version_info(&res);
            // Nacos rejects if editing or reviewing version exists (prevents concurrent drafts)
            if vi.editing_version.is_some() || vi.reviewing_version.is_some() {
                anyhow::bail!(
                    "Skill '{}' already has a working version (editing: {:?}, reviewing: {:?})",
                    name,
                    vi.editing_version,
                    vi.reviewing_version
                );
            }
            vi.editing_version = Some(version.clone());
            self.update_version_info_cas(namespace_id, name, res.meta_version, &vi)
                .await?;
        } else {
            // Create resource
            let vi = SkillVersionInfo {
                editing_version: Some(version.clone()),
                ..Default::default()
            };
            let vi_json = serde_json::to_string(&vi)?;

            let info = AiResourceInfo {
                id: 0,
                name: name.to_string(),
                resource_type: SKILL_TYPE.to_string(),
                description: initial_content.and_then(|s| s.description.clone()),
                status: Some(RESOURCE_STATUS_ENABLE.to_string()),
                namespace_id: namespace_id.to_string(),
                biz_tags: None,
                ext: None,
                from: SKILL_DEFAULT_FROM.to_string(),
                version_info: Some(vi_json),
                meta_version: 1,
                scope: SCOPE_PRIVATE.to_string(),
                owner: author.to_string(),
                download_count: 0,
                gmt_create: Some(now.clone()),
                gmt_modified: Some(now.clone()),
            };
            self.persistence.ai_resource_insert(&info).await?;
        }

        // Create draft version
        let ver_info = AiResourceVersionInfo {
            id: 0,
            resource_type: SKILL_TYPE.to_string(),
            author: Some(author.to_string()),
            name: name.to_string(),
            description: initial_content.and_then(|s| s.description.clone()),
            status: VERSION_STATUS_DRAFT.to_string(),
            version: version.clone(),
            namespace_id: namespace_id.to_string(),
            storage: Some(storage_json),
            publish_pipeline_info: None,
            download_count: 0,
            gmt_create: Some(now.clone()),
            gmt_modified: Some(now),
        };
        self.persistence
            .ai_resource_version_insert(&ver_info)
            .await?;

        debug!(
            "Created draft '{}' version '{}' in namespace '{}'",
            name, version, namespace_id
        );
        Ok(version)
    }

    /// Update a draft version
    pub async fn update_draft(
        &self,
        namespace_id: &str,
        name: &str,
        skill: &Skill,
    ) -> anyhow::Result<()> {
        let resource = self
            .find_resource(namespace_id, name)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Skill '{}' not found", name))?;

        let vi = Self::parse_version_info(&resource);
        let draft_version = vi
            .editing_version
            .ok_or_else(|| anyhow::anyhow!("Skill '{}' has no editing version", name))?;

        let storage = self.skill_to_storage(skill);
        let storage_json = serde_json::to_string(&storage)?;

        self.persistence
            .ai_resource_version_update_storage(
                namespace_id,
                name,
                SKILL_TYPE,
                &draft_version,
                &storage_json,
                skill.description.as_deref(),
            )
            .await?;

        debug!(
            "Updated draft '{}' version '{}' in namespace '{}'",
            name, draft_version, namespace_id
        );
        Ok(())
    }

    /// Delete a draft version
    pub async fn delete_draft(&self, namespace_id: &str, name: &str) -> anyhow::Result<()> {
        let resource = self
            .find_resource(namespace_id, name)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Skill '{}' not found", name))?;

        let vi = Self::parse_version_info(&resource);
        let draft_version = vi
            .editing_version
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Skill '{}' has no editing version", name))?;

        // Delete the draft version record
        self.persistence
            .ai_resource_version_delete(namespace_id, name, SKILL_TYPE, &draft_version)
            .await?;

        // Clear editing_version
        let mut new_vi = vi;
        new_vi.editing_version = None;
        self.update_version_info_cas(namespace_id, name, resource.meta_version, &new_vi)
            .await?;

        debug!(
            "Deleted draft '{}' version '{}' in namespace '{}'",
            name, draft_version, namespace_id
        );
        Ok(())
    }

    /// Submit a version for review (draft → reviewing)
    pub async fn submit(
        &self,
        namespace_id: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<String> {
        let resource = self
            .find_resource(namespace_id, name)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Skill '{}' not found", name))?;

        // Verify version exists and is draft
        let ver = self
            .find_version(namespace_id, name, version)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Version '{}' not found", version))?;

        if ver.status != VERSION_STATUS_DRAFT {
            anyhow::bail!("Version '{}' is not in draft status", version);
        }

        // Update version status to reviewing
        self.persistence
            .ai_resource_version_update_status(
                namespace_id,
                name,
                SKILL_TYPE,
                version,
                VERSION_STATUS_REVIEWING,
            )
            .await?;

        // Update version_info
        let mut vi = Self::parse_version_info(&resource);
        vi.editing_version = None;
        vi.reviewing_version = Some(version.to_string());
        self.update_version_info_cas(namespace_id, name, resource.meta_version, &vi)
            .await?;

        debug!(
            "Submitted skill '{}' version '{}' for review in namespace '{}'",
            name, version, namespace_id
        );
        Ok(version.to_string())
    }

    /// Publish a version (reviewing → online)
    pub async fn publish(
        &self,
        namespace_id: &str,
        name: &str,
        version: &str,
        update_latest_label: bool,
    ) -> anyhow::Result<()> {
        let resource = self
            .find_resource(namespace_id, name)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Skill '{}' not found", name))?;

        let ver = self
            .find_version(namespace_id, name, version)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Version '{}' not found", version))?;

        // Nacos allows publishing reviewing or already-online versions (idempotent)
        if ver.status != VERSION_STATUS_REVIEWING && ver.status != VERSION_STATUS_ONLINE {
            anyhow::bail!(
                "Version '{}' must be in reviewing or online status to publish (current: '{}')",
                version,
                ver.status
            );
        }

        // Update version status to online
        self.persistence
            .ai_resource_version_update_status(
                namespace_id,
                name,
                SKILL_TYPE,
                version,
                VERSION_STATUS_ONLINE,
            )
            .await?;

        // Update version_info
        let mut vi = Self::parse_version_info(&resource);
        vi.reviewing_version = None;
        vi.online_cnt += 1;
        if update_latest_label {
            vi.labels.insert("latest".to_string(), version.to_string());
        }
        self.update_version_info_cas(namespace_id, name, resource.meta_version, &vi)
            .await?;

        debug!(
            "Published skill '{}' version '{}' in namespace '{}'",
            name, version, namespace_id
        );
        Ok(())
    }

    /// Update label→version mappings.
    /// Validates that labels only point to online versions (not draft/reviewing).
    pub async fn update_labels(
        &self,
        namespace_id: &str,
        name: &str,
        labels: HashMap<String, String>,
    ) -> anyhow::Result<()> {
        let resource = self
            .find_resource(namespace_id, name)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Skill '{}' not found", name))?;

        // Validate all label targets point to online versions
        for (label, version) in &labels {
            if let Some(ver) = self.find_version(namespace_id, name, version).await? {
                if ver.status != VERSION_STATUS_ONLINE {
                    anyhow::bail!(
                        "Label '{}' cannot point to version '{}' with status '{}' (must be online)",
                        label,
                        version,
                        ver.status
                    );
                }
            } else {
                anyhow::bail!(
                    "Label '{}' points to non-existent version '{}'",
                    label,
                    version
                );
            }
        }

        let mut vi = Self::parse_version_info(&resource);
        vi.labels = labels;
        self.update_version_info_cas(namespace_id, name, resource.meta_version, &vi)
            .await?;

        debug!(
            "Updated labels for skill '{}' in namespace '{}'",
            name, namespace_id
        );
        Ok(())
    }

    /// Update business tags
    pub async fn update_biz_tags(
        &self,
        namespace_id: &str,
        name: &str,
        biz_tags: &str,
    ) -> anyhow::Result<()> {
        self.persistence
            .ai_resource_update_biz_tags(namespace_id, name, SKILL_TYPE, biz_tags)
            .await?;

        debug!(
            "Updated biz_tags for skill '{}' in namespace '{}'",
            name, namespace_id
        );
        Ok(())
    }

    /// Change online status (online/offline) for a skill or a specific version
    pub async fn change_online_status(
        &self,
        namespace_id: &str,
        name: &str,
        scope: Option<&str>,
        version: Option<&str>,
        online: bool,
    ) -> anyhow::Result<()> {
        let resource = self
            .find_resource(namespace_id, name)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Skill '{}' not found", name))?;

        if scope == Some("skill") {
            // Skill-level: enable/disable the whole skill
            let new_status = if online {
                RESOURCE_STATUS_ENABLE
            } else {
                RESOURCE_STATUS_DISABLE
            };
            self.persistence
                .ai_resource_update_status(namespace_id, name, SKILL_TYPE, new_status)
                .await?;
        } else {
            // Version-level online/offline
            let version = version.ok_or_else(|| anyhow::anyhow!("version is required"))?;
            let new_status = if online {
                VERSION_STATUS_ONLINE
            } else {
                VERSION_STATUS_OFFLINE
            };

            self.persistence
                .ai_resource_version_update_status(
                    namespace_id,
                    name,
                    SKILL_TYPE,
                    version,
                    new_status,
                )
                .await?;

            // Recalculate online_cnt
            let online_count = self
                .persistence
                .ai_resource_version_count_by_status(
                    namespace_id,
                    name,
                    SKILL_TYPE,
                    VERSION_STATUS_ONLINE,
                )
                .await? as i64;

            let mut vi = Self::parse_version_info(&resource);
            vi.online_cnt = online_count;
            self.update_version_info_cas(namespace_id, name, resource.meta_version, &vi)
                .await?;
        }

        debug!(
            "Changed online status for skill '{}' in namespace '{}'",
            name, namespace_id
        );
        Ok(())
    }

    /// Update visibility scope (PUBLIC/PRIVATE)
    pub async fn update_scope(
        &self,
        namespace_id: &str,
        name: &str,
        scope: &str,
    ) -> anyhow::Result<()> {
        if scope != SCOPE_PUBLIC && scope != SCOPE_PRIVATE {
            anyhow::bail!("Invalid scope '{}', must be PUBLIC or PRIVATE", scope);
        }

        self.persistence
            .ai_resource_update_scope(namespace_id, name, SKILL_TYPE, scope)
            .await?;

        debug!(
            "Updated scope for skill '{}' to '{}' in namespace '{}'",
            name, scope, namespace_id
        );
        Ok(())
    }

    // ========================================================================
    // Client operations
    // ========================================================================

    /// Query skill by label/version/latest (client runtime)
    pub async fn query_skill(
        &self,
        namespace_id: &str,
        name: &str,
        version: Option<&str>,
        label: Option<&str>,
    ) -> anyhow::Result<Option<Skill>> {
        let resource = match self.find_resource(namespace_id, name).await? {
            Some(r) => r,
            None => return Ok(None),
        };

        // Check skill is enabled
        if resource.status.as_deref() != Some(RESOURCE_STATUS_ENABLE) {
            return Ok(None);
        }

        let vi = Self::parse_version_info(&resource);

        // Resolve version: label > explicit version > latest label
        let resolved_version = if let Some(lbl) = label {
            vi.labels.get(lbl).cloned()
        } else if let Some(v) = version {
            Some(v.to_string())
        } else {
            vi.labels.get("latest").cloned()
        };

        let resolved_version = match resolved_version {
            Some(v) => v,
            None => return Ok(None),
        };

        // Get version detail, only if online
        let ver = match self
            .find_version(namespace_id, name, &resolved_version)
            .await?
        {
            Some(v) if v.status == VERSION_STATUS_ONLINE => v,
            _ => return Ok(None),
        };

        let storage: SkillStorage = ver
            .storage
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();

        let mut resource_map = HashMap::new();
        let mut skill_md = None;
        for file in &storage.files {
            if file.name == "SKILL.md" {
                skill_md = file.content.clone();
            } else {
                let sr = SkillResource {
                    name: file.name.clone(),
                    resource_type: file.file_type.clone(),
                    content: file.content.clone(),
                    metadata: HashMap::new(),
                };
                resource_map.insert(sr.resource_identifier(), sr);
            }
        }

        Ok(Some(Skill {
            namespace_id: namespace_id.to_string(),
            name: name.to_string(),
            description: ver.description,
            skill_md,
            resource: resource_map,
        }))
    }

    /// Bootstrap a skill from ZIP bytes — bypasses draft/pipeline flow.
    /// Creates resource + online version directly. Used for server-side initialization.
    /// Skips if skill already exists.
    pub async fn bootstrap_skill_from_zip(
        &self,
        namespace_id: &str,
        zip_bytes: &[u8],
        from: &str,
    ) -> anyhow::Result<()> {
        let skill = crate::service::skill_zip::parse_skill_from_zip(zip_bytes, namespace_id)?;
        let name = &skill.name;

        // Skip if already exists
        if self.find_resource(namespace_id, name).await?.is_some() {
            debug!(
                "Skill '{}' already exists in namespace '{}', skipping bootstrap",
                name, namespace_id
            );
            return Ok(());
        }

        let version = crate::service::skill_zip::extract_version_from_skill_md(&skill)
            .unwrap_or_else(|| SKILL_DEFAULT_VERSION.to_string());

        let storage = self.skill_to_storage(&skill);
        let storage_json = serde_json::to_string(&storage)?;
        let now = Utc::now().naive_utc().to_string();

        // Create resource with online status and labels
        let mut labels = HashMap::new();
        labels.insert("latest".to_string(), version.clone());
        let vi = SkillVersionInfo {
            online_cnt: 1,
            labels,
            ..Default::default()
        };
        let vi_json = serde_json::to_string(&vi)?;

        let info = AiResourceInfo {
            id: 0,
            name: name.to_string(),
            resource_type: SKILL_TYPE.to_string(),
            description: skill.description.clone(),
            status: Some(RESOURCE_STATUS_ENABLE.to_string()),
            namespace_id: namespace_id.to_string(),
            biz_tags: None,
            ext: None,
            from: from.to_string(),
            version_info: Some(vi_json),
            meta_version: 1,
            scope: SCOPE_PUBLIC.to_string(),
            owner: "-".to_string(),
            download_count: 0,
            gmt_create: Some(now.clone()),
            gmt_modified: Some(now.clone()),
        };
        self.persistence.ai_resource_insert(&info).await?;

        // Create online version directly (skip draft/reviewing)
        let ver_info = AiResourceVersionInfo {
            id: 0,
            resource_type: SKILL_TYPE.to_string(),
            author: Some("-".to_string()),
            name: name.to_string(),
            description: skill.description.clone(),
            status: VERSION_STATUS_ONLINE.to_string(),
            version: version.clone(),
            namespace_id: namespace_id.to_string(),
            storage: Some(storage_json),
            publish_pipeline_info: None,
            download_count: 0,
            gmt_create: Some(now.clone()),
            gmt_modified: Some(now),
        };
        self.persistence
            .ai_resource_version_insert(&ver_info)
            .await?;

        debug!(
            "Bootstrapped skill '{}' version '{}' in namespace '{}'",
            name, version, namespace_id
        );
        Ok(())
    }

    /// Search skills for client discovery (only enabled skills with online versions)
    pub async fn search_skills(
        &self,
        namespace_id: &str,
        keyword: Option<&str>,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<SkillBasicInfo>> {
        // Fetch all enabled resources to filter by online_cnt > 0
        let all_resources = self
            .persistence
            .ai_resource_find_all(namespace_id, SKILL_TYPE)
            .await?;

        // Filter for enabled skills with keyword match and at least one online version
        let filtered: Vec<SkillBasicInfo> = all_resources
            .iter()
            .filter(|r| r.status.as_deref() == Some(RESOURCE_STATUS_ENABLE))
            .filter(|r| {
                if let Some(kw) = keyword
                    && !kw.is_empty()
                {
                    r.name.contains(kw)
                } else {
                    true
                }
            })
            .filter(|r| {
                let vi = Self::parse_version_info(r);
                vi.online_cnt > 0
            })
            .map(|r| SkillBasicInfo {
                namespace_id: r.namespace_id.clone(),
                name: r.name.clone(),
                description: r.description.clone(),
                update_time: r.gmt_modified.clone(),
            })
            .collect();

        let total = filtered.len() as u64;
        let start = ((page_no.saturating_sub(1)) * page_size) as usize;
        let page_items: Vec<SkillBasicInfo> = filtered
            .into_iter()
            .skip(start)
            .take(page_size as usize)
            .collect();

        Ok(Page::new(total, page_no, page_size, page_items))
    }

    // ========================================================================
    // Internal helpers
    // ========================================================================

    fn skill_to_storage(&self, skill: &Skill) -> SkillStorage {
        let mut files = Vec::new();

        if let Some(ref md) = skill.skill_md {
            files.push(SkillStorageFile {
                name: "SKILL.md".to_string(),
                file_type: "markdown".to_string(),
                content: Some(md.clone()),
            });
        }

        for res in skill.resource.values() {
            files.push(SkillStorageFile {
                name: res.name.clone(),
                file_type: res.resource_type.clone(),
                content: res.content.clone(),
            });
        }

        SkillStorage {
            files,
            storage_key: None,
        }
    }

    async fn resolve_upload_version(
        &self,
        namespace_id: &str,
        name: &str,
        _skill: &Skill,
    ) -> anyhow::Result<String> {
        // Try to find existing versions to determine next version
        let versions = self.list_versions_for_skill(namespace_id, name).await?;

        if versions.is_empty() {
            return Ok(SKILL_DEFAULT_VERSION.to_string());
        }

        // Find highest version and increment patch
        let mut max_version = SKILL_DEFAULT_VERSION.to_string();
        for v in &versions {
            if compare_versions(&v.version, &max_version) == std::cmp::Ordering::Greater {
                max_version = v.version.clone();
            }
        }

        let next = next_patch_version(&max_version);

        // Make sure the version doesn't exist
        if self
            .find_version(namespace_id, name, &next)
            .await?
            .is_some()
        {
            Ok(next_patch_version(&next))
        } else {
            Ok(next)
        }
    }
}
