//! Generic JSON-backed key/value helper for embedded (RocksDB) persistence.
//!
//! Stores sea_orm `Model` values as JSON inside a RocksDB column family.
//! This avoids the bincode/`Stored*` round-trip and handles chrono types
//! transparently via `serde_json`.

use std::sync::Arc;

use anyhow::Result;
use rocksdb::DB;
use serde::de::DeserializeOwned;
use serde::Serialize;

/// A typed view over one RocksDB column family storing JSON values.
pub struct JsonStore {
    db: Arc<DB>,
    cf: &'static str,
}

impl JsonStore {
    /// Creates a new `JsonStore`.
    pub fn new(db: Arc<DB>, cf: &'static str) -> Self {
        Self { db, cf }
    }

    fn cfh(&self) -> Result<&rocksdb::ColumnFamily> {
        self.db
            .cf_handle(self.cf)
            .ok_or_else(|| anyhow::anyhow!("Column family '{}' not found", self.cf))
    }

    /// Performs the `get` operation.
    pub fn get<T: DeserializeOwned>(&self, key: &[u8]) -> Result<Option<T>> {
        let cf = self.cfh()?;
        match self.db.get_cf(cf, key)? {
            Some(v) => Ok(Some(serde_json::from_slice(&v)?)),
            None => Ok(None),
        }
    }

    /// Performs the `put` operation.
    pub fn put<T: Serialize>(&self, key: &[u8], value: &T) -> Result<()> {
        let cf = self.cfh()?;
        let bytes = serde_json::to_vec(value)?;
        self.db
            .put_cf(cf, key, bytes)
            .map_err(|e| anyhow::anyhow!("RocksDB put error on {}: {}", self.cf, e))?;
        Ok(())
    }

    /// Performs the `delete` operation.
    pub fn delete(&self, key: &[u8]) -> Result<()> {
        let cf = self.cfh()?;
        self.db
            .delete_cf(cf, key)
            .map_err(|e| anyhow::anyhow!("RocksDB delete error on {}: {}", self.cf, e))?;
        Ok(())
    }

    /// Performs the `scan_all` operation.
    pub fn scan_all<T: DeserializeOwned>(&self) -> Result<Vec<T>> {
        let cf = self.cfh()?;
        let mut out = Vec::new();
        for item in self.db.iterator_cf(cf, rocksdb::IteratorMode::Start) {
            let (_, v) = item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            out.push(serde_json::from_slice(&v)?);
        }
        Ok(out)
    }

    /// Performs the `scan_prefix` operation.
    pub fn scan_prefix<T: DeserializeOwned>(&self, prefix: &[u8]) -> Result<Vec<T>> {
        let cf = self.cfh()?;
        let mut out = Vec::new();
        for item in self.db.prefix_iterator_cf(cf, prefix) {
            let (_, v) = item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            out.push(serde_json::from_slice(&v)?);
        }
        Ok(out)
    }
}
