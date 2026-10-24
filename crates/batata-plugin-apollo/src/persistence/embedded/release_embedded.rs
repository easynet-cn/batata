//! Embedded implementation of ReleasePersistence trait

use crate::bincode;
use std::sync::Arc;

use async_trait::async_trait;
use rocksdb::DB;

use batata_consistency::raft::state_machine::CF_APOLLO_RELEASE;

use super::id_generator::IdGenerator;
use crate::persistence::shared::StoredRelease;
use crate::persistence::traits::ReleasePersistence;

/// Represents the `ReleaseEmbedded` entity.
pub struct ReleaseEmbedded {
    db: Arc<DB>,
    id_gen: Arc<IdGenerator>,
}

impl ReleaseEmbedded {
    /// Creates a new `ReleaseEmbedded`.
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
    fn key_by_id(id: i64) -> String {
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
    fn index_key(app_id: &str, cluster: &str, namespace: &str, release_id: i64) -> String {
        format!(
            "release_by_ns:{}:{}:{}:{}",
            app_id, cluster, namespace, release_id
        )
    }

    /// Newest row in the namespace index that is neither deleted nor abandoned.
    ///
    /// This is the embedded-side equivalent of upstream
    /// `ReleaseService.findLatestActiveRelease`
    /// (`ORDER BY id DESC LIMIT 1` over `IsAbandoned = false AND IsDeleted = false`).
    /// Callers must have already written the row's index entry (or removed it)
    /// so the scan sees the post-write state.
    fn newest_active(
        db: &DB,
        cf: &rocksdb::ColumnFamily,
        app_id: &str,
        cluster: &str,
        namespace: &str,
    ) -> anyhow::Result<Option<StoredRelease>> {
        let prefix = Self::prefix_by_namespace(app_id, cluster, namespace);
        let mut newest: Option<StoredRelease> = None;
        let iter = db.prefix_iterator_cf(cf, prefix.as_bytes());
        for item in iter {
            let (key, value) =
                item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let key_str = String::from_utf8_lossy(&key);
            if !key_str.starts_with(&prefix) {
                break;
            }
            let release: StoredRelease = bincode::deserialize(&value)?;
            if release.is_deleted || release.is_abandoned {
                continue;
            }
            if newest.as_ref().map(|n| release.id > n.id).unwrap_or(true) {
                newest = Some(release);
            }
        }
        Ok(newest)
    }

    /// Keeps the `release_latest:` pointer aligned with
    /// `findLatestActiveRelease` after a row write/delete.
    ///
    /// The pointer is a cache, so it must never end up pointing at a row that
    /// `get_latest` would filter out: an unconditional overwrite (what a plain
    /// `put` does) leaves the pointer on an abandoned row after a rollback and
    /// `get_latest` then reports "no release at all". Active rows move the
    /// pointer forward only; inactive rows trigger a recompute when they were
    /// the pointer.
    fn refresh_latest(
        db: &DB,
        cf: &rocksdb::ColumnFamily,
        release: &StoredRelease,
    ) -> anyhow::Result<()> {
        let latest_key = Self::key_latest(
            &release.app_id,
            &release.cluster_name,
            &release.namespace_name,
        );
        let current = match db.get_cf(cf, latest_key.as_bytes())? {
            Some(data) => bincode::deserialize::<StoredRelease>(&data).ok(),
            None => None,
        };

        if !release.is_deleted && !release.is_abandoned {
            // Active row: advance, never rewind (an out-of-order replay of an
            // older release must not displace a newer one).
            if current.as_ref().map(|c| release.id >= c.id).unwrap_or(true) {
                let bytes = bincode::serialize(release)?;
                db.put_cf(cf, latest_key.as_bytes(), &bytes)
                    .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;
            }
            return Ok(());
        }

        // Inactive row: only relevant when it still owns the pointer.
        if current
            .as_ref()
            .map(|c| c.id == release.id)
            .unwrap_or(false)
        {
            match Self::newest_active(
                db,
                cf,
                &release.app_id,
                &release.cluster_name,
                &release.namespace_name,
            )? {
                Some(newest) => {
                    let b = bincode::serialize(&newest)?;
                    db.put_cf(cf, latest_key.as_bytes(), &b)
                        .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;
                }
                None => {
                    db.delete_cf(cf, latest_key.as_bytes())
                        .map_err(|e| anyhow::anyhow!("RocksDB delete error: {}", e))?;
                }
            }
        }
        Ok(())
    }

    /// Writes a fully-formed `StoredRelease` (including its `id`) to RocksDB.
    ///
    /// Deterministic write path shared by the local `create` (`id` from
    /// `IdGenerator`) and the Raft apply phase (`id` derived from the Raft
    /// `log_index`). Operates on a `&DB` so it can be called from the Raft
    /// state machine.
    pub fn write_raw(db: &DB, release: &StoredRelease) -> anyhow::Result<StoredRelease> {
        let cf = db
            .cf_handle(CF_APOLLO_RELEASE)
            .ok_or_else(|| anyhow::anyhow!("CF {} not found", CF_APOLLO_RELEASE))?;
        let bytes = bincode::serialize(release)?;

        // Store by id
        let key_id = Self::key_by_id(release.id);
        db.put_cf(cf, key_id.as_bytes(), &bytes)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;

        // Store index for listing
        let index_key = Self::index_key(
            &release.app_id,
            &release.cluster_name,
            &release.namespace_name,
            release.id,
        );
        db.put_cf(cf, index_key.as_bytes(), &bytes)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;

        // Maintain the latest-release pointer (upstream `findLatestActiveRelease`).
        Self::refresh_latest(db, cf, release)?;

        Ok(release.clone())
    }

    /// Deletes a release by `id` (removes the id key and namespace index key).
    /// Deterministic counterpart to `write_raw`.
    pub fn delete_raw(db: &DB, id: i64) -> anyhow::Result<()> {
        let cf = db
            .cf_handle(CF_APOLLO_RELEASE)
            .ok_or_else(|| anyhow::anyhow!("CF {} not found", CF_APOLLO_RELEASE))?;
        // First get to find all keys
        let key_id = Self::key_by_id(id);
        if let Some(data) = db.get_cf(cf, key_id.as_bytes())? {
            let release: StoredRelease = bincode::deserialize(&data)?;
            let index_key = Self::index_key(
                &release.app_id,
                &release.cluster_name,
                &release.namespace_name,
                release.id,
            );
            // Delete all keys
            db.delete_cf(cf, key_id.as_bytes())
                .map_err(|e| anyhow::anyhow!("RocksDB delete error: {}", e))?;
            db.delete_cf(cf, index_key.as_bytes())
                .map_err(|e| anyhow::anyhow!("RocksDB delete error: {}", e))?;
            // A removed row can no longer be the latest active one.
            let mut removed = release.clone();
            removed.is_deleted = true;
            Self::refresh_latest(db, cf, &removed)?;
        }
        Ok(())
    }
}

#[async_trait]
impl ReleasePersistence for ReleaseEmbedded {
    async fn create(&self, release: StoredRelease) -> anyhow::Result<StoredRelease> {
        let mut release = release;
        release.id = self.id_gen.next_id();
        Self::write_raw(&self.db, &release)
    }

