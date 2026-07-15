//! Embedded implementation of InstancePersistence trait

use crate::bincode;
use std::sync::Arc;

use async_trait::async_trait;
use rocksdb::DB;

use batata_consistency::raft::state_machine::CF_APOLLO_INSTANCE;

use crate::persistence::shared::StoredInstance;
use crate::persistence::traits::InstancePersistence;
use super::id_generator::IdGenerator;

/// Embedded Instance persistence using RocksDB
pub struct InstanceEmbedded {
    db: Arc<DB>,
    id_gen: Arc<IdGenerator>,
}

impl InstanceEmbedded {
    /// Create from RocksDB
    pub fn new(db: Arc<DB>, id_gen: Arc<IdGenerator>) -> Self {
        Self { db, id_gen }
    }

    /// Get column family handle
    fn cf(&self) -> anyhow::Result<&rocksdb::ColumnFamily> {
        self.db
            .cf_handle(CF_APOLLO_INSTANCE)
            .ok_or_else(|| anyhow::anyhow!("Column family '{}' not found", CF_APOLLO_INSTANCE))
    }

    /// Build key for instance: "instance:{app_id}:{ip}"
    fn key(app_id: &str, ip: &str) -> String {
        format!("instance:{}:{}", app_id, ip)
    }

    /// Build prefix for listing by app: "instance:{app_id}:"
    fn prefix_by_app(app_id: &str) -> String {
        format!("instance:{}:", app_id)
    }
}

#[async_trait]
impl InstancePersistence for InstanceEmbedded {
    async fn upsert(&self, instance: StoredInstance) -> anyhow::Result<StoredInstance> {
        let cf = self.cf()?;
        let key = Self::key(&instance.app_id, &instance.ip);
        // Check if exists to preserve id
        let mut instance = instance;
        if let Some(data) = self.db.get_cf(cf, key.as_bytes())? {
            let existing: StoredInstance = bincode::deserialize(&data)?;
            instance.id = existing.id;
            instance.data_change_created_time = existing.data_change_created_time;
        } else {
            instance.id = self.id_gen.next_id();
        }
        instance.data_change_last_time = Some(chrono::Utc::now().timestamp_millis());

        let bytes = bincode::serialize(&instance)?;
        self.db
            .put_cf(cf, key.as_bytes(), &bytes)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;
        Ok(instance)
    }

    async fn get_by_app(
        &self,
        app_id: &str,
        cluster_name: Option<&str>,
    ) -> anyhow::Result<Vec<StoredInstance>> {
        let cf = self.cf()?;
        let prefix = Self::prefix_by_app(app_id);
        let mut results = Vec::new();
        let iter = self.db.prefix_iterator_cf(cf, prefix.as_bytes());
        for item in iter {
            let (key, value) = item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let key_str = String::from_utf8_lossy(&key);
            if !key_str.starts_with(&prefix) {
                break;
            }
            let instance: StoredInstance = bincode::deserialize(&value)?;
            // Filter by cluster if provided
            if let Some(cluster) = cluster_name {
                if instance.cluster_name != cluster {
                    continue;
                }
            }
            results.push(instance);
        }
        Ok(results)
    }

    async fn delete_expired(&self, before_timestamp: i64) -> anyhow::Result<usize> {
        let cf = self.cf()?;
        let mut keys_to_delete = Vec::new();
        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);
        for item in iter {
            let (key, value) = item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let instance: StoredInstance = bincode::deserialize(&value)?;
            let last_time = instance.data_change_last_time.unwrap_or(instance.data_change_created_time);
            if last_time < before_timestamp {
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

    async fn list_all(&self) -> anyhow::Result<Vec<StoredInstance>> {
        let cf = self.cf()?;
        let mut results = Vec::new();
        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);
        for item in iter {
            let (_, value) = item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let instance: StoredInstance = bincode::deserialize(&value)?;
            results.push(instance);
        }
        Ok(results)
    }
}