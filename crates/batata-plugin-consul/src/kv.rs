// Consul KV Store API HTTP handlers
// Implements Consul-compatible key-value store endpoints
// Supports both in-memory storage and persistent storage via ConfigService

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use actix_web::{HttpRequest, HttpResponse, web};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use dashmap::DashMap;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};

use crate::acl::{AclService, ResourceType};
use crate::model::ConsulError;

// Constants for ConfigService mapping
const CONSUL_KV_NAMESPACE: &str = "public";
const CONSUL_KV_GROUP: &str = "consul-kv";

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
#[derive(Debug, Clone)]
struct StoredKV {
    pair: KVPair,
    #[allow(dead_code)] // Reserved for future CAS and watch operations
    created_at: i64,
    modified_at: i64,
}

/// Query parameters for KV endpoints
#[derive(Debug, Clone, Deserialize, Default)]
pub struct KVQueryParams {
    /// Return raw value (not JSON wrapped)
    pub raw: Option<bool>,

    /// Return only keys (no values)
    pub keys: Option<bool>,

    /// Recursively get all keys under prefix
    pub recurse: Option<bool>,

    /// Check-and-set index for conditional writes
    pub cas: Option<u64>,

    /// Custom flags to store with the key
    pub flags: Option<u64>,

    /// Datacenter
    pub dc: Option<String>,

    /// Namespace (Enterprise)
    pub ns: Option<String>,

    /// Separator for keys listing
    pub separator: Option<String>,

    /// Wait for index to be >= this value (blocking wait for watch)
    pub index: Option<u64>,

    /// Wait time in milliseconds for blocking wait
    pub wait: Option<u64>,
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
/// In-memory key-value store with Consul-compatible API
#[derive(Clone)]
pub struct ConsulKVService {
    /// Key-value storage: key -> StoredKV
    store: Arc<DashMap<String, StoredKV>>,
    /// Global index counter
    index: Arc<std::sync::atomic::AtomicU64>,
}

impl ConsulKVService {
    pub fn new() -> Self {
        Self {
            store: Arc::new(DashMap::new()),
            index: Arc::new(std::sync::atomic::AtomicU64::new(1)),
        }
    }

    /// Get the next index
    fn next_index(&self) -> u64 {
        self.index.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }

