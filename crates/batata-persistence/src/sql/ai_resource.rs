//! AiResourcePersistence implementation for ExternalDbPersistService

use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{
    prelude::Expr,
    sea_query::{Asterisk, OnConflict},
    *,
};

use crate::entity::{
    ai_resource, ai_resource_search_chunk, ai_resource_search_document, ai_resource_task,
    ai_resource_version, pipeline_execution,
};
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

fn search_document_model_to_info(
    m: ai_resource_search_document::Model,
) -> AiResourceSearchDocumentInfo {
    AiResourceSearchDocumentInfo {
        id: m.id,
        namespace_id: m.namespace_id,
        resource_type: m.resource_type,
        resource_name: m.resource_name,
        resource_version: m.resource_version,
        display_name: m.display_name,
        description: m.c_desc,
        tags: m.tags,
        capabilities: m.capabilities,
        representative_queries: m.representative_queries,
        metadata: m.metadata,
        source_digest: m.source_digest,
        status: m.status,
        generate_mode: m.generate_mode,
        gmt_create: m.gmt_create.map(|dt| dt.to_string()),
        gmt_modified: m.gmt_modified.map(|dt| dt.to_string()),
    }
}

fn search_chunk_model_to_info(m: ai_resource_search_chunk::Model) -> AiResourceSearchChunkInfo {
    AiResourceSearchChunkInfo {
        id: m.id,
        document_id: m.document_id,
        namespace_id: m.namespace_id,
        resource_type: m.resource_type,
        resource_name: m.resource_name,
        resource_version: m.resource_version,
        chunk_type: m.chunk_type,
        chunk_text: m.chunk_text,
        canonical_text: m.canonical_text,
        language: m.language,
        chunk_hash: m.chunk_hash,
        metadata: m.metadata,
        status: m.status,
        gmt_create: m.gmt_create.map(|dt| dt.to_string()),
        gmt_modified: m.gmt_modified.map(|dt| dt.to_string()),
    }
}

