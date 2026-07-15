use async_trait::async_trait;

use crate::persistence::shared::StoredRelease;

#[async_trait]
pub trait ReleasePersistence: Send + Sync {
    async fn create(&self, release: StoredRelease) -> anyhow::Result<StoredRelease>;
    async fn get_by_id(&self, id: i32) -> anyhow::Result<Option<StoredRelease>>;
    async fn get_latest(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<Option<StoredRelease>>;
    async fn list_by_namespace(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<Vec<StoredRelease>>;
    async fn delete(&self, id: i32) -> anyhow::Result<()>;
    async fn get_by_release_id(&self, release_id: i64) -> anyhow::Result<Option<StoredRelease>>;
}