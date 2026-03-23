/// Consul-dedicated Raft state machine.
///
/// Owns its own RocksDB instance at `{consul_data_dir}/raft/state/`.
/// All CreateIndex/ModifyIndex values are set from the Raft log index,
/// matching Consul's original behavior.
use std::path::Path;
use std::sync::Arc;

use openraft::storage::RaftStateMachine;
use openraft::{EntryPayload, OptionalSend, StorageError, StoredMembership};
use rocksdb::{ColumnFamilyDescriptor, DB, Options, WriteBatch};
use tokio::sync::RwLock;
use tracing::{debug, error, info};

use super::request::{ConsulRaftRequest, ConsulRaftResponse};
use super::types::*;
use crate::index_provider::{ConsulTable, ConsulTableIndex};

// Column family names
const CF_CONSUL_KV: &str = "consul_kv";
const CF_CONSUL_SESSIONS: &str = "consul_sessions";
const CF_CONSUL_ACL: &str = "consul_acl";
const CF_CONSUL_QUERIES: &str = "consul_queries";
const CF_META: &str = "meta";

// Meta keys
const KEY_LAST_APPLIED: &[u8] = b"last_applied";
const KEY_LAST_MEMBERSHIP: &[u8] = b"last_membership";

pub struct ConsulStateMachine {
    db: Arc<DB>,
    last_applied: RwLock<Option<ConsulLogId>>,
    last_membership: RwLock<ConsulStoredMembership>,
    /// Per-table index — updated on every apply to notify blocking queries.
    /// Shared with ConsulServices so GET handlers can wait_for_change().
    table_index: ConsulTableIndex,
}

impl ConsulStateMachine {
    pub async fn open(
        dir: impl AsRef<Path>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let path = dir.as_ref();
        std::fs::create_dir_all(path)?;

        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);

        let cf_opts = Options::default();
        let cfs = vec![
            ColumnFamilyDescriptor::new(CF_CONSUL_KV, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_CONSUL_SESSIONS, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_CONSUL_ACL, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_CONSUL_QUERIES, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_META, cf_opts),
        ];

        let db = Arc::new(DB::open_cf_descriptors(&db_opts, path, cfs)?);
        info!("Consul state machine opened at {:?}", path);

        let last_applied = Self::load_last_applied(&db);
        let last_membership = Self::load_last_membership(&db);

        let table_index = ConsulTableIndex::new();
        // Initialize table indexes from last_applied
        if let Some(ref la) = last_applied {
            table_index.update(ConsulTable::KVS, la.index);
            table_index.update(ConsulTable::Sessions, la.index);
            table_index.update(ConsulTable::Catalog, la.index);
        }

