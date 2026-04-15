//! Apply-back hook for replicated health-check status updates.
//!
//! When the Raft state machine applies a `HealthCheckStatusUpdate` or
//! `HealthCheckTtlUpdate` entry, it invokes the registered hook so every
//! cluster node updates its in-memory `InstanceCheckRegistry` consistently.
//!
//! The hook lives in `batata-consistency` to avoid a reverse dependency on
//! `batata-naming`. Mirror of `naming_hook` for naming instances.

use std::sync::Arc;

/// Callback invoked by `RocksStateMachine` after a health-check status
/// update is committed through Raft. Implementations must be idempotent —
/// the state machine may replay an apply during snapshot install or
/// cold-start recovery.
pub trait HealthCheckApplyHook: Send + Sync {
    /// Active probe result with success/failure threshold semantics.
    /// `status` is one of `"passing" | "warning" | "critical"`.
    fn on_status_update(
        &self,
        check_key: &str,
        success: bool,
        output: &str,
        response_time_ms: u64,
        timestamp_ms: i64,
    );

    /// TTL-style update — no consecutive-success/failure thresholding.
    /// Used by Consul's session-bound checks (`/v1/agent/check/pass|fail|warn`).
    fn on_ttl_update(
        &self,
        check_key: &str,
        status: &str,
        output: Option<&str>,
        timestamp_ms: i64,
    );
}

/// Shared slot holding the optional hook. Registration happens after both
/// `RaftNode` and `InstanceCheckRegistry` are constructed.
pub type SharedHealthCheckHook =
    Arc<tokio::sync::RwLock<Option<Arc<dyn HealthCheckApplyHook>>>>;

pub fn new_shared_health_check_hook() -> SharedHealthCheckHook {
    Arc::new(tokio::sync::RwLock::new(None))
}
