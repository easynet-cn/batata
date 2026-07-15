use async_trait::async_trait;

use crate::persistence::shared::StoredAccessKey;

#[async_trait]
pub trait AccessKeyPersistence: Send + Sync {
    async fn create(&self, access_key: StoredAccessKey) -> anyhow::Result<StoredAccessKey>;
    async fn get_by_app(&self, app_id: &str) -> anyhow::Result<Vec<StoredAccessKey>>;
    async fn get_by_secret(&self, secret: &str) -> anyhow::Result<Option<StoredAccessKey>>;
    async fn update(&self, access_key: StoredAccessKey) -> anyhow::Result<StoredAccessKey>;
    async fn delete(&self, id: i32) -> anyhow::Result<()>;
}