        Ok(Self {
            db,
            last_applied: RwLock::new(last_applied),
            last_membership: RwLock::new(last_membership.unwrap_or_else(|| {
                StoredMembership::new(None, ConsulMembership::new(vec![], None))
            })),
            table_index,
        })
    }

    /// Get a shared handle to the underlying RocksDB.
    pub fn db(&self) -> Arc<DB> {
        self.db.clone()
    }

    /// Get the per-table index tracker.
    /// Share this with ConsulServices so GET handlers can wait_for_change().
    pub fn table_index(&self) -> ConsulTableIndex {
        self.table_index.clone()
    }

    // ========================================================================
    // Column family helpers
    // ========================================================================

    fn cf_kv(&self) -> &rocksdb::ColumnFamily {
        self.db
            .cf_handle(CF_CONSUL_KV)
            .expect("CF consul_kv must exist")
    }

    fn cf_sessions(&self) -> &rocksdb::ColumnFamily {
        self.db
            .cf_handle(CF_CONSUL_SESSIONS)
            .expect("CF consul_sessions must exist")
    }

    fn cf_meta(&self) -> &rocksdb::ColumnFamily {
        self.db.cf_handle(CF_META).expect("CF meta must exist")
    }

    // ========================================================================
    // Persistence helpers
    // ========================================================================

    fn load_last_applied(db: &DB) -> Option<ConsulLogId> {
        let cf = db.cf_handle(CF_META)?;
        let bytes = db.get_cf(cf, KEY_LAST_APPLIED).ok()??;
        serde_json::from_slice(&bytes).ok()
    }

    fn load_last_membership(db: &DB) -> Option<ConsulStoredMembership> {
        let cf = db.cf_handle(CF_META)?;
        let bytes = db.get_cf(cf, KEY_LAST_MEMBERSHIP).ok()??;
        serde_json::from_slice(&bytes).ok()
    }

    async fn save_last_applied(&self, log_id: ConsulLogId) -> Result<(), StorageError<NodeId>> {
        let bytes = serde_json::to_vec(&log_id).map_err(|e| sm_error(e))?;
        self.db
            .put_cf(self.cf_meta(), KEY_LAST_APPLIED, &bytes)
            .map_err(|e| sm_error(e))?;
        *self.last_applied.write().await = Some(log_id);
        Ok(())
    }

    async fn save_membership(
        &self,
        membership: ConsulStoredMembership,
    ) -> Result<(), StorageError<NodeId>> {
        let bytes = serde_json::to_vec(&membership).map_err(|e| sm_error(e))?;
        self.db
            .put_cf(self.cf_meta(), KEY_LAST_MEMBERSHIP, &bytes)
            .map_err(|e| sm_error(e))?;
        *self.last_membership.write().await = membership;
        Ok(())
    }

    // ========================================================================
    // Apply dispatcher — log_index is passed to every operation
    // ========================================================================

    fn apply_request(&self, request: ConsulRaftRequest, log_index: u64) -> ConsulRaftResponse {
        match request {
            ConsulRaftRequest::KVPut {
                key,
                stored_kv_json,
                session_index_key,
            } => self.apply_kv_put(
                &key,
                &stored_kv_json,
                session_index_key.as_deref(),
                log_index,
            ),
            ConsulRaftRequest::KVDelete {
                key,
                session_index_cleanup,
            } => self.apply_kv_delete(&key, session_index_cleanup.as_deref(), log_index),
            ConsulRaftRequest::KVDeletePrefix { prefix } => {
                self.apply_kv_delete_prefix(&prefix, log_index)
            }
            ConsulRaftRequest::KVAcquireSession {
                key,
                session_id,
                stored_kv_json,
            } => self.apply_kv_acquire_session(&key, &session_id, &stored_kv_json, log_index),
            ConsulRaftRequest::KVReleaseSessionKey {
                key,
                session_id,
                stored_kv_json,
            } => self.apply_kv_release_session_key(&key, &session_id, &stored_kv_json, log_index),
            ConsulRaftRequest::KVReleaseSession {
                session_id,
                updates,
                index_keys_to_delete,
            } => {
                self.apply_kv_release_session(&session_id, updates, index_keys_to_delete, log_index)
            }
            ConsulRaftRequest::KVCas {
                key,
                stored_kv_json,
                expected_modify_index,
            } => self.apply_kv_cas(&key, &stored_kv_json, expected_modify_index, log_index),
            ConsulRaftRequest::KVTransaction {
                puts,
                deletes,
                session_index_puts,
                session_index_deletes,
            } => self.apply_kv_transaction(
                puts,
                deletes,
                session_index_puts,
                session_index_deletes,
                log_index,
            ),
            ConsulRaftRequest::SessionCreate {
                session_id,
                stored_session_json,
            } => self.apply_session_create(&session_id, &stored_session_json, log_index),
            ConsulRaftRequest::SessionDestroy { session_id } => {
                self.apply_session_destroy(&session_id, log_index)
            }
            ConsulRaftRequest::SessionRenew {
                session_id,
                stored_session_json,
            } => self.apply_session_renew(&session_id, &stored_session_json, log_index),
            ConsulRaftRequest::SessionCleanupExpired {
                expired_session_ids,
            } => self.apply_session_cleanup_expired(expired_session_ids, log_index),
            ConsulRaftRequest::Noop => ConsulRaftResponse::success(),
        }
    }

    // ========================================================================
    // KV apply methods
    //
    // Key difference from old shared state machine: `log_index` is used to
    // set ModifyIndex/CreateIndex on StoredKV entries, matching Consul behavior.
    // ========================================================================

    fn apply_kv_put(
        &self,
        key: &str,
        stored_kv_json: &str,
        session_index_key: Option<&str>,
        log_index: u64,
    ) -> ConsulRaftResponse {
        // Rewrite ModifyIndex/CreateIndex with the Raft log index
        let json = self.rewrite_kv_indexes(key, stored_kv_json, log_index);

        let cf_kv = self.cf_kv();
        if let Some(idx_key) = session_index_key {
            let mut batch = WriteBatch::default();
            batch.put_cf(cf_kv, key.as_bytes(), json.as_bytes());
            batch.put_cf(self.cf_sessions(), idx_key.as_bytes(), b"");
            self.write_batch(batch, "KV put")
        } else {
            match self.db.put_cf(cf_kv, key.as_bytes(), json.as_bytes()) {
                Ok(_) => ConsulRaftResponse::success(),
                Err(e) => ConsulRaftResponse::failure(format!("KV put failed: {}", e)),
            }
        }
    }

    fn apply_kv_delete(
        &self,
        key: &str,
        session_index_cleanup: Option<&str>,
        _log_index: u64,
    ) -> ConsulRaftResponse {
        let cf_kv = self.cf_kv();
        if let Some(idx_key) = session_index_cleanup {
            let mut batch = WriteBatch::default();
            batch.delete_cf(cf_kv, key.as_bytes());
            batch.delete_cf(self.cf_sessions(), idx_key.as_bytes());
            self.write_batch(batch, "KV delete")
        } else {
            match self.db.delete_cf(cf_kv, key.as_bytes()) {
                Ok(_) => ConsulRaftResponse::success(),
                Err(e) => ConsulRaftResponse::failure(format!("KV delete failed: {}", e)),
            }
        }
    }

    fn apply_kv_delete_prefix(&self, prefix: &str, _log_index: u64) -> ConsulRaftResponse {
        let cf_kv = self.cf_kv();
        let cf_sessions = self.cf_sessions();

        let mode = if prefix.is_empty() {
            rocksdb::IteratorMode::Start
        } else {
            rocksdb::IteratorMode::From(prefix.as_bytes(), rocksdb::Direction::Forward)
        };

        let mut batch = WriteBatch::default();
        let mut count = 0u32;

        for item in self.db.iterator_cf(cf_kv, mode).flatten() {
            let (key_bytes, value_bytes) = item;
            let key = match String::from_utf8(key_bytes.to_vec()) {
                Ok(k) => k,
                Err(_) => continue,
            };
            if !prefix.is_empty() && !key.starts_with(prefix) {
                break;
            }

            batch.delete_cf(cf_kv, key.as_bytes());

            // Clean up session index if key had a session
            if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&value_bytes)
                && let Some(pair) = val.get("pair")
                && let Some(session_id) = pair.get("Session").and_then(|s| s.as_str())
            {
                let idx_key = format!("kidx:{}:{}", session_id, key);
                batch.delete_cf(cf_sessions, idx_key.as_bytes());
            }

            count += 1;
        }

        if count > 0 {
            self.write_batch(batch, "KV delete prefix")
        } else {
            ConsulRaftResponse::success()
        }
    }

    fn apply_kv_acquire_session(
        &self,
        key: &str,
        session_id: &str,
        stored_kv_json: &str,
        log_index: u64,
    ) -> ConsulRaftResponse {
        let json = self.rewrite_kv_indexes(key, stored_kv_json, log_index);

        let mut batch = WriteBatch::default();
        batch.put_cf(self.cf_kv(), key.as_bytes(), json.as_bytes());
        let idx_key = format!("kidx:{}:{}", session_id, key);
        batch.put_cf(self.cf_sessions(), idx_key.as_bytes(), b"");
        self.write_batch(batch, "KV acquire session")
    }

    fn apply_kv_release_session_key(
        &self,
        key: &str,
        session_id: &str,
        stored_kv_json: &str,
        log_index: u64,
    ) -> ConsulRaftResponse {
        let json = self.rewrite_kv_indexes(key, stored_kv_json, log_index);

        let mut batch = WriteBatch::default();
        batch.put_cf(self.cf_kv(), key.as_bytes(), json.as_bytes());
        let idx_key = format!("kidx:{}:{}", session_id, key);
        batch.delete_cf(self.cf_sessions(), idx_key.as_bytes());
        self.write_batch(batch, "KV release session key")
    }

    fn apply_kv_release_session(
        &self,
        _session_id: &str,
        updates: Vec<(String, String)>,
        index_keys_to_delete: Vec<String>,
        log_index: u64,
    ) -> ConsulRaftResponse {
        let mut batch = WriteBatch::default();

        for (kv_key, stored_kv_json) in &updates {
            let json = self.rewrite_kv_indexes(kv_key, stored_kv_json, log_index);
            batch.put_cf(self.cf_kv(), kv_key.as_bytes(), json.as_bytes());
        }

        for idx_key in &index_keys_to_delete {
            batch.delete_cf(self.cf_sessions(), idx_key.as_bytes());
        }

        self.write_batch(batch, "KV release session")
    }

    fn apply_kv_cas(
        &self,
        key: &str,
        stored_kv_json: &str,
        expected_modify_index: u64,
        log_index: u64,
    ) -> ConsulRaftResponse {
        let cf_kv = self.cf_kv();

        // Read existing entry and check modify_index
        if let Ok(Some(existing_bytes)) = self.db.get_cf(cf_kv, key.as_bytes()) {
            if let Ok(existing) = serde_json::from_slice::<serde_json::Value>(&existing_bytes) {
                let current_index = existing
                    .get("pair")
                    .and_then(|p| p.get("ModifyIndex"))
                    .and_then(|i| i.as_u64())
                    .unwrap_or(0);

                if current_index != expected_modify_index {
                    return ConsulRaftResponse::failure("CAS failed: index mismatch".into());
                }
            }
        } else if expected_modify_index != 0 {
            return ConsulRaftResponse::failure("CAS failed: key not found".into());
        }

        let json = self.rewrite_kv_indexes(key, stored_kv_json, log_index);
        match self.db.put_cf(cf_kv, key.as_bytes(), json.as_bytes()) {
            Ok(_) => ConsulRaftResponse::success(),
            Err(e) => ConsulRaftResponse::failure(format!("CAS write failed: {}", e)),
        }
    }

    fn apply_kv_transaction(
        &self,
        puts: Vec<(String, String)>,
        deletes: Vec<String>,
        session_index_puts: Vec<String>,
        session_index_deletes: Vec<String>,
        log_index: u64,
    ) -> ConsulRaftResponse {
        let cf_kv = self.cf_kv();
        let cf_sessions = self.cf_sessions();
        let mut batch = WriteBatch::default();

        for (key, stored_kv_json) in &puts {
            let json = self.rewrite_kv_indexes(key, stored_kv_json, log_index);
            batch.put_cf(cf_kv, key.as_bytes(), json.as_bytes());
        }

        for key in &deletes {
            batch.delete_cf(cf_kv, key.as_bytes());
        }

        for idx_key in &session_index_puts {
            batch.put_cf(cf_sessions, idx_key.as_bytes(), b"");
        }

        for idx_key in &session_index_deletes {
            batch.delete_cf(cf_sessions, idx_key.as_bytes());
        }

        self.write_batch(batch, "KV transaction")
    }

    // ========================================================================
    // Session apply methods
    // ========================================================================

    fn apply_session_create(
        &self,
        session_id: &str,
        stored_session_json: &str,
        log_index: u64,
    ) -> ConsulRaftResponse {
        let json = rewrite_json_index(stored_session_json, log_index, true);
        match self
            .db
            .put_cf(self.cf_sessions(), session_id.as_bytes(), json.as_bytes())
        {
            Ok(_) => ConsulRaftResponse::success(),
            Err(e) => ConsulRaftResponse::failure(format!("Session create failed: {}", e)),
        }
    }

    fn apply_session_destroy(&self, session_id: &str, _log_index: u64) -> ConsulRaftResponse {
        match self.db.delete_cf(self.cf_sessions(), session_id.as_bytes()) {
            Ok(_) => ConsulRaftResponse::success(),
            Err(e) => ConsulRaftResponse::failure(format!("Session destroy failed: {}", e)),
        }
    }

    fn apply_session_renew(
        &self,
        session_id: &str,
        stored_session_json: &str,
        log_index: u64,
    ) -> ConsulRaftResponse {
        let json = rewrite_json_index(stored_session_json, log_index, false);
        match self
            .db
            .put_cf(self.cf_sessions(), session_id.as_bytes(), json.as_bytes())
        {
            Ok(_) => ConsulRaftResponse::success(),
            Err(e) => ConsulRaftResponse::failure(format!("Session renew failed: {}", e)),
        }
    }

    fn apply_session_cleanup_expired(
        &self,
        expired_session_ids: Vec<String>,
        _log_index: u64,
    ) -> ConsulRaftResponse {
        let cf = self.cf_sessions();
        let mut batch = WriteBatch::default();
        for id in &expired_session_ids {
            batch.delete_cf(cf, id.as_bytes());
        }
        self.write_batch(batch, "Session cleanup expired")
    }

    // ========================================================================
    // Helpers
    // ========================================================================

    /// Rewrite CreateIndex/ModifyIndex on a StoredKV JSON entry.
    ///
    /// - ModifyIndex is always set to `log_index`
    /// - CreateIndex is set to `log_index` only if the key is new (no existing entry in DB)
    ///   Otherwise, the existing CreateIndex is preserved.
    fn rewrite_kv_indexes(&self, key: &str, stored_kv_json: &str, log_index: u64) -> String {
        let mut val: serde_json::Value = match serde_json::from_str(stored_kv_json) {
            Ok(v) => v,
            Err(_) => return stored_kv_json.to_string(),
        };

        if let Some(pair) = val.get_mut("pair") {
            // Always set ModifyIndex to Raft log index
            pair["ModifyIndex"] = serde_json::json!(log_index);

            // Preserve CreateIndex from existing DB entry, or set to log_index for new keys
            let existing_create_index = self.get_existing_create_index(key);
            if let Some(ci) = existing_create_index {
                pair["CreateIndex"] = serde_json::json!(ci);
            } else {
                pair["CreateIndex"] = serde_json::json!(log_index);
            }
        }

        serde_json::to_string(&val).unwrap_or_else(|_| stored_kv_json.to_string())
    }

    fn get_existing_create_index(&self, key: &str) -> Option<u64> {
        let cf = self.cf_kv();
        let bytes = self.db.get_cf(cf, key.as_bytes()).ok()??;
        let val: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
        val.get("pair")?.get("CreateIndex")?.as_u64()
    }

    fn write_batch(&self, batch: WriteBatch, op: &str) -> ConsulRaftResponse {
        match self.db.write(batch) {
            Ok(_) => {
                debug!("Consul {}", op);
                ConsulRaftResponse::success()
            }
            Err(e) => {
                error!("Consul {} failed: {}", op, e);
                ConsulRaftResponse::failure(format!("{} failed: {}", op, e))
            }
        }
    }
}

