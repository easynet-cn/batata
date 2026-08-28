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
    /// List soft-deleted items for a namespace (RocksDB + SQL).
    async fn list_deleted_items(&self, namespace_id: i32) -> anyhow::Result<Vec<StoredItem>>;
    /// Distinct namespace ids holding a live item with this key, across all
    /// namespaces — upstream `itemService.findItemsByKey` + id-set extraction
    /// used by `/namespaces/find-by-item`.
    async fn find_namespace_ids_by_item_key(&self, key: &str) -> anyhow::Result<Vec<i32>>;
}