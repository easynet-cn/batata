use async_trait::async_trait;

use super::event::{TraceEvent, TraceEventKind};
use super::registry::TraceSubscriber;

/// Default trace subscriber that writes each event to `tracing::debug!`.
/// Mirrors the role of Nacos' `SimpleTraceSubscriber`.
///
/// Trace events fire once per config write, so an INFO-level default made this
/// single subscriber emit ~87k lines per cluster node in one perf run. DEBUG
/// keeps the events available (`RUST_LOG=batata::trace=debug`) without making
/// them part of the normal production log volume. Other subscribers still
/// receive every event unchanged.
pub struct LoggingTraceSubscriber {
    name: String,
}

impl LoggingTraceSubscriber {
    /// Creates a new `LoggingTraceSubscriber` with the default name `"logging"`.
    pub fn new() -> Self {
        Self {
            name: "logging".to_string(),
        }
    }

    /// Creates a new `LoggingTraceSubscriber` with the given name.
    pub fn with_name(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

impl Default for LoggingTraceSubscriber {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TraceSubscriber for LoggingTraceSubscriber {
    fn name(&self) -> &str {
        &self.name
    }

    fn subscribed_kinds(&self) -> &[TraceEventKind] {
        &[]
    }

    async fn on_event(&self, event: &TraceEvent) {
        tracing::debug!(
            target: "batata::trace",
            kind = ?event.kind(),
            event_time = event.event_time(),
            namespace = event.namespace(),
            event = ?event,
            "trace event",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The success path still dispatches after the level change; no tracing
    /// subscriber is installed in unit tests, so `debug!` is a cheap no-op.
    #[tokio::test]
    async fn on_event_still_dispatches_every_event() {
        let subscriber = LoggingTraceSubscriber::new();
        assert_eq!(subscriber.name(), "logging");
        assert!(subscriber.subscribed_kinds().is_empty());
        subscriber
            .on_event(&TraceEvent::ConfigPublish {
                event_time: 1,
                namespace: "public".to_string(),
                group: "DEFAULT_GROUP".to_string(),
                data_id: "app.yaml".to_string(),
                client_ip: "127.0.0.1".to_string(),
            })
            .await;
    }

    /// Architecture guard: reverting the event log to INFO brings back the
    /// per-write log volume this subscriber was responsible for.
    #[test]
    fn trace_events_are_logged_at_debug() {
        let src = include_str!("default.rs");
        let tests_start = src
            .find("#[cfg(test)]\nmod tests")
            .expect("tests module must exist");
        let production = &src[..tests_start];
        assert!(production.contains("tracing::debug!("));
        assert!(
            !production.contains("tracing::info!("),
            "trace events must not be logged at INFO by default"
        );
    }
}