    async fn get_by_id(&self, id: i64) -> anyhow::Result<Option<StoredRelease>> {
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
            let (key, value) =
                item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
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

    async fn delete(&self, id: i64) -> anyhow::Result<()> {
        Self::delete_raw(&self.db, id)
    }

    async fn update(&self, release: StoredRelease) -> anyhow::Result<StoredRelease> {
        let cf = self.cf()?;
        let bytes = bincode::serialize(&release)?;

        // Rewrite the row and its namespace index.
        self.db
            .put_cf(cf, Self::key_by_id(release.id).as_bytes(), &bytes)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;
        self.db
            .put_cf(
                cf,
                Self::index_key(
                    &release.app_id,
                    &release.cluster_name,
                    &release.namespace_name,
                    release.id,
                )
                .as_bytes(),
                &bytes,
            )
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;

        // Keep the latest pointer consistent with `findLatestActiveRelease`.
        Self::refresh_latest(&self.db, cf, &release)?;

        Ok(release)
    }

    async fn list_active(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<Vec<StoredRelease>> {
        // list_by_namespace already filters abandoned/deleted.
        ReleasePersistence::list_by_namespace(self, app_id, cluster_name, namespace_name).await
    }

    async fn get_by_release_id(&self, release_id: i64) -> anyhow::Result<Option<StoredRelease>> {
        let cf = self.cf()?;
        let prefix = "release:";
        let iter = self.db.prefix_iterator_cf(cf, prefix.as_bytes());
        for item in iter {
            let (_, value) = item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let release: StoredRelease = bincode::deserialize(&value)?;
            if release.release_id == Some(release_id)
                && !release.is_deleted
                && !release.is_abandoned
            {
                return Ok(Some(release));
            }
        }
        Ok(None)
    }

    async fn list_all_by_namespace(
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
            let (key, value) =
                item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let key_str = String::from_utf8_lossy(&key);
            if !key_str.starts_with(&prefix) {
                break;
            }
            let release: StoredRelease = bincode::deserialize(&value)?;
            if !release.is_deleted {
                results.push(release);
            }
        }
        // Newest id first.
        results.sort_by(|a, b| b.id.cmp(&a.id));
        Ok(results)
    }
}
