//! Apply-back hook for the Consul Raft plugin handler.
//!
//! On follower nodes, `ConsulRaftPluginHandler::apply` writes committed
//! Raft entries to RocksDB column families, but the in-memory caches
//! (`ConsulNamingStore` DashMap) are never touched. Reads against the
//! `ConsulNamingStore` then return stale data on followers.
//!
//! This hook bridges the gap: after a Raft apply succeeds, the handler
//! notifies the hook so concrete implementations can refresh their
//! in-memory views. Mirrors the naming-side `NamingApplyHook` pattern.

use std::sync::Arc;

/// Callback invoked by `ConsulRaftPluginHandler::apply` after a committed
/// Raft entry has been written to RocksDB. Implementations must be
/// idempotent — the state machine may replay applies during snapshot
/// install or cold-start recovery.
///
/// Methods take &str slices and raw JSON so the trait has no dependency
/// on Consul model types — keeps the hook cheap to implement in tests.
pub trait ConsulApplyHook: Send + Sync {
    /// A `CatalogRegister` was committed. `key` is the `ConsulNamingStore`
    /// key (`"{namespace}/{service_name}/{service_id}"`). `registration_json`
    /// is the serialized `AgentServiceRegistration`.
    fn on_catalog_register(&self, _key: &str, _registration_json: &str) {}

    /// A `CatalogDeregister` was committed.
    fn on_catalog_deregister(&self, _key: &str) {}

    /// A `HealthCheckRegister` was committed. `check_key` is the
    /// node-qualified registry key, `config_json` the serialized
    /// `InstanceCheckConfig`. Implementations should mirror the config into
    /// their local registry so queries on any node see the check.
    ///
    /// The check must NOT be scheduled for probing here: only the node that
    /// owns it executes the probe, and its result arrives separately through
    /// the health-status replication channel.
    fn on_health_check_register(&self, _check_key: &str, _config_json: &str) {}

    /// A `HealthCheckDeregister` was committed.
    fn on_health_check_deregister(&self, _check_key: &str) {}

    /// Any ACL write (token/policy/role/auth-method/binding-rule set or
    /// delete, or bootstrap) was committed. Implementations should
    /// invalidate their in-memory ACL caches so follower authorization
    /// decisions reflect the new state immediately rather than waiting
    /// for TTL expiry. The hook intentionally does not identify which
    /// specific entity changed — blunt invalidation is cheap, and
    /// tracking every variant would double the trait surface.
    fn on_acl_change(&self) {}

    /// Any Connect CA mutation (root set, config update, key set) or
    /// intention change was committed. Implementations should refresh their
    /// in-memory CA material from RocksDB so follower-side leaf signing and
    /// SDS state reflect the committed change within the apply latency.
    fn on_ca_change(&self) {}

    /// A `SessionDestroy` was committed. `keys` are the KV keys the session
    /// held, and `lock_delay_nanos` is its configured `LockDelay`. Each node
    /// records wall time locally; this must never be part of FSM equality.
    fn on_session_lock_delay(&self, _keys: &[String], _lock_delay_nanos: u64) {}
}

/// Shared slot holding the optional hook. Registered once at plugin
/// construction and consulted on every apply.
///
/// This is a **synchronous** lock on purpose. `ConsulRaftPluginHandler::apply`
/// runs on Tokio's blocking pool, so it cannot await an async lock; the old
/// `tokio::sync::RwLock` forced a `try_read()` there, which silently dropped a
/// committed notification whenever a writer held the slot. A blocking read is
/// safe because writes are startup-only and never hold the guard across an
/// await or a hook dispatch.
pub type SharedConsulApplyHook = Arc<std::sync::RwLock<Option<Arc<dyn ConsulApplyHook>>>>;

/// Create a new, initially-empty hook slot.
pub fn new_shared_hook() -> SharedConsulApplyHook {
    Arc::new(std::sync::RwLock::new(None))
}

/// Resolve the installed hook for one committed apply.
///
/// Clones the owned `Arc` out and drops the guard before returning, so a hook
/// that installs or replaces the slot cannot deadlock against its own read.
/// Poison is recovered rather than propagated: a panicking startup writer must
/// not permanently disable every later notification on this node.
pub fn current_apply_hook(slot: &SharedConsulApplyHook) -> Option<Arc<dyn ConsulApplyHook>> {
    let guard = slot.read().unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.clone()
}

/// Install (or replace) the hook. Poison-tolerant for the same reason as
/// [`current_apply_hook`].
pub fn install_apply_hook(slot: &SharedConsulApplyHook, hook: Arc<dyn ConsulApplyHook>) {
    let mut guard = slot
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *guard = Some(hook);
}
