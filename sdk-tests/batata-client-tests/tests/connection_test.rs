//! Connection health check and auto-reconnection tests (no server required).
//!
//! Tests the ConnectionHealthChecker exponential backoff logic,
//! state transitions, and health check timing.

use std::time::Duration;

use batata_client::grpc::health::ConnectionHealthChecker;

#[test]
fn test_health_checker_initial_state() {
    let checker = ConnectionHealthChecker::default();
    assert!(checker.is_connected());
    assert_eq!(checker.failure_count(), 0);
    assert_eq!(checker.retry_turns(), 0);
    assert!(checker.idle_millis() < 100);
}

#[test]
fn test_health_checker_success_resets_all() {
    let checker = ConnectionHealthChecker::default();
    checker.record_failure();
    checker.record_failure();
    checker.next_reconnect_delay(); // increment retry turns

    checker.record_success();
    assert!(checker.is_connected());
    assert_eq!(checker.failure_count(), 0);
    assert_eq!(checker.retry_turns(), 0);
}

#[test]
fn test_health_checker_failure_threshold() {
    let checker = ConnectionHealthChecker::new(3, Duration::from_secs(5));

    // Below threshold
    assert!(!checker.record_failure()); // 1/3
    assert!(checker.is_connected());
    assert!(!checker.record_failure()); // 2/3
    assert!(checker.is_connected());

    // At threshold
    assert!(checker.record_failure()); // 3/3 → triggers reconnect
    assert!(!checker.is_connected());
}

#[test]
fn test_health_checker_reconnect_resets_state() {
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
fn test_exponential_backoff_progression() {
    let checker = ConnectionHealthChecker::default();

    // Verify the progression: min(turn+1, 50) * 100ms
    let delays: Vec<Duration> = (0..10)
        .map(|_| checker.next_reconnect_delay())
        .collect();

    assert_eq!(delays[0], Duration::from_millis(100));   // min(0+1, 50) * 100
    assert_eq!(delays[1], Duration::from_millis(200));   // min(1+1, 50) * 100
    assert_eq!(delays[2], Duration::from_millis(300));   // min(2+1, 50) * 100
    assert_eq!(delays[3], Duration::from_millis(400));   // min(3+1, 50) * 100
    assert_eq!(delays[4], Duration::from_millis(500));   // min(4+1, 50) * 100
    assert_eq!(delays[9], Duration::from_millis(1000));  // min(9+1, 50) * 100
}

#[test]
fn test_exponential_backoff_cap_at_5_seconds() {
    let checker = ConnectionHealthChecker::default();

    // Advance to beyond the cap
    for _ in 0..60 {
        checker.next_reconnect_delay();
    }

    // Should be capped at 5000ms = min(60+1, 50) * 100
    let delay = checker.next_reconnect_delay();
    assert_eq!(delay, Duration::from_millis(5000));
}

#[test]
fn test_reset_retry_turns() {
    let checker = ConnectionHealthChecker::default();

    checker.next_reconnect_delay();
    checker.next_reconnect_delay();
    checker.next_reconnect_delay();
    assert_eq!(checker.retry_turns(), 3);

    checker.reset_retry_turns();
    assert_eq!(checker.retry_turns(), 0);

    // After reset, starts from beginning
    assert_eq!(checker.next_reconnect_delay(), Duration::from_millis(100));
}

#[test]
fn test_health_checker_custom_configuration() {
    let checker = ConnectionHealthChecker::new(5, Duration::from_secs(10));
    assert_eq!(checker.interval(), Duration::from_secs(10));
    assert_eq!(checker.health_check_retry_times(), 3); // default retry times

    // Need 5 failures to trigger
    for _ in 0..4 {
        assert!(!checker.record_failure());
    }
    assert!(checker.record_failure()); // 5th failure triggers
}

#[test]
fn test_health_checker_set_reconnecting() {
    let checker = ConnectionHealthChecker::default();
    assert!(checker.is_connected());

    checker.set_reconnecting();
    assert!(!checker.is_connected());
}

#[test]
fn test_health_checker_concurrent_safe() {
    use std::sync::Arc;
    use std::thread;

    let checker = Arc::new(ConnectionHealthChecker::default());

    let handles: Vec<_> = (0..10)
        .map(|i| {
            let c = checker.clone();
            thread::spawn(move || {
                if i % 2 == 0 {
                    c.record_failure();
                } else {
                    c.record_success();
                }
                c.next_reconnect_delay();
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Just verify no panics — exact values depend on scheduling
    let _ = checker.failure_count();
    let _ = checker.retry_turns();
}
