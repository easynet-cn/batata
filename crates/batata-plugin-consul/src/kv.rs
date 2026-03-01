// Consul KV Store API HTTP handlers
// Implements Consul-compatible key-value store endpoints
// Uses RocksDB as the sole storage backend

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use actix_web::{HttpRequest, HttpResponse, web};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use rocksdb::{DB, WriteBatch};
use serde::{Deserialize, Serialize};
use tokio::sync::Notify;
use tracing::{error, info};

use batata_consistency::RaftNode;
use batata_consistency::raft::request::RaftRequest;
use batata_consistency::raft::state_machine::{CF_CONSUL_KV, CF_CONSUL_SESSIONS};

use crate::acl::{AclService, ResourceType};
use crate::model::ConsulError;

// ============================================================================
// KV Store Models
// ============================================================================

/// Consul KV Pair
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KVPair {
    #[serde(rename = "Key")]
    pub key: String,

    #[serde(rename = "CreateIndex")]
    pub create_index: u64,

    #[serde(rename = "ModifyIndex")]
    pub modify_index: u64,

    #[serde(rename = "LockIndex")]
    pub lock_index: u64,

    #[serde(rename = "Flags")]
    pub flags: u64,

    #[serde(rename = "Value", skip_serializing_if = "Option::is_none")]
    pub value: Option<String>, // Base64 encoded

    #[serde(rename = "Session", skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
}

impl KVPair {
    /// Create a new KV pair with encoded value
    pub fn new(key: String, value: &str) -> Self {
        let now = current_index();
        Self {
            key,
            create_index: now,
            modify_index: now,
            lock_index: 0,
            flags: 0,
            value: Some(BASE64.encode(value.as_bytes())),
            session: None,
        }
    }

    /// Create an empty KV pair (for keys-only response)
    pub fn key_only(key: String) -> Self {
        Self {
            key,
            create_index: 0,
            modify_index: 0,
            lock_index: 0,
            flags: 0,
            value: None,
            session: None,
        }
    }

    /// Decode base64 value to string
    pub fn decoded_value(&self) -> Option<String> {
        self.value.as_ref().and_then(|v| {
            BASE64
                .decode(v)
                .ok()
                .and_then(|bytes| String::from_utf8(bytes).ok())
        })
    }

    /// Get raw bytes of the value
    pub fn raw_value(&self) -> Option<Vec<u8>> {
        self.value.as_ref().and_then(|v| BASE64.decode(v).ok())
    }
}

/// Stored KV entry with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredKV {
    pair: KVPair,
    created_at: i64,
    modified_at: i64,
}

/// Query parameters for KV endpoints
#[derive(Debug, Clone, Deserialize, Default)]
pub struct KVQueryParams {
    /// Return raw value (not JSON wrapped)
    #[serde(default, deserialize_with = "crate::model::consul_bool::deserialize")]
    pub raw: Option<bool>,

    /// Return only keys (no values)
    #[serde(default, deserialize_with = "crate::model::consul_bool::deserialize")]
    pub keys: Option<bool>,

    /// Recursively get all keys under prefix
    #[serde(default, deserialize_with = "crate::model::consul_bool::deserialize")]
    pub recurse: Option<bool>,

    /// Check-and-set index for conditional writes
    #[serde(default, deserialize_with = "crate::model::consul_u64::deserialize")]
    pub cas: Option<u64>,

    /// Custom flags to store with the key
    #[serde(default, deserialize_with = "crate::model::consul_u64::deserialize")]
    pub flags: Option<u64>,

    /// Datacenter
    pub dc: Option<String>,

    /// Namespace (Enterprise)
    pub ns: Option<String>,

    /// Separator for keys listing
    pub separator: Option<String>,

    /// Wait for index to be >= this value (blocking wait for watch)
    #[serde(default, deserialize_with = "crate::model::consul_u64::deserialize")]
    pub index: Option<u64>,

    /// Wait time in milliseconds for blocking wait
    #[serde(default, deserialize_with = "crate::model::consul_u64::deserialize")]
    pub wait: Option<u64>,

    /// Acquire session lock
    pub acquire: Option<String>,

    /// Release session lock
    pub release: Option<String>,
}

/// Transaction operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxnOp {
    #[serde(rename = "KV", skip_serializing_if = "Option::is_none")]
    pub kv: Option<KVTxnOp>,
}

/// KV transaction operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KVTxnOp {
    #[serde(rename = "Verb")]
    pub verb: String, // "set", "get", "delete", "cas", "delete-cas", "delete-tree"

    #[serde(rename = "Key")]
    pub key: String,

    #[serde(rename = "Value", skip_serializing_if = "Option::is_none")]
    pub value: Option<String>, // Base64 encoded

    #[serde(rename = "Flags", skip_serializing_if = "Option::is_none")]
    pub flags: Option<u64>,

    #[serde(rename = "Index", skip_serializing_if = "Option::is_none")]
    pub index: Option<u64>,
}

/// Transaction result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxnResult {
    #[serde(rename = "Results", skip_serializing_if = "Option::is_none")]
    pub results: Option<Vec<TxnResultItem>>,

    #[serde(rename = "Errors", skip_serializing_if = "Option::is_none")]
    pub errors: Option<Vec<TxnError>>,
}

/// Transaction result item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxnResultItem {
    #[serde(rename = "KV")]
    pub kv: KVPair,
}

/// Transaction error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxnError {
    #[serde(rename = "OpIndex")]
    pub op_index: u32,

    #[serde(rename = "What")]
    pub what: String,
}

// ============================================================================
// KV Store Service
// ============================================================================

/// Consul KV Store service
/// Uses RocksDB as the sole storage backend.
/// In cluster mode, writes go through Raft for replication.
#[derive(Clone)]
pub struct ConsulKVService {
    /// RocksDB handle (always present)
    db: Arc<DB>,
    /// Global index counter
    index: Arc<std::sync::atomic::AtomicU64>,
    /// Notification channel for blocking queries
    notify: Arc<Notify>,
    /// Keeps temp directory alive for tests/in-memory mode
    _temp_dir: Option<Arc<tempfile::TempDir>>,
    /// Optional Raft node for cluster mode replication
    raft_node: Option<Arc<RaftNode>>,
}

impl ConsulKVService {
    /// Create a new KV service with a temporary RocksDB (for tests/in-memory mode).
    pub fn new() -> Self {
        let (db, temp_dir) = open_temp_consul_db();
        Self {
            db,
            index: Arc::new(std::sync::atomic::AtomicU64::new(1)),
            notify: Arc::new(Notify::new()),
            _temp_dir: Some(temp_dir),
            raft_node: None,
        }
    }

    /// Create a new KV service backed by an existing RocksDB instance.
    /// Scans existing entries to initialize the index counter.
    pub fn with_rocks(db: Arc<DB>) -> Self {
        let max_index = Self::scan_max_index(&db);

        Self {
            db,
            index: Arc::new(std::sync::atomic::AtomicU64::new(max_index + 1)),
            notify: Arc::new(Notify::new()),
            _temp_dir: None,
            raft_node: None,
        }
    }

    /// Create a new KV service backed by a Raft-replicated RocksDB instance.
    /// Writes go through Raft consensus; reads are local.
    pub fn with_raft(db: Arc<DB>, raft_node: Arc<RaftNode>) -> Self {
        let max_index = Self::scan_max_index(&db);

        Self {
            db,
            index: Arc::new(std::sync::atomic::AtomicU64::new(max_index + 1)),
            notify: Arc::new(Notify::new()),
            _temp_dir: None,
            raft_node: Some(raft_node),
        }
    }

