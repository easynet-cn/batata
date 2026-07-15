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
    fn key_by_id(id: i32) -> String {
        format!("gray_id:{}", id)
    }

    /// Build prefix for listing by app: "gray:{app_id}:"
    fn prefix_by_app(app_id: &str) -> String {
        format!("gray:{}:", app_id)
    }
}

#[async_trait]
impl GrayReleasePersistence for GrayReleaseEmbedded {
    async fn create(&self, rule: StoredGrayReleaseRule) -> anyhow::Result<StoredGrayReleaseRule> {
        let cf = self.cf()?;
        let mut rule = rule;
        rule.id = self.id_gen.next_id();
        let bytes = bincode::serialize(&rule)?;

        // Store by id
        let key_id = Self::key_by_id(rule.id);
        self.db
            .put_cf(cf, key_id.as_bytes(), &bytes)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;

        // Store by namespace composite key
        let key_comp = Self::key(&rule.app_id, &rule.cluster_name, &rule.namespace_name);
        self.db
            .put_cf(cf, key_comp.as_bytes(), &bytes)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;

        Ok(rule)
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
        id: i32,
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

        let bytes = bincode::serialize(&rule)?;

        // Update both keys
        self.db
            .put_cf(cf, key_id.as_bytes(), &bytes)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;

        let key_comp = Self::key(&rule.app_id, &rule.cluster_name, &rule.namespace_name);
        self.db
            .put_cf(cf, key_comp.as_bytes(), &bytes)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;

        Ok(rule)
    }

    async fn delete(&self, id: i32) -> anyhow::Result<()> {
        let cf = self.cf()?;
        // First get to find composite key
        let key_id = Self::key_by_id(id);
        if let Some(data) = self.db.get_cf(cf, key_id.as_bytes())? {
            let rule: StoredGrayReleaseRule = bincode::deserialize(&data)?;
            let key_comp = Self::key(&rule.app_id, &rule.cluster_name, &rule.namespace_name);
            // Delete both keys
            self.db
                .delete_cf(cf, key_id.as_bytes())
                .map_err(|e| anyhow::anyhow!("RocksDB delete error: {}", e))?;
            self.db
                .delete_cf(cf, key_comp.as_bytes())
                .map_err(|e| anyhow::anyhow!("RocksDB delete error: {}", e))?;
        }
        Ok(())
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