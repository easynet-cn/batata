//! Leader-local Consul lock-delay state.
//!
//! Upstream Consul records lock delay in server-local wall time when a session
//! is invalidated, and checks it only on the leader before committing a lock
//! acquisition (`agent/consul/state/delay_ce.go`, `agent/consul/kvs_endpoint.go`).
//! Keeping the map out of the FSM avoids replicas disagreeing about a
//! wall-clock boundary. Every node records the delay on session-destroy apply;
//! only the leader consults it in the pre-commit write guard. The leader also
//! records it in the guard's post-validation hook so a reacquire cannot win
//! the race before the FSM apply callback runs.

use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use batata_consistency::raft::{RaftRequest, RaftWriteGuard};
use dashmap::DashMap;
use rocksdb::DB;

use crate::kv::TxnOp;
use crate::raft::{CONSUL_PLUGIN_ID, ConsulRaftRequest};

/// Upstream `structs.MaxLockDelay`.
pub const MAX_LOCK_DELAY: Duration = Duration::from_secs(60);

/// Shared leader-local expiration map keyed by Consul KV key.
#[derive(Clone, Default)]
pub struct ConsulLockDelay {
    expirations: Arc<DashMap<String, Instant>>,
}

impl ConsulLockDelay {
    /// Record a delay for one key. Zero delays are no-ops, and an already
    /// active delay is kept: replaying the same committed destroy must not
    /// move the expiration forward.
    pub fn set(&self, key: &str, delay: Duration) {
        if delay.is_zero() {
            return;
        }
        let capped = delay.min(MAX_LOCK_DELAY);
        let expires = Instant::now() + capped;
        self.expirations
            .entry(key.to_string())
            .and_modify(|current| {
                if *current <= Instant::now() {
                    *current = expires;
                }
            })
            .or_insert(expires);
    }

    /// Record the same capped delay for every key force-released by a session.
    pub fn set_keys(&self, keys: &[String], lock_delay_nanos: u64) {
        if lock_delay_nanos == 0 {
            return;
        }
        // `set` owns the upstream 60s cap; do not cap twice here or a mutant
        // that removes the cap in `set` would still pass every test.
        let delay = Duration::from_nanos(lock_delay_nanos);
        for key in keys {
            self.set(key, delay);
        }
    }

    /// Whether a key is currently delayed. Expired entries are removed lazily.
    pub fn is_active(&self, key: &str) -> bool {
        let active = self
            .expirations
            .get(key)
            .is_some_and(|expires| *expires > Instant::now());
        if !active {
            self.expirations.remove(key);
        }
        active
    }

    /// First transaction `lock` verb targeting a delayed key, if any.
    pub fn first_active_txn_key(&self, ops: &[TxnOp]) -> Option<String> {
        ops.iter().find_map(|op| {
            op.kv.as_ref().and_then(|kv| {
                let locked = kv.verb.eq_ignore_ascii_case("lock")
                    && kv
                        .session
                        .as_deref()
                        .is_some_and(|session| !session.is_empty());
                (locked && self.is_active(&kv.key)).then(|| kv.key.clone())
            })
        })
    }

    /// Number of not-yet-expired entries. Used by tests and diagnostics.
    pub fn active_count(&self) -> usize {
        self.expirations
            .retain(|_, expires| *expires > Instant::now());
        self.expirations.len()
    }
}

/// Leader-side pre-commit guard that rejects acquisitions of delayed keys.
pub struct ConsulLockDelayGuard {
    delay: ConsulLockDelay,
    db: Arc<DB>,
}

impl ConsulLockDelayGuard {
    /// Build a guard sharing the given lock-delay map and committed state.
    pub fn new(delay: ConsulLockDelay, db: Arc<DB>) -> Self {
        Self { delay, db }
    }