    /// Scan existing KV entries to find the max modify_index.
    fn scan_max_index(db: &Arc<DB>) -> u64 {
        let mut max_index = 0u64;

        if let Some(cf) = db.cf_handle(CF_CONSUL_KV) {
            let iter = db.iterator_cf(cf, rocksdb::IteratorMode::Start);
            let mut count = 0u64;
            for item in iter.flatten() {
                let (_key_bytes, value_bytes) = item;
                if let Ok(stored) = serde_json::from_slice::<StoredKV>(&value_bytes) {
                    if stored.pair.modify_index > max_index {
                        max_index = stored.pair.modify_index;
                    }
                    count += 1;
                }
            }
            info!(
                "Scanned {} KV entries from RocksDB (max_index={})",
                count, max_index
            );
        }

        max_index
    }

    // ========================================================================
    // Internal helpers
    // ========================================================================

    /// Get the next index
    fn next_index(&self) -> u64 {
        self.index.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }

    /// Get current index for watch operations
    pub fn current_index(&self) -> u64 {
        self.index.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Read a stored KV entry from RocksDB
    fn get_stored(&self, key: &str) -> Option<StoredKV> {
        let cf = self.db.cf_handle(CF_CONSUL_KV)?;
        let bytes = self.db.get_cf(cf, key.as_bytes()).ok()??;
        serde_json::from_slice(&bytes).ok()
    }

    /// Write a stored KV entry to RocksDB
    fn put_stored(&self, key: &str, stored: &StoredKV) {
        if let Some(cf) = self.db.cf_handle(CF_CONSUL_KV) {
            match serde_json::to_vec(stored) {
                Ok(bytes) => {
                    if let Err(e) = self.db.put_cf(cf, key.as_bytes(), &bytes) {
                        error!("Failed to write KV entry '{}': {}", key, e);
                    }
                }
                Err(e) => error!("Failed to serialize KV entry '{}': {}", key, e),
            }
        }
    }

    /// Write a stored KV entry into a WriteBatch
    fn batch_put_stored(&self, batch: &mut WriteBatch, key: &str, stored: &StoredKV) {
        if let Some(cf) = self.db.cf_handle(CF_CONSUL_KV) {
            match serde_json::to_vec(stored) {
                Ok(bytes) => {
                    batch.put_cf(cf, key.as_bytes(), &bytes);
                }
                Err(e) => error!("Failed to serialize KV entry '{}': {}", key, e),
            }
        }
    }

    // ========================================================================
    // Public API
    // ========================================================================

    /// Wait for index to change (event-driven with tokio::sync::Notify)
    /// Returns true if index reached, false on timeout
    pub async fn wait_for_index(&self, target_index: u64, timeout_ms: u64) -> bool {
        if self.current_index() > target_index {
            return true;
        }
        tokio::select! {
            _ = async {
                loop {
                    self.notify.notified().await;
                    if self.current_index() > target_index {
                        break;
                    }
                }
            } => true,
            _ = tokio::time::sleep(Duration::from_millis(timeout_ms)) => {
                self.current_index() > target_index
            }
        }
    }

    /// Get a single key
    pub fn get(&self, key: &str) -> Option<KVPair> {
        self.get_stored(key).map(|s| s.pair)
    }

    /// Get keys with prefix (recursive) — O(k) via sorted iterator
    pub fn get_prefix(&self, prefix: &str) -> Vec<KVPair> {
        let Some(cf) = self.db.cf_handle(CF_CONSUL_KV) else {
            return Vec::new();
        };
        let mode = if prefix.is_empty() {
            rocksdb::IteratorMode::Start
        } else {
            rocksdb::IteratorMode::From(prefix.as_bytes(), rocksdb::Direction::Forward)
        };
        let iter = self.db.iterator_cf(cf, mode);
        let mut results = Vec::new();
        for item in iter.flatten() {
            let (key_bytes, value_bytes) = item;
            let key = match String::from_utf8(key_bytes.to_vec()) {
                Ok(k) => k,
                Err(_) => continue,
            };
            if !prefix.is_empty() && !key.starts_with(prefix) {
                break; // Sorted order, no more matches
            }
            if let Ok(stored) = serde_json::from_slice::<StoredKV>(&value_bytes) {
                results.push(stored.pair);
            }
        }
        results
    }

    /// Get keys only (no values) — O(k) via sorted iterator
    pub fn get_keys(&self, prefix: &str, separator: Option<&str>) -> Vec<String> {
        let Some(cf) = self.db.cf_handle(CF_CONSUL_KV) else {
            return Vec::new();
        };
        let mode = if prefix.is_empty() {
            rocksdb::IteratorMode::Start
        } else {
            rocksdb::IteratorMode::From(prefix.as_bytes(), rocksdb::Direction::Forward)
        };
        let iter = self.db.iterator_cf(cf, mode);
        let mut keys = Vec::new();
        for item in iter.flatten() {
            let (key_bytes, _) = item;
            let key = match String::from_utf8(key_bytes.to_vec()) {
                Ok(k) => k,
                Err(_) => continue,
            };
            if !prefix.is_empty() && !key.starts_with(prefix) {
                break;
            }
            keys.push(key);
        }

        // Handle separator for folder-like listing
        if let Some(sep) = separator {
            let prefix_len = prefix.len();
            let mut unique_keys: std::collections::HashSet<String> =
                std::collections::HashSet::new();

            for key in keys.drain(..) {
                let remainder = &key[prefix_len..];
                if let Some(pos) = remainder.find(sep) {
                    // Include up to and including the separator
                    unique_keys.insert(format!("{}{}", prefix, &remainder[..=pos]));
                } else {
                    unique_keys.insert(key);
                }
            }

            keys = unique_keys.into_iter().collect();
        }

        keys.sort();
        keys
    }

    /// Build a StoredKV for a put operation (new or update).
    fn build_stored_kv(
        &self,
        key: &str,
        encoded_value: Option<String>,
        flags: Option<u64>,
    ) -> StoredKV {
        let index = self.next_index();
        let now = current_timestamp();

        if let Some(mut existing) = self.get_stored(key) {
            existing.pair.modify_index = index;
            existing.pair.value = encoded_value;
            if let Some(f) = flags {
                existing.pair.flags = f;
            }
            existing.modified_at = now;
            existing
        } else {
            let pair = KVPair {
                key: key.to_string(),
                create_index: index,
                modify_index: index,
                lock_index: 0,
                flags: flags.unwrap_or(0),
                value: encoded_value,
                session: None,
            };
            StoredKV {
                pair,
                created_at: now,
                modified_at: now,
            }
        }
    }

    /// Put a key-value pair with pre-encoded base64 value (for transactions).
    pub async fn put_base64(
        &self,
        key: String,
        base64_value: Option<String>,
        flags: Option<u64>,
    ) -> KVPair {
        let stored = self.build_stored_kv(&key, base64_value, flags);

        if let Some(ref raft) = self.raft_node {
            let json = serde_json::to_string(&stored).unwrap_or_default();
            match raft
                .write(RaftRequest::ConsulKVPut {
                    key: key.clone(),
                    value: json,
                    session_index_key: None,
                })
                .await
            {
                Ok(r) if r.success => {
                    self.notify.notify_waiters();
                }
                Ok(r) => {
                    error!("Raft ConsulKVPut rejected: {:?}", r.message);
                }
                Err(e) => {
                    error!("Raft ConsulKVPut failed: {}", e);
                }
            }
        } else {
            self.put_stored(&key, &stored);
            self.notify.notify_waiters();
        }

        stored.pair
    }

    /// Put a key-value pair
    pub async fn put(&self, key: String, value: &str, flags: Option<u64>) -> KVPair {
        let encoded = Some(BASE64.encode(value.as_bytes()));
        let stored = self.build_stored_kv(&key, encoded, flags);

        if let Some(ref raft) = self.raft_node {
            let json = serde_json::to_string(&stored).unwrap_or_default();
            match raft
                .write(RaftRequest::ConsulKVPut {
                    key: key.clone(),
                    value: json,
                    session_index_key: None,
                })
                .await
            {
                Ok(r) if r.success => {
                    self.notify.notify_waiters();
                }
                Ok(r) => {
                    error!("Raft ConsulKVPut rejected: {:?}", r.message);
                }
                Err(e) => {
                    error!("Raft ConsulKVPut failed: {}", e);
                }
            }
        } else {
            self.put_stored(&key, &stored);
            self.notify.notify_waiters();
        }

        stored.pair
    }

    /// Check-and-set: only update if modify_index matches
    pub async fn cas(&self, key: String, value: &str, cas_index: u64, flags: Option<u64>) -> bool {
        if let Some(mut existing) = self.get_stored(&key) {
            if existing.pair.modify_index == cas_index {
                let index = self.next_index();
                existing.pair.modify_index = index;
                existing.pair.value = Some(BASE64.encode(value.as_bytes()));
                if let Some(f) = flags {
                    existing.pair.flags = f;
                }
                existing.modified_at = current_timestamp();

                if let Some(ref raft) = self.raft_node {
                    let json = serde_json::to_string(&existing).unwrap_or_default();
                    match raft
                        .write(RaftRequest::ConsulKVCas {
                            key: key.clone(),
                            stored_kv_json: json,
                            expected_modify_index: cas_index,
                        })
                        .await
                    {
                        Ok(r) if r.success => {
                            self.notify.notify_waiters();
                            return true;
                        }
                        Ok(_) => return false,
                        Err(e) => {
                            error!("Raft ConsulKVCas failed: {}", e);
                            return false;
                        }
                    }
                } else {
                    self.put_stored(&key, &existing);
                    self.notify.notify_waiters();
                    return true;
                }
            }
            false
        } else if cas_index == 0 {
            // cas=0 means create only if doesn't exist
            self.put(key, value, flags).await;
            true
        } else {
            false
        }
    }

    /// Acquire a session lock on a key.
    /// Returns false if the key is locked by a different session, or the key does not exist.
    pub async fn acquire_session(&self, key: &str, session_id: &str) -> bool {
        let Some(mut stored) = self.get_stored(key) else {
            return false;
        };

        // Check if already locked by another session
        if let Some(ref holder) = stored.pair.session
            && holder != session_id
        {
            return false;
        }

        stored.pair.session = Some(session_id.to_string());
        stored.pair.lock_index += 1;
        stored.pair.modify_index = self.next_index();
        stored.modified_at = current_timestamp();

        if let Some(ref raft) = self.raft_node {
            let json = serde_json::to_string(&stored).unwrap_or_default();
            match raft
                .write(RaftRequest::ConsulKVAcquireSession {
                    key: key.to_string(),
                    session_id: session_id.to_string(),
                    stored_kv_json: json,
                })
                .await
            {
                Ok(r) if r.success => {
                    self.notify.notify_waiters();
                    true
                }
                Ok(r) => {
                    error!("Raft ConsulKVAcquireSession rejected: {:?}", r.message);
                    false
                }
                Err(e) => {
                    error!("Raft ConsulKVAcquireSession failed: {}", e);
                    false
                }
            }
        } else {
            // Standalone: direct RocksDB write
            let mut batch = WriteBatch::default();
            self.batch_put_stored(&mut batch, key, &stored);
            if let Some(cf_sessions) = self.db.cf_handle(CF_CONSUL_SESSIONS) {
                let idx_key = format!("kidx:{}:{}", session_id, key);
                batch.put_cf(cf_sessions, idx_key.as_bytes(), b"");
            }

            if let Err(e) = self.db.write(batch) {
                error!("Failed to acquire session on '{}': {}", key, e);
                return false;
            }

            self.notify.notify_waiters();
            true
        }
    }

    /// Release a session lock on a single key.
    /// Returns false if the key is not held by the given session.
    pub async fn release_session_key(&self, key: &str, session_id: &str) -> bool {
        let Some(mut stored) = self.get_stored(key) else {
            return false;
        };

        if stored.pair.session.as_deref() != Some(session_id) {
            return false;
        }

        stored.pair.session = None;
        stored.pair.modify_index = self.next_index();
        stored.modified_at = current_timestamp();

        if let Some(ref raft) = self.raft_node {
            let json = serde_json::to_string(&stored).unwrap_or_default();
            match raft
                .write(RaftRequest::ConsulKVReleaseSessionKey {
                    key: key.to_string(),
                    session_id: session_id.to_string(),
                    stored_kv_json: json,
                })
                .await
            {
                Ok(r) if r.success => {
                    self.notify.notify_waiters();
                    true
                }
                Ok(r) => {
                    error!("Raft ConsulKVReleaseSessionKey rejected: {:?}", r.message);
                    false
                }
                Err(e) => {
                    error!("Raft ConsulKVReleaseSessionKey failed: {}", e);
                    false
                }
            }
        } else {
            let mut batch = WriteBatch::default();
            self.batch_put_stored(&mut batch, key, &stored);
            if let Some(cf_sessions) = self.db.cf_handle(CF_CONSUL_SESSIONS) {
                let idx_key = format!("kidx:{}:{}", session_id, key);
                batch.delete_cf(cf_sessions, idx_key.as_bytes());
            }

            if let Err(e) = self.db.write(batch) {
                error!("Failed to release session on '{}': {}", key, e);
                return false;
            }

            self.notify.notify_waiters();
            true
        }
    }

    /// Release all KV keys held by a session (called on session destroy).
    /// Uses session-to-key index in CF_CONSUL_SESSIONS for O(k) lookup.
    pub async fn release_session(&self, session_id: &str) {
        let Some(cf_kv) = self.db.cf_handle(CF_CONSUL_KV) else {
            return;
        };
        let Some(cf_sessions) = self.db.cf_handle(CF_CONSUL_SESSIONS) else {
            return;
        };

        let idx_prefix = format!("kidx:{}:", session_id);
        let iter = self.db.iterator_cf(
            cf_sessions,
            rocksdb::IteratorMode::From(idx_prefix.as_bytes(), rocksdb::Direction::Forward),
        );

        // Collect updates for both Raft and standalone paths
        let mut updates: Vec<(String, String)> = Vec::new();
        let mut index_keys_to_delete: Vec<String> = Vec::new();

        for item in iter.flatten() {
            let (idx_key_bytes, _) = item;
            let idx_key = match String::from_utf8(idx_key_bytes.to_vec()) {
                Ok(k) => k,
                Err(_) => continue,
            };
            if !idx_key.starts_with(&idx_prefix) {
                break;
            }

            // Extract KV key from index key: "kidx:{session_id}:{kv_key}"
            let kv_key = &idx_key[idx_prefix.len()..];

            // Read and update the KV entry
            if let Ok(Some(kv_bytes)) = self.db.get_cf(cf_kv, kv_key.as_bytes())
                && let Ok(mut stored) = serde_json::from_slice::<StoredKV>(&kv_bytes)
                && stored.pair.session.as_deref() == Some(session_id)
            {
                stored.pair.session = None;
                stored.pair.modify_index = self.next_index();
                stored.modified_at = current_timestamp();
                if let Ok(json) = serde_json::to_string(&stored) {
                    updates.push((kv_key.to_string(), json));
                }
            }

            index_keys_to_delete.push(idx_key);
        }

        if updates.is_empty() && index_keys_to_delete.is_empty() {
            return;
        }

        if let Some(ref raft) = self.raft_node {
            match raft
                .write(RaftRequest::ConsulKVReleaseSession {
                    session_id: session_id.to_string(),
                    updates,
                    index_keys_to_delete,
                })
                .await
            {
                Ok(r) if r.success => {
                    self.notify.notify_waiters();
                }
                Ok(r) => {
                    error!("Raft ConsulKVReleaseSession rejected: {:?}", r.message);
                }
                Err(e) => {
                    error!("Raft ConsulKVReleaseSession failed: {}", e);
                }
            }
        } else {
            let mut batch = WriteBatch::default();
            for (kv_key, json) in &updates {
                batch.put_cf(cf_kv, kv_key.as_bytes(), json.as_bytes());
            }
            for idx_key in &index_keys_to_delete {
                batch.delete_cf(cf_sessions, idx_key.as_bytes());
            }
            if let Err(e) = self.db.write(batch) {
                error!("Failed to release session '{}': {}", session_id, e);
                return;
            }
            self.notify.notify_waiters();
        }
    }

    /// Delete a key
    pub async fn delete(&self, key: &str) -> bool {
        // Check if key exists and get session info for cleanup
        let stored = self.get_stored(key);
        if stored.is_none() {
            return false;
        }
        let stored = stored.unwrap();

        let session_index_cleanup = stored
            .pair
            .session
            .as_ref()
            .map(|sid| format!("kidx:{}:{}", sid, key));

        if let Some(ref raft) = self.raft_node {
            match raft
                .write(RaftRequest::ConsulKVDelete {
                    key: key.to_string(),
                    session_index_cleanup,
                })
                .await
            {
                Ok(r) if r.success => {
                    self.notify.notify_waiters();
                    true
                }
                Ok(r) => {
                    error!("Raft ConsulKVDelete rejected: {:?}", r.message);
                    false
                }
                Err(e) => {
                    error!("Raft ConsulKVDelete failed: {}", e);
                    false
                }
            }
        } else {
            let Some(cf) = self.db.cf_handle(CF_CONSUL_KV) else {
                return false;
            };

            let mut batch = WriteBatch::default();
            batch.delete_cf(cf, key.as_bytes());

            if let Some(ref idx_key) = session_index_cleanup
                && let Some(cf_sessions) = self.db.cf_handle(CF_CONSUL_SESSIONS)
            {
                batch.delete_cf(cf_sessions, idx_key.as_bytes());
            }

            if let Err(e) = self.db.write(batch) {
                error!("Failed to delete KV entry '{}': {}", key, e);
                return false;
            }
            self.notify.notify_waiters();
            true
        }
    }

    /// Delete keys with prefix — O(k) via sorted iterator + WriteBatch
    pub async fn delete_prefix(&self, prefix: &str) -> u32 {
        if let Some(ref raft) = self.raft_node {
            // For Raft mode, send the prefix to the state machine which handles
            // the scan + batch delete atomically on all nodes
            match raft
                .write(RaftRequest::ConsulKVDeletePrefix {
                    prefix: prefix.to_string(),
                })
                .await
            {
                Ok(r) if r.success => {
                    self.notify.notify_waiters();
                    // We don't know exact count from Raft response, return 1 to indicate success
                    1
                }
                Ok(r) => {
                    error!("Raft ConsulKVDeletePrefix rejected: {:?}", r.message);
                    0
                }
                Err(e) => {
                    error!("Raft ConsulKVDeletePrefix failed: {}", e);
                    0
                }
            }
        } else {
            let Some(cf_kv) = self.db.cf_handle(CF_CONSUL_KV) else {
                return 0;
            };

            let mode = if prefix.is_empty() {
                rocksdb::IteratorMode::Start
            } else {
                rocksdb::IteratorMode::From(prefix.as_bytes(), rocksdb::Direction::Forward)
            };
            let iter = self.db.iterator_cf(cf_kv, mode);

            let mut batch = WriteBatch::default();
            let mut count = 0u32;

            for item in iter.flatten() {
                let (key_bytes, value_bytes) = item;
                let key = match String::from_utf8(key_bytes.to_vec()) {
                    Ok(k) => k,
                    Err(_) => continue,
                };
                if !prefix.is_empty() && !key.starts_with(prefix) {
                    break;
                }

                batch.delete_cf(cf_kv, key.as_bytes());

                // Clean up session index if this key had a session
                if let Ok(stored) = serde_json::from_slice::<StoredKV>(&value_bytes)
                    && let Some(ref session_id) = stored.pair.session
                    && let Some(cf_sessions) = self.db.cf_handle(CF_CONSUL_SESSIONS)
                {
                    let idx_key = format!("kidx:{}:{}", session_id, &key);
                    batch.delete_cf(cf_sessions, idx_key.as_bytes());
                }

                count += 1;
            }

            if count > 0 {
                if let Err(e) = self.db.write(batch) {
                    error!("Failed to delete prefix '{}': {}", prefix, e);
                    return 0;
                }
                self.notify.notify_waiters();
            }
            count
        }
    }

    /// Export all KV pairs
    pub fn export_all(&self) -> Vec<KVPair> {
        self.get_prefix("")
    }

    /// Increment lock_index on a key (for transaction "lock" verb)
    fn increment_lock_index(&self, key: &str) {
        if let Some(mut stored) = self.get_stored(key) {
            stored.pair.lock_index += 1;
            self.put_stored(key, &stored);
        }
    }

    /// Clear session on a key (for transaction "unlock" verb)
    fn clear_session(&self, key: &str) -> Option<KVPair> {
        let mut stored = self.get_stored(key)?;
        let old_session = stored.pair.session.take();
        stored.pair.modify_index = self.next_index();
        stored.modified_at = current_timestamp();

        let mut batch = WriteBatch::default();
        self.batch_put_stored(&mut batch, key, &stored);

        // Clean up session index
        if let Some(ref session_id) = old_session
            && let Some(cf_sessions) = self.db.cf_handle(CF_CONSUL_SESSIONS)
        {
            let idx_key = format!("kidx:{}:{}", session_id, key);
            batch.delete_cf(cf_sessions, idx_key.as_bytes());
        }

        if let Err(e) = self.db.write(batch) {
            error!("Failed to clear session on '{}': {}", key, e);
            return None;
        }

        self.notify.notify_waiters();
        Some(stored.pair)
    }

    /// Execute a transaction with two-phase validation
    pub async fn transaction(&self, ops: Vec<TxnOp>) -> TxnResult {
        // Phase 1: Validate all operations and collect planned changes
        let mut errors: Vec<TxnError> = Vec::new();

        // Validate check-index and check-not-exists first
        for (idx, op) in ops.iter().enumerate() {
            if let Some(ref kv_op) = op.kv {
                match kv_op.verb.to_lowercase().as_str() {
                    "check-index" => {
                        let expected_index = kv_op.index.unwrap_or(0);
                        match self.get(&kv_op.key) {
                            Some(pair) => {
                                if pair.modify_index != expected_index {
                                    errors.push(TxnError {
                                        op_index: idx as u32,
                                        what: format!(
                                            "current modify index {} does not match expected {}",
                                            pair.modify_index, expected_index
                                        ),
                                    });
                                }
                            }
                            None => {
                                errors.push(TxnError {
                                    op_index: idx as u32,
                                    what: format!(
                                        "key '{}' doesn't exist for check-index",
                                        kv_op.key
                                    ),
                                });
                            }
                        }
                    }
                    "check-not-exists" => {
                        if self.get(&kv_op.key).is_some() {
                            errors.push(TxnError {
                                op_index: idx as u32,
                                what: format!("key '{}' exists when it should not", kv_op.key),
                            });
                        }
                    }
                    "cas" => {
                        let cas_index = kv_op.index.unwrap_or(0);
                        if cas_index > 0 {
                            match self.get(&kv_op.key) {
                                Some(pair) => {
                                    if pair.modify_index != cas_index {
                                        errors.push(TxnError {
                                            op_index: idx as u32,
                                            what: "CAS failed: index mismatch".to_string(),
                                        });
                                    }
                                }
                                None => {
                                    errors.push(TxnError {
                                        op_index: idx as u32,
                                        what: format!("key '{}' not found", kv_op.key),
                                    });
                                }
                            }
                        }
                    }
                    "delete-cas" => {
                        let cas_index = kv_op.index.unwrap_or(0);
                        match self.get(&kv_op.key) {
                            Some(pair) => {
                                if pair.modify_index != cas_index {
                                    errors.push(TxnError {
                                        op_index: idx as u32,
                                        what: "CAS failed: index mismatch".to_string(),
                                    });
                                }
                            }
                            None => {
                                errors.push(TxnError {
                                    op_index: idx as u32,
                                    what: format!("key '{}' not found", kv_op.key),
                                });
                            }
                        }
                    }
                    "get" | "set" | "delete" | "delete-tree" | "get-tree" | "lock" | "unlock" => {}
                    verb => {
                        errors.push(TxnError {
                            op_index: idx as u32,
                            what: format!("unknown verb: {}", verb),
                        });
                    }
                }
            }
        }

        // If validation errors, return immediately without applying anything
        if !errors.is_empty() {
            return TxnResult {
                results: None,
                errors: Some(errors),
            };
        }

        // Phase 2: Apply all operations
        let mut results: Vec<TxnResultItem> = Vec::new();

        for op in ops.into_iter() {
            if let Some(kv_op) = op.kv {
                match kv_op.verb.to_lowercase().as_str() {
                    "get" => {
                        if let Some(pair) = self.get(&kv_op.key) {
                            results.push(TxnResultItem { kv: pair });
                        }
                    }
                    "get-tree" => {
                        let pairs = self.get_prefix(&kv_op.key);
                        for pair in pairs {
                            results.push(TxnResultItem { kv: pair });
                        }
                    }
                    "set" => {
                        let base64_val = txn_value_base64(&kv_op.value);
                        let pair = self.put_base64(kv_op.key, base64_val, kv_op.flags).await;
                        results.push(TxnResultItem { kv: pair });
                    }
                    "cas" => {
                        let value = decode_txn_value(&kv_op.value);
                        let cas_index = kv_op.index.unwrap_or(0);
                        self.cas(kv_op.key.clone(), &value, cas_index, kv_op.flags)
                            .await;
                        if let Some(pair) = self.get(&kv_op.key) {
                            results.push(TxnResultItem { kv: pair });
                        }
                    }
                    "delete" => {
                        self.delete(&kv_op.key).await;
                        results.push(TxnResultItem {
                            kv: KVPair::key_only(kv_op.key),
                        });
                    }
                    "delete-tree" => {
                        self.delete_prefix(&kv_op.key).await;
                    }
                    "delete-cas" => {
                        self.delete(&kv_op.key).await;
                        results.push(TxnResultItem {
                            kv: KVPair::key_only(kv_op.key),
                        });
                    }
                    "check-index" | "check-not-exists" => {
                        // Already validated in phase 1, these are check-only verbs
                        if let Some(pair) = self.get(&kv_op.key) {
                            results.push(TxnResultItem { kv: pair });
                        }
                    }
                    "lock" => {
                        let base64_val = txn_value_base64(&kv_op.value);
                        let pair = self
                            .put_base64(kv_op.key.clone(), base64_val, kv_op.flags)
                            .await;
                        self.increment_lock_index(&kv_op.key);
                        results.push(TxnResultItem { kv: pair });
                    }
                    "unlock" => {
                        if let Some(pair) = self.clear_session(&kv_op.key) {
                            results.push(TxnResultItem { kv: pair });
                        }
                    }
                    _ => {}
                }
            }
        }

        TxnResult {
            results: if results.is_empty() {
                None
            } else {
                Some(results)
            },
            errors: None,
        }
    }
}

/// Get the base64 value from a transaction operation.
/// The Go SDK sends values already base64-encoded. Since KVPair stores values
/// as base64, we pass them through directly without decode/re-encode.
fn txn_value_base64(value: &Option<String>) -> Option<String> {
    value.clone().filter(|v| !v.is_empty())
}

/// Decode base64 value from transaction operation to a UTF-8 string for put().
/// Falls back to lossy conversion for non-UTF-8 binary data.
fn decode_txn_value(value: &Option<String>) -> String {
    value
        .as_ref()
        .and_then(|v| BASE64.decode(v).ok())
        .map(|bytes| String::from_utf8_lossy(&bytes).to_string())
        .unwrap_or_default()
}

impl Default for ConsulKVService {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Shared helper: open a temporary RocksDB with all Consul column families
// ============================================================================

/// Open a temporary RocksDB instance with all Consul column families.
/// Used for tests and in-memory mode.
pub fn open_temp_consul_db() -> (Arc<DB>, Arc<tempfile::TempDir>) {
    use batata_consistency::raft::state_machine::{CF_CONSUL_ACL, CF_CONSUL_QUERIES};

    let temp_dir = Arc::new(tempfile::tempdir().expect("Failed to create temp directory"));
    let mut db_opts = rocksdb::Options::default();
    db_opts.create_if_missing(true);
    db_opts.create_missing_column_families(true);

    let cf_opts = rocksdb::Options::default();
    let cfs = vec![
        rocksdb::ColumnFamilyDescriptor::new(CF_CONSUL_KV, cf_opts.clone()),
        rocksdb::ColumnFamilyDescriptor::new(CF_CONSUL_SESSIONS, cf_opts.clone()),
        rocksdb::ColumnFamilyDescriptor::new(CF_CONSUL_ACL, cf_opts.clone()),
        rocksdb::ColumnFamilyDescriptor::new(CF_CONSUL_QUERIES, cf_opts),
    ];

    let db = DB::open_cf_descriptors(&db_opts, temp_dir.path(), cfs)
        .expect("Failed to open temp RocksDB");
    (Arc::new(db), temp_dir)
}

// ============================================================================
// HTTP Handlers
// ============================================================================

/// GET /v1/kv/{key:.*}
/// Get a key or keys with prefix
pub async fn get_kv(
    kv_service: web::Data<ConsulKVService>,
    acl_service: web::Data<AclService>,
    req: HttpRequest,
    query: web::Query<KVQueryParams>,
) -> HttpResponse {
    // Extract key from path - handle the wildcard pattern
    let key = req.match_info().get("key").unwrap_or("").to_string();

    // Check ACL authorization for key read
    let authz = acl_service.authorize_request(&req, ResourceType::Key, &key, false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let raw = query.raw.unwrap_or(false);
    let keys_only = query.keys.unwrap_or(false);
    let recurse = query.recurse.unwrap_or(false);

    // Handle blocking watch request
    if let Some(target_index) = query.index {
        let wait_ms = query.wait.unwrap_or(5000); // Default 5 second timeout
        // Wait for index to be reached
        let reached = kv_service.wait_for_index(target_index, wait_ms).await;
        if !reached {
            // Timeout - return 404 or empty response based on existing data
            let has_data = kv_service.get(&key).is_some()
                || kv_service
                    .get_prefix(&key)
                    .iter()
                    .any(|p| !p.key.is_empty());
            if !has_data {
                return HttpResponse::NotFound().finish();
            }
            // Return current data even though timeout occurred
        }
    }

    let current_idx = kv_service.current_index();

    // Handle keys-only request
    if keys_only {
        let keys = kv_service.get_keys(&key, query.separator.as_deref());
        let body = serde_json::to_vec(&keys).unwrap_or_default();
        let len = body.len();
        // Return empty array instead of 404 for empty keys (Consul standard behavior)
        return HttpResponse::Ok()
            .insert_header(("X-Consul-Index", current_idx.to_string()))
            .content_type("application/json")
            .insert_header(("Content-Length", len))
            .body(body);
    }

    // Handle recursive get
    if recurse {
        let pairs = kv_service.get_prefix(&key);
        let body = serde_json::to_vec(&pairs).unwrap_or_default();
        let len = body.len();
        // Return empty array instead of 404 for empty results (Consul standard behavior)
        return HttpResponse::Ok()
            .insert_header(("X-Consul-Index", current_idx.to_string()))
            .content_type("application/json")
            .insert_header(("Content-Length", len))
            .body(body);
    }

    // Single key get
    match kv_service.get(&key) {
        Some(pair) => {
            if raw {
                // Return raw value
                match pair.raw_value() {
                    Some(bytes) => {
                        let body = bytes.clone();
                        HttpResponse::Ok()
                            .insert_header(("X-Consul-Index", current_idx.to_string()))
                            .content_type("application/octet-stream")
                            .insert_header(("Content-Length", body.len()))
                            .body(body)
                    }
                    None => HttpResponse::NotFound().finish(),
                }
            } else {
                let body = serde_json::to_vec(&vec![pair]).unwrap_or_default();
                let len = body.len();
                HttpResponse::Ok()
                    .insert_header(("X-Consul-Index", current_idx.to_string()))
                    .content_type("application/json")
                    .insert_header(("Content-Length", len))
                    .body(body)
            }
        }
        None => HttpResponse::NotFound().finish(),
    }
}

/// PUT /v1/kv/{key:.*}
/// Put a key-value pair
pub async fn put_kv(
    kv_service: web::Data<ConsulKVService>,
    acl_service: web::Data<AclService>,
    req: HttpRequest,
    query: web::Query<KVQueryParams>,
    body: web::Bytes,
) -> HttpResponse {
    let key = req.match_info().get("key").unwrap_or("").to_string();

    if key.is_empty() {
        return HttpResponse::BadRequest().json(ConsulError::new("Key cannot be empty"));
    }

    // Check ACL authorization for key write
    let authz = acl_service.authorize_request(&req, ResourceType::Key, &key, true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let value = String::from_utf8_lossy(&body).to_string();

    // Handle acquire (session-based lock)
    if let Some(ref session_id) = query.acquire {
        // Check if locked by another session first
        if let Some(existing) = kv_service.get(&key)
            && existing.session.is_some()
            && existing.session.as_deref() != Some(session_id)
        {
            // Another session holds the lock
            return HttpResponse::Ok().json(false);
        }
        // Write the value
        kv_service.put(key.clone(), &value, query.flags).await;
        // Acquire the session lock
        kv_service.acquire_session(&key, session_id).await;
        return HttpResponse::Ok().json(true);
    }

    // Handle release (session-based unlock)
    if let Some(ref session_id) = query.release {
        if let Some(existing) = kv_service.get(&key)
            && existing.session.as_deref() == Some(session_id)
        {
            // Release the lock: update the value and clear session
            kv_service.put(key.clone(), &value, query.flags).await;
            kv_service.release_session_key(&key, session_id).await;
            return HttpResponse::Ok().json(true);
        }
        return HttpResponse::Ok().json(false);
    }

    // Check-and-set if cas parameter is provided
    if let Some(cas_index) = query.cas {
        let success = kv_service.cas(key, &value, cas_index, query.flags).await;
        return HttpResponse::Ok().json(success);
    }

    // Regular put
    kv_service.put(key, &value, query.flags).await;
    HttpResponse::Ok().json(true)
}

/// DELETE /v1/kv/{key:.*}
/// Delete a key or keys with prefix
pub async fn delete_kv(
    kv_service: web::Data<ConsulKVService>,
    acl_service: web::Data<AclService>,
    req: HttpRequest,
    query: web::Query<KVQueryParams>,
) -> HttpResponse {
    let key = req.match_info().get("key").unwrap_or("").to_string();

    // Check ACL authorization for key write
    let authz = acl_service.authorize_request(&req, ResourceType::Key, &key, true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let recurse = query.recurse.unwrap_or(false);

    if recurse {
        kv_service.delete_prefix(&key).await;
    } else {
        kv_service.delete(&key).await;
    }

    HttpResponse::Ok().json(true)
}

/// PUT /v1/txn
/// Execute a transaction
pub async fn txn(
    kv_service: web::Data<ConsulKVService>,
    acl_service: web::Data<AclService>,
    req: HttpRequest,
    body: web::Json<Vec<TxnOp>>,
) -> HttpResponse {
    // Check ACL authorization for key write (transactions need write access)
    let authz = acl_service.authorize_request(&req, ResourceType::Key, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let ops = body.into_inner();
    let result = kv_service.transaction(ops).await;

    // Return 409 Conflict if there are errors
    if result.errors.is_some() {
        HttpResponse::Conflict().json(result)
    } else {
        HttpResponse::Ok().json(result)
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

fn current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn current_index() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as u64
}

// ============================================================================
// Export/Import Handlers
// ============================================================================

/// Export all KV pairs as JSON
/// Returns a complete snapshot of the KV store
pub async fn export_kv(
    kv_service: web::Data<ConsulKVService>,
    acl_service: web::Data<AclService>,
    req: HttpRequest,
) -> HttpResponse {
    // Check ACL authorization for read access to all keys
    let authz = acl_service.authorize_request(&req, ResourceType::Key, "*", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    // Collect all KV pairs
    let pairs = kv_service.export_all();
    let count = pairs.len();

    // Return as JSON with metadata
    #[derive(Serialize)]
    struct ExportResult {
        pairs: Vec<KVPair>,
        count: usize,
        export_time: i64,
    }

    let result = ExportResult {
        pairs,
        count,
        export_time: current_timestamp(),
    };

    HttpResponse::Ok().json(result)
}

/// Import KV pairs from JSON
/// Accepts a list of KV pairs and imports them
pub async fn import_kv(
    kv_service: web::Data<ConsulKVService>,
    acl_service: web::Data<AclService>,
    req: HttpRequest,
    body: web::Json<Vec<KVPair>>,
) -> HttpResponse {
    // Check ACL authorization for write access to all keys
    let authz = acl_service.authorize_request(&req, ResourceType::Key, "*", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let pairs_to_import = body.into_inner();
    let mut success_count = 0;
    let mut failed_count = 0;

    // Import each KV pair
    for pair in pairs_to_import {
        // Decode value if present
        if let Some(decoded) = pair.decoded_value() {
            // Store the pair with original flags
            kv_service
                .put(pair.key.clone(), &decoded, Some(pair.flags))
                .await;
            success_count += 1;
        } else {
            failed_count += 1;
        }
    }

    #[derive(Serialize)]
    struct ImportResult {
        success_count: usize,
        failed_count: usize,
        total_count: usize,
        import_time: i64,
    }

    let result = ImportResult {
        success_count,
        failed_count,
        total_count: success_count + failed_count,
        import_time: current_timestamp(),
    };

    HttpResponse::Ok().json(result)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kv_pair_encoding() {
        let pair = KVPair::new("test/key".to_string(), "hello world");
        assert_eq!(pair.decoded_value(), Some("hello world".to_string()));
    }

    #[tokio::test]
    async fn test_kv_service_put_get() {
        let service = ConsulKVService::new();

        service
            .put("config/database".to_string(), "mysql://localhost", None)
            .await;

        let result = service.get("config/database");
        assert!(result.is_some());
        assert_eq!(
            result.unwrap().decoded_value(),
            Some("mysql://localhost".to_string())
        );
    }

    #[tokio::test]
    async fn test_kv_service_prefix() {
        let service = ConsulKVService::new();

        service
            .put("config/db/host".to_string(), "localhost", None)
            .await;
        service
            .put("config/db/port".to_string(), "3306", None)
            .await;
        service
            .put("config/cache/host".to_string(), "redis", None)
            .await;

        let results = service.get_prefix("config/db/");
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_kv_service_keys() {
        let service = ConsulKVService::new();

        service
            .put("config/db/host".to_string(), "localhost", None)
            .await;
        service
            .put("config/db/port".to_string(), "3306", None)
            .await;
        service
            .put("config/cache/host".to_string(), "redis", None)
            .await;

        let keys = service.get_keys("config/", None);
        assert_eq!(keys.len(), 3);

        // With separator
        let folders = service.get_keys("config/", Some("/"));
        assert_eq!(folders.len(), 2); // config/db/ and config/cache/
    }

    #[tokio::test]
    async fn test_kv_service_cas() {
        let service = ConsulKVService::new();

        // Create new with cas=0
        assert!(service.cas("key1".to_string(), "value1", 0, None).await);

        // Get the modify index
        let pair = service.get("key1").unwrap();
        let modify_index = pair.modify_index;

        // CAS with correct index should succeed
        assert!(
            service
                .cas("key1".to_string(), "value2", modify_index, None)
                .await
        );

        // CAS with wrong index should fail
        assert!(
            !service
                .cas("key1".to_string(), "value3", modify_index, None)
                .await
        );

        // Value should be value2
        assert_eq!(
            service.get("key1").unwrap().decoded_value(),
            Some("value2".to_string())
        );
    }

    #[tokio::test]
    async fn test_kv_service_delete() {
        let service = ConsulKVService::new();

        service.put("key1".to_string(), "value1", None).await;
        service.put("prefix/key2".to_string(), "value2", None).await;
        service.put("prefix/key3".to_string(), "value3", None).await;

        // Delete single key
        assert!(service.delete("key1").await);
        assert!(service.get("key1").is_none());

        // Delete prefix
        let count = service.delete_prefix("prefix/").await;
        assert_eq!(count, 2);
        assert!(service.get("prefix/key2").is_none());
        assert!(service.get("prefix/key3").is_none());
    }

    #[tokio::test]
    async fn test_kv_transaction() {
        let service = ConsulKVService::new();

        let ops = vec![
            TxnOp {
                kv: Some(KVTxnOp {
                    verb: "set".to_string(),
                    key: "txn/key1".to_string(),
                    value: Some(BASE64.encode("value1".as_bytes())),
                    flags: None,
                    index: None,
                }),
            },
            TxnOp {
                kv: Some(KVTxnOp {
                    verb: "set".to_string(),
                    key: "txn/key2".to_string(),
                    value: Some(BASE64.encode("value2".as_bytes())),
                    flags: None,
                    index: None,
                }),
            },
            TxnOp {
                kv: Some(KVTxnOp {
                    verb: "get".to_string(),
                    key: "txn/key1".to_string(),
                    value: None,
                    flags: None,
                    index: None,
                }),
            },
        ];

        let result = service.transaction(ops).await;
        assert!(result.errors.is_none());
        assert_eq!(result.results.as_ref().unwrap().len(), 3);

        // Verify the keys exist
        assert!(service.get("txn/key1").is_some());
        assert!(service.get("txn/key2").is_some());
    }

    #[test]
    fn test_kv_get_nonexistent() {
        let service = ConsulKVService::new();
        assert!(service.get("nonexistent/key").is_none());
    }

    #[tokio::test]
    async fn test_kv_delete_nonexistent() {
        let service = ConsulKVService::new();
        assert!(!service.delete("nonexistent/key").await);
    }

    #[tokio::test]
    async fn test_kv_delete_prefix_empty() {
        let service = ConsulKVService::new();
        let count = service.delete_prefix("nonexistent/").await;
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_kv_put_update_preserves_create_index() {
        let service = ConsulKVService::new();

        let pair1 = service.put("key".to_string(), "value1", None).await;
        let create_index = pair1.create_index;

        let pair2 = service.put("key".to_string(), "value2", None).await;
        // create_index should be preserved on update
        assert_eq!(pair2.create_index, create_index);
        // modify_index should change
        assert!(pair2.modify_index > pair1.modify_index);
        assert_eq!(pair2.decoded_value(), Some("value2".to_string()));
    }

    #[tokio::test]
    async fn test_kv_cas_create_if_not_exists() {
        let service = ConsulKVService::new();

        // cas=0 creates if key doesn't exist
        assert!(service.cas("cas0_key".to_string(), "value", 0, None).await);
        assert!(service.get("cas0_key").is_some());
        assert_eq!(
            service.get("cas0_key").unwrap().decoded_value(),
            Some("value".to_string())
        );
    }

    #[tokio::test]
    async fn test_kv_cas_with_wrong_index_on_nonexistent() {
        let service = ConsulKVService::new();

        // CAS with non-zero index on nonexistent key should fail
        assert!(
            !service
                .cas("nonexistent".to_string(), "value", 999, None)
                .await
        );
    }

    #[tokio::test]
    async fn test_kv_flags() {
        let service = ConsulKVService::new();

        let pair = service.put("flagged".to_string(), "data", Some(42)).await;
        assert_eq!(pair.flags, 42);

        // Update value, keep flags
        let pair2 = service.put("flagged".to_string(), "data2", None).await;
        assert_eq!(pair2.flags, 42); // flags preserved

        // Update flags
        let pair3 = service.put("flagged".to_string(), "data3", Some(100)).await;
        assert_eq!(pair3.flags, 100);
    }

    #[tokio::test]
    async fn test_kv_keys_with_separator() {
        let service = ConsulKVService::new();

        service
            .put("web/config/host".to_string(), "localhost", None)
            .await;
        service
            .put("web/config/port".to_string(), "8080", None)
            .await;
        service
            .put("web/static/index.html".to_string(), "<html>", None)
            .await;
        service.put("web/api".to_string(), "v1", None).await;

        // Without separator: all keys
        let all = service.get_keys("web/", None);
        assert_eq!(all.len(), 4);

        // With separator "/": folder-level grouping
        let folders = service.get_keys("web/", Some("/"));
        assert!(folders.contains(&"web/config/".to_string()));
        assert!(folders.contains(&"web/static/".to_string()));
        assert!(folders.contains(&"web/api".to_string()));
    }

    #[tokio::test]
    async fn test_kv_empty_value() {
        let service = ConsulKVService::new();

        let pair = service.put("empty".to_string(), "", None).await;
        assert_eq!(pair.decoded_value(), Some(String::new()));
    }

    #[tokio::test]
    async fn test_kv_special_characters_in_key() {
        let service = ConsulKVService::new();

        // Keys with dots, dashes, underscores
        service
            .put("service.name-v2_config".to_string(), "val", None)
            .await;
        assert!(service.get("service.name-v2_config").is_some());

        // Keys with unicode
        service
            .put("config/日本語".to_string(), "unicode-value", None)
            .await;
        assert!(service.get("config/日本語").is_some());
    }

    #[tokio::test]
    async fn test_kv_large_value() {
        let service = ConsulKVService::new();

        // 512KB value
        let large = "x".repeat(512 * 1024);
        let pair = service.put("large".to_string(), &large, None).await;
        assert_eq!(pair.decoded_value(), Some(large));
    }

    #[test]
    fn test_kv_raw_value_binary() {
        let pair = KVPair::new("binary".to_string(), "hello\x00world");
        let raw = pair.raw_value().unwrap();
        assert_eq!(raw, b"hello\x00world");
    }

    #[test]
    fn test_kv_key_only() {
        let pair = KVPair::key_only("test/key".to_string());
        assert_eq!(pair.key, "test/key");
        assert!(pair.value.is_none());
        assert!(pair.decoded_value().is_none());
        assert_eq!(pair.create_index, 0);
    }

    #[tokio::test]
    async fn test_kv_index_increments() {
        let service = ConsulKVService::new();

        let idx1 = service.current_index();
        service.put("k1".to_string(), "v1", None).await;
        let idx2 = service.current_index();
        service.put("k2".to_string(), "v2", None).await;
        let idx3 = service.current_index();

        assert!(idx2 > idx1);
        assert!(idx3 > idx2);
    }

    #[tokio::test]
    async fn test_kv_transaction_check_index() {
        let service = ConsulKVService::new();

        // Set a key first
        let pair = service
            .put("txn/checked".to_string(), "original", None)
            .await;
        let modify_index = pair.modify_index;

        // Transaction with check-index (correct index)
        let ops = vec![
            TxnOp {
                kv: Some(KVTxnOp {
                    verb: "check-index".to_string(),
                    key: "txn/checked".to_string(),
                    value: None,
                    flags: None,
                    index: Some(modify_index),
                }),
            },
            TxnOp {
                kv: Some(KVTxnOp {
                    verb: "set".to_string(),
                    key: "txn/checked".to_string(),
                    value: Some(BASE64.encode("updated".as_bytes())),
                    flags: None,
                    index: None,
                }),
            },
        ];
        let result = service.transaction(ops).await;
        assert!(result.errors.is_none());

        // Transaction with wrong check-index should fail
        let ops_fail = vec![TxnOp {
            kv: Some(KVTxnOp {
                verb: "check-index".to_string(),
                key: "txn/checked".to_string(),
                value: None,
                flags: None,
                index: Some(999),
            }),
        }];
        let result_fail = service.transaction(ops_fail).await;
        assert!(result_fail.errors.is_some());
    }

    #[tokio::test]
    async fn test_kv_transaction_check_not_exists() {
        let service = ConsulKVService::new();

        // Should succeed: key doesn't exist
        let ops = vec![TxnOp {
            kv: Some(KVTxnOp {
                verb: "check-not-exists".to_string(),
                key: "new/key".to_string(),
                value: None,
                flags: None,
                index: None,
            }),
        }];
        let result = service.transaction(ops).await;
        assert!(result.errors.is_none());

        // Create the key, then check-not-exists should fail
        service.put("new/key".to_string(), "exists", None).await;
        let ops_fail = vec![TxnOp {
            kv: Some(KVTxnOp {
                verb: "check-not-exists".to_string(),
                key: "new/key".to_string(),
                value: None,
                flags: None,
                index: None,
            }),
        }];
        let result_fail = service.transaction(ops_fail).await;
        assert!(result_fail.errors.is_some());
    }

    #[tokio::test]
    async fn test_kv_transaction_unknown_verb() {
        let service = ConsulKVService::new();

        let ops = vec![TxnOp {
            kv: Some(KVTxnOp {
                verb: "invalid-verb".to_string(),
                key: "key".to_string(),
                value: None,
                flags: None,
                index: None,
            }),
        }];
        let result = service.transaction(ops).await;
        assert!(result.errors.is_some());
        let err = &result.errors.unwrap()[0];
        assert!(err.what.contains("unknown verb"));
    }

    #[tokio::test]
    async fn test_kv_transaction_delete_tree() {
        let service = ConsulKVService::new();

        service.put("tree/a".to_string(), "1", None).await;
        service.put("tree/b".to_string(), "2", None).await;
        service.put("other/c".to_string(), "3", None).await;

        let ops = vec![TxnOp {
            kv: Some(KVTxnOp {
                verb: "delete-tree".to_string(),
                key: "tree/".to_string(),
                value: None,
                flags: None,
                index: None,
            }),
        }];
        let result = service.transaction(ops).await;
        assert!(result.errors.is_none());

        assert!(service.get("tree/a").is_none());
        assert!(service.get("tree/b").is_none());
        assert!(service.get("other/c").is_some()); // untouched
    }

    #[tokio::test]
    async fn test_kv_transaction_get_tree() {
        let service = ConsulKVService::new();

        service.put("prefix/a".to_string(), "1", None).await;
        service.put("prefix/b".to_string(), "2", None).await;

        let ops = vec![TxnOp {
            kv: Some(KVTxnOp {
                verb: "get-tree".to_string(),
                key: "prefix/".to_string(),
                value: None,
                flags: None,
                index: None,
            }),
        }];
        let result = service.transaction(ops).await;
        assert!(result.errors.is_none());
        assert_eq!(result.results.as_ref().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_kv_release_session() {
        let service = ConsulKVService::new();

        // Put keys and acquire session on key1 and key2
        service.put("locked/key1".to_string(), "v1", None).await;
        service.put("locked/key2".to_string(), "v2", None).await;
        service.put("unlocked/key3".to_string(), "v3", None).await;

        // Acquire session on key1 and key2
        service.acquire_session("locked/key1", "session-abc").await;
        service.acquire_session("locked/key2", "session-abc").await;

        // Release session
        service.release_session("session-abc").await;

        // Sessions should be cleared
        assert!(service.get("locked/key1").unwrap().session.is_none());
        assert!(service.get("locked/key2").unwrap().session.is_none());
        assert!(service.get("unlocked/key3").unwrap().session.is_none());
    }

    #[tokio::test]
    async fn test_kv_concurrent_puts() {
        let service = ConsulKVService::new();

        // Simulate rapid sequential puts (testing index correctness)
        for i in 0..100 {
            service
                .put(format!("key/{}", i), &format!("value-{}", i), None)
                .await;
        }

        assert_eq!(service.get_prefix("key/").len(), 100);

        // Each key should have a unique modify_index
        let mut indices: Vec<u64> = service
            .get_prefix("key/")
            .iter()
            .map(|p| p.modify_index)
            .collect();
        indices.sort();
        indices.dedup();
        assert_eq!(indices.len(), 100);
    }

    #[tokio::test]
    async fn test_kv_transaction_cas_failure_rollback() {
        let service = ConsulKVService::new();

        service.put("cas_key".to_string(), "original", None).await;

        // Transaction with CAS that uses wrong index
        let ops = vec![TxnOp {
            kv: Some(KVTxnOp {
                verb: "cas".to_string(),
                key: "cas_key".to_string(),
                value: Some(BASE64.encode("updated".as_bytes())),
                flags: None,
                index: Some(99999), // wrong index
            }),
        }];
        let result = service.transaction(ops).await;
        assert!(result.errors.is_some());

        // Value should remain unchanged
        assert_eq!(
            service.get("cas_key").unwrap().decoded_value(),
            Some("original".to_string())
        );
    }

    #[tokio::test]
    async fn test_kv_acquire_release_session() {
        let service = ConsulKVService::new();

        // Create a key
        service.put("lock/key".to_string(), "value", None).await;

        // Acquire session
        assert!(service.acquire_session("lock/key", "session-1").await);
        let pair = service.get("lock/key").unwrap();
        assert_eq!(pair.session, Some("session-1".to_string()));
        assert_eq!(pair.lock_index, 1);

        // Same session can re-acquire
        assert!(service.acquire_session("lock/key", "session-1").await);

        // Different session cannot acquire
        assert!(!service.acquire_session("lock/key", "session-2").await);

        // Release
        assert!(service.release_session_key("lock/key", "session-1").await);
        let pair = service.get("lock/key").unwrap();
        assert!(pair.session.is_none());

        // Now session-2 can acquire
        assert!(service.acquire_session("lock/key", "session-2").await);
    }

    #[tokio::test]
    async fn test_kv_export_all() {
        let service = ConsulKVService::new();

        service.put("a".to_string(), "1", None).await;
        service.put("b".to_string(), "2", None).await;
        service.put("c".to_string(), "3", None).await;

        let all = service.export_all();
        assert_eq!(all.len(), 3);
    }

    #[tokio::test]
    async fn test_kv_wait_for_index_immediate() {
        let service = ConsulKVService::new();

        // Put some data to advance index
        service.put("k".to_string(), "v", None).await;

        // Waiting for an index below current should return immediately
        let result = service.wait_for_index(0, 100).await;
        assert!(result);
    }

    #[tokio::test]
    async fn test_kv_wait_for_index_timeout() {
        let service = ConsulKVService::new();

        // Waiting for a very high index should timeout
        let result = service.wait_for_index(999999, 50).await;
        assert!(!result);
    }

    #[tokio::test]
    async fn test_kv_wait_for_index_notify() {
        let service = ConsulKVService::new();
        let svc_clone = service.clone();

        // Spawn a task that writes after a small delay
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(20)).await;
            svc_clone.put("trigger".to_string(), "value", None).await;
        });

        let result = service.wait_for_index(1, 2000).await;
        assert!(result);
    }
}
