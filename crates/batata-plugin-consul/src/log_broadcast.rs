//! Log broadcast channel for streaming agent monitor output.
//!
//! Provides a tracing subscriber layer that broadcasts formatted log events
//! to HTTP streaming consumers (e.g., `/v1/agent/monitor`).

use std::sync::OnceLock;

use tokio::sync::broadcast;

/// Maximum number of buffered log lines in the broadcast channel.
const LOG_BUFFER_SIZE: usize = 256;

/// Global broadcast sender for log events.
static LOG_SENDER: OnceLock<broadcast::Sender<LogEntry>> = OnceLock::new();

/// A formatted log entry for streaming.
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub module: String,
    pub message: String,
}

impl LogEntry {
    /// Format as plain text (matching Consul's monitor output).
    pub fn format_plain(&self) -> String {
        format!(
            "{}  [{}] {}: {}\n",
            self.timestamp, self.level, self.module, self.message
        )
    }

    /// Format as JSON (matching Consul's ?logjson=true output).
    pub fn format_json(&self) -> String {
        serde_json::json!({
            "@level": self.level.to_lowercase(),
            "@message": self.message,
            "@module": self.module,
            "@timestamp": self.timestamp,
        })
        .to_string()
            + "\n"
    }
}

/// Get or create the global log broadcast sender.
pub fn log_sender() -> &'static broadcast::Sender<LogEntry> {
    LOG_SENDER.get_or_init(|| {
        let (tx, _) = broadcast::channel(LOG_BUFFER_SIZE);
        tx
    })
}

/// Subscribe to the log broadcast channel.
pub fn subscribe() -> broadcast::Receiver<LogEntry> {
    log_sender().subscribe()
}

/// Publish a log entry to all subscribers. Returns silently if no subscribers.
pub fn publish(entry: LogEntry) {
    // Ignore send errors (no active receivers)
    let _ = log_sender().send(entry);
}

/// Tracing layer that broadcasts events to the monitor channel.
///
/// This is a lightweight layer that formats events and sends them
/// to the broadcast channel. Only active when monitor subscribers exist.
pub struct MonitorLayer;

impl<S> tracing_subscriber::Layer<S> for MonitorLayer
where
    S: tracing::Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        // Only format if there are active subscribers
        if log_sender().receiver_count() == 0 {
            return;
        }

        let metadata = event.metadata();
        let level = metadata.level().to_string();
        let module = metadata
            .module_path()
            .unwrap_or(metadata.target())
            .to_string();

        // Extract the message from the event fields
        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);

        let entry = LogEntry {
            timestamp: chrono::Utc::now().to_rfc3339(),
            level,
            module,
            message: visitor.message,
        };

        publish(entry);
    }
}

/// Visitor that extracts the "message" field from a tracing event.
#[derive(Default)]
struct MessageVisitor {
    message: String,
}

impl tracing::field::Visit for MessageVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{:?}", value);
        } else if self.message.is_empty() {
            // Append other fields as key=value
            self.message = format!("{}={:?}", field.name(), value);
        } else {
            self.message
                .push_str(&format!(" {}={:?}", field.name(), value));
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        } else if self.message.is_empty() {
            self.message = format!("{}={}", field.name(), value);
        } else {
            self.message
                .push_str(&format!(" {}={}", field.name(), value));
        }
    }
}
