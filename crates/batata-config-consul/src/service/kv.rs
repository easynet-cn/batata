//! Consul KV Store — in-memory implementation

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use dashmap::DashMap;
use tokio::sync::Notify;

use crate::error::ConsulKvError;
use crate::model::{
    KVPair, KVQueryMeta, KVQueryParams, TxnError, TxnOp, TxnResult, TxnResultItem, TxnVerb,
};

/// Internal stored KV entry
#[derive(Debug, Clone)]
struct StoredKV {
    pair: KVPair,
}

/// In-memory Consul KV store
///
/// Provides Consul-compatible KV operations with CAS, prefix scan,
/// session locking, and blocking query support.
#[derive(Clone)]
pub struct ConsulKvServiceImpl {
    /// Key: path, Value: stored KV pair
    store: Arc<DashMap<String, StoredKV>>,
    /// Monotonic index counter
    index: Arc<AtomicU64>,
    /// Notification for blocking queries
    notify: Arc<Notify>,
}

impl ConsulKvServiceImpl {
    pub fn new() -> Self {
        Self {
            store: Arc::new(DashMap::new()),
            index: Arc::new(AtomicU64::new(1)),
            notify: Arc::new(Notify::new()),
        }
    }

    fn next_index(&self) -> u64 {
        self.index.fetch_add(1, Ordering::Relaxed) + 1
    }

    fn current_index(&self) -> u64 {
        self.index.load(Ordering::Relaxed)
    }

    fn query_meta(&self) -> KVQueryMeta {
        KVQueryMeta {
            last_index: self.current_index(),
            known_leader: true,
        }
    }

    /// Get a single key
    pub fn get(
        &self,
        key: &str,
        _params: &KVQueryParams,
    ) -> Result<(Option<KVPair>, KVQueryMeta), ConsulKvError> {
        let pair = self.store.get(key).map(|entry| entry.pair.clone());
        Ok((pair, self.query_meta()))
    }

    /// List keys with a prefix (recursive)
    pub fn list(
        &self,
        prefix: &str,
        _params: &KVQueryParams,
    ) -> Result<(Vec<KVPair>, KVQueryMeta), ConsulKvError> {
        let results: Vec<KVPair> = self
            .store
            .iter()
            .filter(|entry| entry.key().starts_with(prefix))
            .map(|entry| entry.value().pair.clone())
            .collect();
        Ok((results, self.query_meta()))
    }

    /// Get only key names with a prefix
    pub fn keys(
        &self,
        prefix: &str,
        separator: Option<&str>,
    ) -> Result<(Vec<String>, KVQueryMeta), ConsulKvError> {
        let mut keys: Vec<String> = if let Some(sep) = separator {
            // Return unique prefixes up to the separator
            let mut prefixes = std::collections::HashSet::new();
            for entry in self.store.iter() {
                let k = entry.key();
                if let Some(suffix) = k.strip_prefix(prefix) {
                    if let Some(pos) = suffix.find(sep) {
                        prefixes.insert(format!("{}{}{}", prefix, &suffix[..pos], sep));
                    } else {
                        prefixes.insert(k.clone());
                    }
                }
            }
            prefixes.into_iter().collect()
        } else {
            self.store
                .iter()
                .filter(|entry| entry.key().starts_with(prefix))
                .map(|entry| entry.key().clone())
                .collect()
        };
        keys.sort();
        Ok((keys, self.query_meta()))
    }

