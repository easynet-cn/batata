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

use std::time::Duration;

use metrics::{counter, gauge, histogram};
use tracing::{info, warn};

use super::types::{RaftMetrics, ServerState};

/// Target used for every structured health event. The regression gate greps
/// this target's `raft_event=` fields; keep the field names stable.
pub const HEALTH_LOG_TARGET: &str = "batata_raft_health";

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
}

impl RaftHealthSnapshot {
    /// How far the local state machine lags behind the local log. A steady
    /// cluster should stay at 0-1; sustained growth points at a blocked FSM.
    pub fn apply_lag(&self) -> u64 {
        self.last_log_index.saturating_sub(self.last_applied_index)
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
    gauge!("batata_raft_steady_state_elections").set(steady_state_elections as f64);
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
                warn!(
                    target: HEALTH_LOG_TARGET,
                    raft_event = "steady_state_election",
                    node_id = node_id,
                    term = *term,
                    from_role = from_role.as_str(),
                    "unexpected Raft election after a leader was already known"
                );
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
        warn!(
            target: HEALTH_LOG_TARGET,
            raft_event = "client_write_timeout",
            elapsed_ms = elapsed.as_millis() as u64,
            "local client_write exceeded its derived deadline"
        );
    }
}

/// Record a follower's wait for its state machine to catch up.
pub fn record_apply_wait(elapsed: Duration, timed_out: bool) {
    histogram!("batata_raft_apply_wait_duration_seconds").record(elapsed.as_secs_f64());
    if timed_out {
        counter!("batata_raft_apply_wait_timeouts_total").increment(1);
        warn!(
            target: HEALTH_LOG_TARGET,
            raft_event = "apply_wait_timeout",
            elapsed_ms = elapsed.as_millis() as u64,
            "follower did not apply the forwarded write within its deadline"
        );
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
    warn!(
        target: HEALTH_LOG_TARGET,
        raft_event = "leaderless_give_up",
        waited_ms = waited.as_millis() as u64,
        "no Raft leader became available within the derived election cycle"
    );
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
        }
    }

    #[test]
    fn apply_lag_is_saturating() {
        let mut s = snap(1, RaftRole::Leader, Some(1));
        assert_eq!(s.apply_lag(), 1);
        s.last_applied_index = 20;
        assert_eq!(s.apply_lag(), 0);
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
}
