use async_trait::async_trait;

use crate::persistence::shared::StoredCluster;

#[async_trait]
pub trait ClusterPersistence: Send + Sync {
    async fn create(&self, cluster: StoredCluster) -> anyhow::Result<StoredCluster>;
    async fn get(&self, app_id: &str, cluster_name: &str) -> anyhow::Result<Option<StoredCluster>>;
    async fn list(&self, app_id: &str) -> anyhow::Result<Vec<StoredCluster>>;
    async fn update(&self, cluster: StoredCluster) -> anyhow::Result<StoredCluster>;
    async fn delete(&self, app_id: &str, cluster_name: &str) -> anyhow::Result<()>;
}
