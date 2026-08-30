use async_trait::async_trait;

use crate::persistence::shared::StoredNamespaceLock;

#[async_trait]
/// Defines the `NamespaceLockPersistence` trait.
pub trait NamespaceLockPersistence: Send + Sync {
    /// Performs the `lock` operation.
    async fn lock(&self, lock: StoredNamespaceLock) -> anyhow::Result<StoredNamespaceLock>;
    /// Performs the `unlock` operation.
    async fn unlock(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<()>;
    /// Performs the `get` operation.
    async fn get(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<Option<StoredNamespaceLock>>;
    /// Returns whether the condition holds.
    async fn is_locked(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<bool>;
}