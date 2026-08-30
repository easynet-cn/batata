use async_trait::async_trait;

use crate::persistence::shared::StoredReleaseMessage;

#[async_trait]
/// Defines the `ReleaseMessagePersistence` trait.
pub trait ReleaseMessagePersistence: Send + Sync {
    /// Performs the `create` operation.
    async fn create(&self, message: StoredReleaseMessage) -> anyhow::Result<StoredReleaseMessage>;
    /// Latest message row for one watch key ("appId+cluster+namespace").
    ///
    /// Mirrors upstream `ReleaseMessageRepository.findFirstByMessageOrderByIdDesc`.
    async fn find_latest_by_message(
        &self,
        message: &str,
    ) -> anyhow::Result<Option<StoredReleaseMessage>>;
    /// Returns the requested value.
    async fn get_latest(&self) -> anyhow::Result<Option<StoredReleaseMessage>>;
    /// Returns the requested value.
    async fn list_all(&self) -> anyhow::Result<Vec<StoredReleaseMessage>>;
    /// Delete one exact row by id.
    async fn delete_by_id(&self, id: i32) -> anyhow::Result<()>;
    /// Delete rows with id < before_id (upstream prunes old rows per key).
    async fn delete_old(&self, before_id: i32) -> anyhow::Result<usize>;
}
