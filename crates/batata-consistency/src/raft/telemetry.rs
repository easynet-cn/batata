//! Raft health telemetry.
//!
//! The consensus layer used to have no metrics of its own: diagnosing a
//! spurious election meant grepping free-form log lines spread over several
//! files. This module turns the openraft `RaftMetrics` watcher into a small
//! state machine (`RaftHealthTracker`) that emits Prometheus gauges/counters/
//! histograms and structured `batata_raft_health` log events.
//!
//! The tracker is deliberately pure (no clock, no I/O): `observe` takes the
//! current snapshot and a caller-supplied millisecond clock, so every
//! transition is unit-testable. Emission is a thin separate layer.

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use metrics::{counter, gauge, histogram};
use tracing::{info, warn};

use super::types::{RaftMetrics, ServerState};

/// Target used for every structured health event. The regression gate greps
/// this target's `raft_event=` fields; keep the field names stable.
pub const HEALTH_LOG_TARGET: &str = "batata_raft_health";

/// CPU utilisation at or above which a Raft anomaly is additionally flagged
/// as resource pressure. Advisory only: it never changes Raft behaviour or
/// timeouts, and it never turns a healthy run into a gate failure.
pub const HIGH_CPU_PCT: f64 = 85.0;

/// Sentinel used when a resource value was never sampled. Keep it numeric so
/// the structured log stays machine-parseable.
pub const RESOURCE_UNKNOWN: f64 = -1.0;

/// Latest process resource reading, in the units used by structured logs.
///
/// The consensus crate deliberately does not depend on `sysinfo`: the server
/// owns sampling and installs a provider. `None` means "no sample available"
/// (e.g. the system-stats reporter is disabled), which is distinct from a
/// genuine 0% reading.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResourceSnapshot {
    /// System-wide CPU utilisation in percent, `0.0..=100.0` (averaged across
    /// cores by the server's reporter, not per-thread or per-process).
    pub cpu_pct: Option<f64>,
    /// Resident memory of this process in MiB.
    pub process_memory_mb: Option<f64>,
}

impl ResourceSnapshot {
    /// Snapshot to use when no sampler is installed.
    pub const UNKNOWN: Self = Self {
        cpu_pct: None,
        process_memory_mb: None,
    };

    /// Whether the sampled CPU is at or above [`HIGH_CPU_PCT`].
    pub fn high_cpu(&self) -> bool {
        self.cpu_pct.is_some_and(|cpu| cpu >= HIGH_CPU_PCT)
    }

    /// CPU value for structured logging; [`RESOURCE_UNKNOWN`] when absent.
    pub fn log_cpu_pct(&self) -> f64 {
        self.cpu_pct.unwrap_or(RESOURCE_UNKNOWN)
    }

    /// Memory value for structured logging; [`RESOURCE_UNKNOWN`] when absent.
    pub fn log_memory_mb(&self) -> f64 {
        self.process_memory_mb.unwrap_or(RESOURCE_UNKNOWN)
    }
}

/// A cheap sampler for the most recent resource reading. It must not perform
/// I/O: anomaly logging happens on Raft write paths and on the metrics watcher.
pub type ResourceSnapshotProvider = Arc<dyn Fn() -> ResourceSnapshot + Send + Sync>;

static RESOURCE_PROVIDER: OnceLock<ResourceSnapshotProvider> = OnceLock::new();

/// Install the process-wide resource sampler.
///
/// Returns `false` if one was already installed; the first provider wins so a
/// duplicate startup cannot silently replace the source of truth. Called by
/// `batata-server` at metrics initialisation.
pub fn set_resource_snapshot_provider(provider: ResourceSnapshotProvider) -> bool {
    RESOURCE_PROVIDER.set(provider).is_ok()
}

/// Most recent resource sample, or [`ResourceSnapshot::UNKNOWN`].
pub fn resource_snapshot() -> ResourceSnapshot {
    RESOURCE_PROVIDER
        .get()
        .map(|provider| provider())
        .unwrap_or(ResourceSnapshot::UNKNOWN)
}