    fn consul_request(request: &RaftRequest) -> Option<ConsulRaftRequest> {
        let RaftRequest::PluginWrite {
            plugin_id, payload, ..
        } = request
        else {
            return None;
        };
        if plugin_id != CONSUL_PLUGIN_ID {
            return None;
        }
        serde_json::from_slice(payload).ok()
    }

    fn delayed_key(consul_request: &ConsulRaftRequest) -> Option<&str> {
        match consul_request {
            ConsulRaftRequest::KVAcquireSession { key, .. } => Some(key),
            ConsulRaftRequest::KVTransaction { ops } => ops.iter().find_map(|op| {
                op.kv.as_ref().and_then(|kv| {
                    let locked = kv.verb.eq_ignore_ascii_case("lock")
                        && kv
                            .session
                            .as_deref()
                            .is_some_and(|session| !session.is_empty());
                    locked.then_some(kv.key.as_str())
                })
            }),
            _ => None,
        }
    }
}

#[async_trait]
impl RaftWriteGuard for ConsulLockDelayGuard {
    async fn validate(&self, request: &RaftRequest) -> Result<(), String> {
        let Some(consul_request) = Self::consul_request(request) else {
            return Ok(());
        };
        if let Some(key) = Self::delayed_key(&consul_request)
            && self.delay.is_active(key)
        {
            return Err(format!("Lock delay active for key {}", key));
        }
        Ok(())
    }

