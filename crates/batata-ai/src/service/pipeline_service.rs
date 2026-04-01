//! Pipeline query service — reads pipeline executions via persistence trait

use std::sync::Arc;

use batata_persistence::PersistenceService;
use batata_persistence::model::{Page, PipelineExecutionInfo};

use crate::model::pipeline::*;

/// Pipeline query service (read-only, pipeline execution is managed by the publish workflow)
pub struct PipelineQueryService {
    persistence: Arc<dyn PersistenceService>,
}

impl PipelineQueryService {
    pub fn new(persistence: Arc<dyn PersistenceService>) -> Self {
        Self { persistence }
    }

    /// Get a single pipeline execution by ID
    pub async fn get_pipeline(
        &self,
        execution_id: &str,
    ) -> anyhow::Result<Option<PipelineExecution>> {
        let record = self
            .persistence
            .pipeline_execution_find(execution_id)
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
        let page = self
            .persistence
            .pipeline_execution_list(
                resource_type,
                resource_name,
                namespace_id,
                version,
                page_no,
                page_size,
            )
            .await?;

        let items: Vec<PipelineExecution> = page
            .page_items
            .into_iter()
            .map(Self::model_to_execution)
            .collect();

        Ok(Page::new(page.total_count, page_no, page_size, items))
    }

    fn model_to_execution(m: PipelineExecutionInfo) -> PipelineExecution {
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

#[async_trait::async_trait]
impl super::traits::PipelineService for PipelineQueryService {
    async fn get_pipeline(&self, execution_id: &str) -> anyhow::Result<Option<PipelineExecution>> {
        self.get_pipeline(execution_id).await
    }

    async fn list_pipelines(
        &self,
        resource_type: &str,
        resource_name: Option<&str>,
        namespace_id: Option<&str>,
        version: Option<&str>,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<batata_api::model::Page<PipelineExecution>> {
        let p = self
            .list_pipelines(
                resource_type,
                resource_name,
                namespace_id,
                version,
                page_no,
                page_size,
            )
            .await?;
        Ok(batata_api::model::Page {
            total_count: p.total_count,
            page_number: p.page_number,
            pages_available: p.pages_available,
            page_items: p.page_items,
        })
    }
}
