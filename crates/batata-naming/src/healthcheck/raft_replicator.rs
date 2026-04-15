//! Glue between `InstanceCheckRegistry` and `RaftNode` for cluster-mode
//! status replication. See `project_health_status_raft_sync_plan.md`.

use std::sync::Arc;

use async_trait::async_trait;
use batata_consistency::raft::health_check_hook::HealthCheckApplyHook;
use batata_consistency::raft::request::RaftRequest;
use batata_consistency::RaftNode;
use tracing::warn;

use super::registry::{CheckStatus, HealthStatusReplicator, InstanceCheckRegistry};

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn status_label(success: bool) -> &'static str {
    if success { "passing" } else { "critical" }
}

/// Cluster Raft replicator.
///
/// Every call awaits a Raft write. `RaftNode::write` transparently forwards
/// to the current leader if this node is a follower and blocks until the
/// local state machine has applied the entry, giving us read-your-write
/// consistency on the same node that received an HTTP TTL update.
///
/// Because every node's reactor runs the same probe loop, the leader
/// receives N copies of the same status proposal per probe interval.
/// That's redundant but cheap (Raft de-duplicates via apply order) and
/// correct — leader-only-write would require a responsibility gate which
/// is the subject of a follow-up; doing it without the gate would mean
/// dropping non-leader probes on the floor and losing read-your-write on
/// HTTP requests that hit followers.
pub struct RaftHealthReplicator {
    raft: Arc<RaftNode>,
}

impl RaftHealthReplicator {
    pub fn new(raft: Arc<RaftNode>) -> Self {
        Self { raft }
    }
}

#[async_trait]
impl HealthStatusReplicator for RaftHealthReplicator {
    async fn replicate_status(
        &self,
        check_key: &str,
        success: bool,
        output: &str,
        response_time_ms: u64,
    ) -> bool {
        let req = RaftRequest::HealthCheckStatusUpdate {
            check_key: check_key.to_string(),
            status: status_label(success).to_string(),
            output: output.to_string(),
            response_time_ms,
            timestamp_ms: now_ms(),
        };
        if let Err(e) = self.raft.write(req).await {
            warn!(
                "Raft write for HealthCheckStatusUpdate({}) failed: {}",
                check_key, e
            );
            // Failed to replicate — return false so the registry falls
            // through to a local apply; this preserves status updates on
            // this node even when Raft is temporarily unavailable. Other
            // nodes will diverge briefly until the next successful
            // replication, which is a softer failure mode than dropping
            // the update entirely.
            return false;
        }
        true
    }

    async fn replicate_ttl(
        &self,
        check_key: &str,
        status: &str,
        output: Option<&str>,
    ) -> bool {
        let req = RaftRequest::HealthCheckTtlUpdate {
            check_key: check_key.to_string(),
            status: status.to_string(),
            output: output.map(|s| s.to_string()),
            timestamp_ms: now_ms(),
        };
        if let Err(e) = self.raft.write(req).await {
            warn!(
                "Raft write for HealthCheckTtlUpdate({}) failed: {}",
                check_key, e
            );
            return false;
        }
        true
    }
}

/// Apply-back hook: the state machine calls this on every node after a
/// `HealthCheckStatusUpdate` / `HealthCheckTtlUpdate` Raft entry commits.
/// We delegate to the registry's `*_local` paths so leader and followers
/// converge through the exact same code path that standalone uses.
pub struct RegistryApplyHook {
    registry: Arc<InstanceCheckRegistry>,
}

impl RegistryApplyHook {
    pub fn new(registry: Arc<InstanceCheckRegistry>) -> Self {
        Self { registry }
    }
}

impl HealthCheckApplyHook for RegistryApplyHook {
    fn on_status_update(
        &self,
        check_key: &str,
        success: bool,
        output: &str,
        response_time_ms: u64,
        _timestamp_ms: i64,
    ) {
        self.registry
            .apply_status_local(check_key, success, output.to_string(), response_time_ms);
    }

    fn on_ttl_update(
        &self,
        check_key: &str,
        status: &str,
        output: Option<&str>,
        _timestamp_ms: i64,
    ) {
        let parsed = match status {
            "passing" => CheckStatus::Passing,
            "warning" => CheckStatus::Warning,
            _ => CheckStatus::Critical,
        };
        self.registry
            .apply_ttl_local(check_key, parsed, output.map(|s| s.to_string()));
    }
}
