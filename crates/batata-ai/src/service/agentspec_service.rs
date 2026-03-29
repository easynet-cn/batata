//! AgentSpec operation service — persistence-backed storage via ai_resource / ai_resource_version
//!
//! AgentSpecs use ai_resource (type="agentspec") for governance metadata.
//! Structurally identical to SkillOperationService but with type="agentspec"
//! and manifest.json as the main file instead of SKILL.md.
//!
//! Uses the `PersistenceService` trait abstraction instead of direct DB access.

use std::collections::HashMap;
use std::sync::Arc;

use batata_persistence::PersistenceService;
use batata_persistence::model::{AiResourceInfo, AiResourceVersionInfo, Page};
use tracing::debug;

use crate::model::agentspec::*;

/// AgentSpec operation service backed by ai_resource/ai_resource_version tables
pub struct AgentSpecOperationService {
    persistence: Arc<dyn PersistenceService>,
}

impl AgentSpecOperationService {
    pub fn new(persistence: Arc<dyn PersistenceService>) -> Self {
        Self { persistence }
    }

    const CAS_MAX_RETRIES: u32 = 3;

    // ========================================================================
    // Internal helpers
    // ========================================================================

    async fn find_resource(
        &self,
        namespace_id: &str,
        name: &str,
    ) -> anyhow::Result<Option<AiResourceInfo>> {
        self.persistence
            .ai_resource_find(namespace_id, name, AGENTSPEC_TYPE)
            .await
    }