    /// Put a key-value pair
    ///
    /// Supports CAS, session acquire/release via params.
    pub fn put(
        &self,
        key: &str,
        value: &[u8],
        params: &KVQueryParams,
    ) -> Result<bool, ConsulKvError> {
        let new_index = self.next_index();
        let encoded = BASE64.encode(value);

        // CAS operation
        if let Some(cas_index) = params.cas {
            let current = self.store.get(key);
            let current_index = current.as_ref().map(|e| e.pair.modify_index).unwrap_or(0);

            if cas_index != current_index {
                return Ok(false); // CAS failed
            }
        }

        // Session acquire
        if let Some(ref session_id) = params.acquire
            && let Some(existing) = self.store.get(key)
        {
            // Key already locked by another session
            if let Some(ref held) = existing.pair.session
                && held != session_id
            {
                return Ok(false);
            }
        }

        let existing = self.store.get(key);
        let create_index = existing
            .as_ref()
            .map(|e| e.pair.create_index)
            .unwrap_or(new_index);
        let lock_index = existing.as_ref().map(|e| e.pair.lock_index).unwrap_or(0);
        let flags = params
            .flags
            .unwrap_or_else(|| existing.as_ref().map(|e| e.pair.flags).unwrap_or(0));
        let session = if params.acquire.is_some() {
            params.acquire.clone()
        } else if params.release.is_some() {
            None // Release the lock
        } else {
            existing.as_ref().and_then(|e| e.pair.session.clone())
        };
        let new_lock_index = if params.acquire.is_some() {
            lock_index + 1
        } else {
            lock_index
        };

        drop(existing);

        self.store.insert(
            key.to_string(),
            StoredKV {
                pair: KVPair {
                    key: key.to_string(),
                    create_index,
                    modify_index: new_index,
                    lock_index: new_lock_index,
                    flags,
                    value: Some(encoded),
                    session,
                },
            },
        );

        self.notify.notify_waiters();
        Ok(true)
    }

    /// Delete a key or prefix
    pub fn delete(&self, key: &str, params: &KVQueryParams) -> Result<bool, ConsulKvError> {
        // CAS delete
        if let Some(cas_index) = params.cas
            && let Some(existing) = self.store.get(key)
            && existing.pair.modify_index != cas_index
        {
            return Ok(false);
        }

        if params.recurse {
            // Delete all keys with prefix
            let keys_to_remove: Vec<String> = self
                .store
                .iter()
                .filter(|entry| entry.key().starts_with(key))
                .map(|entry| entry.key().clone())
                .collect();
            for k in keys_to_remove {
                self.store.remove(&k);
            }
        } else {
            self.store.remove(key);
        }

        self.notify.notify_waiters();
        Ok(true)
    }

    /// Execute a transaction
    pub fn txn(&self, ops: Vec<TxnOp>) -> Result<TxnResult, ConsulKvError> {
        let mut results = Vec::new();
        let mut errors = Vec::new();

        // Validate all operations first (check phase)
        for (i, op) in ops.iter().enumerate() {
            match op.kv.verb {
                TxnVerb::CheckIndex => {
                    if let Some(expected) = op.kv.index {
                        let actual = self
                            .store
                            .get(&op.kv.key)
                            .map(|e| e.pair.modify_index)
                            .unwrap_or(0);
                        if actual != expected {
                            errors.push(TxnError {
                                op_index: i as u32,
                                what: format!("index mismatch: expected {expected}, got {actual}"),
                            });
                        }
                    }
                }
                TxnVerb::CheckSession => {
                    let actual = self
                        .store
                        .get(&op.kv.key)
                        .and_then(|e| e.pair.session.clone());
                    if actual.as_deref() != op.kv.session.as_deref() {
                        errors.push(TxnError {
                            op_index: i as u32,
                            what: "session mismatch".to_string(),
                        });
                    }
                }
                _ => {}
            }
        }

        // If any check failed, abort
        if !errors.is_empty() {
            return Ok(TxnResult {
                results: None,
                errors: Some(errors),
            });
        }

        // Execute phase
        for (i, op) in ops.iter().enumerate() {
            match op.kv.verb {
                TxnVerb::Get => {
                    if let Some(entry) = self.store.get(&op.kv.key) {
                        results.push(TxnResultItem {
                            kv: entry.pair.clone(),
                        });
                    }
                }
                TxnVerb::Set => {
                    let params = KVQueryParams::default();
                    let value = op
                        .kv
                        .value
                        .as_ref()
                        .and_then(|v| BASE64.decode(v).ok())
                        .unwrap_or_default();
                    if self.put(&op.kv.key, &value, &params).unwrap_or(false)
                        && let Some(entry) = self.store.get(&op.kv.key)
                    {
                        results.push(TxnResultItem {
                            kv: entry.pair.clone(),
                        });
                    }
                }
                TxnVerb::Cas => {
                    let params = KVQueryParams {
                        cas: op.kv.index,
                        ..Default::default()
                    };
                    let value = op
                        .kv
                        .value
                        .as_ref()
                        .and_then(|v| BASE64.decode(v).ok())
                        .unwrap_or_default();
                    if !self.put(&op.kv.key, &value, &params).unwrap_or(false) {
                        errors.push(TxnError {
                            op_index: i as u32,
                            what: "CAS failed".to_string(),
                        });
                    }
                }
                TxnVerb::Delete => {
                    let params = KVQueryParams::default();
                    self.delete(&op.kv.key, &params).ok();
                }
                TxnVerb::DeleteTree => {
                    let params = KVQueryParams {
                        recurse: true,
                        ..Default::default()
                    };
                    self.delete(&op.kv.key, &params).ok();
                }
                TxnVerb::DeleteCas => {
                    let params = KVQueryParams {
                        cas: op.kv.index,
                        ..Default::default()
                    };
                    if !self.delete(&op.kv.key, &params).unwrap_or(false) {
                        errors.push(TxnError {
                            op_index: i as u32,
                            what: "CAS delete failed".to_string(),
                        });
                    }
                }
                TxnVerb::Lock => {
                    let params = KVQueryParams {
                        acquire: op.kv.session.clone(),
                        ..Default::default()
                    };
                    let value = op
                        .kv
                        .value
                        .as_ref()
                        .and_then(|v| BASE64.decode(v).ok())
                        .unwrap_or_default();
                    if !self.put(&op.kv.key, &value, &params).unwrap_or(false) {
                        errors.push(TxnError {
                            op_index: i as u32,
                            what: "lock failed".to_string(),
                        });
                    }
                }
                TxnVerb::Unlock => {
                    let params = KVQueryParams {
                        release: op.kv.session.clone(),
                        ..Default::default()
                    };
                    let value = op
                        .kv
                        .value
                        .as_ref()
                        .and_then(|v| BASE64.decode(v).ok())
                        .unwrap_or_default();
                    self.put(&op.kv.key, &value, &params).ok();
                }
                TxnVerb::CheckIndex | TxnVerb::CheckSession => {
                    // Already handled in check phase
                }
            }
        }

        Ok(TxnResult {
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
        })
    }