fn task_model_to_info(m: ai_resource_task::Model) -> AiResourceTaskInfo {
    AiResourceTaskInfo {
        task_key: m.task_key,
        namespace_id: m.namespace_id,
        task_type: m.task_type,
        task_stage: m.task_stage,
        status: m.status,
        task_payload: m.task_payload,
        task_result: m.task_result,
        retry_count: m.retry_count,
        revision: m.revision,
        lease_token: m.lease_token,
        next_execute_at: m.next_execute_at,
        lease_expire_at: m.lease_expire_at,
        last_error: m.last_error,
        gmt_create: m.gmt_create.map(|dt| dt.to_string()),
        gmt_modified: m.gmt_modified.map(|dt| dt.to_string()),
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

    async fn ai_resource_update_description(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        description: &str,
    ) -> anyhow::Result<()> {
        let now = Utc::now().naive_utc();
        ai_resource::Entity::update_many()
            .col_expr(
                ai_resource::Column::CDesc,
                Expr::value(Some(description.to_string())),
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
        filter: &AiResourceListFilter<'_>,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<AiResourceInfo>> {
        use sea_orm::Condition;

        let mut count_select = ai_resource::Entity::find()
            .filter(ai_resource::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource::Column::Type.eq(resource_type));
        let mut query_select = ai_resource::Entity::find()
            .filter(ai_resource::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource::Column::Type.eq(resource_type));

        // Name filter
        if let Some(name) = filter.name_filter
            && !name.is_empty()
        {
            if filter.search_accurate {
                count_select = count_select.filter(ai_resource::Column::Name.eq(name));
                query_select = query_select.filter(ai_resource::Column::Name.eq(name));
            } else {
                count_select = count_select.filter(ai_resource::Column::Name.contains(name));
                query_select = query_select.filter(ai_resource::Column::Name.contains(name));
            }
        }

        // Scope filter
        if let Some(scope) = filter.scope_filter {
            count_select = count_select.filter(ai_resource::Column::Scope.eq(scope));
            query_select = query_select.filter(ai_resource::Column::Scope.eq(scope));
        }

        // Owner filter (with optional PUBLIC inclusion)
        if let Some(owner) = filter.owner_filter {
            if filter.include_public_for_owner {
                let cond = Condition::any()
                    .add(ai_resource::Column::Owner.eq(owner))
                    .add(ai_resource::Column::Scope.eq("PUBLIC"));
                count_select = count_select.filter(cond.clone());
                query_select = query_select.filter(cond);
            } else {
                count_select = count_select.filter(ai_resource::Column::Owner.eq(owner));
                query_select = query_select.filter(ai_resource::Column::Owner.eq(owner));
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

        if filter.order_by_downloads {
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

    // ========================================================================
    // ai_resource_search_document operations
    // ========================================================================

    async fn search_document_find(
        &self,
        namespace_id: &str,
        resource_type: &str,
        resource_name: &str,
        resource_version: &str,
    ) -> anyhow::Result<Option<AiResourceSearchDocumentInfo>> {
        let result = ai_resource_search_document::Entity::find()
            .filter(ai_resource_search_document::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource_search_document::Column::ResourceType.eq(resource_type))
            .filter(ai_resource_search_document::Column::ResourceName.eq(resource_name))
            .filter(ai_resource_search_document::Column::ResourceVersion.eq(resource_version))
            .one(&self.db)
            .await?
            .map(search_document_model_to_info);

        Ok(result)
    }

    async fn search_document_upsert(
        &self,
        document: &AiResourceSearchDocumentInfo,
    ) -> anyhow::Result<i64> {
        let now = Utc::now().naive_utc();
        let model = ai_resource_search_document::ActiveModel {
            id: NotSet,
            gmt_create: Set(Some(now)),
            gmt_modified: Set(Some(now)),
            namespace_id: Set(document.namespace_id.clone()),
            resource_type: Set(document.resource_type.clone()),
            resource_name: Set(document.resource_name.clone()),
            resource_version: Set(document.resource_version.clone()),
            display_name: Set(document.display_name.clone()),
            c_desc: Set(document.description.clone()),
            tags: Set(document.tags.clone()),
            capabilities: Set(document.capabilities.clone()),
            representative_queries: Set(document.representative_queries.clone()),
            metadata: Set(document.metadata.clone()),
            source_digest: Set(document.source_digest.clone()),
            status: Set(document.status.clone()),
            generate_mode: Set(document.generate_mode.clone()),
        };

        // `last_insert_id` is unreliable on the update branch of an upsert, so
        // re-read by the unique key to return a stable ID.
        ai_resource_search_document::Entity::insert(model)
            .on_conflict(
                // PostgreSQL requires an explicit conflict target; MySQL
                // ignores it and relies on the unique key.
                OnConflict::columns([
                    ai_resource_search_document::Column::NamespaceId,
                    ai_resource_search_document::Column::ResourceType,
                    ai_resource_search_document::Column::ResourceName,
                    ai_resource_search_document::Column::ResourceVersion,
                ])
                .update_columns([
                    ai_resource_search_document::Column::GmtModified,
                    ai_resource_search_document::Column::DisplayName,
                    ai_resource_search_document::Column::CDesc,
                    ai_resource_search_document::Column::Tags,
                    ai_resource_search_document::Column::Capabilities,
                    ai_resource_search_document::Column::RepresentativeQueries,
                    ai_resource_search_document::Column::Metadata,
                    ai_resource_search_document::Column::SourceDigest,
                    ai_resource_search_document::Column::Status,
                    ai_resource_search_document::Column::GenerateMode,
                ])
                .to_owned(),
            )
            .exec(&self.db)
            .await?;

        let stored = self
            .search_document_find(
                &document.namespace_id,
                &document.resource_type,
                &document.resource_name,
                &document.resource_version,
            )
            .await?;

        Ok(stored.map(|d| d.id).unwrap_or_default())
    }

    async fn search_document_delete(
        &self,
        namespace_id: &str,
        resource_type: &str,
        resource_name: &str,
        resource_version: &str,
    ) -> anyhow::Result<u64> {
        let result = ai_resource_search_document::Entity::delete_many()
            .filter(ai_resource_search_document::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource_search_document::Column::ResourceType.eq(resource_type))
            .filter(ai_resource_search_document::Column::ResourceName.eq(resource_name))
            .filter(ai_resource_search_document::Column::ResourceVersion.eq(resource_version))
            .exec(&self.db)
            .await?;

        Ok(result.rows_affected)
    }

    // ========================================================================
    // ai_resource_search_chunk operations
    // ========================================================================

    async fn search_chunk_replace(
        &self,
        namespace_id: &str,
        resource_type: &str,
        resource_name: &str,
        resource_version: &str,
        chunks: &[AiResourceSearchChunkInfo],
    ) -> anyhow::Result<u64> {
        self.search_chunk_delete(namespace_id, resource_type, resource_name, resource_version)
            .await?;

        if chunks.is_empty() {
            return Ok(0);
        }

        let document_id = self
            .search_document_find(namespace_id, resource_type, resource_name, resource_version)
            .await?
            .map(|d| d.id)
            .unwrap_or_default();

        let now = Utc::now().naive_utc();
        let models = chunks
            .iter()
            .map(|c| ai_resource_search_chunk::ActiveModel {
                id: NotSet,
                gmt_create: Set(Some(now)),
                gmt_modified: Set(Some(now)),
                document_id: Set(document_id),
                namespace_id: Set(c.namespace_id.clone()),
                resource_type: Set(c.resource_type.clone()),
                resource_name: Set(c.resource_name.clone()),
                resource_version: Set(c.resource_version.clone()),
                chunk_type: Set(c.chunk_type.clone()),
                chunk_text: Set(c.chunk_text.clone()),
                canonical_text: Set(c.canonical_text.clone()),
                language: Set(c.language.clone()),
                chunk_hash: Set(c.chunk_hash.clone()),
                metadata: Set(c.metadata.clone()),
                status: Set(c.status.clone()),
            });

        ai_resource_search_chunk::Entity::insert_many(models)
            .exec(&self.db)
            .await?;

        Ok(chunks.len() as u64)
    }

    async fn search_chunk_list(
        &self,
        namespace_id: &str,
        resource_type: &str,
        resource_name: &str,
        resource_version: &str,
    ) -> anyhow::Result<Vec<AiResourceSearchChunkInfo>> {
        let items = ai_resource_search_chunk::Entity::find()
            .filter(ai_resource_search_chunk::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource_search_chunk::Column::ResourceType.eq(resource_type))
            .filter(ai_resource_search_chunk::Column::ResourceName.eq(resource_name))
            .filter(ai_resource_search_chunk::Column::ResourceVersion.eq(resource_version))
            .all(&self.db)
            .await?
            .into_iter()
            .map(search_chunk_model_to_info)
            .collect();

        Ok(items)
    }

    async fn search_chunk_delete(
        &self,
        namespace_id: &str,
        resource_type: &str,
        resource_name: &str,
        resource_version: &str,
    ) -> anyhow::Result<u64> {
        let result = ai_resource_search_chunk::Entity::delete_many()
            .filter(ai_resource_search_chunk::Column::NamespaceId.eq(namespace_id))
            .filter(ai_resource_search_chunk::Column::ResourceType.eq(resource_type))
            .filter(ai_resource_search_chunk::Column::ResourceName.eq(resource_name))
            .filter(ai_resource_search_chunk::Column::ResourceVersion.eq(resource_version))
            .exec(&self.db)
            .await?;

        Ok(result.rows_affected)
    }

    // ========================================================================
    // ai_resource_task operations
    // ========================================================================

    async fn task_upsert(&self, task: &AiResourceTaskInfo) -> anyhow::Result<()> {
        let now = Utc::now().naive_utc();
        let model = ai_resource_task::ActiveModel {
            task_key: Set(task.task_key.clone()),
            namespace_id: Set(task.namespace_id.clone()),
            task_type: Set(task.task_type.clone()),
            task_stage: Set(task.task_stage.clone()),
            status: Set(task.status.clone()),
            task_payload: Set(task.task_payload.clone()),
            task_result: Set(task.task_result.clone()),
            retry_count: Set(task.retry_count),
            revision: Set(task.revision),
            lease_token: Set(task.lease_token),
            next_execute_at: Set(task.next_execute_at),
            lease_expire_at: Set(task.lease_expire_at),
            last_error: Set(task.last_error.clone()),
            gmt_create: Set(Some(now)),
            gmt_modified: Set(Some(now)),
        };

        // `gmt_create` is deliberately excluded so an upsert keeps the original
        // creation time.
        ai_resource_task::Entity::insert(model)
            .on_conflict(
                // PostgreSQL requires an explicit conflict target; MySQL
                // ignores it and relies on the primary key.
                OnConflict::column(ai_resource_task::Column::TaskKey)
                    .update_columns([
                        ai_resource_task::Column::NamespaceId,
                        ai_resource_task::Column::TaskType,
                        ai_resource_task::Column::TaskStage,
                        ai_resource_task::Column::Status,
                        ai_resource_task::Column::TaskPayload,
                        ai_resource_task::Column::TaskResult,
                        ai_resource_task::Column::RetryCount,
                        ai_resource_task::Column::Revision,
                        ai_resource_task::Column::LeaseToken,
                        ai_resource_task::Column::NextExecuteAt,
                        ai_resource_task::Column::LeaseExpireAt,
                        ai_resource_task::Column::LastError,
                        ai_resource_task::Column::GmtModified,
                    ])
                    .to_owned(),
            )
            .exec(&self.db)
            .await?;

        Ok(())
    }

    async fn task_find(&self, task_key: &str) -> anyhow::Result<Option<AiResourceTaskInfo>> {
        let result = ai_resource_task::Entity::find_by_id(task_key.to_string())
            .one(&self.db)
            .await?
            .map(task_model_to_info);

        Ok(result)
    }

    async fn task_find_due(
        &self,
        task_type: &str,
        now_millis: i64,
        limit: u64,
    ) -> anyhow::Result<Vec<AiResourceTaskInfo>> {
        let items = ai_resource_task::Entity::find()
            .filter(ai_resource_task::Column::TaskType.eq(task_type))
            .filter(ai_resource_task::Column::NextExecuteAt.lte(now_millis))
            .filter(
                Condition::any()
                    .add(ai_resource_task::Column::LeaseExpireAt.is_null())
                    .add(ai_resource_task::Column::LeaseExpireAt.lte(now_millis)),
            )
            .order_by_asc(ai_resource_task::Column::NextExecuteAt)
            .limit(limit)
            .all(&self.db)
            .await?
            .into_iter()
            .map(task_model_to_info)
            .collect();

        Ok(items)
    }

    async fn task_delete(&self, task_key: &str) -> anyhow::Result<u64> {
        let result = ai_resource_task::Entity::delete_many()
            .filter(ai_resource_task::Column::TaskKey.eq(task_key))
            .exec(&self.db)
            .await?;

        Ok(result.rows_affected)
    }

    async fn search_chunk_search(
        &self,
        namespace_id: &str,
        text: &str,
        resource_types: &[&str],
        limit: u64,
    ) -> anyhow::Result<Vec<AiResourceSearchHitInfo>> {
        if text.trim().is_empty() || limit == 0 {
            return Ok(Vec::new());
        }

        let backend = self.db.get_database_backend();
        let like = format!("%{}%", text.trim().to_lowercase());

        // Placeholder style differs: PostgreSQL numbers them, MySQL uses `?`.
        let mut sql = String::new();
        let mut values: Vec<sea_orm::Value> = Vec::new();
        let mut next_index = 1usize;
        let mut placeholder = |sql: &mut String| {
            match backend {
                DatabaseBackend::Postgres => {
                    sql.push_str(&format!("${next_index}"));
                }
                _ => sql.push('?'),
            }
            next_index += 1;
        };

        // A bare CASE yields DECIMAL on MySQL, which does not decode into f64;
        // the cast makes the column type explicit on both dialects.
        let double_type = match backend {
            DatabaseBackend::Postgres => "DOUBLE PRECISION",
            _ => "DOUBLE",
        };

        sql.push_str(
            "SELECT document_id, id AS chunk_id, resource_type, resource_name, \
             resource_version, chunk_type, CAST(CASE WHEN LOWER(canonical_text) LIKE ",
        );
        placeholder(&mut sql);
        sql.push_str(" THEN 1.0 WHEN LOWER(chunk_text) LIKE ");
        placeholder(&mut sql);
        sql.push_str(" THEN 0.8 ELSE 0.4 END AS ");
        sql.push_str(double_type);
        sql.push_str(") AS score FROM ai_resource_search_chunk WHERE namespace_id = ");
        placeholder(&mut sql);
        sql.push_str(" AND status = ");
        placeholder(&mut sql);
        sql.push_str(" AND (LOWER(canonical_text) LIKE ");
        placeholder(&mut sql);
        sql.push_str(" OR LOWER(chunk_text) LIKE ");
        placeholder(&mut sql);
        sql.push(')');

        values.push(like.clone().into());
        values.push(like.clone().into());
        values.push(namespace_id.to_string().into());
        values.push(search_status_enabled().into());
        values.push(like.clone().into());
        values.push(like.clone().into());

        if !resource_types.is_empty() {
            sql.push_str(" AND resource_type IN (");
            for (i, resource_type) in resource_types.iter().enumerate() {
                if i > 0 {
                    sql.push_str(", ");
                }
                placeholder(&mut sql);
                values.push((*resource_type).to_string().into());
            }
            sql.push(')');
        }

        sql.push_str(" ORDER BY score DESC LIMIT ");
        placeholder(&mut sql);
        values.push(limit.into());

        let rows = self
            .db
            .query_all_raw(Statement::from_sql_and_values(backend, &sql, values))
            .await?;

        let mut hits = Vec::with_capacity(rows.len());
        for row in rows {
            hits.push(AiResourceSearchHitInfo {
                document_id: row.try_get("", "document_id")?,
                chunk_id: row.try_get("", "chunk_id")?,
                resource_type: row.try_get("", "resource_type")?,
                resource_name: row.try_get("", "resource_name")?,
                resource_version: row.try_get("", "resource_version")?,
                chunk_type: row.try_get("", "chunk_type")?,
                score: row.try_get("", "score")?,
            });
        }

        Ok(hits)
    }
}

/// Status of chunks eligible for search.
///
/// Kept as a named function so the raw SQL and the entity-based code cannot
/// drift apart.
fn search_status_enabled() -> &'static str {
    "enabled"
}
