//! TTL health check processor
//!
//! TTL checks do NOT make outbound connections. Instead, the check is considered
//! expired if last_updated + ttl < now. Clients must call /check/pass periodically.

use std::time::Duration;

use super::registry::InstanceCheckStatus;

/// TTL check expiration detector
pub struct TtlHealthCheckProcessor;

impl TtlHealthCheckProcessor {
    /// Check if a TTL check has expired.
    /// Returns true if expired (status should transition to Critical).
    pub fn check_expiration(status: &InstanceCheckStatus, ttl: Duration) -> bool {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let ttl_ms = ttl.as_millis() as i64;
        now_ms - status.last_updated > ttl_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::healthcheck::registry::CheckStatus;

    #[test]
    fn test_not_expired() {
        let status = InstanceCheckStatus {
            status: CheckStatus::Passing,
            output: String::new(),
            last_updated: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64,
            consecutive_successes: 1,
            consecutive_failures: 0,
            critical_since: None,
            last_response_time_ms: 0,
        };

        assert!(!TtlHealthCheckProcessor::check_expiration(
            &status,
            Duration::from_secs(30)
        ));
    }

    #[test]
    fn test_expired() {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let status = InstanceCheckStatus {
            status: CheckStatus::Passing,
            output: String::new(),
            last_updated: now_ms - 60_000, // 60 seconds ago
            consecutive_successes: 1,
            consecutive_failures: 0,
            critical_since: None,
            last_response_time_ms: 0,
        };

        // TTL of 30 seconds → should be expired
        assert!(TtlHealthCheckProcessor::check_expiration(
            &status,
            Duration::from_secs(30)
        ));
    }
}
