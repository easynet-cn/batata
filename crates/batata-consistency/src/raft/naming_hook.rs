//! Apply-back hook for persistent naming instances.
//!
//! When the Raft state machine applies a `PersistentInstance*` variant, it
//! writes to RocksDB `CF_INSTANCES` and then invokes the registered hook to
//! flow the change into the in-memory `NamingService` DashMap so reads see
//! it without restart. The hook lives in `batata-consistency` to avoid a
//! reverse dependency on `batata-naming`.

use std::sync::Arc;

use super::state_machine::StoredInstance;

/// Callback invoked by `RocksStateMachine` after a persistent instance
/// apply succeeds. Implementations must be idempotent — the state machine
/// may replay an apply during snapshot install or cold-start recovery.
pub trait NamingApplyHook: Send + Sync {
    /// Called after a persistent instance register/deregister-update apply succeeds.
    fn on_register(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        instance_id: &str,
        ip: &str,
        port: u16,
        weight: f64,
        healthy: bool,
        enabled: bool,
        metadata: &str,
        cluster_name: &str,
    );

    /// Called after a persistent instance deregister apply succeeds.
    fn on_deregister(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        instance_id: &str,
    );

    /// Called after a persistent instance update apply succeeds.
    #[allow(clippy::too_many_arguments)]
    fn on_update(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        instance_id: &str,
        ip: Option<&str>,
        port: Option<u16>,
        weight: Option<f64>,
        healthy: Option<bool>,
        enabled: Option<bool>,
        metadata: Option<&str>,
    );

    /// Reconcile the in-memory view after a snapshot install replaced the
    /// whole state machine.
    ///
    /// `instances` is the authoritative full set read back from
    /// `CF_INSTANCES` right after the install. Implementations must make
    /// their in-memory view match it: register new entries, update changed
    /// ones, and **remove** any persistent instance the snapshot does not
    /// contain. A snapshot can delete state as well as add it, which is why
    /// the incremental `on_deregister` alone cannot cover this path -- a
    /// purely additive replay leaves instances the rest of the cluster
    /// already deleted still being served by this node.
    ///
    /// Mirrors Nacos' `PersistentInstanceSnapshotOperation.loadSnapshot()`,
    /// which updates existing clients, adds new ones, and then calls
    /// `removeDeadClient()` for everything the snapshot omits.
    ///
    /// Defaults to a no-op so hook implementations that keep no derived
    /// in-memory state are unaffected.
    fn on_snapshot_loaded(&self, _instances: Vec<StoredInstance>) {}
}

/// Shared slot holding the optional hook. Registration happens after both
/// `RaftNode` and `NamingService` are constructed (same pattern as
/// `PluginRegistry`).
pub type SharedNamingHook = Arc<tokio::sync::RwLock<Option<Arc<dyn NamingApplyHook>>>>;

/// Create a new, initially-empty shared naming apply hook slot.
pub fn new_shared_naming_hook() -> SharedNamingHook {
    Arc::new(tokio::sync::RwLock::new(None))
}
