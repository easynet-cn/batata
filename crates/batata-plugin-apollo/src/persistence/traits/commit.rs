use async_trait::async_trait;

use crate::persistence::shared::StoredCommit;

#[async_trait]
/// Defines the `CommitPersistence` trait.
pub trait CommitPersistence: Send + Sync {
    /// Performs the `create` operation.
    async fn create(&self, commit: StoredCommit) -> anyhow::Result<StoredCommit>;
    /// Returns the requested value.
    async fn get_by_id(&self, id: i32) -> anyhow::Result<Option<StoredCommit>>;
    /// Returns the requested value.
    async fn list_by_namespace(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<Vec<StoredCommit>>;
    /// Returns the requested value.
    async fn get_latest(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<Option<StoredCommit>>;
    /// Performs the `update` operation.
    async fn update(&self, commit: StoredCommit) -> anyhow::Result<StoredCommit>;
}