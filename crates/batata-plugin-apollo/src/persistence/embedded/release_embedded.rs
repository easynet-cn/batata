//! Embedded implementation of ReleasePersistence trait

use crate::bincode;
use std::sync::Arc;

use async_trait::async_trait;
use rocksdb::DB;

use batata_consistency::raft::state_machine::CF_APOLLO_RELEASE;

use crate::persistence::shared::StoredRelease;
use crate::persistence::traits::ReleasePersistence;
use super::id_generator::IdGenerator;

pub struct ReleaseEmbedded {
    db: Arc<DB>,
    id_gen: Arc<IdGenerator>,
}

impl ReleaseEmbedded {
    pub fn new(db: Arc<DB>, id_gen: Arc<IdGenerator>) -> Self {
        Self { db, id_gen }
    }

    /// Get column family handle
    fn cf(&self) -> anyhow::Result<&rocksdb::ColumnFamily> {
        self.db
            .cf_handle(CF_APOLLO_RELEASE)
            .ok_or_else(|| anyhow::anyhow!("Column family '{}' not found", CF_APOLLO_RELEASE))
    }

    /// Build key for release by id: "release:{release_id}"
    fn key_by_id(id: i32) -> String {
        format!("release:{}", id)
    }

    /// Build key for latest release: "release_latest:{app_id}:{cluster}:{namespace}"
    fn key_latest(app_id: &str, cluster: &str, namespace: &str) -> String {
        format!("release_latest:{}:{}:{}", app_id, cluster, namespace)
    }

    /// Build prefix for listing by namespace: "release_by_ns:{app_id}:{cluster}:{namespace}:"
    fn prefix_by_namespace(app_id: &str, cluster: &str, namespace: &str) -> String {
        format!("release_by_ns:{}:{}:{}:", app_id, cluster, namespace)
    }

    /// Build index key for listing: "release_by_ns:{app_id}:{cluster}:{namespace}:{release_id}"
    fn index_key(app_id: &str, cluster: &str, namespace: &str, release_id: i32) -> String {
        format!("release_by_ns:{}:{}:{}:{}", app_id, cluster, namespace, release_id)
    }
}

#[async_trait]
impl ReleasePersistence for ReleaseEmbedded {
    async fn create(&self, release: StoredRelease) -> anyhow::Result<StoredRelease> {
        let cf = self.cf()?;
        let mut release = release;
        release.id = self.id_gen.next_id();
        let bytes = bincode::serialize(&release)?;

        // Store by id
        let key_id = Self::key_by_id(release.id);
        self.db
            .put_cf(cf, key_id.as_bytes(), &bytes)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;

        // Store index for listing
        let index_key = Self::index_key(
            &release.app_id,
            &release.cluster_name,
            &release.namespace_name,
            release.id,
        );
        self.db
            .put_cf(cf, index_key.as_bytes(), &bytes)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;

        // Update latest release pointer
        let key_latest = Self::key_latest(
            &release.app_id,
            &release.cluster_name,
            &release.namespace_name,
        );
        // Store the release id as the latest (just the id, not full data)
        self.db
            .put_cf(cf, key_latest.as_bytes(), &bytes)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;

        Ok(release)
    }

    async fn get_by_id(&self, id: i32) -> anyhow::Result<Option<StoredRelease>> {
        let cf = self.cf()?;
        let key = Self::key_by_id(id);
        match self.db.get_cf(cf, key.as_bytes())? {
            Some(data) => {
                let release: StoredRelease = bincode::deserialize(&data)?;
                // Only return non-deleted and non-abandoned
                if !release.is_deleted && !release.is_abandoned {
                    Ok(Some(release))
                } else {
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }

    async fn get_latest(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<Option<StoredRelease>> {
        let cf = self.cf()?;
        let key_latest = Self::key_latest(app_id, cluster_name, namespace_name);
        match self.db.get_cf(cf, key_latest.as_bytes())? {
            Some(data) => {
                let release: StoredRelease = bincode::deserialize(&data)?;
                // Only return non-deleted and non-abandoned
                if !release.is_deleted && !release.is_abandoned {
                    Ok(Some(release))
                } else {
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }

    async fn list_by_namespace(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<Vec<StoredRelease>> {
        let cf = self.cf()?;
        let prefix = Self::prefix_by_namespace(app_id, cluster_name, namespace_name);
        let mut results = Vec::new();
        let iter = self.db.prefix_iterator_cf(cf, prefix.as_bytes());
        for item in iter {
            let (key, value) = item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let key_str = String::from_utf8_lossy(&key);
            if !key_str.starts_with(&prefix) {
                break;
            }
            let release: StoredRelease = bincode::deserialize(&value)?;
            // Only return non-deleted and non-abandoned
            if !release.is_deleted && !release.is_abandoned {
                results.push(release);
            }
        }
        // Sort by id descending (most recent first)
        results.sort_by(|a, b| b.id.cmp(&a.id));
        Ok(results)
    }

    async fn delete(&self, id: i32) -> anyhow::Result<()> {
        let cf = self.cf()?;
        // First get to find all keys
        let key_id = Self::key_by_id(id);
        if let Some(data) = self.db.get_cf(cf, key_id.as_bytes())? {
            let release: StoredRelease = bincode::deserialize(&data)?;
            let index_key = Self::index_key(
                &release.app_id,
                &release.cluster_name,
                &release.namespace_name,
                release.id,
            );
            // Delete all keys
            self.db
                .delete_cf(cf, key_id.as_bytes())
                .map_err(|e| anyhow::anyhow!("RocksDB delete error: {}", e))?;
            self.db
                .delete_cf(cf, index_key.as_bytes())
                .map_err(|e| anyhow::anyhow!("RocksDB delete error: {}", e))?;
            // Note: we don't delete the latest pointer as there might be an older release
        }
        Ok(())
    }

    async fn get_by_release_id(&self, release_id: i64) -> anyhow::Result<Option<StoredRelease>> {
        let cf = self.cf()?;
        let prefix = "release:";
        let mut iter = self.db.prefix_iterator_cf(cf, prefix.as_bytes());
        while let Some(item) = iter.next() {
            let (_, value) = item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let release: StoredRelease = bincode::deserialize(&value)?;
            if release.release_id == Some(release_id) && !release.is_deleted && !release.is_abandoned {
                return Ok(Some(release));
            }
        }
        Ok(None)
    }
}