/// Rewrite ModifyIndex (and optionally CreateIndex) on a generic JSON entry.
/// Used for session records.
fn rewrite_json_index(json_str: &str, log_index: u64, set_create_index: bool) -> String {
    if let Ok(mut val) = serde_json::from_str::<serde_json::Value>(json_str) {
        val["ModifyIndex"] = serde_json::json!(log_index);
        if set_create_index {
            val["CreateIndex"] = serde_json::json!(log_index);
        }
        serde_json::to_string(&val).unwrap_or_else(|_| json_str.to_string())
    } else {
        json_str.to_string()
    }
}

fn sm_error(e: impl std::fmt::Display) -> StorageError<NodeId> {
    StorageError::from_io_error(
        openraft::ErrorSubject::StateMachine,
        openraft::ErrorVerb::Write,
        std::io::Error::other(e.to_string()),
    )
}

// ============================================================================
// OpenRaft StateMachine implementation
// ============================================================================

impl RaftStateMachine<ConsulTypeConfig> for ConsulStateMachine {
    type SnapshotBuilder = Self;

    async fn applied_state(
        &mut self,
    ) -> Result<(Option<ConsulLogId>, ConsulStoredMembership), StorageError<NodeId>> {
        let applied = self.last_applied.read().await.clone();
        let membership = self.last_membership.read().await.clone();
        Ok((applied, membership))
    }

