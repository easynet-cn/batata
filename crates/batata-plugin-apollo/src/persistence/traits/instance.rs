use async_trait::async_trait;

use crate::persistence::shared::StoredInstance;

#[async_trait]
pub trait InstancePersistence: Send + Sync {
    async fn upsert(&self, instance: StoredInstance) -> anyhow::Result<StoredInstance>;
    async fn get_by_app(
        &self,
        app_id: &str,
        cluster_name: Option<&str>,
    ) -> anyhow::Result<Vec<StoredInstance>>;
    async fn delete_expired(&self, before_timestamp: i64) -> anyhow::Result<usize>;
    async fn list_all(&self) -> anyhow::Result<Vec<StoredInstance>>;
}