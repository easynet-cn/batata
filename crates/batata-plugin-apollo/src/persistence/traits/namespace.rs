use async_trait::async_trait;

use crate::persistence::shared::StoredNamespace;

#[async_trait]
/// Defines the `NamespacePersistence` trait.
pub trait NamespacePersistence: Send + Sync {
    /// Performs the `create` operation.
    async fn create(&self, namespace: StoredNamespace) -> anyhow::Result<StoredNamespace>;
    /// Performs the `get` operation.
    async fn get(&self, id: i32) -> anyhow::Result<Option<StoredNamespace>>;
    /// Returns the requested value.
    async fn get_by_app_cluster(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<Option<StoredNamespace>>;
    /// Returns the requested value.
    async fn list_by_app(&self, app_id: &str) -> anyhow::Result<Vec<StoredNamespace>>;
    /// Returns the requested value.
    async fn list_all(&self) -> anyhow::Result<Vec<StoredNamespace>>;
    /// Performs the `update` operation.
    async fn update(&self, namespace: StoredNamespace) -> anyhow::Result<StoredNamespace>;
    /// Performs the `delete` operation.
    async fn delete(&self, id: i32) -> anyhow::Result<()>;
}