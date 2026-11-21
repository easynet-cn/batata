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
    fn on_ttl_update(&self, check_key: &str, status: &str, output: Option<&str>, timestamp_ms: i64);
}

/// Legacy standalone hook slot.
///
/// Production no longer uses this type: all apply-time handles live in
/// [`super::hooks::HookSet`] behind a single lock, and installation goes
/// through [`super::hooks::install_health_check_hook`]. The alias is kept for
/// the historical micro-benchmark, which reproduces the pre-P1 per-slot shape.
///
/// The slot was **single-owner**: the Raft state machine fans a committed
/// health-check update out to exactly one hook. That is still true today
/// because only one facade writes through this channel — the Consul plugin.
/// If a second owner is ever added (Nacos included), the hook set must become
/// a multicast list or a key-prefix dispatcher first.
pub type SharedHealthCheckHook = Arc<tokio::sync::RwLock<Option<Arc<dyn HealthCheckApplyHook>>>>;

/// Create a new, initially-empty legacy hook slot (bench/test helper).
pub fn new_shared_health_check_hook() -> SharedHealthCheckHook {
    Arc::new(tokio::sync::RwLock::new(None))
}