    async fn on_accepted(&self, request: &RaftRequest) {
        let Some(ConsulRaftRequest::SessionDestroy { session_id }) = Self::consul_request(request)
        else {
            return;
        };
        let info = crate::raft::plugin_handler::session_delay_info(&self.db, &session_id);
        self.delay.set_keys(&info.keys, info.lock_delay_nanos);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::constants::CF_CONSUL_SESSIONS;
    use crate::kv::{KVTxnOp, open_temp_consul_db};

    fn txn_op(verb: &str, key: &str, session: Option<&str>) -> TxnOp {
        TxnOp {
            kv: Some(KVTxnOp {
                verb: verb.to_string(),
                key: key.to_string(),
                value: None,
                flags: None,
                index: None,
                session: session.map(str::to_string),
            }),
            ..TxnOp::default()
        }
    }

    fn plugin_write(request: &ConsulRaftRequest, plugin_id: &str) -> RaftRequest {
        RaftRequest::PluginWrite {
            plugin_id: plugin_id.to_string(),
            op_type: request.op_type().to_string(),
            payload: serde_json::to_vec(request).unwrap(),
        }
    }

    fn guard_for(delay: &ConsulLockDelay) -> (ConsulLockDelayGuard, Arc<tempfile::TempDir>) {
        let (db, dir) = open_temp_consul_db();
        (ConsulLockDelayGuard::new(delay.clone(), db), dir)
    }

    #[test]
    fn zero_delay_is_a_noop() {
        let delay = ConsulLockDelay::default();
        delay.set("key", Duration::ZERO);
        assert!(!delay.is_active("key"));
        assert_eq!(delay.active_count(), 0);
    }

    #[test]
    fn caps_lock_delay_at_upstream_max() {
        let delay = ConsulLockDelay::default();
        delay.set_keys(
            &["key".to_string()],
            MAX_LOCK_DELAY.as_nanos() as u64 + 1_000_000_000,
        );
        let expires = delay.expirations.get("key").map(|e| *e).unwrap();
        assert!(expires <= Instant::now() + MAX_LOCK_DELAY);
    }

    #[test]
    fn expires_per_key() {
        let delay = ConsulLockDelay::default();
        delay.set("a", Duration::from_millis(40));
        delay.set("b", Duration::from_secs(1));
        assert!(delay.is_active("a"));
        assert!(delay.is_active("b"));
        std::thread::sleep(Duration::from_millis(60));
        assert!(!delay.is_active("a"));
        assert!(delay.is_active("b"));
        assert_eq!(delay.active_count(), 1);
    }

    #[test]
    fn replay_does_not_extend_an_active_delay() {
        let delay = ConsulLockDelay::default();
        delay.set("key", Duration::from_millis(50));
        let first = *delay.expirations.get("key").unwrap();
        std::thread::sleep(Duration::from_millis(10));
        delay.set("key", Duration::from_millis(50));
        assert_eq!(*delay.expirations.get("key").unwrap(), first);
    }

    #[tokio::test]
    async fn guard_rejects_delayed_acquire_on_consul_plugin_only() {
        let delay = ConsulLockDelay::default();
        delay.set("locked/key", Duration::from_secs(1));
        let (guard, _dir) = guard_for(&delay);
        let request = ConsulRaftRequest::KVAcquireSession {
            key: "locked/key".to_string(),
            session_id: "session-2".to_string(),
            stored_kv_json: "{}".to_string(),
        };

        assert!(
            guard
                .validate(&plugin_write(&request, CONSUL_PLUGIN_ID))
                .await
                .is_err()
        );
        assert!(
            guard
                .validate(&plugin_write(&request, "other-plugin"))
                .await
                .is_ok()
        );
        assert!(guard.validate(&RaftRequest::Noop).await.is_ok());
    }

    #[tokio::test]
    async fn guard_checks_txn_lock_verbs() {
        let delay = ConsulLockDelay::default();
        delay.set("locked/key", Duration::from_secs(1));
        let (guard, _dir) = guard_for(&delay);
        let ops = vec![txn_op("LOCK", "locked/key", Some("session-2"))];
        let request = ConsulRaftRequest::KVTransaction { ops };

        assert!(
            guard
                .validate(&plugin_write(&request, CONSUL_PLUGIN_ID))
                .await
                .is_err()
        );

        let read_only = ConsulRaftRequest::KVTransaction {
            ops: vec![txn_op("get", "locked/key", None)],
        };
        assert!(
            guard
                .validate(&plugin_write(&read_only, CONSUL_PLUGIN_ID))
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn guard_checks_only_active_keys() {
        let delay = ConsulLockDelay::default();
        delay.set("locked/key", Duration::from_millis(40));
        let (guard, _dir) = guard_for(&delay);
        let request = ConsulRaftRequest::KVAcquireSession {
            key: "locked/key".to_string(),
            session_id: "session-2".to_string(),
            stored_kv_json: "{}".to_string(),
        };

        std::thread::sleep(Duration::from_millis(60));
        assert!(
            guard
                .validate(&plugin_write(&request, CONSUL_PLUGIN_ID))
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn accepted_session_destroy_records_held_key_delay() {
        let delay = ConsulLockDelay::default();
        let (db, _dir) = open_temp_consul_db();
        let cf = db.cf_handle(CF_CONSUL_SESSIONS).unwrap();
        db.put_cf(
            cf,
            b"session-1",
            r#"{"session":{"ID":"session-1","LockDelay":1000000000}}"#.as_bytes(),
        )
        .unwrap();
        db.put_cf(cf, b"kidx:session-1:lock/key", b"").unwrap();
        let guard = ConsulLockDelayGuard::new(delay.clone(), db);

        let request = ConsulRaftRequest::SessionDestroy {
            session_id: "session-1".to_string(),
        };
        guard
            .on_accepted(&plugin_write(&request, CONSUL_PLUGIN_ID))
            .await;

        assert!(delay.is_active("lock/key"));
    }

    #[test]
    fn first_active_txn_key_requires_lock_with_session() {
        let delay = ConsulLockDelay::default();
        delay.set("no-session", Duration::from_secs(1));
        delay.set("locked/key", Duration::from_secs(1));
        let ops = vec![
            txn_op("get", "locked/key", None),
            txn_op("lock", "no-session", None),
            txn_op("lock", "locked/key", Some("session-2")),
        ];
        assert_eq!(
            delay.first_active_txn_key(&ops).as_deref(),
            Some("locked/key")
        );
    }
}
