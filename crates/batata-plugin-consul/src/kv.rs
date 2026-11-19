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

use crate::constants::{CF_CONSUL_ACL, CF_CONSUL_KV, CF_CONSUL_QUERIES, CF_CONSUL_SESSIONS};
use crate::raft::{ConsulRaftRequest, ConsulRaftResponse, ConsulRaftWriter};

use crate::acl::{AclService, ResourceType};
use crate::consul_meta::{
    ConsulResponseMeta, apply_kv_raw_security_headers, consul_not_found, consul_ok,
};
use crate::index_provider::{ConsulIndexProvider, ConsulTable};
use crate::model::ConsulError;
use crate::model::ConsulErrorBody;

// ============================================================================
// KV Store Models
// ============================================================================

/// Consul KV Pair
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct KVPair {
    /// The `key` field.
    pub key: String,

    /// The `create_index` field.
    pub create_index: u64,

    /// The `modify_index` field.
    pub modify_index: u64,

    /// The `lock_index` field.
    pub lock_index: u64,

    /// The `flags` field.
    pub flags: u64,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `value` field.
    pub value: Option<String>, // Base64 encoded

    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `session` field.
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

    /// Legacy misspelling of separator (Consul supports both)
    pub seperator: Option<String>,

    /// Wait for index to be >= this value (blocking wait for watch)
    pub index: Option<u64>,

    /// Wait time for blocking query (Consul format: "5s", "30s", "5m")
    pub wait: Option<String>,

    /// Acquire session lock
    pub acquire: Option<String>,

    /// Release session lock
    pub release: Option<String>,
}

/// Transaction operation. At least one field should be set.
///
/// Consul's `/v1/txn` supports KV, Node, Service, and Check operations
/// in a single atomic transaction.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct TxnOp {
    #[serde(rename = "KV", skip_serializing_if = "Option::is_none", default)]
    /// The `kv` field.
    pub kv: Option<KVTxnOp>,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// The `node` field.
    pub node: Option<NodeTxnOp>,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// The `service` field.
    pub service: Option<ServiceTxnOp>,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// The `check` field.
    pub check: Option<CheckTxnOp>,
}

/// KV transaction operation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct KVTxnOp {
    /// The `verb` field.
    pub verb: String, // "set", "get", "delete", "cas", "delete-cas", "delete-tree"

    /// The `key` field.
    pub key: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `value` field.
    pub value: Option<String>, // Base64 encoded

    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `flags` field.
    pub flags: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `index` field.
    pub index: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// Session used by `lock` and `unlock` verbs.
    pub session: Option<String>,
}

/// Transaction result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TxnResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `results` field.
    pub results: Option<Vec<TxnResultItem>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `errors` field.
    pub errors: Option<Vec<TxnError>>,
}

/// Node transaction operation (Consul-compatible).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct NodeTxnOp {
    /// The `verb` field.
    pub verb: String,
    /// The `node` field.
    pub node: NodeTxnEntry,
}

/// Node entry used in transactions.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NodeTxnEntry {
    #[serde(rename = "ID", default)]
    /// The `id` field.
    pub id: String,
    #[serde(rename = "Node", default)]
    /// The `node` field.
    pub node: String,
    #[serde(rename = "Address", default)]
    /// The `address` field.
    pub address: String,
    #[serde(
        rename = "Datacenter",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    /// The `datacenter` field.
    pub datacenter: Option<String>,
    #[serde(
        rename = "TaggedAddresses",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    /// The `tagged_addresses` field.
    pub tagged_addresses: Option<std::collections::HashMap<String, String>>,
    #[serde(rename = "Meta", default, skip_serializing_if = "Option::is_none")]
    /// The `meta` field.
    pub meta: Option<std::collections::HashMap<String, String>>,
    #[serde(rename = "CreateIndex", default)]
    /// The `create_index` field.
    pub create_index: u64,
    #[serde(rename = "ModifyIndex", default)]
    /// The `modify_index` field.
    pub modify_index: u64,
}

/// Service transaction operation (Consul-compatible).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ServiceTxnOp {
    /// The `verb` field.
    pub verb: String,
    /// The `node` field.
    pub node: String,
    /// The `service` field.
    pub service: ServiceTxnEntry,
}

/// Service entry used in transactions.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct ServiceTxnEntry {
    #[serde(rename = "ID", default)]
    /// The `id` field.
    pub id: String,
    #[serde(default)]
    /// The `service` field.
    pub service: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The `tags` field.
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    /// The `address` field.
    pub address: String,
    #[serde(default)]
    /// The `port` field.
    pub port: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The `meta` field.
    pub meta: Option<std::collections::HashMap<String, String>>,
    #[serde(default)]
    /// The `create_index` field.
    pub create_index: u64,
    #[serde(default)]
    /// The `modify_index` field.
    pub modify_index: u64,
}

/// Check transaction operation (Consul-compatible).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CheckTxnOp {
    /// The `verb` field.
    pub verb: String,
    /// The `check` field.
    pub check: CheckTxnEntry,
}

/// Check entry used in transactions.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct CheckTxnEntry {
    #[serde(default)]
    /// The `node` field.
    pub node: String,
    #[serde(rename = "CheckID", default)]
    /// The `check_id` field.
    pub check_id: String,
    #[serde(default)]
    /// The `name` field.
    pub name: String,
    #[serde(default)]
    /// The `status` field.
    pub status: String,
    #[serde(default)]
    /// The `notes` field.
    pub notes: String,
    #[serde(default)]
    /// The `output` field.
    pub output: String,
    #[serde(rename = "ServiceID", default)]
    /// The `service_id` field.
    pub service_id: String,
    #[serde(default)]
    /// The `service_name` field.
    pub service_name: String,
    #[serde(default)]
    /// The `create_index` field.
    pub create_index: u64,
    #[serde(default)]
    /// The `modify_index` field.
    pub modify_index: u64,
}

/// Transaction result item (one of the fields is populated per op).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct TxnResultItem {
    #[serde(rename = "KV", skip_serializing_if = "Option::is_none", default)]
    /// The `kv` field.
    pub kv: Option<KVPair>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// The `node` field.
    pub node: Option<NodeTxnEntry>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// The `service` field.
    pub service: Option<ServiceTxnEntry>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// The `check` field.
    pub check: Option<CheckTxnEntry>,
}

