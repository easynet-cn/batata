use crate::bincode;
use std::sync::Arc;

use async_trait::async_trait;
use rocksdb::DB;

use batata_consistency::raft::state_machine::CF_APOLLO_CLUSTER;

use crate::persistence::shared::StoredCluster;
use crate::persistence::traits::ClusterPersistence;

pub struct ClusterEmbedded {
    db: Arc<DB>,
}

impl ClusterEmbedded {
    pub fn new(db: Arc<DB>) -> Self {
        Self { db }
    }

    fn cf(&self) -> anyhow::Result<&rocksdb::ColumnFamily> {
        self.db
            .cf_handle(CF_APOLLO_CLUSTER)
            .ok_or_else(|| anyhow::anyhow!("Column family '{}' not found", CF_APOLLO_CLUSTER))
    }

    fn key(app_id: &str, cluster_name: &str) -> String {
        format!("cluster:{}:{}", app_id, cluster_name)
    }
}

#[async_trait]
impl ClusterPersistence for ClusterEmbedded {
    async fn create(&self, cluster: StoredCluster) -> anyhow::Result<StoredCluster> {
        let cf = self.cf()?;
        let key = Self::key(&cluster.app_id, &cluster.name);
        let bytes = bincode::serialize(&cluster)?;
        self.db
            .put_cf(cf, key.as_bytes(), &bytes)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;
        Ok(cluster)
    }

    async fn get(&self, app_id: &str, cluster_name: &str) -> anyhow::Result<Option<StoredCluster>> {
        let cf = self.cf()?;
        let key = Self::key(app_id, cluster_name);
        match self.db.get_cf(cf, key.as_bytes())? {
            Some(data) => {
                let cluster: StoredCluster = bincode::deserialize(&data)?;
                if !cluster.is_deleted {
                    Ok(Some(cluster))
                } else {
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }

    async fn list(&self, app_id: &str) -> anyhow::Result<Vec<StoredCluster>> {
        let cf = self.cf()?;
        let mut results = Vec::new();
        let prefix = format!("cluster:{}:", app_id);
        let iter = self.db.prefix_iterator_cf(cf, prefix.as_bytes());
        for item in iter {
            let (_, value) = item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let cluster: StoredCluster = bincode::deserialize(&value)?;
            if !cluster.is_deleted {
                results.push(cluster);
            }
        }
        Ok(results)
    }

    async fn update(&self, cluster: StoredCluster) -> anyhow::Result<StoredCluster> {
        let cf = self.cf()?;
        let key = Self::key(&cluster.app_id, &cluster.name);
        if self.db.get_cf(cf, key.as_bytes())?.is_none() {
            anyhow::bail!("Cluster not found: {}/{}", cluster.app_id, cluster.name);
        }
        let bytes = bincode::serialize(&cluster)?;
        self.db
            .put_cf(cf, key.as_bytes(), &bytes)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;
        Ok(cluster)
    }

    async fn delete(&self, app_id: &str, cluster_name: &str) -> anyhow::Result<()> {
        let cf = self.cf()?;
        let key = Self::key(app_id, cluster_name);
        match self.db.get_cf(cf, key.as_bytes())? {
            Some(data) => {
                let mut cluster: StoredCluster = bincode::deserialize(&data)?;
                cluster.is_deleted = true;
                cluster.deleted_at = chrono::Utc::now().timestamp_millis();
                let bytes = bincode::serialize(&cluster)?;
                self.db
                    .put_cf(cf, key.as_bytes(), &bytes)
                    .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;
                Ok(())
            }
            None => anyhow::bail!("Cluster not found: {}/{}", app_id, cluster_name),
        }
    }
}
