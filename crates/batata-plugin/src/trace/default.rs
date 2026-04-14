use async_trait::async_trait;

use super::event::{TraceEvent, TraceEventKind};
use super::registry::TraceSubscriber;

/// Default trace subscriber that writes each event to `tracing::info!`.
/// Mirrors the role of Nacos' `SimpleTraceSubscriber`.
pub struct LoggingTraceSubscriber {
    name: String,
}

impl LoggingTraceSubscriber {
    pub fn new() -> Self {
        Self {
            name: "logging".to_string(),
        }
    }

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
        tracing::info!(
            target: "batata::trace",
            kind = ?event.kind(),
            event_time = event.event_time(),
            namespace = event.namespace(),
            event = ?event,
            "trace event",
        );
    }
}