/// Transaction error
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TxnError {
    /// The `op_index` field.
    pub op_index: u32,

    /// The `what` field.
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
    /// Optional Raft writer for cluster mode replication (via core Raft PluginWrite)
    raft_node: Option<Arc<ConsulRaftWriter>>,
    /// Leader-local lock-delay map (standalone enforcement; cluster guard uses
    /// the same shared instance on the leader).
    lock_delay: crate::lock_delay::ConsulLockDelay,
    /// Test-only switch that makes `try_put` behave as if Raft rejected the
    /// write, so the HTTP fail-closed wiring can be exercised without a live
    /// Raft cluster.
    #[cfg(test)]
    fail_raft_writes: Arc<std::sync::atomic::AtomicBool>,
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
            lock_delay: crate::lock_delay::ConsulLockDelay::default(),
            #[cfg(test)]
            fail_raft_writes: Arc::new(std::sync::atomic::AtomicBool::new(false)),
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
            lock_delay: crate::lock_delay::ConsulLockDelay::default(),
            #[cfg(test)]
            fail_raft_writes: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Create a new KV service backed by a Raft-replicated RocksDB instance.
    /// Writes go through Raft consensus; reads are local.
    pub fn with_raft(db: Arc<DB>, raft_node: Arc<ConsulRaftWriter>) -> Self {
        let max_index = Self::scan_max_index(&db);

        Self {
            db,
            index: Arc::new(std::sync::atomic::AtomicU64::new(max_index + 1)),
            notify: Arc::new(Notify::new()),
            _temp_dir: None,
            raft_node: Some(raft_node),
            lock_delay: crate::lock_delay::ConsulLockDelay::default(),
            #[cfg(test)]
            fail_raft_writes: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Share the leader-local lock-delay map with the session service.
    pub fn with_lock_delay(mut self, lock_delay: crate::lock_delay::ConsulLockDelay) -> Self {
        self.lock_delay = lock_delay;
        self
    }

    /// Satisfy a linearizable read guarantee when the caller requested
    /// `?consistent` mode. On the leader this blocks until the local
    /// state machine has applied every entry committed at the time of
    /// the call, matching Consul's `default=consistent` semantics. On a
    /// standalone node (no Raft), this is a no-op. On a follower, this
    /// returns `NotLeader { leader_addr }` so the HTTP handler can emit
    /// a clean 503 response with the leader hint, rather than surfacing
    /// openraft's raw `ForwardToLeader` error text.
    ///
    /// Call this before any read that honors `?consistent`.
    pub async fn ensure_linearizable_read(
        &self,
    ) -> Result<(), crate::consul_meta::LinearizableReadError> {
        let Some(ref raft) = self.raft_node else {
            return Ok(());
        };
        if !raft.is_leader() {
            return Err(crate::consul_meta::LinearizableReadError::NotLeader {
                leader_addr: raft.leader_addr(),
            });
        }
        raft.linearizable_read().await.map_err(|e| {
            crate::consul_meta::LinearizableReadError::Other(format!(
                "linearizable_read failed: {}",
                e
            ))
        })
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

    /// Toggle the test-only simulated Raft write failure.
    #[cfg(test)]
    pub fn set_fail_raft_writes(&self, fail: bool) {
        self.fail_raft_writes
            .store(fail, std::sync::atomic::Ordering::Relaxed);
    }

    /// Replicate one mutation through the shared Raft group.
    ///
    /// Cluster-mode writes used to log-and-ignore Raft failures while the HTTP
    /// handler still returned `true`. Every mutation now goes through here so
    /// `try_*` callers can fail closed when the write never reached the state
    /// machine (no leader, timeout, transport error).
    async fn replicate_response(
        &self,
        request: ConsulRaftRequest,
    ) -> Result<ConsulRaftResponse, String> {
        let Some(ref raft) = self.raft_node else {
            return Ok(ConsulRaftResponse::success());
        };
        raft.write(request).await.map_err(|e| e.to_string())
    }

    async fn replicate(&self, request: ConsulRaftRequest) -> Result<bool, String> {
        let response = self.replicate_response(request).await?;
        Ok(response.success)
    }

    /// Checked `put`: a Raft failure is an error, never a reported success.
    /// HTTP handlers must use this (or another `try_*`) so a leaderless window
    /// surfaces as a 5xx instead of a silent `true`.
    pub async fn try_put(
        &self,
        key: String,
        value: &str,
        flags: Option<u64>,
    ) -> Result<KVPair, String> {
        #[cfg(test)]
        if self
            .fail_raft_writes
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            return Err("injected Raft write failure".to_string());
        }
        let encoded = Some(BASE64.encode(value.as_bytes()));
        let stored = self.build_stored_kv(&key, encoded, flags);
        if self.raft_node.is_some() {
            let json = serde_json::to_string(&stored)
                .map_err(|e| format!("failed to encode KV '{}': {}", key, e))?;
            let applied = self
                .replicate(ConsulRaftRequest::KVPut {
                    key: key.clone(),
                    stored_kv_json: json,
                    session_index_key: None,
                })
                .await?;
            if !applied {
                return Err(format!("raft rejected KV put for '{}'", key));
            }
        } else {
            self.put_stored(&key, &stored);
        }
        self.notify.notify_waiters();
        Ok(stored.pair)
    }

    /// Checked variant of `put_base64` (used by transactions).
    pub async fn try_put_base64(
        &self,
        key: String,
        base64_value: Option<String>,
        flags: Option<u64>,
    ) -> Result<KVPair, String> {
        let stored = self.build_stored_kv(&key, base64_value, flags);
        if self.raft_node.is_some() {
            let json = serde_json::to_string(&stored)
                .map_err(|e| format!("failed to encode KV '{}': {}", key, e))?;
            let applied = self
                .replicate(ConsulRaftRequest::KVPut {
                    key: key.clone(),
                    stored_kv_json: json,
                    session_index_key: None,
                })
                .await?;
            if !applied {
                return Err(format!("raft rejected KV put for '{}'", key));
            }
        } else {
            self.put_stored(&key, &stored);
        }
        self.notify.notify_waiters();
        Ok(stored.pair)
    }

    /// Checked `cas`: `Ok(false)` is a normal index conflict, `Err` is a Raft
    /// failure that must not be reported as a CAS miss.
    pub async fn try_cas(
        &self,
        key: String,
        value: &str,
        cas_index: u64,
        flags: Option<u64>,
    ) -> Result<bool, String> {
        if let Some(mut existing) = self.get_stored(&key) {
            if existing.pair.modify_index == cas_index {
                existing.pair.modify_index = self.next_index();
                existing.pair.value = Some(BASE64.encode(value.as_bytes()));
                if let Some(f) = flags {
                    existing.pair.flags = f;
                }
                existing.modified_at = current_timestamp();
                if self.raft_node.is_some() {
                    let json = serde_json::to_string(&existing)
                        .map_err(|e| format!("failed to encode KV '{}': {}", key, e))?;
                    return self
                        .replicate(ConsulRaftRequest::KVCas {
                            key: key.clone(),
                            stored_kv_json: json,
                            expected_modify_index: cas_index,
                        })
                        .await;
                }
                self.put_stored(&key, &existing);
                self.notify.notify_waiters();
                return Ok(true);
            }
            return Ok(false);
        }
        if cas_index == 0 {
            self.try_put(key, value, flags).await?;
            return Ok(true);
        }
        Ok(false)
    }

    /// Checked `put_and_acquire`: `Ok(false)` means the lock is held by another
    /// session; `Err` means the Raft write did not happen.
    pub async fn try_put_and_acquire(
        &self,
        key: &str,
        value: &str,
        flags: Option<u64>,
        session_id: &str,
    ) -> Result<bool, String> {
        // Cluster mode enforces lock delay in the leader-side Raft write guard;
        // standalone has no Raft node, so check the shared local map here.
        if self.raft_node.is_none() && self.lock_delay.is_active(key) {
            return Ok(false);
        }
        let encoded = Some(BASE64.encode(value.as_bytes()));
        let mut stored = self.build_stored_kv(key, encoded, flags);
        match stored.pair.session.as_deref() {
            Some(holder) if holder == session_id => {}
            Some(_) => return Ok(false),
            None => stored.pair.lock_index += 1,
        }
        stored.pair.session = Some(session_id.to_string());
        stored.pair.modify_index = self.next_index();
        stored.modified_at = current_timestamp();

        if self.raft_node.is_some() {
            let json = serde_json::to_string(&stored)
                .map_err(|e| format!("failed to encode KV '{}': {}", key, e))?;
            return self
                .replicate(ConsulRaftRequest::KVAcquireSession {
                    key: key.to_string(),
                    session_id: session_id.to_string(),
                    stored_kv_json: json,
                })
                .await;
        }
        let mut batch = WriteBatch::default();
        self.batch_put_stored(&mut batch, key, &stored);
        if let Some(cf_sessions) = self.db.cf_handle(CF_CONSUL_SESSIONS) {
            let idx_key = format!("kidx:{}:{}", session_id, key);
            batch.put_cf(cf_sessions, idx_key.as_bytes(), b"");
        }
        self.db
            .write(batch)
            .map_err(|e| format!("failed to acquire session on '{}': {}", key, e))?;
        self.notify.notify_waiters();
        Ok(true)
    }

    /// Checked `put_and_release`.
    pub async fn try_put_and_release(
        &self,
        key: &str,
        value: &str,
        flags: Option<u64>,
        session_id: &str,
    ) -> Result<bool, String> {
        let encoded = Some(BASE64.encode(value.as_bytes()));
        let mut stored = self.build_stored_kv(key, encoded, flags);
        if stored.pair.session.as_deref() != Some(session_id) {
            return Ok(false);
        }
        stored.pair.session = None;
        stored.pair.modify_index = self.next_index();
        stored.modified_at = current_timestamp();

        if self.raft_node.is_some() {
            let json = serde_json::to_string(&stored)
                .map_err(|e| format!("failed to encode KV '{}': {}", key, e))?;
            return self
                .replicate(ConsulRaftRequest::KVReleaseSessionKey {
                    key: key.to_string(),
                    session_id: session_id.to_string(),
                    stored_kv_json: json,
                })
                .await;
        }
        let mut batch = WriteBatch::default();
        self.batch_put_stored(&mut batch, key, &stored);
        if let Some(cf_sessions) = self.db.cf_handle(CF_CONSUL_SESSIONS) {
            let idx_key = format!("kidx:{}:{}", session_id, key);
            batch.delete_cf(cf_sessions, idx_key.as_bytes());
        }
        self.db
            .write(batch)
            .map_err(|e| format!("failed to release session on '{}': {}", key, e))?;
        self.notify.notify_waiters();
        Ok(true)
    }

    /// Checked `delete`: `Ok(false)` means the key was absent locally in
    /// standalone mode; `Err` means the replicated delete did not happen.
    pub async fn try_delete(&self, key: &str) -> Result<bool, String> {
        #[cfg(test)]
        if self
            .fail_raft_writes
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            return Err("injected Raft write failure".to_string());
        }
        let stored = self.get_stored(key);
        if stored.is_none() && self.raft_node.is_none() {
            return Ok(false);
        }
        let session_index_cleanup = stored
            .as_ref()
            .and_then(|s| s.pair.session.as_ref())
            .map(|sid| format!("kidx:{}:{}", sid, key));

        if self.raft_node.is_some() {
            let applied = self
                .replicate(ConsulRaftRequest::KVDelete {
                    key: key.to_string(),
                    session_index_cleanup,
                })
                .await?;
            if applied {
                self.notify.notify_waiters();
            }
            return Ok(applied);
        }
        let Some(cf) = self.db.cf_handle(CF_CONSUL_KV) else {
            return Ok(false);
        };
        let mut batch = WriteBatch::default();
        batch.delete_cf(cf, key.as_bytes());
        if let Some(ref idx_key) = session_index_cleanup
            && let Some(cf_sessions) = self.db.cf_handle(CF_CONSUL_SESSIONS)
        {
            batch.delete_cf(cf_sessions, idx_key.as_bytes());
        }
        self.db
            .write(batch)
            .map_err(|e| format!("failed to delete KV '{}': {}", key, e))?;
        self.notify.notify_waiters();
        Ok(true)
    }

    /// Checked CAS delete. `Ok(false)` means the committed ModifyIndex did not
    /// match; `Err` means the conditional delete was not replicated.
    pub async fn try_cas_delete(&self, key: &str, expected_index: u64) -> Result<bool, String> {
        #[cfg(test)]
        if self
            .fail_raft_writes
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            return Err("injected Raft write failure".to_string());
        }

        if self.raft_node.is_some() {
            let applied = self
                .replicate(ConsulRaftRequest::KVCasDelete {
                    key: key.to_string(),
                    expected_modify_index: expected_index,
                })
                .await?;
            if applied {
                self.notify.notify_waiters();
            }
            return Ok(applied);
        }

        let Some(stored) = self.get_stored(key) else {
            return Ok(expected_index == 0);
        };
        if stored.pair.modify_index != expected_index {
            return Ok(false);
        }

        let Some(cf) = self.db.cf_handle(CF_CONSUL_KV) else {
            return Ok(false);
        };
        let mut batch = WriteBatch::default();
        batch.delete_cf(cf, key.as_bytes());
        if let Some(ref session_id) = stored.pair.session
            && let Some(cf_sessions) = self.db.cf_handle(CF_CONSUL_SESSIONS)
        {
            let idx_key = format!("kidx:{}:{}", session_id, key);
            batch.delete_cf(cf_sessions, idx_key.as_bytes());
        }
        self.db
            .write(batch)
            .map_err(|e| format!("failed to CAS delete KV '{}': {}", key, e))?;
        self.notify.notify_waiters();
        Ok(true)
    }

    /// Checked `delete_prefix`.
    pub async fn try_delete_prefix(&self, prefix: &str) -> Result<u32, String> {
        if self.raft_node.is_some() {
            let applied = self
                .replicate(ConsulRaftRequest::KVDeletePrefix {
                    prefix: prefix.to_string(),
                })
                .await?;
            if !applied {
                return Err(format!("raft rejected KV delete-prefix '{}'", prefix));
            }
            self.notify.notify_waiters();
            return Ok(1);
        }
        let Some(cf_kv) = self.db.cf_handle(CF_CONSUL_KV) else {
            return Ok(0);
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
            if let Ok(stored) = serde_json::from_slice::<StoredKV>(&value_bytes)
                && let Some(ref session_id) = stored.pair.session
                && let Some(cf_sessions) = self.db.cf_handle(CF_CONSUL_SESSIONS)
            {
                let idx_key = format!("kidx:{}:{}", session_id, key);
                batch.delete_cf(cf_sessions, idx_key.as_bytes());
            }
            count += 1;
        }
        if count > 0 {
            self.db
                .write(batch)
                .map_err(|e| format!("failed to delete KV prefix '{}': {}", prefix, e))?;
            self.notify.notify_waiters();
        }
        Ok(count)
    }

    /// Put a key-value pair with pre-encoded base64 value (for transactions).
    pub async fn put_base64(
        &self,
        key: String,
        base64_value: Option<String>,
        flags: Option<u64>,
    ) -> KVPair {
        // Best-effort form for internal callers. HTTP handlers use
        // `try_put_base64` so a Raft failure is never reported as success.
        match self.try_put_base64(key.clone(), base64_value, flags).await {
            Ok(pair) => pair,
            Err(e) => {
                error!("Raft ConsulKVPut failed: {}", e);
                KVPair::new(key, "")
            }
        }
    }

    /// Put a key-value pair
    pub async fn put(&self, key: String, value: &str, flags: Option<u64>) -> KVPair {
        // Best-effort form for internal callers. HTTP handlers use `try_put`.
        match self.try_put(key.clone(), value, flags).await {
            Ok(pair) => pair,
            Err(e) => {
                error!("Raft ConsulKVPut failed: {}", e);
                KVPair::new(key, value)
            }
        }
    }

    /// Check-and-set: only update if modify_index matches
    pub async fn cas(&self, key: String, value: &str, cas_index: u64, flags: Option<u64>) -> bool {
        // Best-effort form for internal callers. HTTP handlers use `try_cas`,
        // which distinguishes a CAS conflict from a Raft failure.
        match self.try_cas(key, value, cas_index, flags).await {
            Ok(applied) => applied,
            Err(e) => {
                error!("Raft ConsulKVCas failed: {}", e);
                false
            }
        }
    }

    /// Put a value and acquire a session lock on a key atomically.
    /// Returns false if the key is locked by a different session.
    /// If the key does not exist, it is created.
    pub async fn put_and_acquire(
        &self,
        key: &str,
        value: &str,
        flags: Option<u64>,
        session_id: &str,
    ) -> bool {
        match self
            .try_put_and_acquire(key, value, flags, session_id)
            .await
        {
            Ok(applied) => applied,
            Err(e) => {
                error!("Raft ConsulKVAcquireSession failed: {}", e);
                false
            }
        }
    }

    /// Acquire a session lock on a key (without changing value).
    /// Returns false if the key is locked by a different session, or the key does not exist.
    pub async fn acquire_session(&self, key: &str, session_id: &str) -> bool {
        // Standalone has no pre-commit guard; the shared local delay map is
        // the only place this can be enforced. Cluster mode checks it on the
        // leader before proposing `KVAcquireSession`.
        if self.raft_node.is_none() && self.lock_delay.is_active(key) {
            return false;
        }
        let Some(mut stored) = self.get_stored(key) else {
            return false;
        };

        match stored.pair.session.as_deref() {
            Some(holder) if holder == session_id => {}
            Some(_) => return false,
            None => {
                stored.pair.lock_index += 1;
            }
        }

        stored.pair.session = Some(session_id.to_string());
        stored.pair.modify_index = self.next_index();
        stored.modified_at = current_timestamp();

        if let Some(ref raft) = self.raft_node {
            let json = serde_json::to_string(&stored).unwrap_or_default();
            match raft
                .write(ConsulRaftRequest::KVAcquireSession {
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
                _ => false,
            }
        } else {
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

    /// Put a value and release a session lock on a key atomically.
    /// Returns false if the key is not held by the given session.
    pub async fn put_and_release(
        &self,
        key: &str,
        value: &str,
        flags: Option<u64>,
        session_id: &str,
    ) -> bool {
        match self
            .try_put_and_release(key, value, flags, session_id)
            .await
        {
            Ok(applied) => applied,
            Err(e) => {
                error!("Raft ConsulKVReleaseSessionKey failed: {}", e);
                false
            }
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
                .write(ConsulRaftRequest::KVReleaseSessionKey {
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
                .write(ConsulRaftRequest::KVReleaseSession {
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
        // Best-effort form for internal callers. HTTP handlers use `try_delete`.
        match self.try_delete(key).await {
            Ok(deleted) => deleted,
            Err(e) => {
                error!("Raft ConsulKVDelete failed: {}", e);
                false
            }
        }
    }

    /// Delete keys with prefix — O(k) via sorted iterator + WriteBatch
    pub async fn delete_prefix(&self, prefix: &str) -> u32 {
        // Best-effort form for internal callers. HTTP handlers use
        // `try_delete_prefix`.
        match self.try_delete_prefix(prefix).await {
            Ok(count) => count,
            Err(e) => {
                error!("Raft ConsulKVDeletePrefix failed: {}", e);
                0
            }
        }
    }

    /// Export all KV pairs
    pub fn export_all(&self) -> Vec<KVPair> {
        self.get_prefix("")
    }

    /// Maximum number of operations allowed in a single transaction.
    /// Matches Consul's `maxTxnOps` (agent/txn_endpoint.go:23).
    pub(crate) const MAX_TXN_OPS: usize = 128;

    /// Execute a transaction with one authoritative state-store decision.
    ///
    /// Cluster mode sends every verb in a single `KVTransaction` Raft entry;
    /// the FSM re-validates all of them against committed state and commits
    /// the whole batch or none of it. Standalone mode calls the same helper
    /// with a local index so both modes share the exact semantics.
    pub async fn transaction(&self, ops: Vec<TxnOp>) -> TxnResult {
        #[cfg(test)]
        if self
            .fail_raft_writes
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            return txn_write_failure(0, "injected Raft write failure");
        }

        // Standalone has no leader-side write guard, so enforce lock delay
        // here for `lock` verbs. Cluster mode enforces the same rule before
        // the entry is proposed.
        if self.raft_node.is_none()
            && let Some(key) = self.lock_delay.first_active_txn_key(&ops)
        {
            return txn_write_failure(0, &format!("Lock delay active for key {}", key));
        }

        if self.raft_node.is_some() {
            let response = match self
                .replicate_response(ConsulRaftRequest::KVTransaction { ops })
                .await
            {
                Ok(response) => response,
                Err(e) => return txn_write_failure(0, &e),
            };
            let Some(data) = response.data.as_ref() else {
                return txn_write_failure(0, "transaction response missing result data");
            };
            return match serde_json::from_slice::<TxnResult>(data) {
                Ok(result) => {
                    if result.errors.is_none() {
                        self.notify.notify_waiters();
                    }
                    result
                }
                Err(e) => txn_write_failure(0, &format!("invalid transaction response: {}", e)),
            };
        }

        let index = self.next_index();
        let result = match apply_kv_transaction_ops(&self.db, &ops, index) {
            Ok(result) => result,
            Err(e) => return txn_write_failure(0, &e),
        };
        if result.errors.is_none() {
            self.notify.notify_waiters();
        }
        result
    }
}

/// Apply an ordered `/v1/txn` batch against one RocksDB snapshot.
///
/// This mirrors upstream `Store::TxnRW`: reads and writes observe a single
/// transaction, every verb is validated against that view, and the RocksDB
/// write batch is committed only when no verb reported an error.
pub(crate) fn apply_kv_transaction_ops(
    db: &DB,
    ops: &[TxnOp],
    index: u64,
) -> Result<TxnResult, String> {
    use std::collections::{BTreeSet, HashMap};

    fn load_stored(
        db: &DB,
        cf: &rocksdb::ColumnFamily,
        key: &str,
    ) -> Result<Option<StoredKV>, String> {
        let bytes = db
            .get_cf(cf, key.as_bytes())
            .map_err(|e| format!("failed to read key '{}': {}", key, e))?;
        match bytes {
            Some(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|e| format!("corrupt stored KV for '{}': {}", key, e)),
            None => Ok(None),
        }
    }

    fn ensure_loaded(
        db: &DB,
        cf: &rocksdb::ColumnFamily,
        key: &str,
        original: &mut HashMap<String, Option<StoredKV>>,
        overlay: &mut HashMap<String, Option<StoredKV>>,
    ) -> Result<(), String> {
        if original.contains_key(key) {
            return Ok(());
        }
        let loaded = load_stored(db, cf, key)?;
        original.insert(key.to_string(), loaded.clone());
        overlay.entry(key.to_string()).or_insert(loaded);
        Ok(())
    }

    fn load_prefix(
        db: &DB,
        cf: &rocksdb::ColumnFamily,
        prefix: &str,
        original: &mut HashMap<String, Option<StoredKV>>,
        overlay: &mut HashMap<String, Option<StoredKV>>,
    ) -> Result<Vec<String>, String> {
        let mode = if prefix.is_empty() {
            rocksdb::IteratorMode::Start
        } else {
            rocksdb::IteratorMode::From(prefix.as_bytes(), rocksdb::Direction::Forward)
        };
        for item in db.iterator_cf(cf, mode).flatten() {
            let (key_bytes, _) = item;
            let key = match String::from_utf8(key_bytes.to_vec()) {
                Ok(key) => key,
                Err(_) => continue,
            };
            if !prefix.is_empty() && !key.starts_with(prefix) {
                break;
            }
            ensure_loaded(db, cf, &key, original, overlay)?;
        }
        let mut keys: Vec<String> = overlay
            .keys()
            .filter(|key| prefix.is_empty() || key.starts_with(prefix))
            .cloned()
            .collect();
        keys.sort();
        keys.dedup();
        Ok(keys)
    }

    fn encoded_value(op: &KVTxnOp) -> Result<Option<String>, String> {
        match op.value.as_deref() {
            None | Some("") => Ok(None),
            Some(value) => {
                BASE64
                    .decode(value)
                    .map_err(|e| format!("invalid base64 value for key '{}': {}", op.key, e))?;
                Ok(Some(value.to_string()))
            }
        }
    }

    fn entry<'a>(
        overlay: &'a HashMap<String, Option<StoredKV>>,
        key: &str,
    ) -> Option<&'a StoredKV> {
        overlay.get(key).and_then(|stored| stored.as_ref())
    }

    fn result_pair(pair: &KVPair, keep_value: bool) -> KVPair {
        let mut pair = pair.clone();
        if !keep_value {
            pair.value = None;
        }
        pair
    }

    fn written_stored(
        existing: Option<&StoredKV>,
        key: &str,
        value: Option<String>,
        flags: Option<u64>,
        index: u64,
    ) -> StoredKV {
        let now = current_timestamp();
        match existing {
            Some(existing) => {
                let mut stored = existing.clone();
                stored.pair.key = key.to_string();
                stored.pair.value = value;
                if let Some(flags) = flags {
                    stored.pair.flags = flags;
                }
                stored.pair.modify_index = index;
                stored.modified_at = now;
                stored
            }
            None => StoredKV {
                pair: KVPair {
                    key: key.to_string(),
                    create_index: index,
                    modify_index: index,
                    lock_index: 0,
                    flags: flags.unwrap_or(0),
                    value,
                    session: None,
                },
                created_at: now,
                modified_at: now,
            },
        }
    }

    fn push_error(errors: &mut Vec<TxnError>, op_index: usize, what: impl Into<String>) {
        errors.push(TxnError {
            op_index: op_index as u32,
            what: what.into(),
        });
    }

    let cf_kv = db
        .cf_handle(CF_CONSUL_KV)
        .ok_or("KV column family is missing")?;
    let cf_sessions = db.cf_handle(CF_CONSUL_SESSIONS);

    let mut overlay: HashMap<String, Option<StoredKV>> = HashMap::new();
    let mut original: HashMap<String, Option<StoredKV>> = HashMap::new();
    let mut dirty: BTreeSet<String> = BTreeSet::new();
    let mut results: Vec<TxnResultItem> = Vec::new();
    let mut errors: Vec<TxnError> = Vec::new();

    for (op_index, op) in ops.iter().enumerate() {
        let Some(kv) = op.kv.as_ref() else {
            // Node/Service/Check writes are response-only in batata; their
            // authoritative storage is the naming/catalog path, not the KV CF.
            if let Some(node) = op.node.as_ref() {
                match node.verb.to_ascii_lowercase().as_str() {
                    "get" | "set" | "cas" | "delete" | "delete-cas" => {
                        results.push(TxnResultItem {
                            node: Some(node.node.clone()),
                            ..Default::default()
                        })
                    }
                    verb => push_error(
                        &mut errors,
                        op_index,
                        format!("unknown node verb: {}", verb),
                    ),
                }
            }
            if let Some(service) = op.service.as_ref() {
                match service.verb.to_ascii_lowercase().as_str() {
                    "get" | "set" | "cas" | "delete" | "delete-cas" => {
                        results.push(TxnResultItem {
                            service: Some(service.service.clone()),
                            ..Default::default()
                        })
                    }
                    verb => push_error(
                        &mut errors,
                        op_index,
                        format!("unknown service verb: {}", verb),
                    ),
                }
            }
            if let Some(check) = op.check.as_ref() {
                match check.verb.to_ascii_lowercase().as_str() {
                    "get" | "set" | "cas" | "delete" | "delete-cas" => {
                        results.push(TxnResultItem {
                            check: Some(check.check.clone()),
                            ..Default::default()
                        })
                    }
                    verb => push_error(
                        &mut errors,
                        op_index,
                        format!("unknown check verb: {}", verb),
                    ),
                }
            }
            if op.kv.is_none() && op.node.is_none() && op.service.is_none() && op.check.is_none() {
                push_error(&mut errors, op_index, "unknown operation type");
            }
            continue;
        };

        let verb = kv.verb.to_ascii_lowercase();
        match verb.as_str() {
            "get" | "get-or-empty" => {
                if let Err(e) = ensure_loaded(db, cf_kv, &kv.key, &mut original, &mut overlay) {
                    push_error(&mut errors, op_index, e);
                    continue;
                }
                match entry(&overlay, &kv.key) {
                    Some(stored) => results.push(TxnResultItem {
                        kv: Some(result_pair(&stored.pair, true)),
                        ..Default::default()
                    }),
                    None if verb == "get-or-empty" => results.push(TxnResultItem {
                        kv: Some(KVPair::key_only(kv.key.clone())),
                        ..Default::default()
                    }),
                    None => push_error(
                        &mut errors,
                        op_index,
                        format!("key '{}' doesn't exist", kv.key),
                    ),
                }
            }
            "get-tree" => {
                let keys = match load_prefix(db, cf_kv, &kv.key, &mut original, &mut overlay) {
                    Ok(keys) => keys,
                    Err(e) => {
                        push_error(&mut errors, op_index, e);
                        continue;
                    }
                };
                for key in keys {
                    if let Some(stored) = entry(&overlay, &key) {
                        results.push(TxnResultItem {
                            kv: Some(result_pair(&stored.pair, true)),
                            ..Default::default()
                        });
                    }
                }
            }
            "set" => {
                let value = match encoded_value(kv) {
                    Ok(value) => value,
                    Err(e) => {
                        push_error(&mut errors, op_index, e);
                        continue;
                    }
                };
                if let Err(e) = ensure_loaded(db, cf_kv, &kv.key, &mut original, &mut overlay) {
                    push_error(&mut errors, op_index, e);
                    continue;
                }
                let stored =
                    written_stored(entry(&overlay, &kv.key), &kv.key, value, kv.flags, index);
                results.push(TxnResultItem {
                    kv: Some(result_pair(&stored.pair, false)),
                    ..Default::default()
                });
                overlay.insert(kv.key.clone(), Some(stored));
                dirty.insert(kv.key.clone());
            }
            "cas" => {
                let expected = kv.index.unwrap_or(0);
                if let Err(e) = ensure_loaded(db, cf_kv, &kv.key, &mut original, &mut overlay) {
                    push_error(&mut errors, op_index, e);
                    continue;
                }
                let existing = entry(&overlay, &kv.key).cloned();
                let matches = match (&existing, expected) {
                    (None, 0) => true,
                    (None, _) => false,
                    (Some(_), 0) => false,
                    (Some(stored), expected) => stored.pair.modify_index == expected,
                };
                if !matches {
                    push_error(&mut errors, op_index, "CAS failed: index mismatch");
                    continue;
                }
                let value = match encoded_value(kv) {
                    Ok(value) => value,
                    Err(e) => {
                        push_error(&mut errors, op_index, e);
                        continue;
                    }
                };
                let stored = written_stored(existing.as_ref(), &kv.key, value, kv.flags, index);
                results.push(TxnResultItem {
                    kv: Some(result_pair(&stored.pair, false)),
                    ..Default::default()
                });
                overlay.insert(kv.key.clone(), Some(stored));
                dirty.insert(kv.key.clone());
            }
            "delete" => {
                if let Err(e) = ensure_loaded(db, cf_kv, &kv.key, &mut original, &mut overlay) {
                    push_error(&mut errors, op_index, e);
                    continue;
                }
                if overlay.contains_key(&kv.key) {
                    overlay.insert(kv.key.clone(), None);
                    dirty.insert(kv.key.clone());
                }
            }
            "delete-cas" => {
                let expected = kv.index.unwrap_or(0);
                if let Err(e) = ensure_loaded(db, cf_kv, &kv.key, &mut original, &mut overlay) {
                    push_error(&mut errors, op_index, e);
                    continue;
                }
                let stored = entry(&overlay, &kv.key);
                if stored.is_none() && expected != 0 {
                    push_error(&mut errors, op_index, format!("key '{}' not found", kv.key));
                    continue;
                }
                if let Some(stored) = stored
                    && stored.pair.modify_index != expected
                {
                    push_error(&mut errors, op_index, "CAS failed: index mismatch");
                    continue;
                }
                if stored.is_some() {
                    overlay.insert(kv.key.clone(), None);
                    dirty.insert(kv.key.clone());
                }
            }
            "delete-tree" => {
                let keys = match load_prefix(db, cf_kv, &kv.key, &mut original, &mut overlay) {
                    Ok(keys) => keys,
                    Err(e) => {
                        push_error(&mut errors, op_index, e);
                        continue;
                    }
                };
                for key in keys {
                    if overlay.get(&key).is_some_and(|stored| stored.is_some()) {
                        overlay.insert(key.clone(), None);
                        dirty.insert(key);
                    }
                }
            }
            "check-index" => {
                if let Err(e) = ensure_loaded(db, cf_kv, &kv.key, &mut original, &mut overlay) {
                    push_error(&mut errors, op_index, e);
                    continue;
                }
                match entry(&overlay, &kv.key) {
                    Some(stored) if stored.pair.modify_index == kv.index.unwrap_or(0) => {
                        results.push(TxnResultItem {
                            kv: Some(result_pair(&stored.pair, false)),
                            ..Default::default()
                        });
                    }
                    Some(stored) => push_error(
                        &mut errors,
                        op_index,
                        format!(
                            "current modify index {} does not match expected {}",
                            stored.pair.modify_index,
                            kv.index.unwrap_or(0)
                        ),
                    ),
                    None => push_error(
                        &mut errors,
                        op_index,
                        format!("key '{}' doesn't exist for check-index", kv.key),
                    ),
                }
            }
            "check-not-exists" => {
                if let Err(e) = ensure_loaded(db, cf_kv, &kv.key, &mut original, &mut overlay) {
                    push_error(&mut errors, op_index, e);
                    continue;
                }
                if entry(&overlay, &kv.key).is_some() {
                    push_error(
                        &mut errors,
                        op_index,
                        format!("key '{}' exists when it should not", kv.key),
                    );
                }
            }
            "check-session" => {
                if let Err(e) = ensure_loaded(db, cf_kv, &kv.key, &mut original, &mut overlay) {
                    push_error(&mut errors, op_index, e);
                    continue;
                }
                let expected = kv.session.as_deref().unwrap_or_default();
                match entry(&overlay, &kv.key) {
                    Some(stored)
                        if stored.pair.session.as_deref().unwrap_or_default() == expected =>
                    {
                        results.push(TxnResultItem {
                            kv: Some(result_pair(&stored.pair, false)),
                            ..Default::default()
                        });
                    }
                    Some(stored) => push_error(
                        &mut errors,
                        op_index,
                        format!(
                            "failed session check for key '{}', current session '{:?}' != '{}'",
                            kv.key, stored.pair.session, expected
                        ),
                    ),
                    None => push_error(
                        &mut errors,
                        op_index,
                        format!("key '{}' doesn't exist for check-session", kv.key),
                    ),
                }
            }
            "lock" => {
                let Some(session) = kv.session.as_deref().filter(|s| !s.is_empty()) else {
                    push_error(&mut errors, op_index, "missing session");
                    continue;
                };
                let cf_sessions = match cf_sessions {
                    Some(cf) => cf,
                    None => {
                        push_error(&mut errors, op_index, "Session column family is missing");
                        continue;
                    }
                };
                let exists = match db.get_cf(cf_sessions, session.as_bytes()) {
                    Ok(value) => value.is_some(),
                    Err(e) => {
                        push_error(
                            &mut errors,
                            op_index,
                            format!("failed session lookup: {}", e),
                        );
                        continue;
                    }
                };
                if !exists {
                    push_error(
                        &mut errors,
                        op_index,
                        format!("invalid session '{}'", session),
                    );
                    continue;
                }
                let value = match encoded_value(kv) {
                    Ok(value) => value,
                    Err(e) => {
                        push_error(&mut errors, op_index, e);
                        continue;
                    }
                };
                if let Err(e) = ensure_loaded(db, cf_kv, &kv.key, &mut original, &mut overlay) {
                    push_error(&mut errors, op_index, e);
                    continue;
                }
                let existing = entry(&overlay, &kv.key).cloned();
                let stored = match existing {
                    Some(mut stored) => {
                        match stored.pair.session.as_deref() {
                            Some(holder) if holder == session => {}
                            Some(_) => {
                                push_error(
                                    &mut errors,
                                    op_index,
                                    format!(
                                        "failed to lock key '{}', lock is already held",
                                        kv.key
                                    ),
                                );
                                continue;
                            }
                            None => {
                                stored.pair.lock_index = stored.pair.lock_index.saturating_add(1)
                            }
                        }
                        stored.pair.value = value;
                        stored.pair.session = Some(session.to_string());
                        if let Some(flags) = kv.flags {
                            stored.pair.flags = flags;
                        }
                        stored.pair.modify_index = index;
                        stored.modified_at = current_timestamp();
                        stored
                    }
                    None => StoredKV {
                        pair: KVPair {
                            key: kv.key.clone(),
                            create_index: index,
                            modify_index: index,
                            lock_index: 1,
                            flags: kv.flags.unwrap_or(0),
                            value,
                            session: Some(session.to_string()),
                        },
                        created_at: current_timestamp(),
                        modified_at: current_timestamp(),
                    },
                };
                results.push(TxnResultItem {
                    kv: Some(result_pair(&stored.pair, false)),
                    ..Default::default()
                });
                overlay.insert(kv.key.clone(), Some(stored));
                dirty.insert(kv.key.clone());
            }
            "unlock" => {
                let Some(session) = kv.session.as_deref().filter(|s| !s.is_empty()) else {
                    push_error(&mut errors, op_index, "missing session");
                    continue;
                };
                let value = match encoded_value(kv) {
                    Ok(value) => value,
                    Err(e) => {
                        push_error(&mut errors, op_index, e);
                        continue;
                    }
                };
                if let Err(e) = ensure_loaded(db, cf_kv, &kv.key, &mut original, &mut overlay) {
                    push_error(&mut errors, op_index, e);
                    continue;
                }
                let Some(mut stored) = entry(&overlay, &kv.key).cloned() else {
                    push_error(&mut errors, op_index, format!("key '{}' not found", kv.key));
                    continue;
                };
                if stored.pair.session.as_deref() != Some(session) {
                    push_error(
                        &mut errors,
                        op_index,
                        format!(
                            "failed to unlock key '{}', lock isn't held by session '{}'",
                            kv.key, session
                        ),
                    );
                    continue;
                }
                stored.pair.value = value;
                stored.pair.session = None;
                if let Some(flags) = kv.flags {
                    stored.pair.flags = flags;
                }
                stored.pair.modify_index = index;
                stored.modified_at = current_timestamp();
                results.push(TxnResultItem {
                    kv: Some(result_pair(&stored.pair, false)),
                    ..Default::default()
                });
                overlay.insert(kv.key.clone(), Some(stored));
                dirty.insert(kv.key.clone());
            }
            verb => push_error(&mut errors, op_index, format!("unknown verb: {}", verb)),
        }
    }

    if !errors.is_empty() {
        return Ok(TxnResult {
            results: None,
            errors: Some(errors),
        });
    }

    let mut batch = WriteBatch::default();
    let mut session_index_puts: BTreeSet<String> = BTreeSet::new();
    let mut session_index_deletes: BTreeSet<String> = BTreeSet::new();

    for key in &dirty {
        let before_session = original
            .get(key)
            .and_then(|stored| stored.as_ref())
            .and_then(|stored| stored.pair.session.as_deref())
            .filter(|session| !session.is_empty());
        let after = overlay.get(key).and_then(|stored| stored.as_ref());

        match after {
            Some(stored) => {
                let bytes = serde_json::to_vec(stored)
                    .map_err(|e| format!("failed to encode key '{}': {}", key, e))?;
                batch.put_cf(cf_kv, key.as_bytes(), &bytes);
            }
            None => batch.delete_cf(cf_kv, key.as_bytes()),
        }

        let after_session = after
            .and_then(|stored| stored.pair.session.as_deref())
            .filter(|session| !session.is_empty());
        if before_session != after_session {
            if let Some(session) = before_session {
                session_index_deletes.insert(format!("kidx:{}:{}", session, key));
            }
            if let Some(session) = after_session {
                session_index_puts.insert(format!("kidx:{}:{}", session, key));
            }
        }
    }

    if let Some(cf_sessions) = cf_sessions {
        for index_key in session_index_deletes {
            batch.delete_cf(cf_sessions, index_key.as_bytes());
        }
        for index_key in session_index_puts {
            batch.put_cf(cf_sessions, index_key.as_bytes(), b"");
        }
    }

    db.write(batch)
        .map_err(|e| format!("KV transaction commit failed: {}", e))?;

    Ok(TxnResult {
        results: if results.is_empty() {
            None
        } else {
            Some(results)
        },
        errors: None,
    })
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
    let temp_dir = Arc::new(tempfile::tempdir().expect("Failed to create temp directory"));
    let mut db_opts = rocksdb::Options::default();
    db_opts.create_if_missing(true);
    db_opts.create_missing_column_families(true);

    let cf_opts = rocksdb::Options::default();
    let cfs = vec![
        rocksdb::ColumnFamilyDescriptor::new(CF_CONSUL_KV, cf_opts.clone()),
        rocksdb::ColumnFamilyDescriptor::new(CF_CONSUL_SESSIONS, cf_opts.clone()),
        rocksdb::ColumnFamilyDescriptor::new(CF_CONSUL_ACL, cf_opts.clone()),
        rocksdb::ColumnFamilyDescriptor::new(CF_CONSUL_QUERIES, cf_opts.clone()),
        rocksdb::ColumnFamilyDescriptor::new(crate::constants::CF_CONSUL_EVENTS, cf_opts),
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
    index_provider: web::Data<ConsulIndexProvider>,
    acl_service: web::Data<AclService>,
    req: HttpRequest,
    query: web::Query<KVQueryParams>,
) -> HttpResponse {
    // Extract key from path - handle the wildcard pattern
    let key = req.match_info().get("key").unwrap_or("").to_string();

    // Check ACL authorization for key read
    let authz = acl_service.authorize_request(&req, ResourceType::Key, &key, false);
    if !authz.allowed {
        crate::api_metrics::incr_endpoint("kv_get", "error");
        return HttpResponse::Forbidden().consul_error(ConsulError::new(&authz.reason));
    }
    crate::api_metrics::incr_endpoint("kv_get", "success");

    // Consistency is handled centrally by `ConsulReadConsistency` on the /v1
    // scope: non-stale reads are proxied to the leader and ?consistent gets a
    // ReadIndex barrier there. Do not re-check here or issue a second barrier.

    let raw = query.raw.unwrap_or(false);
    let keys_only = query.keys.unwrap_or(false);
    let recurse = query.recurse.unwrap_or(false);

    // Handle blocking watch request using the unified index provider.
    // This ensures blocking queries use the same index (Raft log index in cluster mode)
    // as the X-Consul-Index header returned to clients.
    if let Some(target_index) = query.index {
        let wait_dur = query
            .wait
            .as_deref()
            .and_then(ConsulIndexProvider::parse_wait_duration);
        let reached = index_provider
            .wait_for_change(ConsulTable::KVS, target_index, wait_dur)
            .await;
        if !reached {
            let has_data = kv_service.get(&key).is_some()
                || kv_service
                    .get_prefix(&key)
                    .iter()
                    .any(|p| !p.key.is_empty());
            if !has_data {
                let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::KVS));
                return consul_not_found(&meta).finish();
            }
        }
    }

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::KVS));

    // Handle keys-only request — support both ?separator and ?seperator (Consul legacy)
    if keys_only {
        let sep = query.separator.as_deref().or(query.seperator.as_deref());
        let keys = kv_service.get_keys(&key, sep);
        let body = serde_json::to_vec(&keys).unwrap_or_default();
        let len = body.len();
        return consul_ok(&meta)
            .content_type("application/json")
            .insert_header(("Content-Length", len))
            .body(body);
    }

    // Handle recursive get
    if recurse {
        let pairs = kv_service.get_prefix(&key);
        let body = serde_json::to_vec(&pairs).unwrap_or_default();
        let len = body.len();
        return consul_ok(&meta)
            .content_type("application/json")
            .insert_header(("Content-Length", len))
            .body(body);
    }

    // Single key get
    match kv_service.get(&key) {
        Some(pair) => {
            if raw {
                // Return raw value with security headers (XSS prevention)
                match pair.raw_value() {
                    Some(bytes) => {
                        let body = bytes.clone();
                        let mut builder = consul_ok(&meta);
                        apply_kv_raw_security_headers(&mut builder);
                        builder
                            .content_type("text/plain; charset=utf-8")
                            .insert_header(("Content-Length", body.len()))
                            .body(body)
                    }
                    None => consul_not_found(&meta).finish(),
                }
            } else {
                let body = serde_json::to_vec(&vec![pair]).unwrap_or_default();
                let len = body.len();
                consul_ok(&meta)
                    .content_type("application/json")
                    .insert_header(("Content-Length", len))
                    .body(body)
            }
        }
        None => consul_not_found(&meta).finish(),
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
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let key = req.match_info().get("key").unwrap_or("").to_string();

    // Validate key (matches Consul's validateKVKey)
    if let Err(reason) = validate_kv_key(&key) {
        return HttpResponse::BadRequest().consul_error(ConsulError::new(&reason));
    }

    // Enforce max value size (matches Consul's KVMaxValueSize, default 512 KiB)
    if body.len() > DEFAULT_KV_MAX_VALUE_SIZE {
        return HttpResponse::new(actix_web::http::StatusCode::PAYLOAD_TOO_LARGE);
    }

    // Check ACL authorization for key write
    let authz = acl_service.authorize_request(&req, ResourceType::Key, &key, true);
    if !authz.allowed {
        crate::api_metrics::incr_endpoint("kv_put", "error");
        return HttpResponse::Forbidden().consul_error(ConsulError::new(&authz.reason));
    }

    let value = String::from_utf8_lossy(&body).to_string();

    // Every write below goes through a `try_*` method. `Ok(false)` is a
    // business result (CAS/lock conflict); `Err` means Raft never applied the
    // write and must be reported as 503 rather than an acknowledged success.
    let outcome: Result<bool, String> = if let Some(ref session_id) = query.acquire {
        kv_service
            .try_put_and_acquire(&key, &value, query.flags, session_id)
            .await
    } else if let Some(ref session_id) = query.release {
        kv_service
            .try_put_and_release(&key, &value, query.flags, session_id)
            .await
    } else if let Some(cas_index) = query.cas {
        kv_service
            .try_cas(key.clone(), &value, cas_index, query.flags)
            .await
    } else {
        kv_service
            .try_put(key.clone(), &value, query.flags)
            .await
            .map(|_| true)
    };

    let success = match outcome {
        Ok(success) => success,
        Err(e) => return raft_write_unavailable("kv_put", &e),
    };
    // Only count a success after the replicated write is known to have landed;
    // `raft_write_unavailable` records the error counter for the 503 path.
    crate::api_metrics::incr_endpoint("kv_put", "success");
    if success {
        index_provider.increment(ConsulTable::KVS);
    }
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::KVS));
    consul_ok(&meta).json(success)
}

/// DELETE /v1/kv/{key:.*}
/// Delete a key or keys with prefix
pub async fn delete_kv(
    kv_service: web::Data<ConsulKVService>,
    index_provider: web::Data<ConsulIndexProvider>,
    acl_service: web::Data<AclService>,
    req: HttpRequest,
    query: web::Query<KVQueryParams>,
) -> HttpResponse {
    let key = req.match_info().get("key").unwrap_or("").to_string();

    // Check ACL authorization for key write
    let authz = acl_service.authorize_request(&req, ResourceType::Key, &key, true);
    if !authz.allowed {
        crate::api_metrics::incr_endpoint("kv_delete", "error");
        return HttpResponse::Forbidden().consul_error(ConsulError::new(&authz.reason));
    }

    let recurse = query.recurse.unwrap_or(false);

    // Upstream rejects `?cas` together with `?recurse`; a CAS delete is a
    // single-key operation and the FSM cannot apply one index to a tree.
    if recurse && query.cas.is_some() {
        crate::api_metrics::incr_endpoint("kv_delete", "error");
        return HttpResponse::BadRequest().consul_error("Conflicting flags: cas and recurse");
    }

    let cas_delete = query.cas.is_some();
    let outcome: Result<bool, String> = if let Some(cas_index) = query.cas {
        // The index check belongs in the FSM: another proposer can write
        // between this handler's read and the unconditional delete.
        kv_service.try_cas_delete(&key, cas_index).await
    } else if recurse {
        kv_service.try_delete_prefix(&key).await.map(|_| true)
    } else {
        kv_service.try_delete(&key).await
    };
    let applied = match outcome {
        Ok(applied) => applied,
        Err(e) => return raft_write_unavailable("kv_delete", &e),
    };
    crate::api_metrics::incr_endpoint("kv_delete", "success");

    if applied {
        index_provider.increment(ConsulTable::KVS);
    }
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::KVS));
    consul_ok(&meta).json(if cas_delete { applied } else { true })
}

