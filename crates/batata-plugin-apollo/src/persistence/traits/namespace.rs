use async_trait::async_trait;

use crate::persistence::shared::StoredNamespace;

#[async_trait]
pub trait NamespacePersistence: Send + Sync {
    async fn create(&self, namespace: StoredNamespace) -> anyhow::Result<StoredNamespace>;
    async fn get(&self, id: i32) -> anyhow::Result<Option<StoredNamespace>>;
    async fn get_by_app_cluster(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<Option<StoredNamespace>>;
    async fn list_by_app(&self, app_id: &str) -> anyhow::Result<Vec<StoredNamespace>>;
    async fn update(&self, namespace: StoredNamespace) -> anyhow::Result<StoredNamespace>;
    async fn delete(&self, id: i32) -> anyhow::Result<()>;
}