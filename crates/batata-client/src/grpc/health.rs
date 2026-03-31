//! gRPC connection health monitoring
//!
//! Periodic health check for the gRPC connection with automatic
//! reconnection on failure. Matches Nacos Java RpcClient behavior.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Default health check interval in seconds.
pub const DEFAULT_HEALTH_CHECK_INTERVAL_SECS: u64 = 5;

/// Default max failures before triggering reconnect.
pub const DEFAULT_HEALTH_CHECK_MAX_FAILURES: u32 = 3;

/// Default health check retry times per check cycle (matches Nacos).
pub const DEFAULT_HEALTH_CHECK_RETRY_TIMES: u32 = 3;

/// Maximum reconnect delay multiplier (matches Nacos: min(retryTurns+1, 50) * 100ms).
const MAX_RECONNECT_DELAY_MULTIPLIER: u32 = 50;

/// Base reconnect delay unit in milliseconds.
const RECONNECT_DELAY_UNIT_MS: u64 = 100;

/// Tracks connection health with consecutive failure counting
/// and exponential backoff for reconnection attempts.
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
    /// Retry count for health check within a single cycle
    health_check_retry_times: u32,
    /// Reconnection attempt counter for exponential backoff
    retry_turns: AtomicU32,
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
            health_check_retry_times: DEFAULT_HEALTH_CHECK_RETRY_TIMES,
            retry_turns: AtomicU32::new(0),
        }
    }

    /// Record a successful health check
    pub fn record_success(&self) {
        self.consecutive_failures.store(0, Ordering::Relaxed);
        self.connected.store(true, Ordering::Relaxed);
        self.retry_turns.store(0, Ordering::Relaxed);
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

    /// Get the number of retry times per health check cycle
    pub fn health_check_retry_times(&self) -> u32 {
        self.health_check_retry_times
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
        self.retry_turns.store(0, Ordering::Relaxed);
    }

    /// Calculate the next reconnect delay using exponential backoff.
    ///
    /// Matches Nacos Java: `Math.min(retryTurns + 1, 50) * 100L` milliseconds.
    /// Also increments the retry_turns counter.
    pub fn next_reconnect_delay(&self) -> Duration {
        let turns = self.retry_turns.fetch_add(1, Ordering::Relaxed);
        let multiplier = (turns + 1).min(MAX_RECONNECT_DELAY_MULTIPLIER);
        Duration::from_millis(multiplier as u64 * RECONNECT_DELAY_UNIT_MS)
    }

    /// Reset retry turns (called after successful reconnect)
    pub fn reset_retry_turns(&self) {
        self.retry_turns.store(0, Ordering::Relaxed);
    }

    /// Get current retry turns
    pub fn retry_turns(&self) -> u32 {
        self.retry_turns.load(Ordering::Relaxed)
    }
}

impl Default for ConnectionHealthChecker {
    fn default() -> Self {
        Self::new(
            DEFAULT_HEALTH_CHECK_MAX_FAILURES,
            Duration::from_secs(DEFAULT_HEALTH_CHECK_INTERVAL_SECS),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state() {
        let checker = ConnectionHealthChecker::default();
        assert!(checker.is_connected());
        assert_eq!(checker.failure_count(), 0);
        assert_eq!(checker.retry_turns(), 0);
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
        assert_eq!(checker.retry_turns(), 0);
    }

    #[test]
    fn test_failures_trigger_disconnect() {
        let checker = ConnectionHealthChecker::new(3, Duration::from_secs(5));
        assert!(!checker.record_failure()); // 1 < 3
        assert!(!checker.record_failure()); // 2 < 3
        assert!(checker.record_failure()); // 3 >= 3, triggers reconnect
        assert!(!checker.is_connected());
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
        assert_eq!(checker.retry_turns(), 0);
    }

    #[test]
    fn test_idle_millis() {
        let checker = ConnectionHealthChecker::default();
        // Just created, idle should be very small
        assert!(checker.idle_millis() < 100);
    }

    #[test]
    fn test_exponential_backoff() {
        let checker = ConnectionHealthChecker::default();

        // First delay: min(0+1, 50) * 100 = 100ms
        assert_eq!(checker.next_reconnect_delay(), Duration::from_millis(100));
        // Second: min(1+1, 50) * 100 = 200ms
        assert_eq!(checker.next_reconnect_delay(), Duration::from_millis(200));
        // Third: min(2+1, 50) * 100 = 300ms
        assert_eq!(checker.next_reconnect_delay(), Duration::from_millis(300));
    }

    #[test]
    fn test_exponential_backoff_max_cap() {
        let checker = ConnectionHealthChecker::default();

        // Simulate many retries to reach the cap
        for _ in 0..60 {
            checker.next_reconnect_delay();
        }
        // At turn 60, should be capped: min(60+1, 50) * 100 = 5000ms
        assert_eq!(checker.next_reconnect_delay(), Duration::from_millis(5000));
    }

    #[test]
    fn test_reset_retry_turns() {
        let checker = ConnectionHealthChecker::default();
        checker.next_reconnect_delay();
        checker.next_reconnect_delay();
        assert_eq!(checker.retry_turns(), 2);

        checker.reset_retry_turns();
        assert_eq!(checker.retry_turns(), 0);
        // First delay again after reset
        assert_eq!(checker.next_reconnect_delay(), Duration::from_millis(100));
    }
}