    /// Get store size
    pub fn len(&self) -> usize {
        self.store.len()
    }

    /// Check if store is empty
    pub fn is_empty(&self) -> bool {
        self.store.is_empty()
    }
}

impl Default for ConsulKvServiceImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::KVTxnOp;

    #[test]
    fn test_put_and_get() {
        let kv = ConsulKvServiceImpl::new();
        let params = KVQueryParams::default();

        kv.put("config/app/db_url", b"postgres://localhost", &params)
            .unwrap();

        let (pair, meta) = kv.get("config/app/db_url", &params).unwrap();
        assert!(pair.is_some());
        let pair = pair.unwrap();
        assert_eq!(pair.key, "config/app/db_url");
        assert_eq!(
            pair.decoded_value(),
            Some("postgres://localhost".to_string())
        );
        assert!(meta.last_index > 0);
    }

    #[test]
    fn test_get_nonexistent() {
        let kv = ConsulKvServiceImpl::new();
        let params = KVQueryParams::default();
        let (pair, _) = kv.get("nonexistent", &params).unwrap();
        assert!(pair.is_none());
    }

    #[test]
    fn test_prefix_list() {
        let kv = ConsulKvServiceImpl::new();
        let params = KVQueryParams::default();

        kv.put("config/app/db_url", b"postgres://localhost", &params)
            .unwrap();
        kv.put("config/app/redis_url", b"redis://localhost", &params)
            .unwrap();
        kv.put("config/other/key", b"value", &params).unwrap();

        let (results, _) = kv.list("config/app/", &params).unwrap();
        assert_eq!(results.len(), 2);

        let (results, _) = kv.list("config/", &params).unwrap();
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_keys_with_separator() {
        let kv = ConsulKvServiceImpl::new();
        let params = KVQueryParams::default();

        kv.put("config/app/db", b"v1", &params).unwrap();
        kv.put("config/app/redis", b"v2", &params).unwrap();
        kv.put("config/other/key", b"v3", &params).unwrap();

        // With separator "/" at prefix "config/"
        let (keys, _) = kv.keys("config/", Some("/")).unwrap();
        assert_eq!(keys.len(), 2); // "config/app/" and "config/other/"
        assert!(keys.contains(&"config/app/".to_string()));
        assert!(keys.contains(&"config/other/".to_string()));
    }

    #[test]
    fn test_cas_success() {
        let kv = ConsulKvServiceImpl::new();
        let params = KVQueryParams::default();

        kv.put("key", b"v1", &params).unwrap();

        let (pair, _) = kv.get("key", &params).unwrap();
        let modify_index = pair.unwrap().modify_index;

        // CAS with correct index → success
        let cas_params = KVQueryParams {
            cas: Some(modify_index),
            ..Default::default()
        };
        assert!(kv.put("key", b"v2", &cas_params).unwrap());

        let (pair, _) = kv.get("key", &params).unwrap();
        assert_eq!(pair.unwrap().decoded_value(), Some("v2".to_string()));
    }

    #[test]
    fn test_cas_failure() {
        let kv = ConsulKvServiceImpl::new();
        let params = KVQueryParams::default();

        kv.put("key", b"v1", &params).unwrap();

        // CAS with wrong index → failure
        let cas_params = KVQueryParams {
            cas: Some(999),
            ..Default::default()
        };
        assert!(!kv.put("key", b"v2", &cas_params).unwrap());

        // Value unchanged
        let (pair, _) = kv.get("key", &params).unwrap();
        assert_eq!(pair.unwrap().decoded_value(), Some("v1".to_string()));
    }

    #[test]
    fn test_delete() {
        let kv = ConsulKvServiceImpl::new();
        let params = KVQueryParams::default();

        kv.put("key", b"value", &params).unwrap();
        assert_eq!(kv.len(), 1);

        kv.delete("key", &params).unwrap();
        assert_eq!(kv.len(), 0);
    }

    #[test]
    fn test_recursive_delete() {
        let kv = ConsulKvServiceImpl::new();
        let params = KVQueryParams::default();

        kv.put("config/a", b"1", &params).unwrap();
        kv.put("config/b", b"2", &params).unwrap();
        kv.put("other/c", b"3", &params).unwrap();
        assert_eq!(kv.len(), 3);

        let del_params = KVQueryParams {
            recurse: true,
            ..Default::default()
        };
        kv.delete("config/", &del_params).unwrap();
        assert_eq!(kv.len(), 1);
    }

    #[test]
    fn test_session_lock() {
        let kv = ConsulKvServiceImpl::new();

        // Acquire lock
        let acquire_params = KVQueryParams {
            acquire: Some("session-1".to_string()),
            ..Default::default()
        };
        assert!(kv.put("lock/leader", b"node-1", &acquire_params).unwrap());

        // Another session tries to acquire → fails
        let acquire_params2 = KVQueryParams {
            acquire: Some("session-2".to_string()),
            ..Default::default()
        };
        assert!(!kv.put("lock/leader", b"node-2", &acquire_params2).unwrap());

        // Same session can re-acquire
        assert!(
            kv.put("lock/leader", b"node-1-updated", &acquire_params)
                .unwrap()
        );

        // Release lock
        let release_params = KVQueryParams {
            release: Some("session-1".to_string()),
            ..Default::default()
        };
        kv.put("lock/leader", b"", &release_params).unwrap();

        // Now session-2 can acquire
        assert!(kv.put("lock/leader", b"node-2", &acquire_params2).unwrap());
    }

    #[test]
    fn test_transaction() {
        let kv = ConsulKvServiceImpl::new();
        let params = KVQueryParams::default();

        kv.put("key1", b"v1", &params).unwrap();
        kv.put("key2", b"v2", &params).unwrap();

        let ops = vec![
            TxnOp {
                kv: KVTxnOp {
                    verb: TxnVerb::Get,
                    key: "key1".to_string(),
                    value: None,
                    flags: None,
                    index: None,
                    session: None,
                },
            },
            TxnOp {
                kv: KVTxnOp {
                    verb: TxnVerb::Set,
                    key: "key3".to_string(),
                    value: Some(BASE64.encode(b"v3")),
                    flags: None,
                    index: None,
                    session: None,
                },
            },
            TxnOp {
                kv: KVTxnOp {
                    verb: TxnVerb::Delete,
                    key: "key2".to_string(),
                    value: None,
                    flags: None,
                    index: None,
                    session: None,
                },
            },
        ];

        let result = kv.txn(ops).unwrap();
        assert!(result.errors.is_none());
        assert_eq!(result.results.as_ref().unwrap().len(), 2); // get + set results

        // key1 still exists, key2 deleted, key3 created
        assert!(kv.get("key1", &params).unwrap().0.is_some());
        assert!(kv.get("key2", &params).unwrap().0.is_none());
        assert!(kv.get("key3", &params).unwrap().0.is_some());
    }
}
