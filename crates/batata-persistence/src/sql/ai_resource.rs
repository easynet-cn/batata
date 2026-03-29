//! AiResourcePersistence implementation for ExternalDbPersistService

use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{prelude::Expr, sea_query::Asterisk, *};

use crate::entity::{ai_resource, ai_resource_version, pipeline_execution};
use crate::model::*;
use crate::traits::*;

use super::ExternalDbPersistService;

// ============================================================================
// Conversion helpers
// ============================================================================

fn ai_resource_model_to_info(m: ai_resource::Model) -> AiResourceInfo {
    AiResourceInfo {
        id: m.id,
        name: m.name,
        resource_type: m.r#type,
        description: m.c_desc,
        status: m.status,
        namespace_id: m.namespace_id,
        biz_tags: m.biz_tags,
        ext: m.ext,
        from: m.c_from,
        version_info: m.version_info,
        meta_version: m.meta_version,
        scope: m.scope,
        owner: m.owner,
        download_count: m.download_count,
        gmt_create: m.gmt_create.map(|dt| dt.to_string()),
        gmt_modified: m.gmt_modified.map(|dt| dt.to_string()),
    }
}

fn ai_resource_version_model_to_info(m: ai_resource_version::Model) -> AiResourceVersionInfo {
    AiResourceVersionInfo {
        id: m.id,
        resource_type: m.r#type,
        author: m.author,
        name: m.name,
        description: m.c_desc,
        status: m.status,
        version: m.version,
        namespace_id: m.namespace_id,
        storage: m.storage,
        publish_pipeline_info: m.publish_pipeline_info,
        download_count: m.download_count,
        gmt_create: m.gmt_create.map(|dt| dt.to_string()),
        gmt_modified: m.gmt_modified.map(|dt| dt.to_string()),
    }
}

fn pipeline_execution_model_to_info(m: pipeline_execution::Model) -> PipelineExecutionInfo {
    PipelineExecutionInfo {
        execution_id: m.execution_id,
        resource_type: m.resource_type,
        resource_name: m.resource_name,
        namespace_id: m.namespace_id,
        version: m.version,
        status: m.status,
        pipeline: m.pipeline,
        create_time: m.create_time,
        update_time: m.update_time,
    }
}

/// Apply the common namespace_id + name + type filter for ai_resource queries.
fn ai_resource_filter<E: EntityTrait>(
    select: Select<E>,
    namespace_id: &str,
    name: &str,
    resource_type: &str,
) -> Select<E>
where
    <E as EntityTrait>::Column: From<ai_resource::Column>,
{
    select
        .filter(ai_resource::Column::NamespaceId.eq(namespace_id))
        .filter(ai_resource::Column::Name.eq(name))
        .filter(ai_resource::Column::Type.eq(resource_type))
}

/// Apply the common namespace_id + name + type + version filter for ai_resource_version queries.
fn ai_resource_version_filter<E: EntityTrait>(
    select: Select<E>,
    namespace_id: &str,
    name: &str,
    resource_type: &str,
    version: &str,
) -> Select<E>
where
    <E as EntityTrait>::Column: From<ai_resource_version::Column>,
{
    select
        .filter(ai_resource_version::Column::NamespaceId.eq(namespace_id))
        .filter(ai_resource_version::Column::Name.eq(name))
        .filter(ai_resource_version::Column::Type.eq(resource_type))
        .filter(ai_resource_version::Column::Version.eq(version))
}

// ============================================================================
// AiResourcePersistence implementation
// ============================================================================

#[async_trait]
impl AiResourcePersistence for ExternalDbPersistService {
    // ========================================================================
    // ai_resource operations
    // ========================================================================

    async fn ai_resource_find(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
    ) -> anyhow::Result<Option<AiResourceInfo>> {
        let result = ai_resource_filter(
            ai_resource::Entity::find(),
            namespace_id,
            name,
            resource_type,
        )
        .one(&self.db)
        .await?
        .map(ai_resource_model_to_info);

        Ok(result)
    }

    async fn ai_resource_insert(&self, resource: &AiResourceInfo) -> anyhow::Result<i64> {
        let now = Utc::now().naive_utc();
        let entity = ai_resource::ActiveModel {
            id: NotSet,
            gmt_create: Set(Some(now)),
            gmt_modified: Set(Some(now)),
            name: Set(resource.name.clone()),
            r#type: Set(resource.resource_type.clone()),
            c_desc: Set(resource.description.clone()),
            status: Set(resource.status.clone()),
            namespace_id: Set(resource.namespace_id.clone()),
            biz_tags: Set(resource.biz_tags.clone()),
            ext: Set(resource.ext.clone()),
            c_from: Set(resource.from.clone()),
            version_info: Set(resource.version_info.clone()),
            meta_version: Set(resource.meta_version),
            scope: Set(resource.scope.clone()),
            owner: Set(resource.owner.clone()),
            download_count: Set(resource.download_count),
        };

        let result = ai_resource::Entity::insert(entity).exec(&self.db).await?;
        Ok(result.last_insert_id)
    }

