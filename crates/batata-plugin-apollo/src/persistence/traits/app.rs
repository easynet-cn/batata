use async_trait::async_trait;

use crate::persistence::shared::StoredApp;

#[async_trait]
/// Defines the `AppPersistence` trait.
pub trait AppPersistence: Send + Sync {
    /// Performs the `create` operation.
    async fn create(&self, app: StoredApp) -> anyhow::Result<StoredApp>;
    /// Performs the `get` operation.
    async fn get(&self, app_id: &str) -> anyhow::Result<Option<StoredApp>>;
    /// Returns the requested value.
    async fn get_by_ids(&self, app_ids: &[String]) -> anyhow::Result<Vec<StoredApp>>;
    /// Performs the `list` operation.
    async fn list(&self) -> anyhow::Result<Vec<StoredApp>>;
    /// Performs the `update` operation.
    async fn update(&self, app: StoredApp) -> anyhow::Result<StoredApp>;
    /// Performs the `delete` operation.
    async fn delete(&self, app_id: &str) -> anyhow::Result<()>;
}