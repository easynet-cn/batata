use async_trait::async_trait;

use crate::persistence::shared::StoredInstance;

#[async_trait]
/// Defines the `InstancePersistence` trait.
pub trait InstancePersistence: Send + Sync {
    /// Performs the `upsert` operation.
    async fn upsert(&self, instance: StoredInstance) -> anyhow::Result<StoredInstance>;
    /// Returns the requested value.
    async fn get_by_app(
        &self,
        app_id: &str,
        cluster_name: Option<&str>,
    ) -> anyhow::Result<Vec<StoredInstance>>;
    /// Deletes the specified resource.
    async fn delete_expired(&self, before_timestamp: i64) -> anyhow::Result<usize>;
    /// Returns the requested value.
    async fn list_all(&self) -> anyhow::Result<Vec<StoredInstance>>;
    /// Looks up a single instance by its primary key.
    async fn get_instance_by_id(&self, id: i64) -> anyhow::Result<Option<StoredInstance>>;
}