    async fn ai_resource_update_version_info_cas(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        expected_meta_version: i64,
        version_info: &str,
        new_meta_version: i64,
    ) -> anyhow::Result<bool> {
        let now = Utc::now().naive_utc();
        let result = ai_resource::Entity::update_many()
            .col_expr(
                ai_resource::Column::VersionInfo,
                Expr::value(version_info.to_string()),
            )
            .col_expr(
                ai_resource::Column::MetaVersion,
                Expr::value(new_meta_version),
            )
            .col_expr(ai_resource::Column::GmtModified, Expr::value(now))
            .filter(ai_resource::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource::Column::Name.eq(name))
            .filter(ai_resource::Column::Type.eq(resource_type))
            .filter(ai_resource::Column::MetaVersion.eq(expected_meta_version))
            .exec(&self.db)
            .await?;

        Ok(result.rows_affected > 0)
    }

    async fn ai_resource_update_biz_tags(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        biz_tags: &str,
    ) -> anyhow::Result<()> {
        let now = Utc::now().naive_utc();
        ai_resource::Entity::update_many()
            .col_expr(
                ai_resource::Column::BizTags,
                Expr::value(Some(biz_tags.to_string())),
            )
            .col_expr(ai_resource::Column::GmtModified, Expr::value(now))
            .filter(ai_resource::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource::Column::Name.eq(name))
            .filter(ai_resource::Column::Type.eq(resource_type))
            .exec(&self.db)
            .await?;

        Ok(())
    }

    async fn ai_resource_update_status(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        status: &str,
    ) -> anyhow::Result<()> {
        let now = Utc::now().naive_utc();
        ai_resource::Entity::update_many()
            .col_expr(
                ai_resource::Column::Status,
                Expr::value(Some(status.to_string())),
            )
            .col_expr(ai_resource::Column::GmtModified, Expr::value(now))
            .filter(ai_resource::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource::Column::Name.eq(name))
            .filter(ai_resource::Column::Type.eq(resource_type))
            .exec(&self.db)
            .await?;

        Ok(())
    }

    async fn ai_resource_update_scope(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        scope: &str,
    ) -> anyhow::Result<()> {
        let now = Utc::now().naive_utc();
        ai_resource::Entity::update_many()
            .col_expr(ai_resource::Column::Scope, Expr::value(scope.to_string()))
            .col_expr(ai_resource::Column::GmtModified, Expr::value(now))
            .filter(ai_resource::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource::Column::Name.eq(name))
            .filter(ai_resource::Column::Type.eq(resource_type))
            .exec(&self.db)
            .await?;

        Ok(())
    }

    async fn ai_resource_increment_download_count(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        increment: i64,
    ) -> anyhow::Result<()> {
        ai_resource::Entity::update_many()
            .col_expr(
                ai_resource::Column::DownloadCount,
                Expr::col(ai_resource::Column::DownloadCount).add(increment),
            )
            .filter(ai_resource::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource::Column::Name.eq(name))
            .filter(ai_resource::Column::Type.eq(resource_type))
            .exec(&self.db)
            .await?;

        Ok(())
    }

    async fn ai_resource_delete(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
    ) -> anyhow::Result<u64> {
        let result = ai_resource::Entity::delete_many()
            .filter(ai_resource::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource::Column::Name.eq(name))
            .filter(ai_resource::Column::Type.eq(resource_type))
            .exec(&self.db)
            .await?;

        Ok(result.rows_affected)
    }

