//! Lifecycle phase timing for startup and shutdown.
//!
//! Upstream Nacos reports total startup cost together with deployment mode and
//! storage (`NacosCoreStartUp.logStarted`, `NacosWebStartUp.logStarted`). Batata
//! keeps that shape and adds per-phase structured logs plus `metrics` recorder
//! samples. This is deliberately observability-only: no wire, admin API, or
//! persisted schema changes.

use std::fmt;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use metrics::{counter, describe_counter, describe_gauge, describe_histogram, gauge, histogram};

pub(crate) const PHASE_DURATION_METRIC: &str = "batata_server_lifecycle_phase_duration_seconds";
pub(crate) const TOTAL_DURATION_METRIC: &str = "batata_server_lifecycle_total_duration_seconds";
pub(crate) const PHASE_FAILURES_METRIC: &str = "batata_server_lifecycle_phase_failures_total";

/// Stable phase labels. Keeping them in one place prevents log/metric drift.
pub(crate) mod phases {
    pub(crate) const CONFIG: &str = "config";
    pub(crate) const PLUGINS_REGISTER: &str = "plugins.register";
    pub(crate) const PERSISTENCE: &str = "persistence";
    pub(crate) const SERVICES: &str = "services";
    pub(crate) const SHUTDOWN_AI: &str = "shutdown_ai";
    pub(crate) const CONSOLE_REMOTE: &str = "console_remote";
    pub(crate) const GRPC: &str = "grpc";
    pub(crate) const BACKGROUND_TASKS: &str = "background_tasks";
    pub(crate) const CLUSTER_INIT: &str = "cluster_init";
    pub(crate) const XDS: &str = "xds";
    pub(crate) const PLUGINS_INIT: &str = "plugins.init";
    pub(crate) const HTTP_BIND: &str = "http.bind";
    pub(crate) const STARTUP_BARRIER: &str = "startup.barrier";
    pub(crate) const HTTP_DRAIN: &str = "http.drain";
    pub(crate) const HEALTH_CHECK: &str = "health_check";
    pub(crate) const ISTIO_MCP: &str = "istio_mcp";
    pub(crate) const PLUGINS_SHUTDOWN: &str = "plugins.shutdown";
    pub(crate) const CLUSTER_MANAGER: &str = "cluster_manager";
    pub(crate) const RAFT: &str = "raft";
    pub(crate) const DATABASE: &str = "database";
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LifecycleKind {
    Startup,
    Shutdown,
}

impl LifecycleKind {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Startup => "startup",
            Self::Shutdown => "shutdown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PhaseOutcome {
    Ok,
    Failed,
}

impl PhaseOutcome {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PhaseRecord {
    pub(crate) phase: &'static str,
    pub(crate) kind: LifecycleKind,
    pub(crate) duration_ms: u64,
    pub(crate) outcome: PhaseOutcome,
}

pub(crate) fn duration_ms(duration: Duration) -> u64 {
    duration.as_millis().min(u64::MAX as u128) as u64
}

#[derive(Debug)]
struct TimelineInner {
    created_at: Instant,
    records: Mutex<Vec<PhaseRecord>>,
    shutdown_started: OnceLock<Instant>,
}

/// A shared, cloneable timeline. `AppBuilder` owns one copy and hands clones to
/// the HTTP/shutdown helpers that run outside its mutable phase chain.
#[derive(Clone, Debug)]
pub(crate) struct LifecycleTimeline(Arc<TimelineInner>);

impl Default for LifecycleTimeline {
    fn default() -> Self {
        Self::new()
    }
}

impl LifecycleTimeline {
    pub(crate) fn new() -> Self {
        Self(Arc::new(TimelineInner {
            created_at: Instant::now(),
            records: Mutex::new(Vec::new()),
            shutdown_started: OnceLock::new(),
        }))
    }

    pub(crate) fn begin(&self, phase: &'static str, kind: LifecycleKind) -> PhaseTimer {
        PhaseTimer {
            timeline: self.clone(),
            phase,
            kind,
            started_at: Instant::now(),
            recorded: false,
        }
    }

    fn push(&self, phase: &'static str, kind: LifecycleKind, ms: u64, outcome: PhaseOutcome) {
        let record = PhaseRecord {
            phase,
            kind,
            duration_ms: ms,
            outcome,
        };
        let mut records = match self.0.records.lock() {
            Ok(records) => records,
            Err(poisoned) => poisoned.into_inner(),
        };
        records.push(record);
    }