/// Resource context attached to every Raft anomaly WARN event.
///
/// Built once at the moment of the anomaly so the log line and the exported
/// metrics describe the same sample. Keeping this as a plain value makes the
/// correlation logic unit-testable without a tracing subscriber.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnomalyContext {
    /// Stable `raft_event` name of the anomaly.
    pub event: &'static str,
    /// Sampled CPU percent, or [`RESOURCE_UNKNOWN`].
    pub cpu_pct: f64,
    /// Sampled process memory in MiB, or [`RESOURCE_UNKNOWN`].
    pub process_memory_mb: f64,
    /// Whether the anomaly coincided with high CPU.
    pub resource_pressure: bool,
}

impl AnomalyContext {
    /// Capture the current resource sample for one anomaly event.
    pub fn for_event(event: &'static str) -> Self {
        let resources = resource_snapshot();
        Self {
            event,
            cpu_pct: resources.log_cpu_pct(),
            process_memory_mb: resources.log_memory_mb(),
            resource_pressure: resources.high_cpu(),
        }
    }

    /// Whether a real resource sample was available for this anomaly.
    pub fn sampled(&self) -> bool {
        self.cpu_pct != RESOURCE_UNKNOWN || self.process_memory_mb != RESOURCE_UNKNOWN
    }
}

/// Publish the anomaly/resource correlation metrics under the Raft namespace.
///
/// These make a scrape self-contained: the same `event` label lets a dashboard
/// join an anomaly counter with the CPU and memory observed at that instant,
/// instead of requiring a human to align two time series by hand.
pub fn record_anomaly_context(context: &AnomalyContext) {
    let event = context.event.to_string();
    gauge!("batata_raft_anomaly_cpu_percent", "event" => event.clone()).set(context.cpu_pct);
    gauge!("batata_raft_anomaly_process_memory_mb", "event" => event.clone())
        .set(context.process_memory_mb);
    counter!("batata_raft_anomalies_with_resource_context_total", "event" => event.clone())
        .increment(1);
    if context.resource_pressure {
        counter!("batata_raft_high_cpu_anomalies_total", "event" => event).increment(1);
    }
}

/// Emit the extra structured alert when an anomaly coincided with high CPU.
///
/// `raft_event=resource_pressure` is intentionally NOT one of the four gate
/// blockers: it is diagnostic context for an already-reported anomaly, not a
/// new failure mode. The health checker only counts the four anomaly names.
pub fn emit_resource_pressure(context: &AnomalyContext) {
    if !context.resource_pressure {
        return;
    }
    warn!(
        target: HEALTH_LOG_TARGET,
        raft_event = "resource_pressure",
        anomaly_event = context.event,
        cpu_pct = context.cpu_pct,
        process_memory_mb = context.process_memory_mb,
        "Raft anomaly coincided with high system CPU; check runtime starvation before adjusting timeouts"
    );
}

/// Raft role as reported by openraft, reduced to what health checks need.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RaftRole {
    /// This node is the cluster leader.
    Leader,
    /// This node is campaigning for leadership.
    Candidate,
    /// This node is a voting member following the leader.
    Follower,
    /// This node is a non-voting member (or not yet initialized).
    Learner,
    /// openraft has shut this node down.
    Shutdown,
}

impl RaftRole {
    /// Numeric role for the `batata_raft_role` gauge.
    pub fn code(self) -> f64 {
        match self {
            Self::Learner => 0.0,
            Self::Follower => 1.0,
            Self::Candidate => 2.0,
            Self::Leader => 3.0,
            Self::Shutdown => -1.0,
        }
    }

    /// Stable string used in log fields and gate assertions.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Leader => "leader",
            Self::Candidate => "candidate",
            Self::Follower => "follower",
            Self::Learner => "learner",
            Self::Shutdown => "shutdown",
        }
    }
}

impl From<&ServerState> for RaftRole {
    fn from(state: &ServerState) -> Self {
        match state {
            ServerState::Leader => Self::Leader,
            ServerState::Candidate => Self::Candidate,
            ServerState::Follower => Self::Follower,
            ServerState::Learner => Self::Learner,
            ServerState::Shutdown => Self::Shutdown,
        }
    }
}