/// PUT /v1/txn
/// Execute a transaction
pub async fn txn(
    kv_service: web::Data<ConsulKVService>,
    acl_service: web::Data<AclService>,
    index_provider: web::Data<ConsulIndexProvider>,
    req: HttpRequest,
    body: web::Json<Vec<TxnOp>>,
) -> HttpResponse {
    // Check ACL authorization for key write (transactions need write access)
    let authz = acl_service.authorize_request(&req, ResourceType::Key, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(&authz.reason));
    }

    let ops = body.into_inner();
    if ops.len() > ConsulKVService::MAX_TXN_OPS {
        return HttpResponse::build(actix_web::http::StatusCode::PAYLOAD_TOO_LARGE).consul_error(
            ConsulError::new(format!(
                "Transaction contains too many operations ({} > {})",
                ops.len(),
                ConsulKVService::MAX_TXN_OPS
            )),
        );
    }

    // Upstream validates keys and value sizes before proposing the Raft entry.
    for op in &ops {
        let Some(kv) = op.kv.as_ref() else {
            continue;
        };
        if let Err(reason) = validate_kv_key(&kv.key) {
            return HttpResponse::BadRequest().consul_error(ConsulError::new(format!(
                "Invalid key in transaction: {}",
                reason
            )));
        }
        if let Some(value) = kv.value.as_deref()
            && !value.is_empty()
        {
            match BASE64.decode(value) {
                Ok(bytes) if bytes.len() > DEFAULT_KV_MAX_VALUE_SIZE => {
                    return HttpResponse::build(actix_web::http::StatusCode::PAYLOAD_TOO_LARGE)
                        .consul_error(ConsulError::new(format!(
                            "Value for key '{}' is too large ({} > {} bytes)",
                            kv.key,
                            bytes.len(),
                            DEFAULT_KV_MAX_VALUE_SIZE
                        )));
                }
                Err(_) => {
                    return HttpResponse::BadRequest().consul_error(ConsulError::new(format!(
                        "Invalid base64 value for key '{}'",
                        kv.key
                    )));
                }
                _ => {}
            }
        }
    }

    let has_write = ops.iter().any(|op| {
        op.kv.as_ref().is_some_and(|kv| {
            matches!(
                kv.verb.to_ascii_lowercase().as_str(),
                "set" | "cas" | "delete" | "delete-cas" | "delete-tree" | "lock" | "unlock"
            )
        })
    });
    let result = kv_service.transaction(ops).await;

    // A failed replicated write is a service error, not a transaction conflict.
    if result.errors.as_ref().is_some_and(|errs| {
        errs.iter()
            .any(|e| e.what.starts_with("Raft write failed:"))
    }) {
        return raft_write_unavailable("kv_txn", "transaction could not be replicated");
    }
    if result.errors.is_some() {
        return HttpResponse::Conflict().json(result);
    }
    if has_write {
        index_provider.increment(ConsulTable::KVS);
    }
    HttpResponse::Ok().json(result)
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Default maximum KV value size in bytes (512 KiB, matches Consul's default).
const DEFAULT_KV_MAX_VALUE_SIZE: usize = 512 * 1024;

/// Validate a KV key following the original Consul rules from `kvs_endpoint.go`.
///
/// Rejects:
/// - Empty keys
/// - Keys starting with `/`
/// - Keys with leading or trailing whitespace
/// - Keys containing `..` path traversal components
///
/// Returns `Ok(())` if valid, `Err(reason)` if invalid.
fn validate_kv_key(key: &str) -> Result<(), String> {
    if key.is_empty() {
        return Err("Key must not be empty".to_string());
    }
    if key.starts_with('/') {
        return Err("Key must not begin with a '/'".to_string());
    }
    if key != key.trim() {
        return Err("Key must not have leading or trailing whitespace".to_string());
    }
    // Check for path traversal in raw key
    for component in key.split('/') {
        if component == ".." {
            return Err("Key must not contain '..' path traversal".to_string());
        }
    }
    // Check for common URL-encoded traversal attacks (%2E%2E = "..", %2F = "/")
    // Actix-web already decodes the URL path, but be defensive about double-encoding.
    let lower = key.to_lowercase();
    if lower.contains("%2e%2e") || lower.contains("%2f..") || lower.contains("..%2f") {
        return Err("Key must not contain encoded path traversal".to_string());
    }
    Ok(())
}

/// Classify the outcome of a replicated Consul write.
///
/// `Ok(bool)` is the state machine's business-level answer: `false` can mean a
/// CAS index conflict or a lock held by another session, and is a legitimate
/// result for the client. `Err` means the request never reached the state
/// machine (no leader, forwarding timeout, transport error) — reporting that
/// as success is what made `PUT /v1/kv` acknowledge writes that never landed.
pub(crate) fn classify_raft_write(outcome: Result<(bool, String), String>) -> Result<bool, String> {
    match outcome {
        Ok((success, _message)) => Ok(success),
        Err(e) => Err(format!("raft write failed: {}", e)),
    }
}

/// Build a 503 for a replicated write that never reached the state machine.
/// The old handlers returned `200 true` here, which is how a leaderless window
/// turned into an acknowledged write that was never persisted.
fn raft_write_unavailable(endpoint: &'static str, err: &str) -> HttpResponse {
    crate::api_metrics::incr_endpoint(endpoint, "error");
    HttpResponse::ServiceUnavailable()
        .consul_error(ConsulError::new(format!("Raft write failed: {}", err)))
}

/// Transaction counterpart: Consul reports transaction failures as 409 with an
/// error list, so a Raft failure becomes an explicit `TxnError` rather than a
/// partial success.
fn txn_write_failure(op_index: usize, err: &str) -> TxnResult {
    TxnResult {
        results: None,
        errors: Some(vec![TxnError {
            op_index: op_index as u32,
            what: format!("Raft write failed: {}", err),
        }]),
    }
}

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
        return HttpResponse::Forbidden().consul_error(ConsulError::new(&authz.reason));
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
        return HttpResponse::Forbidden().consul_error(ConsulError::new(&authz.reason));
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
    use actix_web::{App, http::StatusCode, web};

    #[actix_web::test]
    async fn put_kv_returns_503_when_raft_write_fails() {
        let kv = web::Data::new(ConsulKVService::new());
        kv.set_fail_raft_writes(true);
        let app = actix_web::test::init_service(
            App::new()
                .app_data(kv.clone())
                .app_data(web::Data::new(AclService::disabled()))
                .app_data(web::Data::new(ConsulIndexProvider::new()))
                .route("/v1/kv/{key:.*}", web::put().to(super::put_kv)),
        )
        .await;
        let req = actix_web::test::TestRequest::put()
            .uri("/v1/kv/test/fail")
            .set_payload("hello")
            .to_request();
        let resp = actix_web::test::call_service(&app, req).await;
        assert_eq!(
            resp.status(),
            StatusCode::SERVICE_UNAVAILABLE,
            "a failed Raft write must not be acknowledged as success"
        );
    }

    #[actix_web::test]
    async fn delete_kv_returns_503_when_raft_write_fails() {
        let kv = web::Data::new(ConsulKVService::new());
        kv.set_fail_raft_writes(true);
        let app = actix_web::test::init_service(
            App::new()
                .app_data(kv.clone())
                .app_data(web::Data::new(AclService::disabled()))
                .app_data(web::Data::new(ConsulIndexProvider::new()))
                .route("/v1/kv/{key:.*}", web::delete().to(super::delete_kv)),
        )
        .await;
        let req = actix_web::test::TestRequest::delete()
            .uri("/v1/kv/test/fail")
            .to_request();
        let resp = actix_web::test::call_service(&app, req).await;
        assert_eq!(
            resp.status(),
            StatusCode::SERVICE_UNAVAILABLE,
            "a failed replicated delete must not be acknowledged as success"
        );
    }

    #[actix_web::test]
    async fn put_kv_returns_true_when_write_succeeds() {
        let kv = web::Data::new(ConsulKVService::new());
        let app = actix_web::test::init_service(
            App::new()
                .app_data(kv.clone())
                .app_data(web::Data::new(AclService::disabled()))
                .app_data(web::Data::new(ConsulIndexProvider::new()))
                .route("/v1/kv/{key:.*}", web::put().to(super::put_kv)),
        )
        .await;
        let req = actix_web::test::TestRequest::put()
            .uri("/v1/kv/test/ok")
            .set_payload("hello")
            .to_request();
        let resp = actix_web::test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(actix_web::test::read_body(resp).await, "true");
    }

    #[test]
    fn classify_raft_write_propagates_transport_failure() {
        assert_eq!(
            super::classify_raft_write(Ok((true, String::new()))),
            Ok(true)
        );
        // A business-level rejection (CAS/lock conflict) is not a Raft failure.
        assert_eq!(
            super::classify_raft_write(Ok((false, "cas conflict".to_string()))),
            Ok(false)
        );
        let err = super::classify_raft_write(Err("no leader".to_string()));
        assert!(
            err.is_err(),
            "a transport failure must never be reported as an applied write"
        );
        assert!(err.unwrap_err().contains("no leader"));
    }

    #[test]
    fn raft_write_unavailable_returns_503() {
        let resp = super::raft_write_unavailable("kv_put", "no leader");
        assert_eq!(
            resp.status(),
            actix_web::http::StatusCode::SERVICE_UNAVAILABLE
        );
    }

    #[test]
    fn txn_write_failure_is_marked_as_raft_error() {
        let result = super::txn_write_failure(3, "no leader");
        let errors = result.errors.expect("transaction errors");
        assert_eq!(errors[0].op_index, 3);
        assert!(errors[0].what.starts_with("Raft write failed:"));
    }

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

        // Verify actual key-value content
        let keys: Vec<&str> = results.iter().map(|r| r.key.as_str()).collect();
        assert!(keys.contains(&"config/db/host"));
        assert!(keys.contains(&"config/db/port"));

        let host = results.iter().find(|r| r.key == "config/db/host").unwrap();
        assert_eq!(host.decoded_value(), Some("localhost".to_string()));

        let port = results.iter().find(|r| r.key == "config/db/port").unwrap();
        assert_eq!(port.decoded_value(), Some("3306".to_string()));
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
                    session: None,
                }),
                ..Default::default()
            },
            TxnOp {
                kv: Some(KVTxnOp {
                    verb: "set".to_string(),
                    key: "txn/key2".to_string(),
                    value: Some(BASE64.encode("value2".as_bytes())),
                    flags: None,
                    index: None,
                    session: None,
                }),
                ..Default::default()
            },
            TxnOp {
                kv: Some(KVTxnOp {
                    verb: "get".to_string(),
                    key: "txn/key1".to_string(),
                    value: None,
                    flags: None,
                    index: None,
                    session: None,
                }),
                ..Default::default()
            },
        ];

        let result = service.transaction(ops).await;
        assert!(result.errors.is_none());
        assert_eq!(result.results.as_ref().unwrap().len(), 3);

        // Verify the keys exist and have correct values
        let kv1 = service.get("txn/key1").unwrap();
        assert_eq!(kv1.decoded_value(), Some("value1".to_string()));
        assert_eq!(kv1.key, "txn/key1");

        let kv2 = service.get("txn/key2").unwrap();
        assert_eq!(kv2.decoded_value(), Some("value2".to_string()));
        assert_eq!(kv2.key, "txn/key2");

        // Verify the "get" operation in the transaction returned the correct value
        let txn_results = result.results.unwrap();
        let get_result = &txn_results[2];
        let kv = get_result.kv.as_ref().expect("kv result");
        assert_eq!(kv.key, "txn/key1");
        assert_eq!(kv.decoded_value(), Some("value1".to_string()));
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
                    session: None,
                }),
                ..Default::default()
            },
            TxnOp {
                kv: Some(KVTxnOp {
                    verb: "set".to_string(),
                    key: "txn/checked".to_string(),
                    value: Some(BASE64.encode("updated".as_bytes())),
                    flags: None,
                    index: None,
                    session: None,
                }),
                ..Default::default()
            },
        ];
        let result = service.transaction(ops).await;
        assert!(result.errors.is_none());

        // Verify the value was actually updated
        let updated = service.get("txn/checked").unwrap();
        assert_eq!(updated.decoded_value(), Some("updated".to_string()));

        // Transaction with wrong check-index should fail
        let ops_fail = vec![TxnOp {
            kv: Some(KVTxnOp {
                verb: "check-index".to_string(),
                key: "txn/checked".to_string(),
                value: None,
                flags: None,
                index: Some(999),
                session: None,
            }),
            ..Default::default()
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
                session: None,
            }),
            ..Default::default()
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
                session: None,
            }),
            ..Default::default()
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
                session: None,
            }),
            ..Default::default()
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
                session: None,
            }),
            ..Default::default()
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
                session: None,
            }),
            ..Default::default()
        }];
        let result = service.transaction(ops).await;
        assert!(result.errors.is_none());
        let items = result.results.unwrap();
        assert_eq!(items.len(), 2);

        // Verify the returned keys and values
        let keys: Vec<&str> = items
            .iter()
            .map(|i| i.kv.as_ref().unwrap().key.as_str())
            .collect();
        assert!(keys.contains(&"prefix/a"));
        assert!(keys.contains(&"prefix/b"));

        let item_a = items
            .iter()
            .find(|i| i.kv.as_ref().unwrap().key == "prefix/a")
            .unwrap();
        assert_eq!(
            item_a.kv.as_ref().unwrap().decoded_value(),
            Some("1".to_string())
        );

        let item_b = items
            .iter()
            .find(|i| i.kv.as_ref().unwrap().key == "prefix/b")
            .unwrap();
        assert_eq!(
            item_b.kv.as_ref().unwrap().decoded_value(),
            Some("2".to_string())
        );
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
                session: None,
            }),
            ..Default::default()
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

    fn txn_kv_op(
        verb: &str,
        key: &str,
        value: Option<&str>,
        index: Option<u64>,
        session: Option<&str>,
    ) -> TxnOp {
        TxnOp {
            kv: Some(KVTxnOp {
                verb: verb.to_string(),
                key: key.to_string(),
                value: value.map(|value| BASE64.encode(value.as_bytes())),
                flags: None,
                index,
                session: session.map(str::to_string),
            }),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_kv_transaction_rolls_back_every_write_on_conflict() {
        let service = ConsulKVService::new();
        let seeded = service.put("txn/seed".to_string(), "original", None).await;

        let result = service
            .transaction(vec![
                txn_kv_op("set", "txn/a", Some("a"), None, None),
                txn_kv_op(
                    "delete-cas",
                    "txn/seed",
                    None,
                    Some(seeded.modify_index + 999),
                    None,
                ),
                txn_kv_op("set", "txn/b", Some("b"), None, None),
            ])
            .await;

        assert!(result.errors.is_some());
        assert!(result.results.is_none());
        assert!(service.get("txn/a").is_none());
        assert!(service.get("txn/b").is_none());
        assert_eq!(
            service.get("txn/seed").unwrap().decoded_value(),
            Some("original".to_string())
        );
    }

    #[tokio::test]
    async fn test_kv_transaction_reads_see_earlier_writes() {
        let service = ConsulKVService::new();

        let result = service
            .transaction(vec![
                txn_kv_op("set", "txn/read-after-write", Some("value"), None, None),
                txn_kv_op("get", "txn/read-after-write", None, None, None),
            ])
            .await;

        assert!(result.errors.is_none());
        let results = result.results.expect("transaction results");
        assert_eq!(results.len(), 2);
        assert_eq!(
            results[1].kv.as_ref().expect("get result").decoded_value(),
            Some("value".to_string())
        );
    }

    #[tokio::test]
    async fn test_kv_transaction_delete_cas_missing_zero_is_noop() {
        let service = ConsulKVService::new();

        let ok = service
            .transaction(vec![txn_kv_op(
                "delete-cas",
                "txn/missing",
                None,
                Some(0),
                None,
            )])
            .await;
        assert!(ok.errors.is_none());

        let conflict = service
            .transaction(vec![txn_kv_op(
                "delete-cas",
                "txn/missing",
                None,
                Some(7),
                None,
            )])
            .await;
        assert!(conflict.errors.is_some());
    }

    #[tokio::test]
    async fn test_kv_transaction_lock_unlock_updates_session_index() {
        let service = ConsulKVService::new();
        let cf_sessions = service.db.cf_handle(CF_CONSUL_SESSIONS).unwrap();
        service
            .db
            .put_cf(cf_sessions, b"txn-session", b"{}")
            .expect("seed session");

        let locked = service
            .transaction(vec![txn_kv_op(
                "lock",
                "txn/lock",
                Some("locked"),
                None,
                Some("txn-session"),
            )])
            .await;
        assert!(locked.errors.is_none());
        let pair = service.get("txn/lock").expect("locked key");
        assert_eq!(pair.session.as_deref(), Some("txn-session"));
        assert_eq!(pair.lock_index, 1);
        assert!(
            service
                .db
                .get_cf(cf_sessions, b"kidx:txn-session:txn/lock")
                .expect("read session index")
                .is_some()
        );

        let check = service
            .transaction(vec![txn_kv_op(
                "check-session",
                "txn/lock",
                None,
                None,
                Some("other-session"),
            )])
            .await;
        assert!(check.errors.is_some());

        let unlocked = service
            .transaction(vec![txn_kv_op(
                "unlock",
                "txn/lock",
                None,
                None,
                Some("txn-session"),
            )])
            .await;
        assert!(unlocked.errors.is_none());
        assert!(
            service
                .get("txn/lock")
                .expect("unlocked key")
                .session
                .is_none()
        );
        assert!(
            service
                .db
                .get_cf(cf_sessions, b"kidx:txn-session:txn/lock")
                .expect("read session index")
                .is_none()
        );
    }

    #[tokio::test]
    async fn test_kv_transaction_lock_obeys_standalone_lock_delay() {
        let delay = crate::lock_delay::ConsulLockDelay::default();
        delay.set("txn/delayed", std::time::Duration::from_secs(1));
        let service = ConsulKVService::new().with_lock_delay(delay);
        let cf_sessions = service.db.cf_handle(CF_CONSUL_SESSIONS).unwrap();
        service
            .db
            .put_cf(cf_sessions, b"txn-session", b"{}")
            .expect("seed session");
        service.put("txn/delayed".to_string(), "value", None).await;

        let conflict = service
            .transaction(vec![txn_kv_op(
                "lock",
                "txn/delayed",
                Some("locked"),
                None,
                Some("txn-session"),
            )])
            .await;
        assert!(conflict.errors.is_some());
        assert!(
            conflict
                .errors
                .expect("lock delay error")
                .iter()
                .any(|error| error.what.contains("Lock delay active"))
        );
        assert!(service.get("txn/delayed").unwrap().session.is_none());
    }
}