    pub(crate) fn snapshot(&self) -> Vec<PhaseRecord> {
        match self.0.records.lock() {
            Ok(records) => records.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    pub(crate) fn startup_elapsed_ms(&self) -> u64 {
        duration_ms(self.0.created_at.elapsed())
    }

    /// First shutdown trigger wins: either the signal or an HTTP task exit.
    pub(crate) fn mark_shutdown(&self) {
        let _ = self.0.shutdown_started.set(Instant::now());
    }

    pub(crate) fn shutdown_elapsed_ms(&self) -> Option<u64> {
        self.0
            .shutdown_started
            .get()
            .map(|at| duration_ms(at.elapsed()))
    }

    /// Publish the total duration gauge and return the same value for logs.
    pub(crate) fn record_total(&self, kind: LifecycleKind) -> u64 {
        let ms = match kind {
            LifecycleKind::Startup => self.startup_elapsed_ms(),
            LifecycleKind::Shutdown => self.shutdown_elapsed_ms().unwrap_or(0),
        };
        gauge!(
            TOTAL_DURATION_METRIC,
            "kind" => kind.as_str()
        )
        .set(ms as f64 / 1000.0);
        ms
    }

    pub(crate) fn describe_metrics() {
        describe_histogram!(
            PHASE_DURATION_METRIC,
            "Startup and shutdown phase duration in seconds, by phase/kind/outcome"
        );
        describe_gauge!(
            TOTAL_DURATION_METRIC,
            "Total startup or shutdown duration in seconds"
        );
        describe_counter!(
            PHASE_FAILURES_METRIC,
            "Lifecycle phases that returned an error or unwound before completion"
        );
    }
}

/// Finish a timed startup phase with the real `Result` so the failure outcome
/// and its error text are observable instead of being inferred at drop time.
pub(crate) fn finish_startup_phase<T, E: fmt::Display>(
    timer: &mut PhaseTimer,
    result: Result<T, E>,
) -> Result<T, E> {
    match &result {
        Ok(_) => timer.ok(),
        Err(error) => timer.fail(error),
    }
    result
}

pub(crate) struct PhaseTimer {
    timeline: LifecycleTimeline,
    phase: &'static str,
    kind: LifecycleKind,
    started_at: Instant,
    recorded: bool,
}

impl PhaseTimer {
    pub(crate) fn elapsed_ms(&self) -> u64 {
        duration_ms(self.started_at.elapsed())
    }

    pub(crate) fn ok(&mut self) {
        self.finish(PhaseOutcome::Ok, None);
    }

    pub(crate) fn fail<E: fmt::Display>(&mut self, error: E) {
        self.finish(PhaseOutcome::Failed, Some(&error as &dyn fmt::Display));
    }

    fn finish(&mut self, outcome: PhaseOutcome, error: Option<&dyn fmt::Display>) {
        if self.recorded {
            return;
        }
        self.recorded = true;
        let ms = self.elapsed_ms();
        self.timeline.push(self.phase, self.kind, ms, outcome);

        let kind = self.kind.as_str();
        histogram!(
            PHASE_DURATION_METRIC,
            "phase" => self.phase,
            "kind" => kind,
            "outcome" => outcome.as_str()
        )
        .record(ms as f64 / 1000.0);

        if outcome == PhaseOutcome::Failed {
            counter!(
                PHASE_FAILURES_METRIC,
                "phase" => self.phase,
                "kind" => kind
            )
            .increment(1);
            match error {
                Some(error) => tracing::warn!(
                    phase = self.phase,
                    kind = kind,
                    duration_ms = ms,
                    outcome = outcome.as_str(),
                    error = %error,
                    "lifecycle phase failed"
                ),
                None => tracing::warn!(
                    phase = self.phase,
                    kind = kind,
                    duration_ms = ms,
                    outcome = outcome.as_str(),
                    "lifecycle phase failed"
                ),
            }
        } else {
            tracing::info!(
                phase = self.phase,
                kind = kind,
                duration_ms = ms,
                outcome = outcome.as_str(),
                "lifecycle phase completed"
            );
        }
    }
}

impl Drop for PhaseTimer {
    fn drop(&mut self) {
        // An early `?` return or panic before an explicit `ok()`/`fail()` is a
        // failed/incomplete phase. Record it once, never twice.
        self.finish(PhaseOutcome::Failed, None);
    }
}

/// Nacos-shaped mode label (`stand alone` / `cluster`).
pub(crate) fn mode_label(is_standalone: bool) -> &'static str {
    if is_standalone {
        "stand alone"
    } else {
        "cluster"
    }
}

/// External/embedded split matches upstream `NacosCoreStartUp.logStarted()`.
pub(crate) fn storage_label(mode: batata_persistence::StorageMode) -> &'static str {
    match mode {
        batata_persistence::StorageMode::ExternalDb => "external",
        batata_persistence::StorageMode::StandaloneEmbedded
        | batata_persistence::StorageMode::DistributedEmbedded => "embedded",
    }
}

pub(crate) fn startup_summary(mode: &str, storage: &str, ms: u64) -> String {
    format!("Batata started successfully in {mode} mode with {storage} storage in {ms} ms")
}

pub(crate) fn shutdown_summary(ms: u64) -> String {
    format!("Batata shutdown complete in {ms} ms")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_ms_converts_without_rounding_up() {
        assert_eq!(duration_ms(Duration::from_millis(1500)), 1500);
        assert_eq!(duration_ms(Duration::from_micros(999)), 0);
        assert_eq!(duration_ms(Duration::from_secs(3)), 3000);
    }

    #[test]
    fn finish_startup_phase_records_success_and_failure_outcomes() {
        let timeline = LifecycleTimeline::new();
        let mut ok_timer = timeline.begin(phases::CONFIG, LifecycleKind::Startup);
        let value = finish_startup_phase(&mut ok_timer, Ok::<_, std::io::Error>(7)).unwrap();
        assert_eq!(value, 7);

        let mut failed_timer = timeline.begin(phases::PERSISTENCE, LifecycleKind::Startup);
        let error = finish_startup_phase(&mut failed_timer, Err::<(), _>("db unavailable"));
        assert!(error.is_err());

        let records = timeline.snapshot();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].phase, phases::CONFIG);
        assert_eq!(records[0].outcome, PhaseOutcome::Ok);
        assert_eq!(records[1].phase, phases::PERSISTENCE);
        assert_eq!(records[1].outcome, PhaseOutcome::Failed);
        assert_eq!(records[1].kind, LifecycleKind::Startup);
    }

