use async_trait::async_trait;

use crate::persistence::shared::StoredGrayReleaseRule;

#[async_trait]
/// Defines the `GrayReleasePersistence` trait.
pub trait GrayReleasePersistence: Send + Sync {
    /// Performs the `create` operation.
    async fn create(&self, rule: StoredGrayReleaseRule) -> anyhow::Result<StoredGrayReleaseRule>;
    /// Returns the requested value.
    async fn get_by_namespace(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<Option<StoredGrayReleaseRule>>;
    /// Updates an existing resource.
    async fn update_rules(
        &self,
        id: i64,
        rules: String,
        release_id: i64,
    ) -> anyhow::Result<StoredGrayReleaseRule>;
    /// Performs the `delete` operation.
    async fn delete(&self, id: i64) -> anyhow::Result<()>;
    /// Returns the requested value.
    async fn list_by_app(&self, app_id: &str) -> anyhow::Result<Vec<StoredGrayReleaseRule>>;
}