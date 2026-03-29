//! AgentSpec operation service — DB-backed storage via ai_resource / ai_resource_version tables
//!
//! AgentSpecs use ai_resource (type="agentspec") for governance metadata.
//! Structurally identical to SkillOperationService but with type="agentspec"
//! and manifest.json as the main file instead of SKILL.md.
//!
//! This delegates to SkillOperationService internally by using the same DB layer
//! with a different resource type filter.

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

use crate::model::agentspec::*;

/// AgentSpec operation service backed by ai_resource/ai_resource_version tables
pub struct AgentSpecOperationService {
    db: Arc<DatabaseConnection>,
}

impl AgentSpecOperationService {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    const CAS_MAX_RETRIES: u32 = 3;

    // ========================================================================
    // Internal DB helpers (same as Skills but with AGENTSPEC_TYPE)
    // ========================================================================

    async fn find_resource(
        &self,
        namespace_id: &str,
        name: &str,
    ) -> anyhow::Result<Option<ai_resource::Model>> {
        Ok(ai_resource::Entity::find()
            .filter(ai_resource::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource::Column::Name.eq(name))
            .filter(ai_resource::Column::Type.eq(AGENTSPEC_TYPE))
            .one(self.db.as_ref())
            .await?)
    }

    async fn find_version(
        &self,
        namespace_id: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<Option<ai_resource_version::Model>> {
        Ok(ai_resource_version::Entity::find()
            .filter(ai_resource_version::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource_version::Column::Name.eq(name))
            .filter(ai_resource_version::Column::Type.eq(AGENTSPEC_TYPE))
            .filter(ai_resource_version::Column::Version.eq(version))
            .one(self.db.as_ref())
            .await?)
    }

    async fn list_versions_for_resource(
        &self,
        namespace_id: &str,
        name: &str,
    ) -> anyhow::Result<Vec<ai_resource_version::Model>> {
        Ok(ai_resource_version::Entity::find()
            .filter(ai_resource_version::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource_version::Column::Name.eq(name))
            .filter(ai_resource_version::Column::Type.eq(AGENTSPEC_TYPE))
            .order_by(ai_resource_version::Column::GmtModified, Order::Desc)
            .all(self.db.as_ref())
            .await?)
    }

    fn parse_version_info(resource: &ai_resource::Model) -> AgentSpecVersionInfo {
        resource
            .version_info
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default()
    }

    fn resource_to_summary(resource: &ai_resource::Model) -> AgentSpecSummary {
        let vi = Self::parse_version_info(resource);
        AgentSpecSummary {
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

    fn version_to_summary(v: &ai_resource_version::Model) -> AgentSpecVersionSummary {
        AgentSpecVersionSummary {
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

    async fn update_version_info_cas(
        &self,
        namespace_id: &str,
        name: &str,
        expected_meta_version: i64,
        version_info: &AgentSpecVersionInfo,
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
            .filter(ai_resource::Column::Type.eq(AGENTSPEC_TYPE))
            .filter(ai_resource::Column::MetaVersion.eq(expected_meta_version))
            .exec(self.db.as_ref())
            .await?;

        if result.rows_affected > 0 {
            return Ok(true);
        }

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
                .filter(ai_resource::Column::Type.eq(AGENTSPEC_TYPE))
                .filter(ai_resource::Column::MetaVersion.eq(resource.meta_version))
                .exec(self.db.as_ref())
                .await?;
            if retry_result.rows_affected > 0 {
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
            description: ver.c_desc.clone(),
            content,
            biz_tags: None,
            resource: resources,
        }))
    }

    pub async fn delete(&self, namespace_id: &str, name: &str) -> anyhow::Result<()> {
        ai_resource::Entity::delete_many()
            .filter(ai_resource::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource::Column::Name.eq(name))
            .filter(ai_resource::Column::Type.eq(AGENTSPEC_TYPE))
            .exec(self.db.as_ref())
            .await?;
        ai_resource_version::Entity::delete_many()
            .filter(ai_resource_version::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource_version::Column::Name.eq(name))
            .filter(ai_resource_version::Column::Type.eq(AGENTSPEC_TYPE))
            .exec(self.db.as_ref())
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
        let mut query = ai_resource::Entity::find()
            .filter(ai_resource::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource::Column::Type.eq(AGENTSPEC_TYPE));

        if let Some(name) = name_filter
            && !name.is_empty()
        {
            let st = search.unwrap_or("blur");
            if st == "accurate" {
                query = query.filter(ai_resource::Column::Name.eq(name));
            } else {
                query = query.filter(ai_resource::Column::Name.contains(name));
            }
        }

        query = query.order_by(ai_resource::Column::GmtModified, Order::Desc);
        let total = query.clone().count(self.db.as_ref()).await?;
        let offset = (page_no.saturating_sub(1)) * page_size;
        let resources = query
            .offset(offset)
            .limit(page_size)
            .all(self.db.as_ref())
            .await?;
        let items: Vec<AgentSpecSummary> =
            resources.iter().map(Self::resource_to_summary).collect();
        Ok(Page::new(total, page_no, page_size, items))
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
        let now = Utc::now().naive_utc();

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
            let resource = ai_resource::ActiveModel {
                gmt_create: Set(Some(now)),
                gmt_modified: Set(Some(now)),
                name: Set(name.to_string()),
                r#type: Set(AGENTSPEC_TYPE.to_string()),
                c_desc: Set(spec.description.clone()),
                status: Set(Some(RESOURCE_STATUS_ENABLE.to_string())),
                namespace_id: Set(namespace_id.to_string()),
                c_from: Set(AGENTSPEC_DEFAULT_FROM.to_string()),
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

        if overwrite {
            ai_resource_version::Entity::delete_many()
                .filter(ai_resource_version::Column::NamespaceId.eq(namespace_id))
                .filter(ai_resource_version::Column::Name.eq(name))
                .filter(ai_resource_version::Column::Type.eq(AGENTSPEC_TYPE))
                .filter(ai_resource_version::Column::Status.eq(VERSION_STATUS_DRAFT))
                .exec(self.db.as_ref())
                .await?;
        }

        let ver = ai_resource_version::ActiveModel {
            gmt_create: Set(Some(now)),
            gmt_modified: Set(Some(now)),
            r#type: Set(AGENTSPEC_TYPE.to_string()),
            author: Set(Some(author.to_string())),
            name: Set(name.to_string()),
            c_desc: Set(spec.description.clone()),
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

        let now = Utc::now().naive_utc();
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
            let resource = ai_resource::ActiveModel {
                gmt_create: Set(Some(now)),
                gmt_modified: Set(Some(now)),
                name: Set(name.to_string()),
                r#type: Set(AGENTSPEC_TYPE.to_string()),
                c_desc: Set(initial_content.and_then(|s| s.description.clone())),
                status: Set(Some(RESOURCE_STATUS_ENABLE.to_string())),
                namespace_id: Set(namespace_id.to_string()),
                c_from: Set(AGENTSPEC_DEFAULT_FROM.to_string()),
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

        let ver = ai_resource_version::ActiveModel {
            gmt_create: Set(Some(now)),
            gmt_modified: Set(Some(now)),
            r#type: Set(AGENTSPEC_TYPE.to_string()),
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
        let now = Utc::now().naive_utc();

        ai_resource_version::Entity::update_many()
            .col_expr(
                ai_resource_version::Column::Storage,
                Expr::value(storage_json.clone()),
            )
            .col_expr(
                ai_resource_version::Column::CDesc,
                Expr::value(spec.description.clone()),
            )
            .col_expr(ai_resource_version::Column::GmtModified, Expr::value(now))
            .filter(ai_resource_version::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource_version::Column::Name.eq(name))
            .filter(ai_resource_version::Column::Type.eq(AGENTSPEC_TYPE))
            .filter(ai_resource_version::Column::Version.eq(&draft_version))
            .filter(ai_resource_version::Column::Status.eq(VERSION_STATUS_DRAFT))
            .exec(self.db.as_ref())
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

        ai_resource_version::Entity::delete_many()
            .filter(ai_resource_version::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource_version::Column::Name.eq(name))
            .filter(ai_resource_version::Column::Type.eq(AGENTSPEC_TYPE))
            .filter(ai_resource_version::Column::Version.eq(&draft_version))
            .exec(self.db.as_ref())
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

        let now = Utc::now().naive_utc();
        ai_resource_version::Entity::update_many()
            .col_expr(
                ai_resource_version::Column::Status,
                Expr::value(VERSION_STATUS_REVIEWING),
            )
            .col_expr(ai_resource_version::Column::GmtModified, Expr::value(now))
            .filter(ai_resource_version::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource_version::Column::Name.eq(name))
            .filter(ai_resource_version::Column::Type.eq(AGENTSPEC_TYPE))
            .filter(ai_resource_version::Column::Version.eq(version))
            .exec(self.db.as_ref())
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

        let now = Utc::now().naive_utc();
        ai_resource_version::Entity::update_many()
            .col_expr(
                ai_resource_version::Column::Status,
                Expr::value(VERSION_STATUS_ONLINE),
            )
            .col_expr(ai_resource_version::Column::GmtModified, Expr::value(now))
            .filter(ai_resource_version::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource_version::Column::Name.eq(name))
            .filter(ai_resource_version::Column::Type.eq(AGENTSPEC_TYPE))
            .filter(ai_resource_version::Column::Version.eq(version))
            .exec(self.db.as_ref())
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
        let now = Utc::now().naive_utc();
        ai_resource::Entity::update_many()
            .col_expr(ai_resource::Column::BizTags, Expr::value(biz_tags))
            .col_expr(ai_resource::Column::GmtModified, Expr::value(now))
            .filter(ai_resource::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource::Column::Name.eq(name))
            .filter(ai_resource::Column::Type.eq(AGENTSPEC_TYPE))
            .exec(self.db.as_ref())
            .await?;
        Ok(())
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
        let now = Utc::now().naive_utc();

        if scope == Some("agentspec") {
            let status = if online {
                RESOURCE_STATUS_ENABLE
            } else {
                RESOURCE_STATUS_DISABLE
            };
            ai_resource::Entity::update_many()
                .col_expr(ai_resource::Column::Status, Expr::value(status))
                .col_expr(ai_resource::Column::GmtModified, Expr::value(now))
                .filter(ai_resource::Column::NamespaceId.eq(namespace_id))
                .filter(ai_resource::Column::Name.eq(name))
                .filter(ai_resource::Column::Type.eq(AGENTSPEC_TYPE))
                .exec(self.db.as_ref())
                .await?;
        } else {
            let version = version.ok_or_else(|| anyhow::anyhow!("version is required"))?;
            let status = if online {
                VERSION_STATUS_ONLINE
            } else {
                VERSION_STATUS_OFFLINE
            };
            ai_resource_version::Entity::update_many()
                .col_expr(ai_resource_version::Column::Status, Expr::value(status))
                .col_expr(ai_resource_version::Column::GmtModified, Expr::value(now))
                .filter(ai_resource_version::Column::NamespaceId.eq(namespace_id))
                .filter(ai_resource_version::Column::Name.eq(name))
                .filter(ai_resource_version::Column::Type.eq(AGENTSPEC_TYPE))
                .filter(ai_resource_version::Column::Version.eq(version))
                .exec(self.db.as_ref())
                .await?;

            let online_count = ai_resource_version::Entity::find()
                .filter(ai_resource_version::Column::NamespaceId.eq(namespace_id))
                .filter(ai_resource_version::Column::Name.eq(name))
                .filter(ai_resource_version::Column::Type.eq(AGENTSPEC_TYPE))
                .filter(ai_resource_version::Column::Status.eq(VERSION_STATUS_ONLINE))
                .count(self.db.as_ref())
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
        let now = Utc::now().naive_utc();
        ai_resource::Entity::update_many()
            .col_expr(ai_resource::Column::Scope, Expr::value(scope))
            .col_expr(ai_resource::Column::GmtModified, Expr::value(now))
            .filter(ai_resource::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource::Column::Name.eq(name))
            .filter(ai_resource::Column::Type.eq(AGENTSPEC_TYPE))
            .exec(self.db.as_ref())
            .await?;
        Ok(())
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
            description: ver.c_desc,
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
        let mut query = ai_resource::Entity::find()
            .filter(ai_resource::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource::Column::Type.eq(AGENTSPEC_TYPE))
            .filter(ai_resource::Column::Status.eq(RESOURCE_STATUS_ENABLE));
        if let Some(kw) = keyword
            && !kw.is_empty()
        {
            query = query.filter(ai_resource::Column::Name.contains(kw));
        }
        query = query.order_by(ai_resource::Column::GmtModified, Order::Desc);
        let all = query.all(self.db.as_ref()).await?;
        let filtered: Vec<AgentSpecBasicInfo> = all
            .iter()
            .filter(|r| Self::parse_version_info(r).online_cnt > 0)
            .map(|r| AgentSpecBasicInfo {
                namespace_id: r.namespace_id.clone(),
                name: r.name.clone(),
                description: r.c_desc.clone(),
                update_time: r.gmt_modified.map(|dt| dt.to_string()),
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
