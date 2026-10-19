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
}

/// Shared slot holding the optional hook. Registered once at plugin
/// construction and consulted on every apply.
pub type SharedConsulApplyHook = Arc<tokio::sync::RwLock<Option<Arc<dyn ConsulApplyHook>>>>;

/// The `new_shared_hook` function.
pub fn new_shared_hook() -> SharedConsulApplyHook {
    Arc::new(tokio::sync::RwLock::new(None))
}
