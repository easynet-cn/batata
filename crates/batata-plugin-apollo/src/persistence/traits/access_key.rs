use async_trait::async_trait;

use crate::persistence::shared::StoredAccessKey;

#[async_trait]
/// Defines the `AccessKeyPersistence` trait.
pub trait AccessKeyPersistence: Send + Sync {
    /// Performs the `create` operation.
    async fn create(&self, access_key: StoredAccessKey) -> anyhow::Result<StoredAccessKey>;
    /// Returns the requested value.
    async fn get_by_app(&self, app_id: &str) -> anyhow::Result<Vec<StoredAccessKey>>;
    /// Returns the requested value.
    async fn get_by_secret(&self, secret: &str) -> anyhow::Result<Option<StoredAccessKey>>;
    /// Performs the `update` operation.
    async fn update(&self, access_key: StoredAccessKey) -> anyhow::Result<StoredAccessKey>;
    /// Performs the `delete` operation.
    async fn delete(&self, id: i64) -> anyhow::Result<()>;
}