    async fn apply<I>(
        &mut self,
        entries: I,
    ) -> Result<Vec<ConsulRaftResponse>, StorageError<NodeId>>
    where
        I: IntoIterator<Item = ConsulEntry> + OptionalSend,
    {
        let mut results = Vec::new();

        for entry in entries {
            let log_id = entry.log_id;
            let log_index = log_id.index;

            match entry.payload {
                EntryPayload::Blank => {
                    results.push(ConsulRaftResponse::default());
                }
                EntryPayload::Normal(ref request) => {
                    // Determine which table this request affects
                    let table = match request {
                        ConsulRaftRequest::SessionCreate { .. }
                        | ConsulRaftRequest::SessionDestroy { .. }
                        | ConsulRaftRequest::SessionRenew { .. }
                        | ConsulRaftRequest::SessionCleanupExpired { .. } => ConsulTable::Sessions,
                        _ => ConsulTable::KVS,
                    };
                    let resp = self.apply_request(request.clone(), log_index);
                    // Update table index AFTER apply (data is now in RocksDB)
                    self.table_index.update(table, log_index);
                    results.push(resp);
                }
                EntryPayload::Membership(membership) => {
                    let stored = StoredMembership::new(Some(log_id), membership);
                    self.save_membership(stored).await?;
                    results.push(ConsulRaftResponse::default());
                }
            }

            self.save_last_applied(log_id).await?;
        }

        Ok(results)
    }