/// Point-in-time view of the Raft core, extracted from openraft metrics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RaftHealthSnapshot {
    /// Current Raft term.
    pub term: u64,
    /// Current role of this node.
    pub role: RaftRole,
    /// Leader known to this node, if any.
    pub leader_id: Option<u64>,
    /// Last index appended to this node's log.
    pub last_log_index: u64,
    /// Last index applied to this node's state machine.
    pub last_applied_index: u64,
    /// Last index this node knows to be committed locally (safe to apply).
    ///
    /// openraft 0.10 reports it as `local_committed`; on a follower it can
    /// trail the quorum-granted frontier while AppendEntries is in flight.
    pub local_committed_index: u64,
    /// Last index committed by a quorum, as last reported by the leader.
    ///
    /// This is the only cluster-wide commit view available on a follower;
    /// `last_log_index` and `last_applied_index` are strictly local.
    pub cluster_committed_index: u64,
}

impl RaftHealthSnapshot {
    /// How far the local state machine lags behind the local log. A steady
    /// cluster should stay at 0-1; sustained growth points at a blocked FSM.
    pub fn apply_lag(&self) -> u64 {
        self.last_log_index.saturating_sub(self.last_applied_index)
    }

    /// How far this node's local commit frontier trails the cluster's
    /// quorum-granted commit frontier.
    ///
    /// Unlike `apply_lag`, this can detect replication lag on a follower even
    /// when its state machine has applied everything it received: the entries
    /// themselves have not arrived yet. Saturating because a leader change can
    /// make `cluster_committed` move backwards on a follower (openraft
    /// documents this explicitly).
    pub fn commit_lag(&self) -> u64 {
        self.cluster_committed_index
            .saturating_sub(self.local_committed_index)
    }
}

impl From<&RaftMetrics> for RaftHealthSnapshot {
    fn from(metrics: &RaftMetrics) -> Self {
        Self {
            term: metrics.current_term,
            role: RaftRole::from(&metrics.state),
            leader_id: metrics.current_leader,
            last_log_index: metrics.last_log_index.unwrap_or(0),
            last_applied_index: metrics
                .last_applied
                .as_ref()
                .map(|log_id| log_id.index)
                .unwrap_or(0),
            local_committed_index: metrics
                .local_committed
                .as_ref()
                .map(|log_id| log_id.index)
                .unwrap_or(0),
            cluster_committed_index: metrics
                .cluster_committed
                .as_ref()
                .map(|log_id| log_id.index)
                .unwrap_or(0),
        }
    }
}

/// A transition worth recording as a counter and/or a structured log line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RaftHealthEvent {
    /// Node entered Candidate. `steady_state` is true when a leader had
    /// already been observed, i.e. this is a spurious election.
    ElectionStarted {
        /// Term in which the election started.
        term: u64,
        /// Role the node left to become a candidate.
        from_role: RaftRole,
        /// Whether a leader had already been observed (spurious election).
        steady_state: bool,
    },
    /// Node left Candidate. `elapsed_ms` is the observed election duration.
    ElectionSettled {
        /// Term in which the election settled.
        term: u64,
        /// Role the node settled into.
        role: RaftRole,
        /// Observed election duration in milliseconds.
        elapsed_ms: u64,
    },
    /// The known leader id changed.
    LeaderChanged {
        /// Previously known leader, if any.
        from: Option<u64>,
        /// Newly known leader, if any.
        to: Option<u64>,
        /// Term in which the change was observed.
        term: u64,
    },
    /// The local term changed.
    TermChanged {
        /// Previous term.
        from: u64,
        /// New term.
        to: u64,
    },
}

/// Detects elections, leader/term changes and apply lag from a stream of
/// snapshots. The first election before any leader is known is the cold-start
/// election and is not counted as a steady-state anomaly -- Consul's health
/// criterion is "candidate == 0 while a leader exists", not "elections == 0".
#[derive(Debug, Default)]
pub struct RaftHealthTracker {
    last: Option<RaftHealthSnapshot>,
    election_since_ms: Option<u64>,
    seen_leader: bool,
    /// Elections that happened after at least one leader had been observed.
    pub steady_state_elections: u64,
    /// All elections, including the cold-start one.
    pub elections: u64,
}

