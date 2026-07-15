//! Embedded implementation of AppPersistence trait

use crate::bincode;
use std::sync::Arc;

use async_trait::async_trait;
use rocksdb::DB;

use batata_consistency::raft::state_machine::CF_APOLLO_APP;

use crate::persistence::shared::StoredApp;
use crate::persistence::traits::AppPersistence;

/// Embedded App persistence using RocksDB
pub struct AppEmbedded {
    db: Arc<DB>,
}

impl AppEmbedded {
    /// Create from RocksDB
    pub fn new(db: Arc<DB>) -> Self {
        Self { db }
    }

    /// Get column family handle
    fn cf(&self) -> anyhow::Result<&rocksdb::ColumnFamily> {
        self.db
            .cf_handle(CF_APOLLO_APP)
            .ok_or_else(|| anyhow::anyhow!("Column family '{}' not found", CF_APOLLO_APP))
    }

    /// Build key for app: "app:{app_id}"
    fn key(app_id: &str) -> String {
        format!("app:{}", app_id)
    }
}

#[async_trait]
impl AppPersistence for AppEmbedded {
    async fn create(&self, app: StoredApp) -> anyhow::Result<StoredApp> {
        let cf = self.cf()?;
        let key = Self::key(&app.app_id);
        let bytes = bincode::serialize(&app)?;
        self.db
            .put_cf(cf, key.as_bytes(), &bytes)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;
        Ok(app)
    }

    async fn get(&self, app_id: &str) -> anyhow::Result<Option<StoredApp>> {
        let cf = self.cf()?;
        let key = Self::key(app_id);
        match self.db.get_cf(cf, key.as_bytes())? {
            Some(data) => {
                let app: StoredApp = bincode::deserialize(&data)?;
                Ok(Some(app))
            }
            None => Ok(None),
        }
    }

    async fn get_by_ids(&self, app_ids: &[String]) -> anyhow::Result<Vec<StoredApp>> {
        let cf = self.cf()?;
        let mut results = Vec::with_capacity(app_ids.len());
        for app_id in app_ids {
            let key = Self::key(app_id);
            if let Some(data) = self.db.get_cf(cf, key.as_bytes())? {
                let app: StoredApp = bincode::deserialize(&data)?;
                // Only return non-deleted apps
                if !app.is_deleted {
                    results.push(app);
                }
            }
        }
        Ok(results)
    }

    async fn list(&self) -> anyhow::Result<Vec<StoredApp>> {
        let cf = self.cf()?;
        let mut results = Vec::new();
        let prefix = "app:";
        let iter = self.db.prefix_iterator_cf(cf, prefix.as_bytes());
        for item in iter {
            let (_, value) = item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let app: StoredApp = bincode::deserialize(&value)?;
            // Only return non-deleted apps
            if !app.is_deleted {
                results.push(app);
            }
        }
        Ok(results)
    }

    async fn update(&self, app: StoredApp) -> anyhow::Result<StoredApp> {
        let cf = self.cf()?;
        let key = Self::key(&app.app_id);
        // Check existence
        if self.db.get_cf(cf, key.as_bytes())?.is_none() {
            anyhow::bail!("App '{}' not found", app.app_id);
        }
        let bytes = bincode::serialize(&app)?;
        self.db
            .put_cf(cf, key.as_bytes(), &bytes)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;
        Ok(app)
    }

    async fn delete(&self, app_id: &str) -> anyhow::Result<()> {
        let cf = self.cf()?;
        let key = Self::key(app_id);
        self.db
            .delete_cf(cf, key.as_bytes())
            .map_err(|e| anyhow::anyhow!("RocksDB delete error: {}", e))?;
        Ok(())
    }
}