//! Embedded implementation of NamespaceLockPersistence trait

use crate::bincode;
use std::sync::Arc;

use async_trait::async_trait;
use rocksdb::DB;

use batata_consistency::raft::state_machine::CF_APOLLO_NAMESPACE_LOCK;

use crate::persistence::shared::StoredNamespaceLock;
use crate::persistence::traits::NamespaceLockPersistence;

/// Embedded Namespace Lock persistence using RocksDB
pub struct NamespaceLockEmbedded {
    db: Arc<DB>,
}

impl NamespaceLockEmbedded {
    /// Create from RocksDB
    pub fn new(db: Arc<DB>) -> Self {
        Self { db }
    }

    /// Get column family handle
    fn cf(&self) -> anyhow::Result<&rocksdb::ColumnFamily> {
        self.db
            .cf_handle(CF_APOLLO_NAMESPACE_LOCK)
            .ok_or_else(|| anyhow::anyhow!("Column family '{}' not found", CF_APOLLO_NAMESPACE_LOCK))
    }

    /// Build key for namespace lock: "lock:{app_id}:{cluster}:{namespace}"
    fn key(app_id: &str, cluster: &str, namespace: &str) -> String {
        format!("lock:{}:{}:{}", app_id, cluster, namespace)
    }
}

#[async_trait]
impl NamespaceLockPersistence for NamespaceLockEmbedded {
    async fn lock(&self, lock: StoredNamespaceLock) -> anyhow::Result<StoredNamespaceLock> {
        let cf = self.cf()?;
        let key = Self::key(&lock.app_id, &lock.cluster_name, &lock.namespace_name);

        // Check if already locked
        if let Some(data) = self.db.get_cf(cf, key.as_bytes())? {
            let existing: StoredNamespaceLock = bincode::deserialize(&data)?;
            // If locked by same user, return existing lock
            if existing.locked_by == lock.locked_by {
                return Ok(existing);
            }
            // Otherwise, reject
            anyhow::bail!(
                "Namespace {}+{}+{} is already locked by {}",
                lock.app_id,
                lock.cluster_name,
                lock.namespace_name,
                existing.locked_by
            );
        }

        // Create new lock
        let bytes = bincode::serialize(&lock)?;
        self.db
            .put_cf(cf, key.as_bytes(), &bytes)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;
        Ok(lock)
    }

    async fn unlock(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<()> {
        let cf = self.cf()?;
        let key = Self::key(app_id, cluster_name, namespace_name);
        self.db
            .delete_cf(cf, key.as_bytes())
            .map_err(|e| anyhow::anyhow!("RocksDB delete error: {}", e))?;
        Ok(())
    }

    async fn get(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<Option<StoredNamespaceLock>> {
        let cf = self.cf()?;
        let key = Self::key(app_id, cluster_name, namespace_name);
        match self.db.get_cf(cf, key.as_bytes())? {
            Some(data) => {
                let lock: StoredNamespaceLock = bincode::deserialize(&data)?;
                Ok(Some(lock))
            }
            None => Ok(None),
        }
    }

    async fn is_locked(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<bool> {
        let cf = self.cf()?;
        let key = Self::key(app_id, cluster_name, namespace_name);
        Ok(self.db.get_cf(cf, key.as_bytes())?.is_some())
    }
}