impl RaftHealthTracker {
    /// Consume one snapshot and return the transitions it caused.
    ///
    /// `now_ms` is a caller-supplied monotonic millisecond clock; keeping it
    /// out of the tracker makes election-duration assertions deterministic.
    pub fn observe(&mut self, snapshot: RaftHealthSnapshot, now_ms: u64) -> Vec<RaftHealthEvent> {
        let mut events = Vec::new();
        match self.last {
            Some(previous) => {
                if previous.term != snapshot.term {
                    events.push(RaftHealthEvent::TermChanged {
                        from: previous.term,
                        to: snapshot.term,
                    });
                }
                if previous.leader_id != snapshot.leader_id {
                    events.push(RaftHealthEvent::LeaderChanged {
                        from: previous.leader_id,
                        to: snapshot.leader_id,
                        term: snapshot.term,
                    });
                }
                if previous.role != RaftRole::Candidate && snapshot.role == RaftRole::Candidate {
                    self.elections += 1;
                    let steady_state = self.seen_leader;
                    if steady_state {
                        self.steady_state_elections += 1;
                    }
                    self.election_since_ms = Some(now_ms);
                    events.push(RaftHealthEvent::ElectionStarted {
                        term: snapshot.term,
                        from_role: previous.role,
                        steady_state,
                    });
                }
                if previous.role == RaftRole::Candidate && snapshot.role != RaftRole::Candidate {
                    let elapsed_ms =
                        now_ms.saturating_sub(self.election_since_ms.take().unwrap_or(now_ms));
                    events.push(RaftHealthEvent::ElectionSettled {
                        term: snapshot.term,
                        role: snapshot.role,
                        elapsed_ms,
                    });
                }
            }
            None => {
                // The node may already be campaigning when telemetry starts.
                if snapshot.role == RaftRole::Candidate {
                    self.elections = 1;
                    self.election_since_ms = Some(now_ms);
                    events.push(RaftHealthEvent::ElectionStarted {
                        term: snapshot.term,
                        from_role: RaftRole::Learner,
                        steady_state: false,
                    });
                }
            }
        }
        self.seen_leader |= snapshot.leader_id.is_some();
        self.last = Some(snapshot);
        events
    }
}

/// Publish the gauges scraped by `/prometheus`. Names mirror Consul telemetry
/// (`consul.raft.state.*`, `consul.raft.last_index`, `consul.server.isLeader`)
/// with a `batata_` prefix so they cannot collide with the console's own
/// metric stack.
pub fn record_health(snapshot: &RaftHealthSnapshot, steady_state_elections: u64) {
    gauge!("batata_raft_term").set(snapshot.term as f64);
    gauge!("batata_raft_role").set(snapshot.role.code());
    gauge!("batata_raft_leader_id").set(snapshot.leader_id.map(|id| id as f64).unwrap_or(-1.0));
    gauge!("batata_raft_last_log_index").set(snapshot.last_log_index as f64);
    gauge!("batata_raft_last_applied_index").set(snapshot.last_applied_index as f64);
    gauge!("batata_raft_apply_lag").set(snapshot.apply_lag() as f64);
    gauge!("batata_raft_local_committed_index").set(snapshot.local_committed_index as f64);
    gauge!("batata_raft_cluster_committed_index").set(snapshot.cluster_committed_index as f64);
    gauge!("batata_raft_commit_lag").set(snapshot.commit_lag() as f64);
    gauge!("batata_raft_steady_state_elections").set(steady_state_elections as f64);

    // Publish the latest resource sample alongside the Raft gauges so one
    // scrape contains both sides of the correlation. This reuses the server's
    // existing periodic sampler; no second sysinfo stack is introduced.
    let resources = resource_snapshot();
    if let Some(cpu_pct) = resources.cpu_pct {
        gauge!("batata_raft_resource_cpu_percent").set(cpu_pct);
    }
    if let Some(memory_mb) = resources.process_memory_mb {
        gauge!("batata_raft_resource_process_memory_mb").set(memory_mb);
    }
}

