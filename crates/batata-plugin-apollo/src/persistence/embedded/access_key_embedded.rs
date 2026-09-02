//! Embedded implementation of AccessKeyPersistence trait

use crate::bincode;
use std::sync::Arc;

use async_trait::async_trait;
use rocksdb::DB;

use batata_consistency::raft::state_machine::CF_APOLLO_ACCESS_KEY;

use crate::persistence::shared::StoredAccessKey;
use crate::persistence::traits::AccessKeyPersistence;
use super::id_generator::IdGenerator;

/// Embedded Access Key persistence using RocksDB
pub struct AccessKeyEmbedded {
    db: Arc<DB>,
    id_gen: Arc<IdGenerator>,
}

impl AccessKeyEmbedded {
    /// Create from RocksDB
    pub fn new(db: Arc<DB>, id_gen: Arc<IdGenerator>) -> Self {
        Self { db, id_gen }
    }

    /// Get column family handle
    fn cf(&self) -> anyhow::Result<&rocksdb::ColumnFamily> {
        self.db
            .cf_handle(CF_APOLLO_ACCESS_KEY)
            .ok_or_else(|| anyhow::anyhow!("Column family '{}' not found", CF_APOLLO_ACCESS_KEY))
    }

    /// Build key for access key by app and id: "ak:{app_id}:{id}"
    fn key(app_id: &str, id: i64) -> String {
        format!("ak:{}:{}", app_id, id)
    }

    /// Build key for access key by id only: "ak_id:{id}"
    fn key_by_id(id: i64) -> String {
        format!("ak_id:{}", id)
    }

    /// Build key for secret lookup: "ak_secret:{secret}"
    fn key_by_secret(secret: &str) -> String {
        format!("ak_secret:{}", secret)
    }

    /// Build prefix for listing by app: "ak:{app_id}:"
    fn prefix_by_app(app_id: &str) -> String {
        format!("ak:{}:", app_id)
    }

    /// Writes a fully-formed `StoredAccessKey` (including its `id`) to RocksDB.
    ///
    /// Deterministic write path shared by the local `create` (`id` from
    /// `IdGenerator`) and the Raft apply phase (`id` derived from the Raft
    /// `log_index`). Operates on a `&DB` so it can be called from the Raft
    /// state machine.
    pub fn write_raw(db: &DB, access_key: &StoredAccessKey) -> anyhow::Result<StoredAccessKey> {
        let cf = db
            .cf_handle(CF_APOLLO_ACCESS_KEY)
            .ok_or_else(|| anyhow::anyhow!("CF {} not found", CF_APOLLO_ACCESS_KEY))?;
        let bytes = bincode::serialize(access_key)?;

        // Store by id
        let key_id = Self::key_by_id(access_key.id);
        db.put_cf(cf, key_id.as_bytes(), &bytes)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;

        // Store by app and id
        let key_app = Self::key(&access_key.app_id, access_key.id);
        db.put_cf(cf, key_app.as_bytes(), &bytes)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;

        // Store by secret for lookup
        let key_secret = Self::key_by_secret(&access_key.secret);
        db.put_cf(cf, key_secret.as_bytes(), &bytes)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;

        Ok(access_key.clone())
    }

    /// Deletes an access key by `id` (removes id/app/secret keys).
    /// Deterministic counterpart to `write_raw`.
    pub fn delete_raw(db: &DB, id: i64) -> anyhow::Result<()> {
        let cf = db
            .cf_handle(CF_APOLLO_ACCESS_KEY)
            .ok_or_else(|| anyhow::anyhow!("CF {} not found", CF_APOLLO_ACCESS_KEY))?;
        // First get to find all keys
        let key_id = Self::key_by_id(id);
        if let Some(data) = db.get_cf(cf, key_id.as_bytes())? {
            let ak: StoredAccessKey = bincode::deserialize(&data)?;
            let key_app = Self::key(&ak.app_id, ak.id);
            let key_secret = Self::key_by_secret(&ak.secret);
            // Delete all keys
            db.delete_cf(cf, key_id.as_bytes())
                .map_err(|e| anyhow::anyhow!("RocksDB delete error: {}", e))?;
            db.delete_cf(cf, key_app.as_bytes())
                .map_err(|e| anyhow::anyhow!("RocksDB delete error: {}", e))?;
            db.delete_cf(cf, key_secret.as_bytes())
                .map_err(|e| anyhow::anyhow!("RocksDB delete error: {}", e))?;
        }
        Ok(())
    }
}

#[async_trait]
impl AccessKeyPersistence for AccessKeyEmbedded {
    async fn create(&self, access_key: StoredAccessKey) -> anyhow::Result<StoredAccessKey> {
        let mut access_key = access_key;
        access_key.id = self.id_gen.next_id();
        Self::write_raw(&self.db, &access_key)
    }

    async fn get_by_app(&self, app_id: &str) -> anyhow::Result<Vec<StoredAccessKey>> {
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
            let ak: StoredAccessKey = bincode::deserialize(&value)?;
            // Only return non-deleted and enabled
            if !ak.is_deleted && ak.is_enabled {
                results.push(ak);
            }
        }
        Ok(results)
    }

    async fn get_by_secret(&self, secret: &str) -> anyhow::Result<Option<StoredAccessKey>> {
        let cf = self.cf()?;
        let key = Self::key_by_secret(secret);
        match self.db.get_cf(cf, key.as_bytes())? {
            Some(data) => {
                let ak: StoredAccessKey = bincode::deserialize(&data)?;
                // Only return non-deleted and enabled
                if !ak.is_deleted && ak.is_enabled {
                    Ok(Some(ak))
                } else {
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }

    async fn update(&self, access_key: StoredAccessKey) -> anyhow::Result<StoredAccessKey> {
        let cf = self.cf()?;
        // Check existence
        let key_id = Self::key_by_id(access_key.id);
        if self.db.get_cf(cf, key_id.as_bytes())?.is_none() {
            anyhow::bail!("Access key '{}' not found", access_key.id);
        }
        let bytes = bincode::serialize(&access_key)?;

        // Update all keys
        self.db
            .put_cf(cf, key_id.as_bytes(), &bytes)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;

        let key_app = Self::key(&access_key.app_id, access_key.id);
        self.db
            .put_cf(cf, key_app.as_bytes(), &bytes)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;

        let key_secret = Self::key_by_secret(&access_key.secret);
        self.db
            .put_cf(cf, key_secret.as_bytes(), &bytes)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;

        Ok(access_key)
    }

    async fn delete(&self, id: i64) -> anyhow::Result<()> {
        Self::delete_raw(&self.db, id)
    }
}