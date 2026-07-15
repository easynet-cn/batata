//! Embedded implementation of NamespacePersistence trait

use crate::bincode;
use std::sync::Arc;

use async_trait::async_trait;
use rocksdb::DB;

use batata_consistency::raft::state_machine::CF_APOLLO_NAMESPACE;

use crate::persistence::shared::StoredNamespace;
use crate::persistence::traits::NamespacePersistence;
use super::id_generator::IdGenerator;

pub struct NamespaceEmbedded {
    db: Arc<DB>,
    id_gen: Arc<IdGenerator>,
}

impl NamespaceEmbedded {
    pub fn new(db: Arc<DB>, id_gen: Arc<IdGenerator>) -> Self {
        Self { db, id_gen }
    }

    /// Get column family handle
    fn cf(&self) -> anyhow::Result<&rocksdb::ColumnFamily> {
        self.db
            .cf_handle(CF_APOLLO_NAMESPACE)
            .ok_or_else(|| anyhow::anyhow!("Column family '{}' not found", CF_APOLLO_NAMESPACE))
    }

    /// Build key for namespace by id: "ns_id:{id}"
    fn key_by_id(id: i32) -> String {
        format!("ns_id:{}", id)
    }

    /// Build key for namespace by composite key: "ns:{app_id}:{cluster}:{name}"
    fn key(app_id: &str, cluster: &str, name: &str) -> String {
        format!("ns:{}:{}:{}", app_id, cluster, name)
    }

    /// Build prefix for listing by app: "ns:{app_id}:"
    fn prefix_by_app(app_id: &str) -> String {
        format!("ns:{}:", app_id)
    }
}

#[async_trait]
impl NamespacePersistence for NamespaceEmbedded {
    async fn create(&self, namespace: StoredNamespace) -> anyhow::Result<StoredNamespace> {
        let cf = self.cf()?;
        let mut namespace = namespace;
        namespace.id = self.id_gen.next_id();
        let bytes = bincode::serialize(&namespace)?;
        // Store by id
        let key_id = Self::key_by_id(namespace.id);
        self.db
            .put_cf(cf, key_id.as_bytes(), &bytes)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;

        // Store by composite key for lookup by app/cluster/name
        let key_comp = Self::key(&namespace.app_id, &namespace.cluster_name, &namespace.namespace_name);
        self.db
            .put_cf(cf, key_comp.as_bytes(), &bytes)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;

        Ok(namespace)
    }

    async fn get(&self, id: i32) -> anyhow::Result<Option<StoredNamespace>> {
        let cf = self.cf()?;
        let key = Self::key_by_id(id);
        match self.db.get_cf(cf, key.as_bytes())? {
            Some(data) => {
                let ns: StoredNamespace = bincode::deserialize(&data)?;
                // Only return non-deleted
                if !ns.is_deleted {
                    Ok(Some(ns))
                } else {
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }

    async fn get_by_app_cluster(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<Option<StoredNamespace>> {
        let cf = self.cf()?;
        let key = Self::key(app_id, cluster_name, namespace_name);
        match self.db.get_cf(cf, key.as_bytes())? {
            Some(data) => {
                let ns: StoredNamespace = bincode::deserialize(&data)?;
                // Only return non-deleted
                if !ns.is_deleted {
                    Ok(Some(ns))
                } else {
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }

    async fn list_by_app(&self, app_id: &str) -> anyhow::Result<Vec<StoredNamespace>> {
        let cf = self.cf()?;
        let prefix = Self::prefix_by_app(app_id);
        let mut results = Vec::new();
        let iter = self.db.prefix_iterator_cf(cf, prefix.as_bytes());
        for item in iter {
            let (key, value) = item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let key_str = String::from_utf8_lossy(&key);
            // Only iterate over composite keys (ns:{app_id}:...)
            if !key_str.starts_with(&prefix) {
                break;
            }
            let ns: StoredNamespace = bincode::deserialize(&value)?;
            // Only return non-deleted
            if !ns.is_deleted {
                results.push(ns);
            }
        }
        Ok(results)
    }

    async fn update(&self, namespace: StoredNamespace) -> anyhow::Result<StoredNamespace> {
        let cf = self.cf()?;
        // Check existence by id
        let key_id = Self::key_by_id(namespace.id);
        if self.db.get_cf(cf, key_id.as_bytes())?.is_none() {
            anyhow::bail!("Namespace '{}' not found", namespace.id);
        }
        let bytes = bincode::serialize(&namespace)?;

        // Update both keys
        self.db
            .put_cf(cf, key_id.as_bytes(), &bytes)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;

        let key_comp = Self::key(&namespace.app_id, &namespace.cluster_name, &namespace.namespace_name);
        self.db
            .put_cf(cf, key_comp.as_bytes(), &bytes)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;

        Ok(namespace)
    }

    async fn delete(&self, id: i32) -> anyhow::Result<()> {
        let cf = self.cf()?;
        // First get to find composite key
        let key_id = Self::key_by_id(id);
        if let Some(data) = self.db.get_cf(cf, key_id.as_bytes())? {
            let ns: StoredNamespace = bincode::deserialize(&data)?;
            let key_comp = Self::key(&ns.app_id, &ns.cluster_name, &ns.namespace_name);
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
}