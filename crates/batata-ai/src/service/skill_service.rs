//! Skill operation service — DB-backed storage via ai_resource / ai_resource_version tables
//!
//! Skills use ai_resource (type="skill") for governance metadata and
//! ai_resource_version for version-specific content with draft→reviewing→online lifecycle.

use std::collections::HashMap;
use std::sync::Arc;

use batata_persistence::entity::{ai_resource, ai_resource_version};
use batata_persistence::model::Page;
use batata_persistence::sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, Order, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, Set, prelude::Expr,
};
use chrono::Utc;
use tracing::debug;

use crate::model::skill::*;

/// Skill operation service backed by ai_resource/ai_resource_version tables
pub struct SkillOperationService {
    db: Arc<DatabaseConnection>,
}

impl SkillOperationService {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    // ========================================================================
    // Internal DB helpers
    // ========================================================================

    async fn find_resource(
        &self,
        namespace_id: &str,
        name: &str,
    ) -> anyhow::Result<Option<ai_resource::Model>> {
        let result = ai_resource::Entity::find()
            .filter(ai_resource::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource::Column::Name.eq(name))
            .filter(ai_resource::Column::Type.eq(SKILL_TYPE))
            .one(self.db.as_ref())
            .await?;
        Ok(result)
    }

    async fn find_version(
        &self,
        namespace_id: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<Option<ai_resource_version::Model>> {
        let result = ai_resource_version::Entity::find()
            .filter(ai_resource_version::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource_version::Column::Name.eq(name))
            .filter(ai_resource_version::Column::Type.eq(SKILL_TYPE))
            .filter(ai_resource_version::Column::Version.eq(version))
            .one(self.db.as_ref())
            .await?;
        Ok(result)
    }

    async fn list_versions_for_skill(
        &self,
        namespace_id: &str,
        name: &str,
    ) -> anyhow::Result<Vec<ai_resource_version::Model>> {
        let results = ai_resource_version::Entity::find()
            .filter(ai_resource_version::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource_version::Column::Name.eq(name))
            .filter(ai_resource_version::Column::Type.eq(SKILL_TYPE))
            .order_by(ai_resource_version::Column::GmtModified, Order::Desc)
            .all(self.db.as_ref())
            .await?;
        Ok(results)
    }

    fn parse_version_info(resource: &ai_resource::Model) -> SkillVersionInfo {
        resource
            .version_info
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default()
    }

    fn resource_to_summary(resource: &ai_resource::Model) -> SkillSummary {
        let vi = Self::parse_version_info(resource);
        SkillSummary {
            namespace_id: resource.namespace_id.clone(),
            name: resource.name.clone(),
            description: resource.c_desc.clone(),
            update_time: resource.gmt_modified.map(|dt| dt.to_string()),
            enable: resource.status.as_deref() == Some(RESOURCE_STATUS_ENABLE),
            biz_tags: resource.biz_tags.clone(),
            from: Some(resource.c_from.clone()),
            scope: Some(resource.scope.clone()),
            labels: vi.labels,
            editing_version: vi.editing_version,
            reviewing_version: vi.reviewing_version,
            online_cnt: vi.online_cnt,
            download_count: resource.download_count,
        }
    }

    fn version_to_summary(v: &ai_resource_version::Model) -> SkillVersionSummary {
        SkillVersionSummary {
            version: v.version.clone(),
            status: v.status.clone(),
            author: v.author.clone(),
            description: v.c_desc.clone(),
            create_time: v.gmt_create.map(|dt| dt.to_string()),
            update_time: v.gmt_modified.map(|dt| dt.to_string()),
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
        let now = Utc::now().naive_utc();

        let result = ai_resource::Entity::update_many()
            .col_expr(
                ai_resource::Column::VersionInfo,
                Expr::value(vi_json.clone()),
            )
            .col_expr(ai_resource::Column::GmtModified, Expr::value(now))
            .col_expr(
                ai_resource::Column::MetaVersion,
                Expr::value(expected_meta_version + 1),
            )
            .filter(ai_resource::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource::Column::Name.eq(name))
            .filter(ai_resource::Column::Type.eq(SKILL_TYPE))
            .filter(ai_resource::Column::MetaVersion.eq(expected_meta_version))
            .exec(self.db.as_ref())
            .await?;

        if result.rows_affected > 0 {
            return Ok(true);
        }

        // CAS failed — retry with fresh meta_version
        for _ in 1..Self::CAS_MAX_RETRIES {
            let resource = match self.find_resource(namespace_id, name).await? {
                Some(r) => r,
                None => return Ok(false),
            };
            let retry_now = Utc::now().naive_utc();
            let retry_result = ai_resource::Entity::update_many()
                .col_expr(
                    ai_resource::Column::VersionInfo,
                    Expr::value(vi_json.clone()),
                )
                .col_expr(ai_resource::Column::GmtModified, Expr::value(retry_now))
                .col_expr(
                    ai_resource::Column::MetaVersion,
                    Expr::value(resource.meta_version + 1),
                )
                .filter(ai_resource::Column::NamespaceId.eq(namespace_id))
                .filter(ai_resource::Column::Name.eq(name))
                .filter(ai_resource::Column::Type.eq(SKILL_TYPE))
                .filter(ai_resource::Column::MetaVersion.eq(resource.meta_version))
                .exec(self.db.as_ref())
                .await?;
            if retry_result.rows_affected > 0 {
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
            description: resource.c_desc.clone(),
            update_time: resource.gmt_modified.map(|dt| dt.to_string()),
            enable: resource.status.as_deref() == Some(RESOURCE_STATUS_ENABLE),
            biz_tags: resource.biz_tags.clone(),
            from: Some(resource.c_from.clone()),
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
            description: ver.c_desc.clone(),
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
            let _ = ai_resource_version::Entity::update_many()
                .col_expr(
                    ai_resource_version::Column::DownloadCount,
                    Expr::col(ai_resource_version::Column::DownloadCount).add(1),
                )
                .filter(ai_resource_version::Column::NamespaceId.eq(namespace_id))
                .filter(ai_resource_version::Column::Name.eq(name))
                .filter(ai_resource_version::Column::Type.eq(SKILL_TYPE))
                .filter(ai_resource_version::Column::Version.eq(version))
                .exec(self.db.as_ref())
                .await;

            // Increment download count on resource
            let _ = ai_resource::Entity::update_many()
                .col_expr(
                    ai_resource::Column::DownloadCount,
                    Expr::col(ai_resource::Column::DownloadCount).add(1),
                )
                .filter(ai_resource::Column::NamespaceId.eq(namespace_id))
                .filter(ai_resource::Column::Name.eq(name))
                .filter(ai_resource::Column::Type.eq(SKILL_TYPE))
                .exec(self.db.as_ref())
                .await;
        }

        Ok(skill)
    }

    /// Delete a skill and all its versions.
    /// Order: meta resource first (cuts off discovery), then versions (cleanup).
    pub async fn delete_skill(&self, namespace_id: &str, name: &str) -> anyhow::Result<()> {
        // 1. Delete the resource meta (immediately removes from list/query results)
        ai_resource::Entity::delete_many()
            .filter(ai_resource::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource::Column::Name.eq(name))
            .filter(ai_resource::Column::Type.eq(SKILL_TYPE))
            .exec(self.db.as_ref())
            .await?;

        // 2. Delete all version records
        ai_resource_version::Entity::delete_many()
            .filter(ai_resource_version::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource_version::Column::Name.eq(name))
            .filter(ai_resource_version::Column::Type.eq(SKILL_TYPE))
            .exec(self.db.as_ref())
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
        let mut query = ai_resource::Entity::find()
            .filter(ai_resource::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource::Column::Type.eq(SKILL_TYPE));

        if let Some(name) = skill_name
            && !name.is_empty()
        {
            let search_type = search.unwrap_or("blur");
            if search_type == "accurate" {
                query = query.filter(ai_resource::Column::Name.eq(name));
            } else {
                query = query.filter(ai_resource::Column::Name.contains(name));
            }
        }

        // Order by
        match order_by {
            Some("download_count") => {
                query = query.order_by(ai_resource::Column::DownloadCount, Order::Desc);
            }
            _ => {
                query = query.order_by(ai_resource::Column::GmtModified, Order::Desc);
            }
        }

        let total = query.clone().count(self.db.as_ref()).await?;
        let offset = (page_no.saturating_sub(1)) * page_size;

        let resources = query
            .offset(offset)
            .limit(page_size)
            .all(self.db.as_ref())
            .await?;

        let items: Vec<SkillSummary> = resources.iter().map(Self::resource_to_summary).collect();

        Ok(Page::new(total, page_no, page_size, items))
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

        let now = Utc::now().naive_utc();

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

            let resource = ai_resource::ActiveModel {
                gmt_create: Set(Some(now)),
                gmt_modified: Set(Some(now)),
                name: Set(name.to_string()),
                r#type: Set(SKILL_TYPE.to_string()),
                c_desc: Set(skill.description.clone()),
                status: Set(Some(RESOURCE_STATUS_ENABLE.to_string())),
                namespace_id: Set(namespace_id.to_string()),
                c_from: Set(SKILL_DEFAULT_FROM.to_string()),
                version_info: Set(Some(vi_json)),
                meta_version: Set(1),
                scope: Set(SCOPE_PRIVATE.to_string()),
                owner: Set(author.to_string()),
                download_count: Set(0),
                ..Default::default()
            };
            ai_resource::Entity::insert(resource)
                .exec(self.db.as_ref())
                .await?;
        }

        // Delete existing draft if overwriting
        if overwrite {
            ai_resource_version::Entity::delete_many()
                .filter(ai_resource_version::Column::NamespaceId.eq(namespace_id))
                .filter(ai_resource_version::Column::Name.eq(name))
                .filter(ai_resource_version::Column::Type.eq(SKILL_TYPE))
                .filter(ai_resource_version::Column::Status.eq(VERSION_STATUS_DRAFT))
                .exec(self.db.as_ref())
                .await?;
        }

        // Create draft version
        let ver = ai_resource_version::ActiveModel {
            gmt_create: Set(Some(now)),
            gmt_modified: Set(Some(now)),
            r#type: Set(SKILL_TYPE.to_string()),
            author: Set(Some(author.to_string())),
            name: Set(name.to_string()),
            c_desc: Set(skill.description.clone()),
            status: Set(VERSION_STATUS_DRAFT.to_string()),
            version: Set(version.clone()),
            namespace_id: Set(namespace_id.to_string()),
            storage: Set(Some(storage_json)),
            download_count: Set(0),
            ..Default::default()
        };
        ai_resource_version::Entity::insert(ver)
            .exec(self.db.as_ref())
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

        let now = Utc::now().naive_utc();

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

            let resource = ai_resource::ActiveModel {
                gmt_create: Set(Some(now)),
                gmt_modified: Set(Some(now)),
                name: Set(name.to_string()),
                r#type: Set(SKILL_TYPE.to_string()),
                c_desc: Set(initial_content.and_then(|s| s.description.clone())),
                status: Set(Some(RESOURCE_STATUS_ENABLE.to_string())),
                namespace_id: Set(namespace_id.to_string()),
                c_from: Set(SKILL_DEFAULT_FROM.to_string()),
                version_info: Set(Some(vi_json)),
                meta_version: Set(1),
                scope: Set(SCOPE_PRIVATE.to_string()),
                owner: Set(author.to_string()),
                download_count: Set(0),
                ..Default::default()
            };
            ai_resource::Entity::insert(resource)
                .exec(self.db.as_ref())
                .await?;
        }

        // Create draft version
        let ver = ai_resource_version::ActiveModel {
            gmt_create: Set(Some(now)),
            gmt_modified: Set(Some(now)),
            r#type: Set(SKILL_TYPE.to_string()),
            author: Set(Some(author.to_string())),
            name: Set(name.to_string()),
            c_desc: Set(initial_content.and_then(|s| s.description.clone())),
            status: Set(VERSION_STATUS_DRAFT.to_string()),
            version: Set(version.clone()),
            namespace_id: Set(namespace_id.to_string()),
            storage: Set(Some(storage_json)),
            download_count: Set(0),
            ..Default::default()
        };
        ai_resource_version::Entity::insert(ver)
            .exec(self.db.as_ref())
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
        let now = Utc::now().naive_utc();

        ai_resource_version::Entity::update_many()
            .col_expr(
                ai_resource_version::Column::Storage,
                Expr::value(storage_json.clone()),
            )
            .col_expr(
                ai_resource_version::Column::CDesc,
                Expr::value(skill.description.clone()),
            )
            .col_expr(ai_resource_version::Column::GmtModified, Expr::value(now))
            .filter(ai_resource_version::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource_version::Column::Name.eq(name))
            .filter(ai_resource_version::Column::Type.eq(SKILL_TYPE))
            .filter(ai_resource_version::Column::Version.eq(&draft_version))
            .filter(ai_resource_version::Column::Status.eq(VERSION_STATUS_DRAFT))
            .exec(self.db.as_ref())
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
        ai_resource_version::Entity::delete_many()
            .filter(ai_resource_version::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource_version::Column::Name.eq(name))
            .filter(ai_resource_version::Column::Type.eq(SKILL_TYPE))
            .filter(ai_resource_version::Column::Version.eq(&draft_version))
            .exec(self.db.as_ref())
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

        let now = Utc::now().naive_utc();

        // Update version status to reviewing
        ai_resource_version::Entity::update_many()
            .col_expr(
                ai_resource_version::Column::Status,
                Expr::value(VERSION_STATUS_REVIEWING),
            )
            .col_expr(ai_resource_version::Column::GmtModified, Expr::value(now))
            .filter(ai_resource_version::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource_version::Column::Name.eq(name))
            .filter(ai_resource_version::Column::Type.eq(SKILL_TYPE))
            .filter(ai_resource_version::Column::Version.eq(version))
            .exec(self.db.as_ref())
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

        let now = Utc::now().naive_utc();

        // Update version status to online
        ai_resource_version::Entity::update_many()
            .col_expr(
                ai_resource_version::Column::Status,
                Expr::value(VERSION_STATUS_ONLINE),
            )
            .col_expr(ai_resource_version::Column::GmtModified, Expr::value(now))
            .filter(ai_resource_version::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource_version::Column::Name.eq(name))
            .filter(ai_resource_version::Column::Type.eq(SKILL_TYPE))
            .filter(ai_resource_version::Column::Version.eq(version))
            .exec(self.db.as_ref())
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
        let now = Utc::now().naive_utc();

        ai_resource::Entity::update_many()
            .col_expr(ai_resource::Column::BizTags, Expr::value(biz_tags))
            .col_expr(ai_resource::Column::GmtModified, Expr::value(now))
            .filter(ai_resource::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource::Column::Name.eq(name))
            .filter(ai_resource::Column::Type.eq(SKILL_TYPE))
            .exec(self.db.as_ref())
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

        let now = Utc::now().naive_utc();

        if scope == Some("skill") {
            // Skill-level: enable/disable the whole skill
            let new_status = if online {
                RESOURCE_STATUS_ENABLE
            } else {
                RESOURCE_STATUS_DISABLE
            };
            ai_resource::Entity::update_many()
                .col_expr(ai_resource::Column::Status, Expr::value(new_status))
                .col_expr(ai_resource::Column::GmtModified, Expr::value(now))
                .filter(ai_resource::Column::NamespaceId.eq(namespace_id))
                .filter(ai_resource::Column::Name.eq(name))
                .filter(ai_resource::Column::Type.eq(SKILL_TYPE))
                .exec(self.db.as_ref())
                .await?;
        } else {
            // Version-level online/offline
            let version = version.ok_or_else(|| anyhow::anyhow!("version is required"))?;
            let new_status = if online {
                VERSION_STATUS_ONLINE
            } else {
                VERSION_STATUS_OFFLINE
            };

            ai_resource_version::Entity::update_many()
                .col_expr(ai_resource_version::Column::Status, Expr::value(new_status))
                .col_expr(ai_resource_version::Column::GmtModified, Expr::value(now))
                .filter(ai_resource_version::Column::NamespaceId.eq(namespace_id))
                .filter(ai_resource_version::Column::Name.eq(name))
                .filter(ai_resource_version::Column::Type.eq(SKILL_TYPE))
                .filter(ai_resource_version::Column::Version.eq(version))
                .exec(self.db.as_ref())
                .await?;

            // Recalculate online_cnt
            let online_count = ai_resource_version::Entity::find()
                .filter(ai_resource_version::Column::NamespaceId.eq(namespace_id))
                .filter(ai_resource_version::Column::Name.eq(name))
                .filter(ai_resource_version::Column::Type.eq(SKILL_TYPE))
                .filter(ai_resource_version::Column::Status.eq(VERSION_STATUS_ONLINE))
                .count(self.db.as_ref())
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
        let now = Utc::now().naive_utc();

        ai_resource::Entity::update_many()
            .col_expr(ai_resource::Column::Scope, Expr::value(scope))
            .col_expr(ai_resource::Column::GmtModified, Expr::value(now))
            .filter(ai_resource::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource::Column::Name.eq(name))
            .filter(ai_resource::Column::Type.eq(SKILL_TYPE))
            .exec(self.db.as_ref())
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
            description: ver.c_desc,
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
        let now = Utc::now().naive_utc();

        // Create resource with online status and labels
        let mut labels = HashMap::new();
        labels.insert("latest".to_string(), version.clone());
        let vi = SkillVersionInfo {
            online_cnt: 1,
            labels,
            ..Default::default()
        };
        let vi_json = serde_json::to_string(&vi)?;

        let resource = ai_resource::ActiveModel {
            gmt_create: Set(Some(now)),
            gmt_modified: Set(Some(now)),
            name: Set(name.to_string()),
            r#type: Set(SKILL_TYPE.to_string()),
            c_desc: Set(skill.description.clone()),
            status: Set(Some(RESOURCE_STATUS_ENABLE.to_string())),
            namespace_id: Set(namespace_id.to_string()),
            c_from: Set(from.to_string()),
            version_info: Set(Some(vi_json)),
            meta_version: Set(1),
            scope: Set(SCOPE_PUBLIC.to_string()),
            owner: Set("-".to_string()),
            download_count: Set(0),
            ..Default::default()
        };
        ai_resource::Entity::insert(resource)
            .exec(self.db.as_ref())
            .await?;

        // Create online version directly (skip draft/reviewing)
        let ver = ai_resource_version::ActiveModel {
            gmt_create: Set(Some(now)),
            gmt_modified: Set(Some(now)),
            r#type: Set(SKILL_TYPE.to_string()),
            author: Set(Some("-".to_string())),
            name: Set(name.to_string()),
            c_desc: Set(skill.description.clone()),
            status: Set(VERSION_STATUS_ONLINE.to_string()),
            version: Set(version.clone()),
            namespace_id: Set(namespace_id.to_string()),
            storage: Set(Some(storage_json)),
            download_count: Set(0),
            ..Default::default()
        };
        ai_resource_version::Entity::insert(ver)
            .exec(self.db.as_ref())
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
        let mut query = ai_resource::Entity::find()
            .filter(ai_resource::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource::Column::Type.eq(SKILL_TYPE))
            .filter(ai_resource::Column::Status.eq(RESOURCE_STATUS_ENABLE));

        if let Some(kw) = keyword
            && !kw.is_empty()
        {
            query = query.filter(ai_resource::Column::Name.contains(kw));
        }

        query = query.order_by(ai_resource::Column::GmtModified, Order::Desc);

        let all_resources = query.all(self.db.as_ref()).await?;

        // Filter for skills with at least one online version (onlineCnt > 0)
        let filtered: Vec<SkillBasicInfo> = all_resources
            .iter()
            .filter(|r| {
                let vi = Self::parse_version_info(r);
                vi.online_cnt > 0
            })
            .map(|r| SkillBasicInfo {
                namespace_id: r.namespace_id.clone(),
                name: r.name.clone(),
                description: r.c_desc.clone(),
                update_time: r.gmt_modified.map(|dt| dt.to_string()),
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
