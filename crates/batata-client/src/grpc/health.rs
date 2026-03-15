//! gRPC connection health monitoring
//!
//! Periodic health check for the gRPC connection with automatic
//! reconnection on failure.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Connection health state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Connected,
    Disconnected,
    Reconnecting,
}

/// Tracks connection health with consecutive failure counting
pub struct ConnectionHealthChecker {
    /// Current connection state
    connected: AtomicBool,
    /// Consecutive health check failures
    consecutive_failures: AtomicU32,
    /// Last successful health check time (millis since epoch)
    last_success_time: AtomicU64,
    /// Maximum failures before triggering reconnect
    max_failures: u32,
    /// Health check interval
    check_interval: Duration,
}

impl ConnectionHealthChecker {
    pub fn new(max_failures: u32, check_interval: Duration) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Self {
            connected: AtomicBool::new(true),
            consecutive_failures: AtomicU32::new(0),
            last_success_time: AtomicU64::new(now),
            max_failures,
            check_interval,
        }
    }

    /// Record a successful health check
    pub fn record_success(&self) {
        self.consecutive_failures.store(0, Ordering::Relaxed);
        self.connected.store(true, Ordering::Relaxed);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.last_success_time.store(now, Ordering::Relaxed);
    }

    /// Record a failed health check, returns true if reconnect should be triggered
    pub fn record_failure(&self) -> bool {
        let failures = self.consecutive_failures.fetch_add(1, Ordering::Relaxed) + 1;
        if failures >= self.max_failures {
            self.connected.store(false, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    /// Get current connection state
    pub fn state(&self) -> ConnectionState {
        if self.connected.load(Ordering::Relaxed) {
            ConnectionState::Connected
        } else {
            ConnectionState::Disconnected
        }
    }

    /// Check if connected
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    /// Get consecutive failure count
    pub fn failure_count(&self) -> u32 {
        self.consecutive_failures.load(Ordering::Relaxed)
    }

    /// Get health check interval
    pub fn interval(&self) -> Duration {
        self.check_interval
    }

    /// Get milliseconds since last successful check
    pub fn idle_millis(&self) -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        now.saturating_sub(self.last_success_time.load(Ordering::Relaxed))
    }

    /// Mark as reconnecting
    pub fn set_reconnecting(&self) {
        self.connected.store(false, Ordering::Relaxed);
    }

    /// Mark as connected (after successful reconnect)
    pub fn set_connected(&self) {
        self.connected.store(true, Ordering::Relaxed);
        self.consecutive_failures.store(0, Ordering::Relaxed);
    }
}

impl Default for ConnectionHealthChecker {
    fn default() -> Self {
        Self::new(3, Duration::from_secs(5))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state() {
        let checker = ConnectionHealthChecker::default();
        assert!(checker.is_connected());
        assert_eq!(checker.state(), ConnectionState::Connected);
        assert_eq!(checker.failure_count(), 0);
    }

    #[test]
    fn test_success_resets_failures() {
        let checker = ConnectionHealthChecker::default();
        checker.record_failure();
        checker.record_failure();
        assert_eq!(checker.failure_count(), 2);

        checker.record_success();
        assert_eq!(checker.failure_count(), 0);
        assert!(checker.is_connected());
    }

    #[test]
    fn test_failures_trigger_disconnect() {
        let checker = ConnectionHealthChecker::new(3, Duration::from_secs(5));
        assert!(!checker.record_failure()); // 1 < 3
        assert!(!checker.record_failure()); // 2 < 3
        assert!(checker.record_failure()); // 3 >= 3, triggers reconnect
        assert!(!checker.is_connected());
        assert_eq!(checker.state(), ConnectionState::Disconnected);
    }

    #[test]
    fn test_reconnect_cycle() {
        let checker = ConnectionHealthChecker::new(2, Duration::from_secs(5));
        checker.record_failure();
        checker.record_failure();
        assert!(!checker.is_connected());

        checker.set_connected();
        assert!(checker.is_connected());
        assert_eq!(checker.failure_count(), 0);
    }

    #[test]
    fn test_idle_millis() {
        let checker = ConnectionHealthChecker::default();
        // Just created, idle should be very small
        assert!(checker.idle_millis() < 100);
    }
}
