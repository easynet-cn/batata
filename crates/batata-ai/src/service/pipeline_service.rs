//! Pipeline query service — reads pipeline_execution table

use std::sync::Arc;

use batata_persistence::entity::pipeline_execution;
use batata_persistence::model::Page;
use batata_persistence::sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, Order, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect,
};

use crate::model::pipeline::*;

/// Pipeline query service (read-only, pipeline execution is managed by the publish workflow)
pub struct PipelineQueryService {
    db: Arc<DatabaseConnection>,
}

impl PipelineQueryService {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    /// Get a single pipeline execution by ID
    pub async fn get_pipeline(
        &self,
        execution_id: &str,
    ) -> anyhow::Result<Option<PipelineExecution>> {
        let record = pipeline_execution::Entity::find_by_id(execution_id.to_string())
            .one(self.db.as_ref())
            .await?;

        Ok(record.map(Self::model_to_execution))
    }

    /// List pipeline executions with pagination and filters
    pub async fn list_pipelines(
        &self,
        resource_type: &str,
        resource_name: Option<&str>,
        namespace_id: Option<&str>,
        version: Option<&str>,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<PipelineExecution>> {
        let mut query = pipeline_execution::Entity::find()
            .filter(pipeline_execution::Column::ResourceType.eq(resource_type));

        if let Some(name) = resource_name
            && !name.is_empty()
        {
            query = query.filter(pipeline_execution::Column::ResourceName.eq(name));
        }
        if let Some(ns) = namespace_id
            && !ns.is_empty()
        {
            query = query.filter(pipeline_execution::Column::NamespaceId.eq(ns));
        }
        if let Some(v) = version
            && !v.is_empty()
        {
            query = query.filter(pipeline_execution::Column::Version.eq(v));
        }

        query = query.order_by(pipeline_execution::Column::CreateTime, Order::Desc);

        let total = query.clone().count(self.db.as_ref()).await?;
        let offset = (page_no.saturating_sub(1)) * page_size;

        let records = query
            .offset(offset)
            .limit(page_size)
            .all(self.db.as_ref())
            .await?;

        let items: Vec<PipelineExecution> =
            records.into_iter().map(Self::model_to_execution).collect();

        Ok(Page::new(total, page_no, page_size, items))
    }

    fn model_to_execution(m: pipeline_execution::Model) -> PipelineExecution {
        let pipeline: Vec<PipelineNodeResult> =
            serde_json::from_str(&m.pipeline).unwrap_or_default();

        PipelineExecution {
            execution_id: m.execution_id,
            resource_type: m.resource_type,
            resource_name: m.resource_name,
            namespace_id: m.namespace_id,
            version: m.version,
            status: m.status,
            pipeline,
            create_time: m.create_time,
            update_time: m.update_time,
        }
    }
}
