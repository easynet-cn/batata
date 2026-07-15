use async_trait::async_trait;

use crate::persistence::shared::StoredReleaseMessage;

#[async_trait]
pub trait ReleaseMessagePersistence: Send + Sync {
    async fn create(&self, message: StoredReleaseMessage) -> anyhow::Result<StoredReleaseMessage>;
    async fn get_latest(&self) -> anyhow::Result<Option<StoredReleaseMessage>>;
    async fn list_all(&self) -> anyhow::Result<Vec<StoredReleaseMessage>>;
    async fn delete_old(&self, before_id: i32) -> anyhow::Result<usize>;
}