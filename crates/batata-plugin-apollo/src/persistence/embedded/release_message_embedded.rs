//! Embedded implementation of ReleaseMessagePersistence trait.
//!
//! Layout inside `CF_APOLLO_RELEASE_MSG` (mirrors upstream `ReleaseMessage`
//! table where a background cleaner keeps only the newest row per key):
//!
//! - `rm_id:{id}`              → StoredReleaseMessage (row index, drives scans)
//! - `rm:{appId}+{cluster}+{namespace}` → StoredReleaseMessage (latest-per-key)
//!
//! `create` overwrites the composite key and prunes the previous row's
//! `rm_id` index entry, so exactly one row per watch key is retained.

use std::sync::Arc;

use async_trait::async_trait;
use rocksdb::DB;

use batata_consistency::raft::state_machine::CF_APOLLO_RELEASE_MSG;

use crate::persistence::shared::StoredReleaseMessage;
use crate::persistence::traits::ReleaseMessagePersistence;
use super::id_generator::IdGenerator;
use super::store::JsonStore;

const ROW_PREFIX: &str = "rm_id:";
const KEY_PREFIX: &str = "rm:";

/// Represents the `ReleaseMessageEmbedded` entity.
pub struct ReleaseMessageEmbedded {
    db: Arc<DB>,
    id_gen: Arc<IdGenerator>,
}

impl ReleaseMessageEmbedded {
    /// Creates a new `ReleaseMessageEmbedded`.
    pub fn new(db: Arc<DB>, id_gen: Arc<IdGenerator>) -> Self {
        Self { db, id_gen }
    }

    fn cf(&self) -> anyhow::Result<&rocksdb::ColumnFamily> {
        self.db
            .cf_handle(CF_APOLLO_RELEASE_MSG)
            .ok_or_else(|| anyhow::anyhow!("Column family '{}' not found", CF_APOLLO_RELEASE_MSG))
    }

    /// Composite key for one watch key: "rm:{app}+{cluster}+{namespace}".
    fn key(message: &str) -> String {
        format!("{}{}", KEY_PREFIX, message)
    }

    fn row_key(id: i64) -> String {
        format!("{}{}", ROW_PREFIX, id)
    }
}

#[async_trait]
impl ReleaseMessagePersistence for ReleaseMessageEmbedded {
    async fn create(&self, mut message: StoredReleaseMessage) -> anyhow::Result<StoredReleaseMessage> {
        let store = JsonStore::new(self.db.clone(), CF_APOLLO_RELEASE_MSG);

        // Prune the previous row for this watch key before overwriting it.
        let composite = Self::key(&message.message);
        if let Some(old) = store.get::<StoredReleaseMessage>(composite.as_bytes())? {
            store.delete(Self::row_key(old.id).as_bytes())?;
        }

        message.id = self.id_gen.next_id();
        store.put(composite.as_bytes(), &message)?;
        store.put(Self::row_key(message.id).as_bytes(), &message)?;
        Ok(message)
    }

    async fn find_latest_by_message(
        &self,
        message: &str,
    ) -> anyhow::Result<Option<StoredReleaseMessage>> {
        // O(1): the composite key always holds the newest row for this key.
        JsonStore::new(self.db.clone(), CF_APOLLO_RELEASE_MSG)
            .get::<StoredReleaseMessage>(Self::key(message).as_bytes())
    }

    async fn get_latest(&self) -> anyhow::Result<Option<StoredReleaseMessage>> {
        Ok(self
            .list_all()
            .await?
            .into_iter()
            .max_by_key(|m| m.id))
    }

    async fn list_all(&self) -> anyhow::Result<Vec<StoredReleaseMessage>> {
        let cf = self.cf()?;
        let mut results = Vec::new();
        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);
        for item in iter {
            let (key, value) = item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            if !key.starts_with(ROW_PREFIX.as_bytes()) {
                continue; // skip composite-key duplicates
            }
            results.push(serde_json::from_slice::<StoredReleaseMessage>(&value)?);
        }
        results.sort_by(|a, b| b.id.cmp(&a.id));
        Ok(results)
    }

    async fn delete_by_id(&self, id: i64) -> anyhow::Result<()> {
        JsonStore::new(self.db.clone(), CF_APOLLO_RELEASE_MSG)
            .delete(Self::row_key(id).as_bytes())
    }

    async fn delete_old(&self, before_id: i64) -> anyhow::Result<usize> {
        let cf = self.cf()?;
        let mut keys_to_delete = Vec::new();
        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);
        for item in iter {
            let (key, value) = item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            if !key.starts_with(ROW_PREFIX.as_bytes()) {
                continue;
            }
            let msg: StoredReleaseMessage = serde_json::from_slice(&value)?;
            if msg.id < before_id {
                keys_to_delete.push(key.to_vec());
            }
        }
        let count = keys_to_delete.len();
        if count > 0 {
            let mut batch = rocksdb::WriteBatch::default();
            for key in &keys_to_delete {
                batch.delete_cf(cf, key);
            }
            self.db
                .write(batch)
                .map_err(|e| anyhow::anyhow!("RocksDB batch delete error: {}", e))?;
        }
        Ok(count)
    }
}
