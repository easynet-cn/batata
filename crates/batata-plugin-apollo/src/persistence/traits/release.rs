use async_trait::async_trait;

use crate::persistence::shared::StoredRelease;

#[async_trait]
/// Defines the `ReleasePersistence` trait.
pub trait ReleasePersistence: Send + Sync {
    /// Performs the `create` operation.
    async fn create(&self, release: StoredRelease) -> anyhow::Result<StoredRelease>;
    /// Returns the requested value.
    async fn get_by_id(&self, id: i64) -> anyhow::Result<Option<StoredRelease>>;
    /// Returns the requested value.
    async fn get_latest(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<Option<StoredRelease>>;
    /// Returns the requested value.
    async fn list_by_namespace(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<Vec<StoredRelease>>;
    /// Performs the `delete` operation.
    async fn delete(&self, id: i64) -> anyhow::Result<()>;
    /// Returns the requested value.
    async fn get_by_release_id(&self, release_id: i64) -> anyhow::Result<Option<StoredRelease>>;
    /// Persist mutations to an existing row (e.g. upstream sets
    /// `IsAbandoned=true` on rollback). Must keep secondary indexes consistent.
    async fn update(&self, release: StoredRelease) -> anyhow::Result<StoredRelease>;
    /// Active (not abandoned, not deleted) releases for one namespace,
    /// newest id first — upstream `findFirst500ByAppId...AndIsAbandonedFalse
    /// OrderByIdDesc` family used by rollback.
    async fn list_active(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<Vec<StoredRelease>>;
}