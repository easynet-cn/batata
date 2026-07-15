//! Embedded implementation of ReleaseMessagePersistence trait

use crate::bincode;
use std::sync::Arc;

use async_trait::async_trait;
use rocksdb::DB;

use batata_consistency::raft::state_machine::CF_APOLLO_RELEASE_MSG;

use crate::persistence::shared::StoredReleaseMessage;
use crate::persistence::traits::ReleaseMessagePersistence;
use super::id_generator::IdGenerator;

/// Embedded Release Message persistence using RocksDB
pub struct ReleaseMessageEmbedded {
    db: Arc<DB>,
    id_gen: Arc<IdGenerator>,
}

impl ReleaseMessageEmbedded {
    /// Create from RocksDB
    pub fn new(db: Arc<DB>, id_gen: Arc<IdGenerator>) -> Self {
        Self { db, id_gen }
    }

    /// Get column family handle
    fn cf(&self) -> anyhow::Result<&rocksdb::ColumnFamily> {
        self.db
            .cf_handle(CF_APOLLO_RELEASE_MSG)
            .ok_or_else(|| anyhow::anyhow!("Column family '{}' not found", CF_APOLLO_RELEASE_MSG))
    }

    /// Build key for release message: "rm:{app_id}:{cluster}:{namespace}"
    fn key(app_id: &str, cluster: &str, namespace: &str) -> String {
        format!("rm:{}:{}:{}", app_id, cluster, namespace)
    }

    /// Build key for release message by id: "rm_id:{id}"
    fn key_by_id(id: i32) -> String {
        format!("rm_id:{}", id)
    }
}

#[async_trait]
impl ReleaseMessagePersistence for ReleaseMessageEmbedded {
    async fn create(&self, message: StoredReleaseMessage) -> anyhow::Result<StoredReleaseMessage> {
        let cf = self.cf()?;
        let mut message = message;
        message.id = self.id_gen.next_id();
        let bytes = bincode::serialize(&message)?;

        // Store by id
        let key_id = Self::key_by_id(message.id);
        self.db
            .put_cf(cf, key_id.as_bytes(), &bytes)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;

        // Parse message to extract app_id, cluster, namespace (message format: "{app_id}+{cluster}+{namespace}")
        let parts: Vec<&str> = message.message.split('+').collect();
        if parts.len() == 3 {
            let key_comp = Self::key(parts[0], parts[1], parts[2]);
            self.db
                .put_cf(cf, key_comp.as_bytes(), &bytes)
                .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;
        }

        Ok(message)
    }

    async fn get_latest(&self) -> anyhow::Result<Option<StoredReleaseMessage>> {
        let cf = self.cf()?;
        // Iterate all and find the one with highest id
        let mut latest: Option<StoredReleaseMessage> = None;
        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);
        for item in iter {
            let (_, value) = item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let msg: StoredReleaseMessage = bincode::deserialize(&value)?;
            if latest.is_none() || msg.id > latest.as_ref().unwrap().id {
                latest = Some(msg);
            }
        }
        Ok(latest)
    }

    async fn list_all(&self) -> anyhow::Result<Vec<StoredReleaseMessage>> {
        let cf = self.cf()?;
        let mut results = Vec::new();
        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);
        for item in iter {
            let (_, value) = item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let msg: StoredReleaseMessage = bincode::deserialize(&value)?;
            results.push(msg);
        }
        // Sort by id descending
        results.sort_by(|a, b| b.id.cmp(&a.id));
        Ok(results)
    }

    async fn delete_old(&self, before_id: i32) -> anyhow::Result<usize> {
        let cf = self.cf()?;
        let mut keys_to_delete = Vec::new();
        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);
        for item in iter {
            let (key, value) = item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let msg: StoredReleaseMessage = bincode::deserialize(&value)?;
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