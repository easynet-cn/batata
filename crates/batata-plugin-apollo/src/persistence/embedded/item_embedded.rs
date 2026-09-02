//! Embedded implementation of ItemPersistence trait

use crate::bincode;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use rocksdb::DB;

use batata_consistency::raft::state_machine::CF_APOLLO_ITEM;

use crate::persistence::shared::StoredItem;
use crate::persistence::traits::ItemPersistence;
use super::id_generator::IdGenerator;

/// Represents the `ItemEmbedded` entity.
pub struct ItemEmbedded {
    db: Arc<DB>,
    id_gen: Arc<IdGenerator>,
}

impl ItemEmbedded {
    /// Creates a new `ItemEmbedded`.
    pub fn new(db: Arc<DB>, id_gen: Arc<IdGenerator>) -> Self {
        Self { db, id_gen }
    }

    /// Get column family handle
    fn cf(&self) -> anyhow::Result<&rocksdb::ColumnFamily> {
        self.db
            .cf_handle(CF_APOLLO_ITEM)
            .ok_or_else(|| anyhow::anyhow!("Column family '{}' not found", CF_APOLLO_ITEM))
    }

    /// Build key for item by id: "item_id:{id}"
    fn key_by_id(id: i64) -> String {
        format!("item_id:{}", id)
    }

    /// Build key for item by namespace and key: "item:{namespace_id}:{key}"
    fn key(namespace_id: i64, key: &str) -> String {
        format!("item:{}:{}", namespace_id, key)
    }

    /// Build reverse index key for listing by namespace: "item_by_ns:{namespace_id}:{item_id}"
    fn index_key(namespace_id: i64, item_id: i64) -> String {
        format!("item_by_ns:{}:{}", namespace_id, item_id)
    }

    /// Build prefix for listing by namespace: "item_by_ns:{namespace_id}:"
    fn prefix_by_namespace(namespace_id: i64) -> String {
        format!("item_by_ns:{}:", namespace_id)
    }

    /// Writes a fully-formed `StoredItem` (including its `id`) to RocksDB.
    ///
    /// Deterministic write path shared by the local `create` (`id` from
    /// `IdGenerator`) and the Raft apply phase (`id` derived from the Raft
    /// `log_index`). Operates on a `&DB` so it can be called from the Raft
    /// state machine.
    pub fn write_raw(db: &DB, item: &StoredItem) -> anyhow::Result<StoredItem> {
        let cf = db
            .cf_handle(CF_APOLLO_ITEM)
            .ok_or_else(|| anyhow::anyhow!("CF {} not found", CF_APOLLO_ITEM))?;
        let bytes = bincode::serialize(item)?;

        // Store by id
        let key_id = Self::key_by_id(item.id);
        db.put_cf(cf, key_id.as_bytes(), &bytes)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;

        // Store by namespace and key
        let key_comp = Self::key(item.namespace_id, &item.key);
        db.put_cf(cf, key_comp.as_bytes(), &bytes)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;

        // Store reverse index for listing
        let index_key = Self::index_key(item.namespace_id, item.id);
        db.put_cf(cf, index_key.as_bytes(), &bytes)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;

        Ok(item.clone())
    }

    /// Soft-deletes an item by `id` (marks `is_deleted`, keeps it queryable via
    /// `list_deleted_items`). Deterministic counterpart to `write_raw`.
    pub fn delete_raw(db: &DB, id: i64) -> anyhow::Result<()> {
        let cf = db
            .cf_handle(CF_APOLLO_ITEM)
            .ok_or_else(|| anyhow::anyhow!("CF {} not found", CF_APOLLO_ITEM))?;
        let key_id = Self::key_by_id(id);
        if let Some(data) = db.get_cf(cf, key_id.as_bytes())? {
            let mut item: StoredItem = bincode::deserialize(&data)?;
            // Soft delete so the item remains queryable via list_deleted_items
            // (Apollo semantics: deleted items are tracked for the
            // `.../items/deleted` endpoint).
            item.is_deleted = true;
            item.deleted_at = Utc::now().timestamp_millis();
            let bytes = bincode::serialize(&item)?;
            let key_comp = Self::key(item.namespace_id, &item.key);
            let index_key = Self::index_key(item.namespace_id, item.id);
            db.put_cf(cf, key_id.as_bytes(), &bytes)
                .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;
            db.put_cf(cf, key_comp.as_bytes(), &bytes)
                .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;
            db.put_cf(cf, index_key.as_bytes(), &bytes)
                .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;
        }
        Ok(())
    }
}

#[async_trait]
impl ItemPersistence for ItemEmbedded {
    async fn create(&self, item: StoredItem) -> anyhow::Result<StoredItem> {
        let mut item = item;
        item.id = self.id_gen.next_id();
        Self::write_raw(&self.db, &item)
    }