/// Emit one transition. Anomalies are WARN-level with a stable
/// `raft_event=` field so the regression gate can turn them into blockers.
pub fn emit_event(node_id: u64, event: &RaftHealthEvent) {
    match event {
        RaftHealthEvent::ElectionStarted {
            term,
            from_role,
            steady_state,
        } => {
            counter!("batata_raft_elections_total").increment(1);
            if *steady_state {
                counter!("batata_raft_steady_state_elections_total").increment(1);
                let context = AnomalyContext::for_event("steady_state_election");
                record_anomaly_context(&context);
                warn!(
                    target: HEALTH_LOG_TARGET,
                    raft_event = context.event,
                    node_id = node_id,
                    term = *term,
                    from_role = from_role.as_str(),
                    cpu_pct = context.cpu_pct,
                    process_memory_mb = context.process_memory_mb,
                    resource_pressure = context.resource_pressure,
                    "unexpected Raft election after a leader was already known"
                );
                emit_resource_pressure(&context);
            } else {
                info!(
                    target: HEALTH_LOG_TARGET,
                    raft_event = "cold_start_election",
                    node_id = node_id,
                    term = *term,
                    from_role = from_role.as_str(),
                    "cold-start Raft election"
                );
            }
        }
        RaftHealthEvent::ElectionSettled {
            term,
            role,
            elapsed_ms,
        } => {
            histogram!("batata_raft_election_duration_seconds")
                .record(Duration::from_millis(*elapsed_ms).as_secs_f64());
            info!(
                target: HEALTH_LOG_TARGET,
                raft_event = "election_settled",
                node_id = node_id,
                term = *term,
                role = role.as_str(),
                elapsed_ms = *elapsed_ms,
                "Raft election settled"
            );
        }
        RaftHealthEvent::LeaderChanged { from, to, term } => {
            counter!("batata_raft_leader_changes_total").increment(1);
            info!(
                target: HEALTH_LOG_TARGET,
                raft_event = "leader_changed",
                node_id = node_id,
                term = *term,
                from_leader_id = from.map(|id| id as i64).unwrap_or(-1),
                to_leader_id = to.map(|id| id as i64).unwrap_or(-1),
                "Raft leader changed"
            );
        }
        RaftHealthEvent::TermChanged { from, to } => {
            counter!("batata_raft_term_changes_total").increment(1);
            info!(
                target: HEALTH_LOG_TARGET,
                raft_event = "term_changed",
                node_id = node_id,
                from_term = *from,
                to_term = *to,
                "Raft term changed"
            );
        }
    }
}

/// Record a bounded local `client_write` result.
pub fn record_client_write(elapsed: Duration, timed_out: bool) {
    histogram!("batata_raft_client_write_duration_seconds").record(elapsed.as_secs_f64());
    if timed_out {
        counter!("batata_raft_client_write_timeouts_total").increment(1);
        let context = AnomalyContext::for_event("client_write_timeout");
        record_anomaly_context(&context);
        warn!(
            target: HEALTH_LOG_TARGET,
            raft_event = context.event,
            elapsed_ms = elapsed.as_millis() as u64,
            cpu_pct = context.cpu_pct,
            process_memory_mb = context.process_memory_mb,
            resource_pressure = context.resource_pressure,
            "local client_write exceeded its derived deadline"
        );
        emit_resource_pressure(&context);
    }
}

/// Record a follower's wait for its state machine to catch up.
pub fn record_apply_wait(elapsed: Duration, timed_out: bool) {
    histogram!("batata_raft_apply_wait_duration_seconds").record(elapsed.as_secs_f64());
    if timed_out {
        counter!("batata_raft_apply_wait_timeouts_total").increment(1);
        let context = AnomalyContext::for_event("apply_wait_timeout");
        record_anomaly_context(&context);
        warn!(
            target: HEALTH_LOG_TARGET,
            raft_event = context.event,
            elapsed_ms = elapsed.as_millis() as u64,
            cpu_pct = context.cpu_pct,
            process_memory_mb = context.process_memory_mb,
            resource_pressure = context.resource_pressure,
            "follower did not apply the forwarded write within its deadline"
        );
        emit_resource_pressure(&context);
    }
}