    async fn ai_resource_list(
        &self,
        namespace_id: &str,
        resource_type: &str,
        name_filter: Option<&str>,
        search_accurate: bool,
        order_by_downloads: bool,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<AiResourceInfo>> {
        let mut count_select = ai_resource::Entity::find()
            .filter(ai_resource::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource::Column::Type.eq(resource_type));
        let mut query_select = ai_resource::Entity::find()
            .filter(ai_resource::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource::Column::Type.eq(resource_type));

        if let Some(name) = name_filter
            && !name.is_empty()
        {
            if search_accurate {
                count_select = count_select.filter(ai_resource::Column::Name.eq(name));
                query_select = query_select.filter(ai_resource::Column::Name.eq(name));
            } else {
                count_select = count_select.filter(ai_resource::Column::Name.contains(name));
                query_select = query_select.filter(ai_resource::Column::Name.contains(name));
            }
        }

        let total_count = count_select
            .select_only()
            .column_as(Expr::col(Asterisk).count(), "count")
            .into_tuple::<i64>()
            .one(&self.db)
            .await?
            .unwrap_or_default() as u64;

        if total_count == 0 {
            return Ok(Page::empty());
        }

        if order_by_downloads {
            query_select = query_select.order_by_desc(ai_resource::Column::DownloadCount);
        }

        let offset = (page_no - 1) * page_size;
        let items = query_select
            .offset(offset)
            .limit(page_size)
            .all(&self.db)
            .await?
            .into_iter()
            .map(ai_resource_model_to_info)
            .collect();

        Ok(Page::new(total_count, page_no, page_size, items))
    }

    async fn ai_resource_find_all(
        &self,
        namespace_id: &str,
        resource_type: &str,
    ) -> anyhow::Result<Vec<AiResourceInfo>> {
        let items = ai_resource::Entity::find()
            .filter(ai_resource::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource::Column::Type.eq(resource_type))
            .all(&self.db)
            .await?
            .into_iter()
            .map(ai_resource_model_to_info)
            .collect();

        Ok(items)
    }

    // ========================================================================
    // ai_resource_version operations
    // ========================================================================

    async fn ai_resource_version_find(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        version: &str,
    ) -> anyhow::Result<Option<AiResourceVersionInfo>> {
        let result = ai_resource_version_filter(
            ai_resource_version::Entity::find(),
            namespace_id,
            name,
            resource_type,
            version,
        )
        .one(&self.db)
        .await?
        .map(ai_resource_version_model_to_info);

        Ok(result)
    }

    async fn ai_resource_version_insert(
        &self,
        version: &AiResourceVersionInfo,
    ) -> anyhow::Result<i64> {
        let now = Utc::now().naive_utc();
        let entity = ai_resource_version::ActiveModel {
            id: NotSet,
            gmt_create: Set(Some(now)),
            gmt_modified: Set(Some(now)),
            r#type: Set(version.resource_type.clone()),
            author: Set(version.author.clone()),
            name: Set(version.name.clone()),
            c_desc: Set(version.description.clone()),
            status: Set(version.status.clone()),
            version: Set(version.version.clone()),
            namespace_id: Set(version.namespace_id.clone()),
            storage: Set(version.storage.clone()),
            publish_pipeline_info: Set(version.publish_pipeline_info.clone()),
            download_count: Set(version.download_count),
        };

        let result = ai_resource_version::Entity::insert(entity)
            .exec(&self.db)
            .await?;
        Ok(result.last_insert_id)
    }

    async fn ai_resource_version_update_status(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        version: &str,
        status: &str,
    ) -> anyhow::Result<()> {
        let now = Utc::now().naive_utc();
        ai_resource_version::Entity::update_many()
            .col_expr(
                ai_resource_version::Column::Status,
                Expr::value(status.to_string()),
            )
            .col_expr(ai_resource_version::Column::GmtModified, Expr::value(now))
            .filter(ai_resource_version::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource_version::Column::Name.eq(name))
            .filter(ai_resource_version::Column::Type.eq(resource_type))
            .filter(ai_resource_version::Column::Version.eq(version))
            .exec(&self.db)
            .await?;

        Ok(())
    }

    async fn ai_resource_version_update_storage(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        version: &str,
        storage: &str,
        description: Option<&str>,
    ) -> anyhow::Result<()> {
        let now = Utc::now().naive_utc();
        let mut update = ai_resource_version::Entity::update_many()
            .col_expr(
                ai_resource_version::Column::Storage,
                Expr::value(Some(storage.to_string())),
            )
            .col_expr(ai_resource_version::Column::GmtModified, Expr::value(now));

        if let Some(desc) = description {
            update = update.col_expr(
                ai_resource_version::Column::CDesc,
                Expr::value(Some(desc.to_string())),
            );
        }

        update
            .filter(ai_resource_version::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource_version::Column::Name.eq(name))
            .filter(ai_resource_version::Column::Type.eq(resource_type))
            .filter(ai_resource_version::Column::Version.eq(version))
            .exec(&self.db)
            .await?;

        Ok(())
    }

    async fn ai_resource_version_increment_download_count(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        version: &str,
        increment: i64,
    ) -> anyhow::Result<()> {
        ai_resource_version::Entity::update_many()
            .col_expr(
                ai_resource_version::Column::DownloadCount,
                Expr::col(ai_resource_version::Column::DownloadCount).add(increment),
            )
            .filter(ai_resource_version::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource_version::Column::Name.eq(name))
            .filter(ai_resource_version::Column::Type.eq(resource_type))
            .filter(ai_resource_version::Column::Version.eq(version))
            .exec(&self.db)
            .await?;

        Ok(())
    }

    async fn ai_resource_version_list(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
    ) -> anyhow::Result<Vec<AiResourceVersionInfo>> {
        let items = ai_resource_version::Entity::find()
            .filter(ai_resource_version::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource_version::Column::Name.eq(name))
            .filter(ai_resource_version::Column::Type.eq(resource_type))
            .all(&self.db)
            .await?
            .into_iter()
            .map(ai_resource_version_model_to_info)
            .collect();

        Ok(items)
    }

    async fn ai_resource_version_count_by_status(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        status: &str,
    ) -> anyhow::Result<u64> {
        let count = ai_resource_version::Entity::find()
            .filter(ai_resource_version::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource_version::Column::Name.eq(name))
            .filter(ai_resource_version::Column::Type.eq(resource_type))
            .filter(ai_resource_version::Column::Status.eq(status))
            .count(&self.db)
            .await?;

        Ok(count)
    }

    async fn ai_resource_version_delete(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        version: &str,
    ) -> anyhow::Result<u64> {
        let result = ai_resource_version::Entity::delete_many()
            .filter(ai_resource_version::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource_version::Column::Name.eq(name))
            .filter(ai_resource_version::Column::Type.eq(resource_type))
            .filter(ai_resource_version::Column::Version.eq(version))
            .exec(&self.db)
            .await?;

        Ok(result.rows_affected)
    }

    async fn ai_resource_version_delete_all(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
    ) -> anyhow::Result<u64> {
        let result = ai_resource_version::Entity::delete_many()
            .filter(ai_resource_version::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource_version::Column::Name.eq(name))
            .filter(ai_resource_version::Column::Type.eq(resource_type))
            .exec(&self.db)
            .await?;

        Ok(result.rows_affected)
    }

    async fn ai_resource_version_delete_by_status(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        status: &str,
    ) -> anyhow::Result<u64> {
        let result = ai_resource_version::Entity::delete_many()
            .filter(ai_resource_version::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource_version::Column::Name.eq(name))
            .filter(ai_resource_version::Column::Type.eq(resource_type))
            .filter(ai_resource_version::Column::Status.eq(status))
            .exec(&self.db)
            .await?;

        Ok(result.rows_affected)
    }

    // ========================================================================
    // pipeline_execution operations
    // ========================================================================

    async fn pipeline_execution_find(
        &self,
        execution_id: &str,
    ) -> anyhow::Result<Option<PipelineExecutionInfo>> {
        let result = pipeline_execution::Entity::find_by_id(execution_id.to_string())
            .one(&self.db)
            .await?
            .map(pipeline_execution_model_to_info);

        Ok(result)
    }

    async fn pipeline_execution_list(
        &self,
        resource_type: &str,
        resource_name: Option<&str>,
        namespace_id: Option<&str>,
        version: Option<&str>,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<PipelineExecutionInfo>> {
        let mut count_select = pipeline_execution::Entity::find()
            .filter(pipeline_execution::Column::ResourceType.eq(resource_type));
        let mut query_select = pipeline_execution::Entity::find()
            .filter(pipeline_execution::Column::ResourceType.eq(resource_type));

        if let Some(name) = resource_name {
            count_select = count_select.filter(pipeline_execution::Column::ResourceName.eq(name));
            query_select = query_select.filter(pipeline_execution::Column::ResourceName.eq(name));
        }

        if let Some(ns) = namespace_id {
            count_select =
                count_select.filter(pipeline_execution::Column::NamespaceId.eq(ns.to_string()));
            query_select =
                query_select.filter(pipeline_execution::Column::NamespaceId.eq(ns.to_string()));
        }

        if let Some(ver) = version {
            count_select =
                count_select.filter(pipeline_execution::Column::Version.eq(ver.to_string()));
            query_select =
                query_select.filter(pipeline_execution::Column::Version.eq(ver.to_string()));
        }

        let total_count = count_select
            .select_only()
            .column_as(Expr::col(Asterisk).count(), "count")
            .into_tuple::<i64>()
            .one(&self.db)
            .await?
            .unwrap_or_default() as u64;

        if total_count == 0 {
            return Ok(Page::empty());
        }

        let offset = (page_no - 1) * page_size;
        let items = query_select
            .order_by_desc(pipeline_execution::Column::CreateTime)
            .offset(offset)
            .limit(page_size)
            .all(&self.db)
            .await?
            .into_iter()
            .map(pipeline_execution_model_to_info)
            .collect();

        Ok(Page::new(total_count, page_no, page_size, items))
    }
}