    async fn find_version(
        &self,
        namespace_id: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<Option<AiResourceVersionInfo>> {
        self.persistence
            .ai_resource_version_find(namespace_id, name, AGENTSPEC_TYPE, version)
            .await
    }

    async fn list_versions_for_resource(
        &self,
        namespace_id: &str,
        name: &str,
    ) -> anyhow::Result<Vec<AiResourceVersionInfo>> {
        self.persistence
            .ai_resource_version_list(namespace_id, name, AGENTSPEC_TYPE)
            .await
    }

    fn parse_version_info(resource: &AiResourceInfo) -> AgentSpecVersionInfo {
        resource
            .version_info
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default()
    }

    fn resource_to_summary(resource: &AiResourceInfo) -> AgentSpecSummary {
        let vi = Self::parse_version_info(resource);
        AgentSpecSummary {
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

    fn version_to_summary(v: &AiResourceVersionInfo) -> AgentSpecVersionSummary {
        AgentSpecVersionSummary {
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

    async fn update_version_info_cas(
        &self,
        namespace_id: &str,
        name: &str,
        expected_meta_version: i64,
        version_info: &AgentSpecVersionInfo,
    ) -> anyhow::Result<bool> {
        let vi_json = serde_json::to_string(version_info)?;

        let ok = self
            .persistence
            .ai_resource_update_version_info_cas(
                namespace_id,
                name,
                AGENTSPEC_TYPE,
                expected_meta_version,
                &vi_json,
                expected_meta_version + 1,
            )
            .await?;

        if ok {
            return Ok(true);
        }

        for _ in 1..Self::CAS_MAX_RETRIES {
            let resource = match self.find_resource(namespace_id, name).await? {
                Some(r) => r,
                None => return Ok(false),
            };
            let retry_ok = self
                .persistence
                .ai_resource_update_version_info_cas(
                    namespace_id,
                    name,
                    AGENTSPEC_TYPE,
                    resource.meta_version,
                    &vi_json,
                    resource.meta_version + 1,
                )
                .await?;
            if retry_ok {
                return Ok(true);
            }
        }

        anyhow::bail!("CAS conflict for agentspec '{}' after retries", name)
    }

    // ========================================================================
    // Admin operations
    // ========================================================================

    pub async fn get_detail(
        &self,
        namespace_id: &str,
        name: &str,
    ) -> anyhow::Result<Option<AgentSpecMeta>> {
        let resource = match self.find_resource(namespace_id, name).await? {
            Some(r) => r,
            None => return Ok(None),
        };
        let versions = self.list_versions_for_resource(namespace_id, name).await?;
        let vi = Self::parse_version_info(&resource);
        Ok(Some(AgentSpecMeta {
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

    pub async fn get_version_detail(
        &self,
        namespace_id: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<Option<AgentSpec>> {
        let ver = match self.find_version(namespace_id, name, version).await? {
            Some(v) => v,
            None => return Ok(None),
        };
        let storage: AgentSpecStorage = ver
            .storage
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();

        let mut resources = HashMap::new();
        let mut content = None;
        for file in &storage.files {
            if file.name == AGENTSPEC_MAIN_FILE {
                content = file.content.clone();
            } else {
                let r = AgentSpecResource {
                    name: file.name.clone(),
                    resource_type: file.file_type.clone(),
                    content: file.content.clone(),
                    metadata: HashMap::new(),
                };
                resources.insert(r.resource_identifier(), r);
            }
        }

        Ok(Some(AgentSpec {
            namespace_id: namespace_id.to_string(),
            name: name.to_string(),
            description: ver.description.clone(),
            content,
            biz_tags: None,
            resource: resources,
        }))
    }

    pub async fn delete(&self, namespace_id: &str, name: &str) -> anyhow::Result<()> {
        self.persistence
            .ai_resource_delete(namespace_id, name, AGENTSPEC_TYPE)
            .await?;
        self.persistence
            .ai_resource_version_delete_all(namespace_id, name, AGENTSPEC_TYPE)
            .await?;
        debug!(
            "Deleted agentspec '{}' in namespace '{}'",
            name, namespace_id
        );
        Ok(())
    }

    pub async fn list(
        &self,
        namespace_id: &str,
        name_filter: Option<&str>,
        search: Option<&str>,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<AgentSpecSummary>> {
        let search_accurate = search.unwrap_or("blur") == "accurate";
        let page = self
            .persistence
            .ai_resource_list(
                namespace_id,
                AGENTSPEC_TYPE,
                name_filter,
                search_accurate,
                false,
                page_no,
                page_size,
            )
            .await?;
        let items: Vec<AgentSpecSummary> = page
            .page_items
            .iter()
            .map(Self::resource_to_summary)
            .collect();
        Ok(Page::new(page.total_count, page_no, page_size, items))
    }

    pub async fn upload(
        &self,
        namespace_id: &str,
        name: &str,
        spec: &AgentSpec,
        author: &str,
        overwrite: bool,
    ) -> anyhow::Result<String> {
        let existing = self.find_resource(namespace_id, name).await?;
        if let Some(ref res) = existing {
            let vi = Self::parse_version_info(res);
            if !overwrite && (vi.editing_version.is_some() || vi.reviewing_version.is_some()) {
                anyhow::bail!(
                    "AgentSpec '{}' has a working version, set overwrite=true",
                    name
                );
            }
        }

        let version = self.resolve_upload_version(namespace_id, name).await?;
        let storage = self.spec_to_storage(spec);
        let storage_json = serde_json::to_string(&storage)?;

        if let Some(res) = existing {
            let mut vi = Self::parse_version_info(&res);
            vi.editing_version = Some(version.clone());
            self.update_version_info_cas(namespace_id, name, res.meta_version, &vi)
                .await?;
        } else {
            let vi = AgentSpecVersionInfo {
                editing_version: Some(version.clone()),
                ..Default::default()
            };
            let vi_json = serde_json::to_string(&vi)?;
            let resource = AiResourceInfo {
                name: name.to_string(),
                resource_type: AGENTSPEC_TYPE.to_string(),
                description: spec.description.clone(),
                status: Some(RESOURCE_STATUS_ENABLE.to_string()),
                namespace_id: namespace_id.to_string(),
                from: AGENTSPEC_DEFAULT_FROM.to_string(),
                version_info: Some(vi_json),
                meta_version: 1,
                scope: SCOPE_PRIVATE.to_string(),
                owner: author.to_string(),
                download_count: 0,
                ..Default::default()
            };
            self.persistence.ai_resource_insert(&resource).await?;
        }

        if overwrite {
            self.persistence
                .ai_resource_version_delete_by_status(
                    namespace_id,
                    name,
                    AGENTSPEC_TYPE,
                    VERSION_STATUS_DRAFT,
                )
                .await?;
        }

        let ver = AiResourceVersionInfo {
            resource_type: AGENTSPEC_TYPE.to_string(),
            author: Some(author.to_string()),
            name: name.to_string(),
            description: spec.description.clone(),
            status: VERSION_STATUS_DRAFT.to_string(),
            version: version.clone(),
            namespace_id: namespace_id.to_string(),
            storage: Some(storage_json),
            download_count: 0,
            ..Default::default()
        };
        self.persistence.ai_resource_version_insert(&ver).await?;

        debug!("Uploaded agentspec '{}' version '{}'", name, version);
        Ok(name.to_string())
    }

    pub async fn create_draft(
        &self,
        namespace_id: &str,
        name: &str,
        based_on_version: Option<&str>,
        target_version: Option<&str>,
        initial_content: Option<&AgentSpec>,
        author: &str,
    ) -> anyhow::Result<String> {
        let existing = self.find_resource(namespace_id, name).await?;
        let version = if let Some(tv) = target_version {
            tv.to_string()
        } else if let Some(bv) = based_on_version {
            next_patch_version(bv)
        } else {
            AGENTSPEC_DEFAULT_VERSION.to_string()
        };

        if self
            .find_version(namespace_id, name, &version)
            .await?
            .is_some()
        {
            anyhow::bail!(
                "Version '{}' already exists for agentspec '{}'",
                version,
                name
            );
        }

        let storage_json = if let Some(bv) = based_on_version {
            let base = self
                .find_version(namespace_id, name, bv)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Base version '{}' not found", bv))?;
            base.storage.unwrap_or_default()
        } else if let Some(spec) = initial_content {
            let storage = self.spec_to_storage(spec);
            serde_json::to_string(&storage)?
        } else {
            anyhow::bail!("Either basedOnVersion or agentSpecCard is required");
        };

        if let Some(res) = existing {
            let mut vi = Self::parse_version_info(&res);
            if vi.editing_version.is_some() || vi.reviewing_version.is_some() {
                anyhow::bail!("AgentSpec '{}' already has a working version", name);
            }
            vi.editing_version = Some(version.clone());
            self.update_version_info_cas(namespace_id, name, res.meta_version, &vi)
                .await?;
        } else {
            let vi = AgentSpecVersionInfo {
                editing_version: Some(version.clone()),
                ..Default::default()
            };
            let vi_json = serde_json::to_string(&vi)?;
            let resource = AiResourceInfo {
                name: name.to_string(),
                resource_type: AGENTSPEC_TYPE.to_string(),
                description: initial_content.and_then(|s| s.description.clone()),
                status: Some(RESOURCE_STATUS_ENABLE.to_string()),
                namespace_id: namespace_id.to_string(),
                from: AGENTSPEC_DEFAULT_FROM.to_string(),
                version_info: Some(vi_json),
                meta_version: 1,
                scope: SCOPE_PRIVATE.to_string(),
                owner: author.to_string(),
                download_count: 0,
                ..Default::default()
            };
            self.persistence.ai_resource_insert(&resource).await?;
        }

        let ver = AiResourceVersionInfo {
            resource_type: AGENTSPEC_TYPE.to_string(),
            author: Some(author.to_string()),
            name: name.to_string(),
            description: initial_content.and_then(|s| s.description.clone()),
            status: VERSION_STATUS_DRAFT.to_string(),
            version: version.clone(),
            namespace_id: namespace_id.to_string(),
            storage: Some(storage_json),
            download_count: 0,
            ..Default::default()
        };
        self.persistence.ai_resource_version_insert(&ver).await?;
        debug!("Created agentspec draft '{}' version '{}'", name, version);
        Ok(version)
    }

    pub async fn update_draft(
        &self,
        namespace_id: &str,
        name: &str,
        spec: &AgentSpec,
    ) -> anyhow::Result<()> {
        let resource = self
            .find_resource(namespace_id, name)
            .await?
            .ok_or_else(|| anyhow::anyhow!("AgentSpec '{}' not found", name))?;
        let vi = Self::parse_version_info(&resource);
        let draft_version = vi
            .editing_version
            .ok_or_else(|| anyhow::anyhow!("AgentSpec '{}' has no editing version", name))?;

        let storage = self.spec_to_storage(spec);
        let storage_json = serde_json::to_string(&storage)?;

        self.persistence
            .ai_resource_version_update_storage(
                namespace_id,
                name,
                AGENTSPEC_TYPE,
                &draft_version,
                &storage_json,
                spec.description.as_deref(),
            )
            .await?;
        Ok(())
    }

    pub async fn delete_draft(&self, namespace_id: &str, name: &str) -> anyhow::Result<()> {
        let resource = self
            .find_resource(namespace_id, name)
            .await?
            .ok_or_else(|| anyhow::anyhow!("AgentSpec '{}' not found", name))?;
        let vi = Self::parse_version_info(&resource);
        let draft_version = vi
            .editing_version
            .clone()
            .ok_or_else(|| anyhow::anyhow!("AgentSpec '{}' has no editing version", name))?;

        self.persistence
            .ai_resource_version_delete(namespace_id, name, AGENTSPEC_TYPE, &draft_version)
            .await?;

        let mut new_vi = vi;
        new_vi.editing_version = None;
        self.update_version_info_cas(namespace_id, name, resource.meta_version, &new_vi)
            .await?;
        Ok(())
    }

    pub async fn submit(
        &self,
        namespace_id: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<String> {
        let resource = self
            .find_resource(namespace_id, name)
            .await?
            .ok_or_else(|| anyhow::anyhow!("AgentSpec '{}' not found", name))?;
        let ver = self
            .find_version(namespace_id, name, version)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Version '{}' not found", version))?;
        if ver.status != VERSION_STATUS_DRAFT {
            anyhow::bail!("Version '{}' is not in draft status", version);
        }

        self.persistence
            .ai_resource_version_update_status(
                namespace_id,
                name,
                AGENTSPEC_TYPE,
                version,
                VERSION_STATUS_REVIEWING,
            )
            .await?;

        let mut vi = Self::parse_version_info(&resource);
        vi.editing_version = None;
        vi.reviewing_version = Some(version.to_string());
        self.update_version_info_cas(namespace_id, name, resource.meta_version, &vi)
            .await?;
        Ok(version.to_string())
    }

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
            .ok_or_else(|| anyhow::anyhow!("AgentSpec '{}' not found", name))?;
        let ver = self
            .find_version(namespace_id, name, version)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Version '{}' not found", version))?;
        if ver.status != VERSION_STATUS_REVIEWING && ver.status != VERSION_STATUS_ONLINE {
            anyhow::bail!(
                "Version '{}' must be reviewing or online to publish",
                version
            );
        }

        self.persistence
            .ai_resource_version_update_status(
                namespace_id,
                name,
                AGENTSPEC_TYPE,
                version,
                VERSION_STATUS_ONLINE,
            )
            .await?;

        let mut vi = Self::parse_version_info(&resource);
        vi.reviewing_version = None;
        vi.online_cnt += 1;
        if update_latest_label {
            vi.labels.insert("latest".to_string(), version.to_string());
        }
        self.update_version_info_cas(namespace_id, name, resource.meta_version, &vi)
            .await?;
        Ok(())
    }

    pub async fn update_labels(
        &self,
        namespace_id: &str,
        name: &str,
        labels: HashMap<String, String>,
    ) -> anyhow::Result<()> {
        let resource = self
            .find_resource(namespace_id, name)
            .await?
            .ok_or_else(|| anyhow::anyhow!("AgentSpec '{}' not found", name))?;
        // Validate labels point to online versions
        for (label, version) in &labels {
            if let Some(ver) = self.find_version(namespace_id, name, version).await? {
                if ver.status != VERSION_STATUS_ONLINE {
                    anyhow::bail!(
                        "Label '{}' cannot point to non-online version '{}'",
                        label,
                        version
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
        Ok(())
    }

    pub async fn update_biz_tags(
        &self,
        namespace_id: &str,
        name: &str,
        biz_tags: &str,
    ) -> anyhow::Result<()> {
        self.persistence
            .ai_resource_update_biz_tags(namespace_id, name, AGENTSPEC_TYPE, biz_tags)
            .await
    }

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
            .ok_or_else(|| anyhow::anyhow!("AgentSpec '{}' not found", name))?;

        if scope == Some("agentspec") {
            let status = if online {
                RESOURCE_STATUS_ENABLE
            } else {
                RESOURCE_STATUS_DISABLE
            };
            self.persistence
                .ai_resource_update_status(namespace_id, name, AGENTSPEC_TYPE, status)
                .await?;
        } else {
            let version = version.ok_or_else(|| anyhow::anyhow!("version is required"))?;
            let status = if online {
                VERSION_STATUS_ONLINE
            } else {
                VERSION_STATUS_OFFLINE
            };
            self.persistence
                .ai_resource_version_update_status(
                    namespace_id,
                    name,
                    AGENTSPEC_TYPE,
                    version,
                    status,
                )
                .await?;

            let online_count = self
                .persistence
                .ai_resource_version_count_by_status(
                    namespace_id,
                    name,
                    AGENTSPEC_TYPE,
                    VERSION_STATUS_ONLINE,
                )
                .await? as i64;

            let mut vi = Self::parse_version_info(&resource);
            vi.online_cnt = online_count;
            self.update_version_info_cas(namespace_id, name, resource.meta_version, &vi)
                .await?;
        }
        Ok(())
    }

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
            .ai_resource_update_scope(namespace_id, name, AGENTSPEC_TYPE, scope)
            .await
    }

    // ========================================================================
    // Client operations
    // ========================================================================

    pub async fn query(
        &self,
        namespace_id: &str,
        name: &str,
        version: Option<&str>,
        label: Option<&str>,
    ) -> anyhow::Result<Option<AgentSpec>> {
        let resource = match self.find_resource(namespace_id, name).await? {
            Some(r) => r,
            None => return Ok(None),
        };
        if resource.status.as_deref() != Some(RESOURCE_STATUS_ENABLE) {
            return Ok(None);
        }
        let vi = Self::parse_version_info(&resource);
        let resolved = if let Some(lbl) = label {
            vi.labels.get(lbl).cloned()
        } else if let Some(v) = version {
            Some(v.to_string())
        } else {
            vi.labels.get("latest").cloned()
        };
        let resolved = match resolved {
            Some(v) => v,
            None => return Ok(None),
        };
        let ver = match self.find_version(namespace_id, name, &resolved).await? {
            Some(v) if v.status == VERSION_STATUS_ONLINE => v,
            _ => return Ok(None),
        };

        let storage: AgentSpecStorage = ver
            .storage
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();

        let mut resources = HashMap::new();
        let mut content = None;
        for file in &storage.files {
            if file.name == AGENTSPEC_MAIN_FILE {
                content = file.content.clone();
            } else {
                let r = AgentSpecResource {
                    name: file.name.clone(),
                    resource_type: file.file_type.clone(),
                    content: file.content.clone(),
                    metadata: HashMap::new(),
                };
                resources.insert(r.resource_identifier(), r);
            }
        }

        Ok(Some(AgentSpec {
            namespace_id: namespace_id.to_string(),
            name: name.to_string(),
            description: ver.description,
            content,
            biz_tags: None,
            resource: resources,
        }))
    }

    pub async fn search(
        &self,
        namespace_id: &str,
        keyword: Option<&str>,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<AgentSpecBasicInfo>> {
        // Fetch all enabled resources, then filter to those with online versions
        let all = self
            .persistence
            .ai_resource_find_all(namespace_id, AGENTSPEC_TYPE)
            .await?;
        let filtered: Vec<AgentSpecBasicInfo> = all
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
            .filter(|r| Self::parse_version_info(r).online_cnt > 0)
            .map(|r| AgentSpecBasicInfo {
                namespace_id: r.namespace_id.clone(),
                name: r.name.clone(),
                description: r.description.clone(),
                update_time: r.gmt_modified.clone(),
            })
            .collect();
        let total = filtered.len() as u64;
        let start = ((page_no.saturating_sub(1)) * page_size) as usize;
        let page_items: Vec<AgentSpecBasicInfo> = filtered
            .into_iter()
            .skip(start)
            .take(page_size as usize)
            .collect();
        Ok(Page::new(total, page_no, page_size, page_items))
    }

    // ========================================================================
    // Helpers
    // ========================================================================

    fn spec_to_storage(&self, spec: &AgentSpec) -> AgentSpecStorage {
        let mut files = Vec::new();
        if let Some(ref c) = spec.content {
            files.push(AgentSpecStorageFile {
                name: AGENTSPEC_MAIN_FILE.to_string(),
                file_type: "json".to_string(),
                content: Some(c.clone()),
            });
        }
        for res in spec.resource.values() {
            files.push(AgentSpecStorageFile {
                name: res.name.clone(),
                file_type: res.resource_type.clone(),
                content: res.content.clone(),
            });
        }
        AgentSpecStorage {
            files,
            storage_key: None,
        }
    }

    async fn resolve_upload_version(
        &self,
        namespace_id: &str,
        name: &str,
    ) -> anyhow::Result<String> {
        let versions = self.list_versions_for_resource(namespace_id, name).await?;
        if versions.is_empty() {
            return Ok(AGENTSPEC_DEFAULT_VERSION.to_string());
        }
        let mut max_ver = AGENTSPEC_DEFAULT_VERSION.to_string();
        for v in &versions {
            if compare_versions(&v.version, &max_ver) == std::cmp::Ordering::Greater {
                max_ver = v.version.clone();
            }
        }
        let next = next_patch_version(&max_ver);
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
