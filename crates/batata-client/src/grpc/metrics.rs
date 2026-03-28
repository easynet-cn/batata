//! gRPC client connection metrics
//!
//! Lightweight atomic counters for observability without external dependencies.
//! These can be queried programmatically or exposed via Prometheus if needed.

use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Connection and request metrics for the gRPC client.
///
/// All counters are lock-free (atomic), safe for concurrent access.
#[derive(Default)]
pub struct ClientMetrics {
    /// Total number of requests sent
    pub requests_total: AtomicU64,
    /// Total number of successful requests
    pub requests_success: AtomicU64,
    /// Total number of failed requests
    pub requests_failed: AtomicU64,
    /// Total number of auth retries (403 relogin)
    pub auth_retries: AtomicU64,
    /// Total number of token refreshes
    pub token_refreshes: AtomicU64,
    /// Total number of reconnection attempts
    pub reconnects_total: AtomicU64,
    /// Total number of successful reconnections
    pub reconnects_success: AtomicU64,
    /// Total number of server push messages received
    pub push_received: AtomicU64,
    /// Total number of push handler dispatches
    pub push_dispatched: AtomicU64,
    /// Current connection state (ConnectionState as u8)
    pub current_state: AtomicU8,
    /// Timestamp of last successful request (millis since epoch)
    pub last_request_time: AtomicU64,
    /// Timestamp of last connection established (millis since epoch)
    pub connected_since: AtomicU64,
}

impl ClientMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_request_success(&self) {
        self.requests_total.fetch_add(1, Ordering::Relaxed);
        self.requests_success.fetch_add(1, Ordering::Relaxed);
        self.last_request_time
            .store(current_millis(), Ordering::Relaxed);
    }

    pub fn record_request_failure(&self) {
        self.requests_total.fetch_add(1, Ordering::Relaxed);
        self.requests_failed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_auth_retry(&self) {
        self.auth_retries.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_token_refresh(&self) {
        self.token_refreshes.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_reconnect(&self, success: bool) {
        self.reconnects_total.fetch_add(1, Ordering::Relaxed);
        if success {
            self.reconnects_success.fetch_add(1, Ordering::Relaxed);
            self.connected_since
                .store(current_millis(), Ordering::Relaxed);
        }
    }

    pub fn record_push_received(&self) {
        self.push_received.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_push_dispatched(&self) {
        self.push_dispatched.fetch_add(1, Ordering::Relaxed);
    }

    /// Get a snapshot of all metrics as a map for logging/monitoring.
    pub fn snapshot(&self) -> ClientMetricsSnapshot {
        ClientMetricsSnapshot {
            requests_total: self.requests_total.load(Ordering::Relaxed),
            requests_success: self.requests_success.load(Ordering::Relaxed),
            requests_failed: self.requests_failed.load(Ordering::Relaxed),
            auth_retries: self.auth_retries.load(Ordering::Relaxed),
            token_refreshes: self.token_refreshes.load(Ordering::Relaxed),
            reconnects_total: self.reconnects_total.load(Ordering::Relaxed),
            reconnects_success: self.reconnects_success.load(Ordering::Relaxed),
            push_received: self.push_received.load(Ordering::Relaxed),
            push_dispatched: self.push_dispatched.load(Ordering::Relaxed),
            current_state: self.current_state.load(Ordering::Relaxed),
            last_request_time: self.last_request_time.load(Ordering::Relaxed),
            connected_since: self.connected_since.load(Ordering::Relaxed),
        }
    }
}

/// Immutable snapshot of client metrics for reporting.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ClientMetricsSnapshot {
    pub requests_total: u64,
    pub requests_success: u64,
    pub requests_failed: u64,
    pub auth_retries: u64,
    pub token_refreshes: u64,
    pub reconnects_total: u64,
    pub reconnects_success: u64,
    pub push_received: u64,
    pub push_dispatched: u64,
    pub current_state: u8,
    pub last_request_time: u64,
    pub connected_since: u64,
}

fn current_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_default() {
        let m = ClientMetrics::new();
        assert_eq!(m.requests_total.load(Ordering::Relaxed), 0);
        assert_eq!(m.requests_success.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_record_request() {
        let m = ClientMetrics::new();
        m.record_request_success();
        m.record_request_success();
        m.record_request_failure();

        let s = m.snapshot();
        assert_eq!(s.requests_total, 3);
        assert_eq!(s.requests_success, 2);
        assert_eq!(s.requests_failed, 1);
        assert!(s.last_request_time > 0);
    }

    #[test]
    fn test_record_reconnect() {
        let m = ClientMetrics::new();
        m.record_reconnect(false);
        m.record_reconnect(true);

        let s = m.snapshot();
        assert_eq!(s.reconnects_total, 2);
        assert_eq!(s.reconnects_success, 1);
        assert!(s.connected_since > 0);
    }

    #[test]
    fn test_snapshot_serializable() {
        let m = ClientMetrics::new();
        m.record_request_success();
        let s = m.snapshot();
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"requests_total\":1"));
    }
}
