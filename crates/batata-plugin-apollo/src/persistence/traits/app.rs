use async_trait::async_trait;

use crate::persistence::shared::StoredApp;

#[async_trait]
pub trait AppPersistence: Send + Sync {
    async fn create(&self, app: StoredApp) -> anyhow::Result<StoredApp>;
    async fn get(&self, app_id: &str) -> anyhow::Result<Option<StoredApp>>;
    async fn get_by_ids(&self, app_ids: &[String]) -> anyhow::Result<Vec<StoredApp>>;
    async fn list(&self) -> anyhow::Result<Vec<StoredApp>>;
    async fn update(&self, app: StoredApp) -> anyhow::Result<StoredApp>;
    async fn delete(&self, app_id: &str) -> anyhow::Result<()>;
}