//! Embedded implementation of CommitPersistence trait

use crate::bincode;
use std::sync::Arc;

use async_trait::async_trait;
use rocksdb::DB;

use batata_consistency::raft::state_machine::CF_APOLLO_COMMIT;

use crate::persistence::shared::StoredCommit;
use crate::persistence::traits::CommitPersistence;
use super::id_generator::IdGenerator;

pub struct CommitEmbedded {
    db: Arc<DB>,
    id_gen: Arc<IdGenerator>,
}

impl CommitEmbedded {
    pub fn new(db: Arc<DB>, id_gen: Arc<IdGenerator>) -> Self {
        Self { db, id_gen }
    }

    /// Get column family handle
    fn cf(&self) -> anyhow::Result<&rocksdb::ColumnFamily> {
        self.db
            .cf_handle(CF_APOLLO_COMMIT)
            .ok_or_else(|| anyhow::anyhow!("Column family '{}' not found", CF_APOLLO_COMMIT))
    }

    /// Build key for commit by id: "commit:{commit_id}"
    fn key_by_id(id: i32) -> String {
        format!("commit:{}", id)
    }

    /// Build prefix for listing by namespace: "commit_by_ns:{app_id}:{cluster}:{namespace}:"
    fn prefix_by_namespace(app_id: &str, cluster: &str, namespace: &str) -> String {
        format!("commit_by_ns:{}:{}:{}:", app_id, cluster, namespace)
    }

    /// Build index key for listing: "commit_by_ns:{app_id}:{cluster}:{namespace}:{commit_id}"
    fn index_key(app_id: &str, cluster: &str, namespace: &str, commit_id: i32) -> String {
        format!("commit_by_ns:{}:{}:{}:{}", app_id, cluster, namespace, commit_id)
    }
}

#[async_trait]
impl CommitPersistence for CommitEmbedded {
    async fn create(&self, commit: StoredCommit) -> anyhow::Result<StoredCommit> {
        let cf = self.cf()?;
        let mut commit = commit;
        commit.id = self.id_gen.next_id();
        let bytes = bincode::serialize(&commit)?;

        // Store by id
        let key_id = Self::key_by_id(commit.id);
        self.db
            .put_cf(cf, key_id.as_bytes(), &bytes)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;

        // Store index for listing
        let index_key = Self::index_key(
            &commit.app_id,
            &commit.cluster_name,
            &commit.namespace_name,
            commit.id,
        );
        self.db
            .put_cf(cf, index_key.as_bytes(), &bytes)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;

        Ok(commit)
    }

    async fn get_by_id(&self, id: i32) -> anyhow::Result<Option<StoredCommit>> {
        let cf = self.cf()?;
        let key = Self::key_by_id(id);
        match self.db.get_cf(cf, key.as_bytes())? {
            Some(data) => {
                let commit: StoredCommit = bincode::deserialize(&data)?;
                // Only return non-deleted
                if !commit.is_deleted {
                    Ok(Some(commit))
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
    ) -> anyhow::Result<Vec<StoredCommit>> {
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
            let commit: StoredCommit = bincode::deserialize(&value)?;
            // Only return non-deleted
            if !commit.is_deleted {
                results.push(commit);
            }
        }
        // Sort by id descending (most recent first)
        results.sort_by(|a, b| b.id.cmp(&a.id));
        Ok(results)
    }

    async fn get_latest(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<Option<StoredCommit>> {
        let commits = self.list_by_namespace(app_id, cluster_name, namespace_name).await?;
        Ok(commits.into_iter().next())
    }

    async fn update(&self, commit: StoredCommit) -> anyhow::Result<StoredCommit> {
        let cf = self.cf()?;
        let bytes = bincode::serialize(&commit)?;

        // Update by id
        let key_id = Self::key_by_id(commit.id);
        self.db
            .put_cf(cf, key_id.as_bytes(), &bytes)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;

        // Update index
        let index_key = Self::index_key(
            &commit.app_id,
            &commit.cluster_name,
            &commit.namespace_name,
            commit.id,
        );
        self.db
            .put_cf(cf, index_key.as_bytes(), &bytes)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;

        Ok(commit)
    }
}