use async_trait::async_trait;

use crate::persistence::shared::StoredCommit;

#[async_trait]
pub trait CommitPersistence: Send + Sync {
    async fn create(&self, commit: StoredCommit) -> anyhow::Result<StoredCommit>;
    async fn get_by_id(&self, id: i32) -> anyhow::Result<Option<StoredCommit>>;
    async fn list_by_namespace(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<Vec<StoredCommit>>;
    async fn get_latest(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<Option<StoredCommit>>;
    async fn update(&self, commit: StoredCommit) -> anyhow::Result<StoredCommit>;
}