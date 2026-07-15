use async_trait::async_trait;

use crate::persistence::shared::StoredItem;

#[async_trait]
pub trait ItemPersistence: Send + Sync {
    async fn create(&self, item: StoredItem) -> anyhow::Result<StoredItem>;
    async fn get_by_key(&self, namespace_id: i32, key: &str) -> anyhow::Result<Option<StoredItem>>;
    async fn get_by_id(&self, id: i32) -> anyhow::Result<Option<StoredItem>>;
    async fn list_by_namespace(&self, namespace_id: i32) -> anyhow::Result<Vec<StoredItem>>;
    async fn update(&self, item: StoredItem) -> anyhow::Result<StoredItem>;
    async fn delete(&self, id: i32) -> anyhow::Result<()>;
    async fn batch_create(&self, items: Vec<StoredItem>) -> anyhow::Result<Vec<StoredItem>>;
}