    /// Get current index for watch operations
    pub fn current_index(&self) -> u64 {
        self.index.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Wait for index to be >= target (simple blocking watch implementation)
    /// Returns true if index reached, false on timeout
    pub async fn wait_for_index(&self, target_index: u64, timeout_ms: u64) -> bool {
        let start = std::time::Instant::now();
        let timeout_duration = std::time::Duration::from_millis(timeout_ms);

        loop {
            if self.current_index() >= target_index {
                return true;
            }

            if start.elapsed() >= timeout_duration {
                return false;
            }

            // Poll every 100ms to avoid busy waiting
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }

    /// Get a single key
    pub fn get(&self, key: &str) -> Option<KVPair> {
        self.store.get(key).map(|entry| entry.pair.clone())
    }

    /// Get keys with prefix (recursive)
    pub fn get_prefix(&self, prefix: &str) -> Vec<KVPair> {
        self.store
            .iter()
            .filter(|entry| entry.key().starts_with(prefix))
            .map(|entry| entry.pair.clone())
            .collect()
    }

    /// Get keys only (no values)
    pub fn get_keys(&self, prefix: &str, separator: Option<&str>) -> Vec<String> {
        let mut keys: Vec<String> = self
            .store
            .iter()
            .filter(|entry| entry.key().starts_with(prefix))
            .map(|entry| entry.key().clone())
            .collect();

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

    /// Put a key-value pair
    pub fn put(&self, key: String, value: &str, flags: Option<u64>) -> KVPair {
        let index = self.next_index();
        let now = current_timestamp();

        if let Some(mut existing) = self.store.get_mut(&key) {
            // Update existing
            existing.pair.modify_index = index;
            existing.pair.value = Some(BASE64.encode(value.as_bytes()));
            if let Some(f) = flags {
                existing.pair.flags = f;
            }
            existing.modified_at = now;
            existing.pair.clone()
        } else {
            // Create new
            let pair = KVPair {
                key: key.clone(),
                create_index: index,
                modify_index: index,
                lock_index: 0,
                flags: flags.unwrap_or(0),
                value: Some(BASE64.encode(value.as_bytes())),
                session: None,
            };

            self.store.insert(
                key,
                StoredKV {
                    pair: pair.clone(),
                    created_at: now,
                    modified_at: now,
                },
            );
            pair
        }
    }

    /// Check-and-set: only update if modify_index matches
    pub fn cas(&self, key: String, value: &str, cas_index: u64, flags: Option<u64>) -> bool {
        if let Some(mut existing) = self.store.get_mut(&key) {
            if existing.pair.modify_index == cas_index {
                let index = self.next_index();
                existing.pair.modify_index = index;
                existing.pair.value = Some(BASE64.encode(value.as_bytes()));
                if let Some(f) = flags {
                    existing.pair.flags = f;
                }
                existing.modified_at = current_timestamp();
                return true;
            }
            false
        } else if cas_index == 0 {
            // cas=0 means create only if doesn't exist
            self.put(key, value, flags);
            true
        } else {
            false
        }
    }

    /// Delete a key
    pub fn delete(&self, key: &str) -> bool {
        self.store.remove(key).is_some()
    }

    /// Delete keys with prefix
    pub fn delete_prefix(&self, prefix: &str) -> u32 {
        let keys_to_delete: Vec<String> = self
            .store
            .iter()
            .filter(|entry| entry.key().starts_with(prefix))
            .map(|entry| entry.key().clone())
            .collect();

        let count = keys_to_delete.len() as u32;
        for key in keys_to_delete {
            self.store.remove(&key);
        }
        count
    }

    /// Execute a transaction
    pub fn transaction(&self, ops: Vec<TxnOp>) -> TxnResult {
        let mut results: Vec<TxnResultItem> = Vec::new();
        let mut errors: Vec<TxnError> = Vec::new();

        for (idx, op) in ops.into_iter().enumerate() {
            if let Some(kv_op) = op.kv {
                match kv_op.verb.to_lowercase().as_str() {
                    "get" => {
                        if let Some(pair) = self.get(&kv_op.key) {
                            results.push(TxnResultItem { kv: pair });
                        } else {
                            errors.push(TxnError {
                                op_index: idx as u32,
                                what: format!("key '{}' not found", kv_op.key),
                            });
                        }
                    }
                    "set" => {
                        let value = kv_op
                            .value
                            .as_ref()
                            .and_then(|v| BASE64.decode(v).ok())
                            .and_then(|bytes| String::from_utf8(bytes).ok())
                            .unwrap_or_default();

                        let pair = self.put(kv_op.key, &value, kv_op.flags);
                        results.push(TxnResultItem { kv: pair });
                    }
                    "cas" => {
                        let value = kv_op
                            .value
                            .as_ref()
                            .and_then(|v| BASE64.decode(v).ok())
                            .and_then(|bytes| String::from_utf8(bytes).ok())
                            .unwrap_or_default();

                        let cas_index = kv_op.index.unwrap_or(0);
                        if self.cas(kv_op.key.clone(), &value, cas_index, kv_op.flags) {
                            if let Some(pair) = self.get(&kv_op.key) {
                                results.push(TxnResultItem { kv: pair });
                            }
                        } else {
                            errors.push(TxnError {
                                op_index: idx as u32,
                                what: "CAS failed: index mismatch".to_string(),
                            });
                        }
                    }
                    "delete" => {
                        if self.delete(&kv_op.key) {
                            results.push(TxnResultItem {
                                kv: KVPair::key_only(kv_op.key),
                            });
                        }
                        // Delete is always successful even if key doesn't exist
                    }
                    "delete-tree" => {
                        let count = self.delete_prefix(&kv_op.key);
                        results.push(TxnResultItem {
                            kv: KVPair {
                                key: kv_op.key,
                                create_index: 0,
                                modify_index: count as u64,
                                lock_index: 0,
                                flags: 0,
                                value: None,
                                session: None,
                            },
                        });
                    }
                    "delete-cas" => {
                        let cas_index = kv_op.index.unwrap_or(0);
                        if let Some(existing) = self.store.get(&kv_op.key) {
                            if existing.pair.modify_index == cas_index {
                                drop(existing);
                                self.delete(&kv_op.key);
                                results.push(TxnResultItem {
                                    kv: KVPair::key_only(kv_op.key),
                                });
                            } else {
                                errors.push(TxnError {
                                    op_index: idx as u32,
                                    what: "CAS failed: index mismatch".to_string(),
                                });
                            }
                        } else {
                            errors.push(TxnError {
                                op_index: idx as u32,
                                what: format!("key '{}' not found", kv_op.key),
                            });
                        }
                    }
                    verb => {
                        errors.push(TxnError {
                            op_index: idx as u32,
                            what: format!("unknown verb: {}", verb),
                        });
                    }
                }
            }
        }

        TxnResult {
            results: if results.is_empty() {
                None
            } else {
                Some(results)
            },
            errors: if errors.is_empty() {
                None
            } else {
                Some(errors)
            },
        }
    }
}

impl Default for ConsulKVService {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Persistent KV Store Service (Using ConfigService)
// ============================================================================

/// Stored KV metadata for persistent storage
#[derive(Debug, Clone, Serialize, Deserialize)]
struct KVMetadata {
    value: String, // Base64 encoded
    flags: u64,
    session: Option<String>,
    create_index: u64,
    modify_index: u64,
    lock_index: u64,
}

/// Consul KV Store service with database persistence
/// Uses Batata's ConfigService for storage
#[derive(Clone)]
pub struct ConsulKVServicePersistent {
    db: Arc<DatabaseConnection>,
    /// In-memory cache for performance
    cache: Arc<DashMap<String, StoredKV>>,
    /// Global index counter
    index: Arc<std::sync::atomic::AtomicU64>,
}

impl ConsulKVServicePersistent {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self {
            db,
            cache: Arc::new(DashMap::new()),
            index: Arc::new(std::sync::atomic::AtomicU64::new(1)),
        }
    }

    /// Convert Consul key to ConfigService dataId
    /// Replaces "/" with ":" to avoid path issues
    fn key_to_data_id(key: &str) -> String {
        format!("kv:{}", key.replace('/', ":"))
    }

    /// Convert ConfigService dataId back to Consul key
    fn data_id_to_key(data_id: &str) -> String {
        data_id
            .strip_prefix("kv:")
            .unwrap_or(data_id)
            .replace(':', "/")
    }

    /// Get the next index
    fn next_index(&self) -> u64 {
        self.index.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }

    /// Get current index
    pub fn current_index(&self) -> u64 {
        self.index.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Get a single key from database
    pub async fn get(&self, key: &str) -> Option<KVPair> {
        // Check cache first
        if let Some(cached) = self.cache.get(key) {
            return Some(cached.pair.clone());
        }

        // Query from database
        let data_id = Self::key_to_data_id(key);
        match batata_config::service::config::find_one(
            &self.db,
            &data_id,
            CONSUL_KV_GROUP,
            CONSUL_KV_NAMESPACE,
        )
        .await
        {
            Ok(Some(config)) => {
                // Parse metadata from content
                if let Ok(metadata) =
                    serde_json::from_str::<KVMetadata>(&config.config_info.config_info_base.content)
                {
                    let pair = KVPair {
                        key: key.to_string(),
                        create_index: metadata.create_index,
                        modify_index: metadata.modify_index,
                        lock_index: metadata.lock_index,
                        flags: metadata.flags,
                        value: Some(metadata.value),
                        session: metadata.session,
                    };

                    // Update cache
                    self.cache.insert(
                        key.to_string(),
                        StoredKV {
                            pair: pair.clone(),
                            created_at: config.create_time,
                            modified_at: config.modify_time,
                        },
                    );

                    Some(pair)
                } else {
                    // Fallback: treat content as raw value
                    let pair = KVPair::new(
                        key.to_string(),
                        &config.config_info.config_info_base.content,
                    );
                    Some(pair)
                }
            }
            _ => None,
        }
    }

    /// Get keys with prefix (recursive)
    pub async fn get_prefix(&self, prefix: &str) -> Vec<KVPair> {
        let data_id_prefix = Self::key_to_data_id(prefix);

        // Search for configs with prefix
        match batata_config::service::config::search_page(
            &self.db,
            1,
            1000, // Get up to 1000 results
            CONSUL_KV_NAMESPACE,
            &format!("{}*", data_id_prefix),
            CONSUL_KV_GROUP,
            "",
            vec![],
            vec![],
            "",
        )
        .await
        {
            Ok(page) => {
                let mut pairs = Vec::new();
                for info in page.page_items {
                    let key = Self::data_id_to_key(&info.data_id);
                    if key.starts_with(prefix)
                        && let Some(pair) = self.get(&key).await
                    {
                        pairs.push(pair);
                    }
                }
                pairs
            }
            Err(_) => vec![],
        }
    }

    /// Get keys only (no values)
    pub async fn get_keys(&self, prefix: &str, separator: Option<&str>) -> Vec<String> {
        let pairs = self.get_prefix(prefix).await;
        let mut keys: Vec<String> = pairs.into_iter().map(|p| p.key).collect();

        // Handle separator for folder-like listing
        if let Some(sep) = separator {
            let prefix_len = prefix.len();
            let mut unique_keys: std::collections::HashSet<String> =
                std::collections::HashSet::new();

            for key in keys.drain(..) {
                let remainder = &key[prefix_len..];
                if let Some(pos) = remainder.find(sep) {
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

    /// Put a key-value pair
    pub async fn put(&self, key: String, value: &str, flags: Option<u64>) -> Option<KVPair> {
        let index = self.next_index();
        let now = current_timestamp();
        let data_id = Self::key_to_data_id(&key);

        // Get existing for create_index
        let (create_index, lock_index, session) = if let Some(existing) = self.get(&key).await {
            (existing.create_index, existing.lock_index, existing.session)
        } else {
            (index, 0, None)
        };

        let metadata = KVMetadata {
            value: BASE64.encode(value.as_bytes()),
            flags: flags.unwrap_or(0),
            session,
            create_index,
            modify_index: index,
            lock_index,
        };

        let content = serde_json::to_string(&metadata).ok()?;

        // Save to database
        match batata_config::service::config::create_or_update(
            &self.db,
            &data_id,
            CONSUL_KV_GROUP,
            CONSUL_KV_NAMESPACE,
            &content,
            "consul-kv",
            "system",
            "127.0.0.1",
            "",
            &format!("Consul KV: {}", key),
            "",
            "",
            "json",
            "",
            "",
        )
        .await
        {
            Ok(_) => {
                let pair = KVPair {
                    key: key.clone(),
                    create_index,
                    modify_index: index,
                    lock_index,
                    flags: flags.unwrap_or(0),
                    value: Some(metadata.value),
                    session: metadata.session,
                };

                // Update cache
                self.cache.insert(
                    key,
                    StoredKV {
                        pair: pair.clone(),
                        created_at: now,
                        modified_at: now,
                    },
                );

                Some(pair)
            }
            Err(e) => {
                tracing::error!("Failed to save KV to database: {}", e);
                None
            }
        }
    }

    /// Check-and-set: only update if modify_index matches
    pub async fn cas(&self, key: String, value: &str, cas_index: u64, flags: Option<u64>) -> bool {
        if let Some(existing) = self.get(&key).await {
            if existing.modify_index == cas_index {
                return self.put(key, value, flags).await.is_some();
            }
            false
        } else if cas_index == 0 {
            // cas=0 means create only if doesn't exist
            self.put(key, value, flags).await.is_some()
        } else {
            false
        }
    }

    /// Delete a key
    pub async fn delete(&self, key: &str) -> bool {
        let data_id = Self::key_to_data_id(key);

        // Remove from cache
        self.cache.remove(key);

        // Delete from database
        match batata_config::service::config::delete(
            &self.db,
            &data_id,
            CONSUL_KV_GROUP,
            CONSUL_KV_NAMESPACE,
            "",
            "127.0.0.1",
            "system",
        )
        .await
        {
            Ok(deleted) => deleted,
            Err(e) => {
                tracing::error!("Failed to delete KV from database: {}", e);
                false
            }
        }
    }

    /// Delete keys with prefix
    pub async fn delete_prefix(&self, prefix: &str) -> u32 {
        let keys = self.get_keys(prefix, None).await;
        let mut count = 0;

        for key in keys {
            if self.delete(&key).await {
                count += 1;
            }
        }

        count
    }

    /// Execute a transaction
    pub async fn transaction(&self, ops: Vec<TxnOp>) -> TxnResult {
        let mut results: Vec<TxnResultItem> = Vec::new();
        let mut errors: Vec<TxnError> = Vec::new();

        for (idx, op) in ops.into_iter().enumerate() {
            if let Some(kv_op) = op.kv {
                match kv_op.verb.to_lowercase().as_str() {
                    "get" => {
                        if let Some(pair) = self.get(&kv_op.key).await {
                            results.push(TxnResultItem { kv: pair });
                        } else {
                            errors.push(TxnError {
                                op_index: idx as u32,
                                what: format!("key '{}' not found", kv_op.key),
                            });
                        }
                    }
                    "set" => {
                        let value = kv_op
                            .value
                            .as_ref()
                            .and_then(|v| BASE64.decode(v).ok())
                            .and_then(|bytes| String::from_utf8(bytes).ok())
                            .unwrap_or_default();

                        if let Some(pair) = self.put(kv_op.key, &value, kv_op.flags).await {
                            results.push(TxnResultItem { kv: pair });
                        } else {
                            errors.push(TxnError {
                                op_index: idx as u32,
                                what: "Failed to set key".to_string(),
                            });
                        }
                    }
                    "cas" => {
                        let value = kv_op
                            .value
                            .as_ref()
                            .and_then(|v| BASE64.decode(v).ok())
                            .and_then(|bytes| String::from_utf8(bytes).ok())
                            .unwrap_or_default();

                        let cas_index = kv_op.index.unwrap_or(0);
                        if self
                            .cas(kv_op.key.clone(), &value, cas_index, kv_op.flags)
                            .await
                        {
                            if let Some(pair) = self.get(&kv_op.key).await {
                                results.push(TxnResultItem { kv: pair });
                            }
                        } else {
                            errors.push(TxnError {
                                op_index: idx as u32,
                                what: "CAS failed: index mismatch".to_string(),
                            });
                        }
                    }
                    "delete" => {
                        self.delete(&kv_op.key).await;
                        results.push(TxnResultItem {
                            kv: KVPair::key_only(kv_op.key),
                        });
                    }
                    "delete-tree" => {
                        let count = self.delete_prefix(&kv_op.key).await;
                        results.push(TxnResultItem {
                            kv: KVPair {
                                key: kv_op.key,
                                create_index: 0,
                                modify_index: count as u64,
                                lock_index: 0,
                                flags: 0,
                                value: None,
                                session: None,
                            },
                        });
                    }
                    "delete-cas" => {
                        let cas_index = kv_op.index.unwrap_or(0);
                        if let Some(existing) = self.get(&kv_op.key).await {
                            if existing.modify_index == cas_index {
                                self.delete(&kv_op.key).await;
                                results.push(TxnResultItem {
                                    kv: KVPair::key_only(kv_op.key),
                                });
                            } else {
                                errors.push(TxnError {
                                    op_index: idx as u32,
                                    what: "CAS failed: index mismatch".to_string(),
                                });
                            }
                        } else {
                            errors.push(TxnError {
                                op_index: idx as u32,
                                what: format!("key '{}' not found", kv_op.key),
                            });
                        }
                    }
                    verb => {
                        errors.push(TxnError {
                            op_index: idx as u32,
                            what: format!("unknown verb: {}", verb),
                        });
                    }
                }
            }
        }

        TxnResult {
            results: if results.is_empty() {
                None
            } else {
                Some(results)
            },
            errors: if errors.is_empty() {
                None
            } else {
                Some(errors)
            },
        }
    }

    /// Wait for index (blocking watch)
    pub async fn wait_for_index(&self, target_index: u64, timeout_ms: u64) -> bool {
        let start = std::time::Instant::now();
        let timeout_duration = std::time::Duration::from_millis(timeout_ms);

        loop {
            if self.current_index() >= target_index {
                return true;
            }

            if start.elapsed() >= timeout_duration {
                return false;
            }

            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }
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

    // Handle keys-only request
    if keys_only {
        let keys = kv_service.get_keys(&key, query.separator.as_deref());
        if keys.is_empty() {
            return HttpResponse::NotFound().finish();
        }
        return HttpResponse::Ok().json(keys);
    }

    // Handle recursive get
    if recurse {
        let pairs = kv_service.get_prefix(&key);
        if pairs.is_empty() {
            return HttpResponse::NotFound().finish();
        }
        return HttpResponse::Ok().json(pairs);
    }

    // Single key get
    match kv_service.get(&key) {
        Some(pair) => {
            if raw {
                // Return raw value
                match pair.raw_value() {
                    Some(bytes) => HttpResponse::Ok()
                        .content_type("application/octet-stream")
                        .body(bytes),
                    None => HttpResponse::NotFound().finish(),
                }
            } else {
                HttpResponse::Ok().json(vec![pair])
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

    // Check-and-set if cas parameter is provided
    if let Some(cas_index) = query.cas {
        let success = kv_service.cas(key, &value, cas_index, query.flags);
        return HttpResponse::Ok().json(success);
    }

    // Regular put
    kv_service.put(key, &value, query.flags);
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
        kv_service.delete_prefix(&key);
    } else {
        kv_service.delete(&key);
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
    let result = kv_service.transaction(ops);

    // Return 409 Conflict if there are errors
    if result.errors.is_some() {
        HttpResponse::Conflict().json(result)
    } else {
        HttpResponse::Ok().json(result)
    }
}

// ============================================================================
// Persistent HTTP Handlers
// ============================================================================

/// GET /v1/kv/{key:.*} (Persistent version)
/// Get a key or keys with prefix from database
pub async fn get_kv_persistent(
    kv_service: web::Data<ConsulKVServicePersistent>,
    acl_service: web::Data<AclService>,
    req: HttpRequest,
    query: web::Query<KVQueryParams>,
) -> HttpResponse {
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
        let wait_ms = query.wait.unwrap_or(5000);
        let reached = kv_service.wait_for_index(target_index, wait_ms).await;
        if !reached {
            let has_data = kv_service.get(&key).await.is_some()
                || !kv_service.get_prefix(&key).await.is_empty();
            if !has_data {
                return HttpResponse::NotFound().finish();
            }
        }
    }

    // Handle keys-only request
    if keys_only {
        let keys = kv_service.get_keys(&key, query.separator.as_deref()).await;
        if keys.is_empty() {
            return HttpResponse::NotFound().finish();
        }
        return HttpResponse::Ok().json(keys);
    }

    // Handle recursive get
    if recurse {
        let pairs = kv_service.get_prefix(&key).await;
        if pairs.is_empty() {
            return HttpResponse::NotFound().finish();
        }
        return HttpResponse::Ok().json(pairs);
    }

    // Single key get
    match kv_service.get(&key).await {
        Some(pair) => {
            if raw {
                match pair.raw_value() {
                    Some(bytes) => HttpResponse::Ok()
                        .content_type("application/octet-stream")
                        .body(bytes),
                    None => HttpResponse::NotFound().finish(),
                }
            } else {
                HttpResponse::Ok().json(vec![pair])
            }
        }
        None => HttpResponse::NotFound().finish(),
    }
}

/// PUT /v1/kv/{key:.*} (Persistent version)
/// Put a key-value pair to database
pub async fn put_kv_persistent(
    kv_service: web::Data<ConsulKVServicePersistent>,
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

    // Check-and-set if cas parameter is provided
    if let Some(cas_index) = query.cas {
        let success = kv_service.cas(key, &value, cas_index, query.flags).await;
        return HttpResponse::Ok().json(success);
    }

    // Regular put
    match kv_service.put(key, &value, query.flags).await {
        Some(_) => HttpResponse::Ok().json(true),
        None => HttpResponse::InternalServerError().json(ConsulError::new("Failed to store key")),
    }
}

/// DELETE /v1/kv/{key:.*} (Persistent version)
/// Delete a key or keys with prefix from database
pub async fn delete_kv_persistent(
    kv_service: web::Data<ConsulKVServicePersistent>,
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

/// PUT /v1/txn (Persistent version)
/// Execute a transaction against database
pub async fn txn_persistent(
    kv_service: web::Data<ConsulKVServicePersistent>,
    acl_service: web::Data<AclService>,
    req: HttpRequest,
    body: web::Json<Vec<TxnOp>>,
) -> HttpResponse {
    // Check ACL authorization for key write
    let authz = acl_service.authorize_request(&req, ResourceType::Key, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let ops = body.into_inner();
    let result = kv_service.transaction(ops).await;

    if result.errors.is_some() {
        HttpResponse::Conflict().json(result)
    } else {
        HttpResponse::Ok().json(result)
    }
}

/// Export all KV pairs as JSON (Persistent version)
pub async fn export_kv_persistent(
    kv_service: web::Data<ConsulKVServicePersistent>,
    acl_service: web::Data<AclService>,
    req: HttpRequest,
) -> HttpResponse {
    // Check ACL authorization for read access to all keys
    let authz = acl_service.authorize_request(&req, ResourceType::Key, "*", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    // Get all KV pairs by querying with empty prefix
    let pairs = kv_service.get_prefix("").await;
    let count = pairs.len();

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

/// Import KV pairs from JSON (Persistent version)
pub async fn import_kv_persistent(
    kv_service: web::Data<ConsulKVServicePersistent>,
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

    for pair in pairs_to_import {
        if let Some(decoded) = pair.decoded_value() {
            if kv_service
                .put(pair.key.clone(), &decoded, Some(pair.flags))
                .await
                .is_some()
            {
                success_count += 1;
            } else {
                failed_count += 1;
            }
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

    #[test]
    fn test_kv_service_put_get() {
        let service = ConsulKVService::new();

        service.put("config/database".to_string(), "mysql://localhost", None);

        let result = service.get("config/database");
        assert!(result.is_some());
        assert_eq!(
            result.unwrap().decoded_value(),
            Some("mysql://localhost".to_string())
        );
    }

    #[test]
    fn test_kv_service_prefix() {
        let service = ConsulKVService::new();

        service.put("config/db/host".to_string(), "localhost", None);
        service.put("config/db/port".to_string(), "3306", None);
        service.put("config/cache/host".to_string(), "redis", None);

        let results = service.get_prefix("config/db/");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_kv_service_keys() {
        let service = ConsulKVService::new();

        service.put("config/db/host".to_string(), "localhost", None);
        service.put("config/db/port".to_string(), "3306", None);
        service.put("config/cache/host".to_string(), "redis", None);

        let keys = service.get_keys("config/", None);
        assert_eq!(keys.len(), 3);

        // With separator
        let folders = service.get_keys("config/", Some("/"));
        assert_eq!(folders.len(), 2); // config/db/ and config/cache/
    }

    #[test]
    fn test_kv_service_cas() {
        let service = ConsulKVService::new();

        // Create new with cas=0
        assert!(service.cas("key1".to_string(), "value1", 0, None));

        // Get the modify index
        let pair = service.get("key1").unwrap();
        let modify_index = pair.modify_index;

        // CAS with correct index should succeed
        assert!(service.cas("key1".to_string(), "value2", modify_index, None));

        // CAS with wrong index should fail
        assert!(!service.cas("key1".to_string(), "value3", modify_index, None));

        // Value should be value2
        assert_eq!(
            service.get("key1").unwrap().decoded_value(),
            Some("value2".to_string())
        );
    }

    #[test]
    fn test_kv_service_delete() {
        let service = ConsulKVService::new();

        service.put("key1".to_string(), "value1", None);
        service.put("prefix/key2".to_string(), "value2", None);
        service.put("prefix/key3".to_string(), "value3", None);

        // Delete single key
        assert!(service.delete("key1"));
        assert!(service.get("key1").is_none());

        // Delete prefix
        let count = service.delete_prefix("prefix/");
        assert_eq!(count, 2);
        assert!(service.get("prefix/key2").is_none());
        assert!(service.get("prefix/key3").is_none());
    }

    #[test]
    fn test_kv_transaction() {
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

        let result = service.transaction(ops);
        assert!(result.errors.is_none());
        assert_eq!(result.results.as_ref().unwrap().len(), 3);

        // Verify the keys exist
        assert!(service.get("txn/key1").is_some());
        assert!(service.get("txn/key2").is_some());
    }
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
    let pairs: Vec<KVPair> = kv_service
        .store
        .iter()
        .map(|entry| entry.value().pair.clone())
        .collect();

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
            kv_service.put(pair.key.clone(), &decoded, Some(pair.flags));
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
