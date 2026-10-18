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

/// Shared slot holding the optional hook. Registration happens after both
/// `RaftNode` and `InstanceCheckRegistry` are constructed.
///
/// The slot is **single-owner**: the Raft state machine fans a committed
/// health-check update out to exactly one hook. That is sufficient today
/// because only one facade writes through this channel — the Consul plugin.
/// The Nacos facade keeps persistent-instance health on the
/// `PersistentInstanceUpdate` path instead of this one.
///
/// If a second owner is ever added (Nacos included), this must become a
/// multicast list or a key-prefix dispatcher first: installing a second hook
/// here would silently swallow the first facade's updates.
pub type SharedHealthCheckHook = Arc<tokio::sync::RwLock<Option<Arc<dyn HealthCheckApplyHook>>>>;

/// Create a new, initially-empty shared health-check apply hook slot.
pub fn new_shared_health_check_hook() -> SharedHealthCheckHook {
    Arc::new(tokio::sync::RwLock::new(None))
}

/// Install `hook` into `slot`, reporting whether it displaced a previous one.
///
/// Replacement is allowed (a facade may legitimately re-register after a
/// restart) but is never silent: losing a hook means committed health-check
/// updates stop reaching that facade's registry, which surfaces much later as
/// stale health state rather than as an error.
///
/// Returns `true` when an existing hook was displaced.
pub async fn install_health_check_hook(
    slot: &SharedHealthCheckHook,
    hook: Arc<dyn HealthCheckApplyHook>,
) -> bool {
    let displaced = slot.write().await.replace(hook).is_some();
    if displaced {
        tracing::warn!(
            "health-check apply hook replaced: this slot supports a single owner, \
             so the previously registered facade will no longer receive committed \
             health-check updates"
        );
    }
    displaced
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoopHook;

    impl HealthCheckApplyHook for NoopHook {
        fn on_status_update(
            &self,
            _check_key: &str,
            _success: bool,
            _output: &str,
            _response_time_ms: u64,
            _timestamp_ms: i64,
        ) {
        }

        fn on_ttl_update(
            &self,
            _check_key: &str,
            _status: &str,
            _output: Option<&str>,
            _timestamp_ms: i64,
        ) {
        }
    }

    #[tokio::test]
    async fn install_reports_whether_it_displaced_a_previous_hook() {
        let slot = new_shared_health_check_hook();

        // First install fills an empty slot; the second one displaces it and
        // must report that so the caller can surface the loss.
        assert!(!install_health_check_hook(&slot, Arc::new(NoopHook)).await);
        assert!(install_health_check_hook(&slot, Arc::new(NoopHook)).await);
        assert!(slot.read().await.is_some());
    }
}
