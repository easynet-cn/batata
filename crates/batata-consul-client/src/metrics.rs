//! Consul client request metrics
//!
//! Lightweight atomic counters for observability.

use std::sync::atomic::{AtomicU64, Ordering};

/// Request metrics for the Consul HTTP client.
/// All counters are lock-free (atomic).
#[derive(Default)]
pub struct ConsulClientMetrics {
    /// Total requests sent
    pub requests_total: AtomicU64,
    /// Successful requests (2xx)
    pub requests_success: AtomicU64,
    /// Failed requests (non-2xx or network error)
    pub requests_failed: AtomicU64,
    /// Retried requests
    pub retries_total: AtomicU64,
    /// Server address rotations
    pub address_rotations: AtomicU64,
    /// 404 Not Found responses
    pub not_found: AtomicU64,
    /// 429 Too Many Requests responses
    pub rate_limited: AtomicU64,
}

impl ConsulClientMetrics {
/// Performs the `new` operation.
    pub fn new() -> Self {
        Self::default()
    }

/// Performs the `record_success` operation.
    pub fn record_success(&self) {
        self.requests_total.fetch_add(1, Ordering::Relaxed);
        self.requests_success.fetch_add(1, Ordering::Relaxed);
    }

/// Performs the `record_failure` operation.
    pub fn record_failure(&self) {
        self.requests_total.fetch_add(1, Ordering::Relaxed);
        self.requests_failed.fetch_add(1, Ordering::Relaxed);
    }

/// Performs the `record_retry` operation.
    pub fn record_retry(&self) {
        self.retries_total.fetch_add(1, Ordering::Relaxed);
    }

/// Performs the `record_rotation` operation.
    pub fn record_rotation(&self) {
        self.address_rotations.fetch_add(1, Ordering::Relaxed);
    }

/// Performs the `record_not_found` operation.
    pub fn record_not_found(&self) {
        self.not_found.fetch_add(1, Ordering::Relaxed);
    }

/// Performs the `record_rate_limited` operation.
    pub fn record_rate_limited(&self) {
        self.rate_limited.fetch_add(1, Ordering::Relaxed);
    }

    /// Get a snapshot of all metrics
    pub fn snapshot(&self) -> ConsulMetricsSnapshot {
        ConsulMetricsSnapshot {
            requests_total: self.requests_total.load(Ordering::Relaxed),
            requests_success: self.requests_success.load(Ordering::Relaxed),
            requests_failed: self.requests_failed.load(Ordering::Relaxed),
            retries_total: self.retries_total.load(Ordering::Relaxed),
            address_rotations: self.address_rotations.load(Ordering::Relaxed),
            not_found: self.not_found.load(Ordering::Relaxed),
            rate_limited: self.rate_limited.load(Ordering::Relaxed),
        }
    }
}

/// Immutable snapshot of metrics for reporting
#[derive(Debug, Clone, serde::Serialize)]
pub struct ConsulMetricsSnapshot {
    /// Total number of requests sent.
    pub requests_total: u64,
    /// Number of successful requests.
    pub requests_success: u64,
    /// Number of failed requests.
    pub requests_failed: u64,
    /// Total number of retries performed.
    pub retries_total: u64,
    /// Number of server address rotations.
    pub address_rotations: u64,
    /// Number of 404 responses received.
    pub not_found: u64,
    /// Number of rate-limited responses received.
    pub rate_limited: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics() {
        let m = ConsulClientMetrics::new();
        m.record_success();
        m.record_success();
        m.record_failure();
        m.record_retry();

        let s = m.snapshot();
        assert_eq!(s.requests_total, 3);
        assert_eq!(s.requests_success, 2);
        assert_eq!(s.requests_failed, 1);
        assert_eq!(s.retries_total, 1);
    }
}
