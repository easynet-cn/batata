//! Embedded implementation of GrayReleasePersistence trait

use crate::bincode;
use std::sync::Arc;

use async_trait::async_trait;
use rocksdb::DB;

use batata_consistency::raft::state_machine::CF_APOLLO_GRAY_RULE;

use crate::persistence::shared::StoredGrayReleaseRule;
use crate::persistence::traits::GrayReleasePersistence;
use super::id_generator::IdGenerator;

/// Embedded Gray Release Rule persistence using RocksDB
pub struct GrayReleaseEmbedded {
    db: Arc<DB>,
    id_gen: Arc<IdGenerator>,
}

impl GrayReleaseEmbedded {
    /// Create from RocksDB
    pub fn new(db: Arc<DB>, id_gen: Arc<IdGenerator>) -> Self {
        Self { db, id_gen }
    }

    /// Get column family handle
    fn cf(&self) -> anyhow::Result<&rocksdb::ColumnFamily> {
        self.db
            .cf_handle(CF_APOLLO_GRAY_RULE)
            .ok_or_else(|| anyhow::anyhow!("Column family '{}' not found", CF_APOLLO_GRAY_RULE))
    }

    /// Build key for gray rule by namespace: "gray:{app_id}:{cluster}:{namespace}"
    fn key(app_id: &str, cluster: &str, namespace: &str) -> String {
        format!("gray:{}:{}:{}", app_id, cluster, namespace)
    }

    /// Build key for gray rule by id: "gray_id:{id}"
    fn key_by_id(id: i64) -> String {
        format!("gray_id:{}", id)
    }

    /// Build prefix for listing by app: "gray:{app_id}:"
    fn prefix_by_app(app_id: &str) -> String {
        format!("gray:{}:", app_id)
    }

    /// Writes a fully-formed `StoredGrayReleaseRule` (including its `id`) to
    /// RocksDB, writing both the id key and the namespace composite key.
    ///
    /// Deterministic write path shared by the local `create`/`update_rules`
    /// (`id` from `IdGenerator`) and the Raft apply phase (`id` derived from
    /// the Raft `log_index`). Operates on a `&DB` so it can be called from the
    /// Raft state machine.
    pub fn write_raw(db: &DB, rule: &StoredGrayReleaseRule) -> anyhow::Result<StoredGrayReleaseRule> {
        let cf = db
            .cf_handle(CF_APOLLO_GRAY_RULE)
            .ok_or_else(|| anyhow::anyhow!("CF {} not found", CF_APOLLO_GRAY_RULE))?;
        let bytes = bincode::serialize(rule)?;

        // Store by id
        let key_id = Self::key_by_id(rule.id);
        db.put_cf(cf, key_id.as_bytes(), &bytes)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;

        // Store by namespace composite key
        let key_comp = Self::key(&rule.app_id, &rule.cluster_name, &rule.namespace_name);
        db.put_cf(cf, key_comp.as_bytes(), &bytes)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;

        Ok(rule.clone())
    }

    /// Reads a gray release rule by `id` (used to reconstruct updates before
    /// replication). Operates on a `&DB` so it can be called from the Raft path.
    pub fn get_by_id(db: &DB, id: i64) -> anyhow::Result<StoredGrayReleaseRule> {
        let cf = db
            .cf_handle(CF_APOLLO_GRAY_RULE)
            .ok_or_else(|| anyhow::anyhow!("CF {} not found", CF_APOLLO_GRAY_RULE))?;
        let key_id = Self::key_by_id(id);
        let data = db
            .get_cf(cf, key_id.as_bytes())?
            .ok_or_else(|| anyhow::anyhow!("Gray release rule '{}' not found", id))?;
        let rule: StoredGrayReleaseRule = bincode::deserialize(&data)?;
        Ok(rule)
    }

    /// Deletes a gray release rule by `id` (removes both id and composite keys).
    /// Deterministic counterpart to `write_raw`.
    pub fn delete_raw(db: &DB, id: i64) -> anyhow::Result<()> {
        let cf = db
            .cf_handle(CF_APOLLO_GRAY_RULE)
            .ok_or_else(|| anyhow::anyhow!("CF {} not found", CF_APOLLO_GRAY_RULE))?;
        let key_id = Self::key_by_id(id);
        if let Some(data) = db.get_cf(cf, key_id.as_bytes())? {
            let rule: StoredGrayReleaseRule = bincode::deserialize(&data)?;
            let key_comp = Self::key(&rule.app_id, &rule.cluster_name, &rule.namespace_name);
            db.delete_cf(cf, key_id.as_bytes())
                .map_err(|e| anyhow::anyhow!("RocksDB delete error: {}", e))?;
            db.delete_cf(cf, key_comp.as_bytes())
                .map_err(|e| anyhow::anyhow!("RocksDB delete error: {}", e))?;
        }
        Ok(())
    }
}

#[async_trait]
impl GrayReleasePersistence for GrayReleaseEmbedded {
    async fn create(&self, rule: StoredGrayReleaseRule) -> anyhow::Result<StoredGrayReleaseRule> {
        let mut rule = rule;
        rule.id = self.id_gen.next_id();
        Self::write_raw(&self.db, &rule)
    }

    async fn get_by_namespace(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<Option<StoredGrayReleaseRule>> {
        let cf = self.cf()?;
        let key = Self::key(app_id, cluster_name, namespace_name);
        match self.db.get_cf(cf, key.as_bytes())? {
            Some(data) => {
                let rule: StoredGrayReleaseRule = bincode::deserialize(&data)?;
                // Only return non-deleted
                if !rule.is_deleted {
                    Ok(Some(rule))
                } else {
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }

    async fn update_rules(
        &self,
        id: i64,
        rules: String,
        release_id: i64,
    ) -> anyhow::Result<StoredGrayReleaseRule> {
        let cf = self.cf()?;
        // Get existing rule
        let key_id = Self::key_by_id(id);
        let data = self
            .db
            .get_cf(cf, key_id.as_bytes())?
            .ok_or_else(|| anyhow::anyhow!("Gray release rule '{}' not found", id))?;
        let mut rule: StoredGrayReleaseRule = bincode::deserialize(&data)?;

        // Update fields
        rule.rules = rules;
        rule.release_id = release_id;
        rule.data_change_last_time = Some(chrono::Utc::now().timestamp_millis());

        Self::write_raw(&self.db, &rule)
    }

    async fn delete(&self, id: i64) -> anyhow::Result<()> {
        Self::delete_raw(&self.db, id)
    }

    async fn list_by_app(&self, app_id: &str) -> anyhow::Result<Vec<StoredGrayReleaseRule>> {
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
            let rule: StoredGrayReleaseRule = bincode::deserialize(&value)?;
            // Only return non-deleted
            if !rule.is_deleted {
                results.push(rule);
            }
        }
        Ok(results)
    }
}