    #[test]
    fn dropped_timer_records_failed_phase_once() {
        let timeline = LifecycleTimeline::new();
        {
            let mut timer = timeline.begin(phases::HTTP_DRAIN, LifecycleKind::Shutdown);
            timer.ok();
        }
        {
            let _timer = timeline.begin(phases::HTTP_BIND, LifecycleKind::Startup);
        }
        let records = timeline.snapshot();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].outcome, PhaseOutcome::Ok);
        assert_eq!(records[1].outcome, PhaseOutcome::Failed);
    }

    #[test]
    fn repeated_completion_does_not_duplicate_records() {
        let timeline = LifecycleTimeline::new();
        let mut timer = timeline.begin(phases::GRPC, LifecycleKind::Startup);
        timer.ok();
        timer.ok();
        timer.fail("late error");
        assert_eq!(timeline.snapshot().len(), 1);
    }

    #[test]
    fn snapshot_is_a_copy_not_a_live_view() {
        let timeline = LifecycleTimeline::new();
        let snapshot = timeline.snapshot();
        assert!(snapshot.is_empty());
        let mut timer = timeline.begin(phases::CONFIG, LifecycleKind::Startup);
        timer.ok();
        assert!(
            snapshot.is_empty(),
            "snapshot must not alias timeline storage"
        );
        assert_eq!(timeline.snapshot().len(), 1);
    }

    #[test]
    fn shutdown_mark_is_first_trigger_wins() {
        let timeline = LifecycleTimeline::new();
        assert!(timeline.shutdown_elapsed_ms().is_none());
        timeline.mark_shutdown();
        let first = timeline
            .shutdown_elapsed_ms()
            .expect("mark must start shutdown clock");
        std::thread::sleep(Duration::from_millis(2));
        timeline.mark_shutdown();
        let later = timeline
            .shutdown_elapsed_ms()
            .expect("clock must remain set");
        assert!(
            later >= first,
            "a second trigger must not restart the clock"
        );
    }

    #[test]
    fn record_total_publishes_startup_and_shutdown_elapsed() {
        let timeline = LifecycleTimeline::new();
        // Long enough to be visible at millisecond resolution even on a fast
        // machine, so a shutdown total accidentally backed by the startup
        // clock cannot pass with a rounded zero.
        std::thread::sleep(Duration::from_millis(5));
        let startup = timeline.record_total(LifecycleKind::Startup);
        assert!(
            startup >= 5,
            "startup clock must include elapsed time: {startup}"
        );
        assert_eq!(timeline.record_total(LifecycleKind::Shutdown), 0);
        timeline.mark_shutdown();
        assert!(timeline.shutdown_elapsed_ms().is_some());
    }

    #[test]
    fn summaries_match_upstream_started_successfully_shape() {
        assert_eq!(
            startup_summary("stand alone", "embedded", 1234),
            "Batata started successfully in stand alone mode with embedded storage in 1234 ms"
        );
        assert_eq!(shutdown_summary(56), "Batata shutdown complete in 56 ms");
    }

    #[test]
    fn mode_and_storage_labels_follow_upstream_vocabulary() {
        assert_eq!(mode_label(true), "stand alone");
        assert_eq!(mode_label(false), "cluster");
        assert_eq!(
            storage_label(batata_persistence::StorageMode::StandaloneEmbedded),
            "embedded"
        );
        assert_eq!(
            storage_label(batata_persistence::StorageMode::DistributedEmbedded),
            "embedded"
        );
        assert_eq!(
            storage_label(batata_persistence::StorageMode::ExternalDb),
            "external"
        );
    }

    #[test]
    fn lifecycle_labels_are_stable() {
        assert_eq!(LifecycleKind::Startup.as_str(), "startup");
        assert_eq!(LifecycleKind::Shutdown.as_str(), "shutdown");
        assert_eq!(PhaseOutcome::Ok.as_str(), "ok");
        assert_eq!(PhaseOutcome::Failed.as_str(), "failed");
    }
}
