// Integration tests for CircuitBreaker
// Tests circuit breaker state transitions and protection patterns

use batata::core::service::circuit_breaker::{
    CircuitBreaker, CircuitBreakerConfig, CircuitBreakerError, CircuitState, with_circuit_breaker,
};
use std::time::Duration;

#[test]
fn test_circuit_breaker_lifecycle() {
    let config = CircuitBreakerConfig {
        failure_threshold: 3,
        reset_timeout: Duration::from_millis(100),
        success_threshold: 2,
        failure_window: Duration::from_secs(60),
    };
    let cb = CircuitBreaker::with_config(config);

    // Initial state is closed
    assert_eq!(cb.state(), CircuitState::Closed);
    assert!(cb.allow_request());

    // Record failures until threshold
    cb.record_failure();
    assert_eq!(cb.state(), CircuitState::Closed);
    cb.record_failure();
    assert_eq!(cb.state(), CircuitState::Closed);
    cb.record_failure();

    // Circuit should now be open
    assert_eq!(cb.state(), CircuitState::Open);
    assert!(!cb.allow_request());
}

#[test]
fn test_success_resets_failure_count() {
    let config = CircuitBreakerConfig {
        failure_threshold: 3,
        ..Default::default()
    };
    let cb = CircuitBreaker::with_config(config);

    // Record some failures
    cb.record_failure();
    cb.record_failure();
    assert_eq!(cb.failure_count(), 2);

    // Success should reset
    cb.record_success();
    assert_eq!(cb.failure_count(), 0);
    assert_eq!(cb.state(), CircuitState::Closed);
}

#[test]
fn test_half_open_recovery() {
    let config = CircuitBreakerConfig {
        failure_threshold: 2,
        reset_timeout: Duration::from_millis(50),
        success_threshold: 2,
        failure_window: Duration::from_secs(60),
    };
    let cb = CircuitBreaker::with_config(config);

    // Open the circuit
    cb.record_failure();
    cb.record_failure();
    assert_eq!(cb.state(), CircuitState::Open);

    // Wait for reset timeout
    std::thread::sleep(Duration::from_millis(60));

    // Should transition to half-open on next request
    assert!(cb.allow_request());
    assert_eq!(cb.state(), CircuitState::HalfOpen);

    // Successful requests in half-open should close circuit
    cb.record_success();
    assert_eq!(cb.state(), CircuitState::HalfOpen);
    cb.record_success();
    assert_eq!(cb.state(), CircuitState::Closed);
}

#[test]
fn test_half_open_failure_reopens() {
    let config = CircuitBreakerConfig {
        failure_threshold: 2,
        reset_timeout: Duration::from_millis(50),
        success_threshold: 3,
        failure_window: Duration::from_secs(60),
    };
    let cb = CircuitBreaker::with_config(config);

    // Open the circuit
    cb.record_failure();
    cb.record_failure();
    assert_eq!(cb.state(), CircuitState::Open);

    // Wait for reset timeout
    std::thread::sleep(Duration::from_millis(60));

    // Transition to half-open
    cb.allow_request();
    assert_eq!(cb.state(), CircuitState::HalfOpen);

    // One success
    cb.record_success();
    assert_eq!(cb.state(), CircuitState::HalfOpen);

    // Failure in half-open should reopen
    cb.record_failure();
    assert_eq!(cb.state(), CircuitState::Open);
}

#[test]
fn test_manual_reset() {
    let config = CircuitBreakerConfig {
        failure_threshold: 2,
        ..Default::default()
    };
    let cb = CircuitBreaker::with_config(config);

    // Open the circuit
    cb.record_failure();
    cb.record_failure();
    assert_eq!(cb.state(), CircuitState::Open);

    // Manual reset
    cb.reset();
    assert_eq!(cb.state(), CircuitState::Closed);
    assert_eq!(cb.failure_count(), 0);
    assert!(cb.allow_request());
}

#[tokio::test]
async fn test_with_circuit_breaker_success() {
    let cb = CircuitBreaker::new();

    let result: Result<i32, CircuitBreakerError<std::io::Error>> =
        with_circuit_breaker(&cb, async { Ok(42) }).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 42);
    assert_eq!(cb.failure_count(), 0);
}

#[tokio::test]
async fn test_with_circuit_breaker_failure() {
    let config = CircuitBreakerConfig {
        failure_threshold: 2,
        ..Default::default()
    };
    let cb = CircuitBreaker::with_config(config);

    // First failure
    let result1: Result<i32, CircuitBreakerError<std::io::Error>> = with_circuit_breaker(&cb, async {
        Err(std::io::Error::new(std::io::ErrorKind::Other, "test error"))
    })
    .await;

    assert!(matches!(result1, Err(CircuitBreakerError::OperationFailed(_))));
    assert_eq!(cb.failure_count(), 1);

    // Second failure opens circuit
    let _result2: Result<i32, CircuitBreakerError<std::io::Error>> = with_circuit_breaker(&cb, async {
        Err(std::io::Error::new(std::io::ErrorKind::Other, "test error"))
    })
    .await;

    assert_eq!(cb.state(), CircuitState::Open);
}

#[tokio::test]
async fn test_with_circuit_breaker_rejects_when_open() {
    let config = CircuitBreakerConfig {
        failure_threshold: 1,
        reset_timeout: Duration::from_secs(60),
        ..Default::default()
    };
    let cb = CircuitBreaker::with_config(config);

    // Open the circuit
    cb.record_failure();
    assert_eq!(cb.state(), CircuitState::Open);

    // Request should be rejected
    let result: Result<i32, CircuitBreakerError<std::io::Error>> =
        with_circuit_breaker(&cb, async { Ok(42) }).await;

    assert!(matches!(result, Err(CircuitBreakerError::CircuitOpen)));
}

#[test]
fn test_concurrent_access() {
    use std::sync::Arc;
    use std::thread;

    let cb = Arc::new(CircuitBreaker::with_config(CircuitBreakerConfig {
        failure_threshold: 100,
        ..Default::default()
    }));

    let mut handles = vec![];

    // Spawn multiple threads recording failures
    for _ in 0..10 {
        let cb_clone = cb.clone();
        handles.push(thread::spawn(move || {
            for _ in 0..10 {
                cb_clone.record_failure();
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Should have exactly 100 failures
    assert_eq!(cb.failure_count(), 100);
    assert_eq!(cb.state(), CircuitState::Open);
}

#[test]
fn test_default_config() {
    let config = CircuitBreakerConfig::default();

    assert_eq!(config.failure_threshold, 5);
    assert_eq!(config.reset_timeout, Duration::from_secs(30));
    assert_eq!(config.success_threshold, 3);
    assert_eq!(config.failure_window, Duration::from_secs(60));
}

#[test]
fn test_error_display() {
    let open_error: CircuitBreakerError<std::io::Error> = CircuitBreakerError::CircuitOpen;
    assert_eq!(format!("{}", open_error), "Circuit breaker is open");

    let inner_error = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
    let failed_error: CircuitBreakerError<std::io::Error> =
        CircuitBreakerError::OperationFailed(inner_error);
    assert!(format!("{}", failed_error).contains("Operation failed"));
}
