use async_trait::async_trait;

use crate::persistence::shared::StoredCluster;

#[async_trait]
/// Defines the `ClusterPersistence` trait.
pub trait ClusterPersistence: Send + Sync {
    /// Performs the `create` operation.
    async fn create(&self, cluster: StoredCluster) -> anyhow::Result<StoredCluster>;
    /// Performs the `get` operation.
    async fn get(&self, app_id: &str, cluster_name: &str) -> anyhow::Result<Option<StoredCluster>>;
    /// Performs the `list` operation.
    async fn list(&self, app_id: &str) -> anyhow::Result<Vec<StoredCluster>>;
    /// Performs the `update` operation.
    async fn update(&self, cluster: StoredCluster) -> anyhow::Result<StoredCluster>;
    /// Performs the `delete` operation.
    async fn delete(&self, app_id: &str, cluster_name: &str) -> anyhow::Result<()>;
}