/// Record a follower -> leader forwarding attempt and its latency.
pub fn record_forward(elapsed: Duration, succeeded: bool) {
    histogram!("batata_raft_forward_duration_seconds").record(elapsed.as_secs_f64());
    counter!("batata_raft_forwarded_writes_total").increment(1);
    if !succeeded {
        counter!("batata_raft_forward_failures_total").increment(1);
    }
}

/// Record a write that exhausted leader discovery without a leader.
pub fn record_leaderless_give_up(waited: Duration) {
    counter!("batata_raft_leaderless_give_ups_total").increment(1);
    let context = AnomalyContext::for_event("leaderless_give_up");
    record_anomaly_context(&context);
    warn!(
        target: HEALTH_LOG_TARGET,
        raft_event = context.event,
        waited_ms = waited.as_millis() as u64,
        cpu_pct = context.cpu_pct,
        process_memory_mb = context.process_memory_mb,
        resource_pressure = context.resource_pressure,
        "no Raft leader became available within the derived election cycle"
    );
    emit_resource_pressure(&context);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(term: u64, role: RaftRole, leader: Option<u64>) -> RaftHealthSnapshot {
        RaftHealthSnapshot {
            term,
            role,
            leader_id: leader,
            last_log_index: 10,
            last_applied_index: 9,
            local_committed_index: 9,
            cluster_committed_index: 10,
        }
    }

    #[test]
    fn apply_lag_is_saturating() {
        let mut s = snap(1, RaftRole::Leader, Some(1));
        assert_eq!(s.apply_lag(), 1);
        s.last_applied_index = 20;
        assert_eq!(s.apply_lag(), 0);
    }

    /// `commit_lag` is the cluster-vs-local commit gap, and must clamp to zero
    /// when a new leader reports a lower cluster frontier on a follower.
    #[test]
    fn commit_lag_tracks_cluster_frontier_and_saturates() {
        let mut s = snap(1, RaftRole::Follower, Some(1));
        assert_eq!(s.commit_lag(), 1);
        s.local_committed_index = 10;
        assert_eq!(s.commit_lag(), 0, "a caught-up follower has no commit lag");
        s.cluster_committed_index = 5;
        assert_eq!(
            s.commit_lag(),
            0,
            "cluster_committed may move backwards after a leader change"
        );
        s.local_committed_index = 0;
        assert_eq!(s.commit_lag(), 5);
    }

    /// The snapshot must read the two *different* openraft 0.10 frontiers.
    /// Swapping `cluster_committed` for `local_committed` makes this fail.
    #[test]
    fn snapshot_reads_both_commit_frontiers_from_openraft_metrics() {
        use crate::raft::types::{CommittedLeaderId, LogId};

        let log_id = |index| {
            LogId::new(
                CommittedLeaderId {
                    term: 3,
                    node_id: 1,
                },
                index,
            )
        };
        let mut m = RaftMetrics::new_initial(1u64);
        m.current_term = 3;
        m.last_log_index = Some(20);
        m.local_committed = Some(log_id(15));
        m.cluster_committed = Some(log_id(18));
        m.last_applied = Some(log_id(14));

        let s = RaftHealthSnapshot::from(&m);
        assert_eq!(s.local_committed_index, 15);
        assert_eq!(s.cluster_committed_index, 18);
        assert_eq!(s.commit_lag(), 3);
        assert_eq!(s.apply_lag(), 6);

        // A freshly initialised node has no commit frontier yet.
        let empty = RaftHealthSnapshot::from(&RaftMetrics::new_initial(1u64));
        assert_eq!(empty.local_committed_index, 0);
        assert_eq!(empty.cluster_committed_index, 0);
        assert_eq!(empty.commit_lag(), 0);
    }

    /// Cold start: Learner -> Candidate -> Leader with no known leader is one
    /// normal election and must NOT be flagged as steady-state.
    #[test]
    fn cold_start_election_is_not_a_steady_state_anomaly() {
        let mut tracker = RaftHealthTracker::default();
        let events = tracker.observe(snap(1, RaftRole::Learner, None), 0);
        assert!(events.is_empty());
        let events = tracker.observe(snap(2, RaftRole::Candidate, None), 100);
        assert!(events.iter().any(|e| matches!(
            e,
            RaftHealthEvent::ElectionStarted {
                steady_state: false,
                ..
            }
        )));
        let events = tracker.observe(snap(2, RaftRole::Leader, Some(1)), 1300);
        assert!(events.iter().any(|e| matches!(
            e,
            RaftHealthEvent::ElectionSettled {
                elapsed_ms: 1200,
                ..
            }
        )));
        assert_eq!(tracker.elections, 1);
        assert_eq!(tracker.steady_state_elections, 0);
    }

    /// Once a leader has been seen, any new Candidate state is a spurious
    /// election. This is the exact predicate the regression gate asserts.
    #[test]
    fn election_after_a_known_leader_is_flagged() {
        let mut tracker = RaftHealthTracker::default();
        tracker.observe(snap(1, RaftRole::Follower, Some(1)), 0);
        let events = tracker.observe(snap(2, RaftRole::Candidate, None), 500);
        assert!(events.iter().any(|e| matches!(
            e,
            RaftHealthEvent::ElectionStarted {
                steady_state: true,
                ..
            }
        )));
        assert_eq!(tracker.steady_state_elections, 1);
        // A second election in the same run must also be counted.
        tracker.observe(snap(2, RaftRole::Follower, Some(2)), 900);
        tracker.observe(snap(3, RaftRole::Candidate, None), 1000);
        assert_eq!(tracker.steady_state_elections, 2);
    }

    /// A node that starts in Candidate (telemetry launched mid-election) still
    /// records the cold-start election exactly once.
    #[test]
    fn first_observation_during_an_election_counts_once() {
        let mut tracker = RaftHealthTracker::default();
        let events = tracker.observe(snap(1, RaftRole::Candidate, None), 0);
        assert_eq!(events.len(), 1);
        assert_eq!(tracker.elections, 1);
        assert_eq!(tracker.steady_state_elections, 0);
    }

    #[test]
    fn leader_and_term_changes_are_emitted_only_on_change() {
        let mut tracker = RaftHealthTracker::default();
        tracker.observe(snap(1, RaftRole::Follower, Some(1)), 0);
        let events = tracker.observe(snap(2, RaftRole::Follower, Some(2)), 10);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, RaftHealthEvent::TermChanged { from: 1, to: 2 }))
        );
        assert!(events.iter().any(|e| matches!(
            e,
            RaftHealthEvent::LeaderChanged {
                from: Some(1),
                to: Some(2),
                ..
            }
        )));
        let events = tracker.observe(snap(2, RaftRole::Follower, Some(2)), 20);
        assert!(events.is_empty(), "{events:?}");
    }

    /// An election is one transition into Candidate, not one sample. Polling
    /// several times while campaigning must not inflate the counter.
    #[test]
    fn repeated_candidate_samples_count_as_one_election() {
        let mut tracker = RaftHealthTracker::default();
        tracker.observe(snap(1, RaftRole::Follower, Some(1)), 0);
        tracker.observe(snap(2, RaftRole::Candidate, None), 100);
        let events = tracker.observe(snap(2, RaftRole::Candidate, None), 200);
        assert!(events.is_empty(), "{events:?}");
        assert_eq!(tracker.elections, 1);
        assert_eq!(tracker.steady_state_elections, 1);
    }

    #[test]
    fn role_codes_are_distinct_and_learner_is_zero() {
        assert_eq!(RaftRole::Learner.code(), 0.0);
        let codes = [
            RaftRole::Leader.code(),
            RaftRole::Candidate.code(),
            RaftRole::Follower.code(),
            RaftRole::Learner.code(),
            RaftRole::Shutdown.code(),
        ];
        for (i, a) in codes.iter().enumerate() {
            for b in codes.iter().skip(i + 1) {
                assert_ne!(a, b);
            }
        }
    }

    #[test]
    fn high_cpu_threshold_is_inclusive_and_unknown_is_not_pressure() {
        assert_eq!(HIGH_CPU_PCT, 85.0);
        assert!(!ResourceSnapshot::UNKNOWN.high_cpu());
        assert!(
            !ResourceSnapshot {
                cpu_pct: Some(HIGH_CPU_PCT - 0.1),
                process_memory_mb: None,
            }
            .high_cpu()
        );
        assert!(
            ResourceSnapshot {
                cpu_pct: Some(HIGH_CPU_PCT),
                process_memory_mb: None,
            }
            .high_cpu()
        );
        assert_eq!(
            ResourceSnapshot::UNKNOWN.log_cpu_pct(),
            RESOURCE_UNKNOWN,
            "absent samples must stay machine-parseable"
        );
        assert_eq!(ResourceSnapshot::UNKNOWN.log_memory_mb(), RESOURCE_UNKNOWN);
        assert!(
            !AnomalyContext {
                event: "apply_wait_timeout",
                cpu_pct: RESOURCE_UNKNOWN,
                process_memory_mb: RESOURCE_UNKNOWN,
                resource_pressure: false,
            }
            .sampled()
        );
    }

    /// The only test that installs the process-wide provider. It proves that
    /// anomaly log context comes from the registered sampler and that a late
    /// duplicate registration cannot replace the source of truth.
    #[test]
    fn resource_provider_drives_anomaly_context_and_first_registration_wins() {
        let sample = ResourceSnapshot {
            cpu_pct: Some(HIGH_CPU_PCT + 1.0),
            process_memory_mb: Some(256.0),
        };
        assert!(set_resource_snapshot_provider(Arc::new(move || sample)));
        assert_eq!(resource_snapshot(), sample);
        assert!(
            !set_resource_snapshot_provider(Arc::new(|| ResourceSnapshot::UNKNOWN)),
            "a second provider must not replace the first"
        );
        assert_eq!(resource_snapshot(), sample);

        let context = AnomalyContext::for_event("apply_wait_timeout");
        assert_eq!(context.event, "apply_wait_timeout");
        assert_eq!(context.cpu_pct, HIGH_CPU_PCT + 1.0);
        assert_eq!(context.process_memory_mb, 256.0);
        assert!(context.resource_pressure);
        assert!(context.sampled());
    }

    /// Architecture guard: all four gate-blocking anomaly WARNs must sample
    /// resources, publish the correlation metrics, carry the resource fields
    /// in the log line, and emit the high-CPU alert. A new anomaly that skips
    /// this makes the original diagnosability requirement regress silently.
    #[test]
    fn every_anomaly_warn_carries_resource_context() {
        // Scan production code only: this test's own field-name literals would
        // otherwise satisfy the assertions after a real site is deleted.
        let prod = include_str!("telemetry.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("production section");
        for event in [
            "steady_state_election",
            "client_write_timeout",
            "apply_wait_timeout",
            "leaderless_give_up",
        ] {
            let marker = format!("AnomalyContext::for_event(\"{event}\")");
            let at = prod
                .find(&marker)
                .unwrap_or_else(|| panic!("missing {marker}"));
            let tail = &prod[at..];
            // Bound at the NEXT anomaly site: without this, deleting one
            // site's alert would make `find` land on the next event's alert
            // and the mutant would survive.
            let next = tail[marker.len()..]
                .find("AnomalyContext::for_event(")
                .map(|offset| offset + marker.len())
                .unwrap_or(tail.len());
            let block = &tail[..next];
            for field in [
                "raft_event = context.event",
                "record_anomaly_context(&context)",
                "cpu_pct = context.cpu_pct",
                "process_memory_mb = context.process_memory_mb",
                "resource_pressure = context.resource_pressure",
                "emit_resource_pressure(&context)",
            ] {
                assert!(block.contains(field), "{event} WARN is missing `{field}`");
            }
        }
    }

    /// The four blocker names are the complete anomaly set. A diagnostic
    /// `resource_pressure` alert must not be added to this list: it describes
    /// context around an existing anomaly, not a new failure mode.
    #[test]
    fn anomaly_event_names_are_stable() {
        let prod = include_str!("telemetry.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("production section");
        for event in [
            "steady_state_election",
            "client_write_timeout",
            "apply_wait_timeout",
            "leaderless_give_up",
        ] {
            assert!(
                prod.contains(&format!("AnomalyContext::for_event(\"{event}\")")),
                "{event} lost its anomaly context"
            );
        }
    }
}
