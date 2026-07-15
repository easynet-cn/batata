use async_trait::async_trait;

use crate::persistence::shared::StoredNamespaceLock;

#[async_trait]
pub trait NamespaceLockPersistence: Send + Sync {
    async fn lock(&self, lock: StoredNamespaceLock) -> anyhow::Result<StoredNamespaceLock>;
    async fn unlock(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<()>;
    async fn get(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<Option<StoredNamespaceLock>>;
    async fn is_locked(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<bool>;
}