    async fn get_by_key(&self, namespace_id: i64, key: &str) -> anyhow::Result<Option<StoredItem>> {
        let cf = self.cf()?;
        let comp_key = Self::key(namespace_id, key);
        match self.db.get_cf(cf, comp_key.as_bytes())? {
            Some(data) => {
                let item: StoredItem = bincode::deserialize(&data)?;
                // Only return non-deleted
                if !item.is_deleted {
                    Ok(Some(item))
                } else {
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }

    async fn get_by_id(&self, id: i64) -> anyhow::Result<Option<StoredItem>> {
        let cf = self.cf()?;
        let key = Self::key_by_id(id);
        match self.db.get_cf(cf, key.as_bytes())? {
            Some(data) => {
                let item: StoredItem = bincode::deserialize(&data)?;
                // Only return non-deleted
                if !item.is_deleted {
                    Ok(Some(item))
                } else {
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }

    async fn list_by_namespace(&self, namespace_id: i64) -> anyhow::Result<Vec<StoredItem>> {
        let cf = self.cf()?;
        let prefix = Self::prefix_by_namespace(namespace_id);
        let mut results = Vec::new();
        let iter = self.db.prefix_iterator_cf(cf, prefix.as_bytes());
        for item in iter {
            let (key, value) = item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let key_str = String::from_utf8_lossy(&key);
            if !key_str.starts_with(&prefix) {
                break;
            }
            let stored: StoredItem = bincode::deserialize(&value)?;
            // Only return non-deleted
            if !stored.is_deleted {
                results.push(stored);
            }
        }
        Ok(results)
    }

    async fn update(&self, item: StoredItem) -> anyhow::Result<StoredItem> {
        let cf = self.cf()?;
        // Check existence
        let key_id = Self::key_by_id(item.id);
        if self.db.get_cf(cf, key_id.as_bytes())?.is_none() {
            anyhow::bail!("Item '{}' not found", item.id);
        }
        let bytes = bincode::serialize(&item)?;

        // Update all keys
        self.db
            .put_cf(cf, key_id.as_bytes(), &bytes)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;

        let key_comp = Self::key(item.namespace_id, &item.key);
        self.db
            .put_cf(cf, key_comp.as_bytes(), &bytes)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;

        let index_key = Self::index_key(item.namespace_id, item.id);
        self.db
            .put_cf(cf, index_key.as_bytes(), &bytes)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;

        Ok(item)
    }

    async fn delete(&self, id: i64) -> anyhow::Result<()> {
        Self::delete_raw(&self.db, id)
    }

    async fn batch_create(&self, items: Vec<StoredItem>) -> anyhow::Result<Vec<StoredItem>> {
        let cf = self.cf()?;
        let mut batch = rocksdb::WriteBatch::default();
        let mut result_items = Vec::with_capacity(items.len());
        for item in items {
            let mut item = item;
            item.id = self.id_gen.next_id();
            let bytes = bincode::serialize(&item)?;
            // Store by id
            batch.put_cf(cf, Self::key_by_id(item.id).as_bytes(), &bytes);
            // Store by namespace and key
            batch.put_cf(cf, Self::key(item.namespace_id, &item.key).as_bytes(), &bytes);
            // Store reverse index
            batch.put_cf(cf, Self::index_key(item.namespace_id, item.id).as_bytes(), &bytes);
            result_items.push(item);
        }
        self.db
            .write(batch)
            .map_err(|e| anyhow::anyhow!("RocksDB batch write error: {}", e))?;
        Ok(result_items)
    }

    async fn find_namespace_ids_by_item_key(&self, key: &str) -> anyhow::Result<Vec<i64>> {
        let cf = self.cf()?;
        let mut ids = std::collections::HashSet::new();
        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);
        for item in iter {
            let (_, value) = item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let it: StoredItem = bincode::deserialize(&value)?;
            if !it.is_deleted && it.key == key {
                ids.insert(it.namespace_id);
            }
        }
        Ok(ids.into_iter().collect())
    }

    async fn list_deleted_items(&self, namespace_id: i64) -> anyhow::Result<Vec<StoredItem>> {
        let prefix = Self::prefix_by_namespace(namespace_id);
        let mut results = Vec::new();
        let iter = self.db.prefix_iterator_cf(self.cf()?, prefix.as_bytes());
        for item in iter {
            let (key, value) = item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let key_str = String::from_utf8_lossy(&key);
            if !key_str.starts_with(&prefix) {
                break;
            }
            let stored: StoredItem = bincode::deserialize(&value)?;
            if stored.is_deleted {
                results.push(stored);
            }
        }
        Ok(results)
    }
}