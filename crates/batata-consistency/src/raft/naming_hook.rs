//! Apply-back hook for persistent naming instances.
//!
//! When the Raft state machine applies a `PersistentInstance*` variant, it
//! writes to RocksDB `CF_INSTANCES` and then invokes the registered hook to
//! flow the change into the in-memory `NamingService` DashMap so reads see
//! it without restart. The hook lives in `batata-consistency` to avoid a
//! reverse dependency on `batata-naming`.

use std::sync::Arc;

/// Callback invoked by `RocksStateMachine` after a persistent instance
/// apply succeeds. Implementations must be idempotent — the state machine
/// may replay an apply during snapshot install or cold-start recovery.
pub trait NamingApplyHook: Send + Sync {
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

    fn on_deregister(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        instance_id: &str,
    );

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
}

/// Shared slot holding the optional hook. Registration happens after both
/// `RaftNode` and `NamingService` are constructed (same pattern as
/// `PluginRegistry`).
pub type SharedNamingHook = Arc<tokio::sync::RwLock<Option<Arc<dyn NamingApplyHook>>>>;

pub fn new_shared_naming_hook() -> SharedNamingHook {
    Arc::new(tokio::sync::RwLock::new(None))
}