    async fn get_snapshot_builder(&mut self) -> Self::SnapshotBuilder {
        // The state machine itself implements RaftSnapshotBuilder.
        // Currently returns an empty snapshot; full implementation TODO.
        ConsulStateMachine {
            db: self.db.clone(),
            last_applied: RwLock::new(self.last_applied.read().await.clone()),
            last_membership: RwLock::new(self.last_membership.read().await.clone()),
            table_index: self.table_index.clone(),
        }
    }

    async fn begin_receiving_snapshot(
        &mut self,
    ) -> Result<Box<std::io::Cursor<Vec<u8>>>, StorageError<NodeId>> {
        Ok(Box::new(std::io::Cursor::new(Vec::new())))
    }

    async fn install_snapshot(
        &mut self,
        _meta: &ConsulSnapshotMeta,
        _snapshot: Box<std::io::Cursor<Vec<u8>>>,
    ) -> Result<(), StorageError<NodeId>> {
        // TODO: Implement snapshot installation
        Ok(())
    }

    async fn get_current_snapshot(
        &mut self,
    ) -> Result<Option<openraft::Snapshot<ConsulTypeConfig>>, StorageError<NodeId>> {
        // TODO: Implement snapshot retrieval
        Ok(None)
    }
}

// Snapshot builder — placeholder
impl openraft::RaftSnapshotBuilder<ConsulTypeConfig> for ConsulStateMachine {
    async fn build_snapshot(
        &mut self,
    ) -> Result<openraft::Snapshot<ConsulTypeConfig>, StorageError<NodeId>> {
        // TODO: Build actual snapshot from RocksDB
        let applied = self.last_applied.read().await.clone();
        let membership = self.last_membership.read().await.clone();

        let meta = ConsulSnapshotMeta {
            last_log_id: applied,
            last_membership: membership,
            snapshot_id: format!("consul-{}", applied.map(|l| l.index).unwrap_or(0)),
        };

        Ok(openraft::Snapshot {
            meta,
            snapshot: Box::new(std::io::Cursor::new(Vec::new())),
        })
    }
}
