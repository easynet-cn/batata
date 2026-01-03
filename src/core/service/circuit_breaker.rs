// Circuit Breaker pattern implementation for resilient remote calls
// Provides protection against cascading failures in distributed systems

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::RwLock;
use std::time::{Duration, Instant};

/// Circuit breaker states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Circuit is closed, requests flow normally
    Closed,
    /// Circuit is open, requests are rejected
    Open,
    /// Circuit is half-open, limited requests allowed to test recovery
    HalfOpen,
}

/// Configuration for the circuit breaker
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of failures before opening the circuit
    pub failure_threshold: u32,
    /// Duration to wait before transitioning from Open to HalfOpen
    pub reset_timeout: Duration,
    /// Number of successful calls in HalfOpen state before closing
    pub success_threshold: u32,
    /// Time window for counting failures
    pub failure_window: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            reset_timeout: Duration::from_secs(30),
            success_threshold: 3,
            failure_window: Duration::from_secs(60),
        }
    }
}

/// Circuit breaker for protecting remote calls
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    state: RwLock<CircuitState>,
    failure_count: AtomicU32,
    success_count: AtomicU32,
    last_failure_time: AtomicU64,
    opened_at: RwLock<Option<Instant>>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker with default configuration
    pub fn new() -> Self {
        Self::with_config(CircuitBreakerConfig::default())
    }

    /// Create a new circuit breaker with custom configuration
    pub fn with_config(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: RwLock::new(CircuitState::Closed),
            failure_count: AtomicU32::new(0),
            success_count: AtomicU32::new(0),
            last_failure_time: AtomicU64::new(0),
            opened_at: RwLock::new(None),
        }
    }

    /// Check if a request should be allowed
    pub fn allow_request(&self) -> bool {
        let state = *self.state.read().unwrap_or_else(|e| e.into_inner());

        match state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if we should transition to half-open
                if let Some(opened_at) = *self.opened_at.read().unwrap_or_else(|e| e.into_inner())
                {
                    if opened_at.elapsed() >= self.config.reset_timeout {
                        self.transition_to_half_open();
                        return true;
                    }
                }
                false
            }
            CircuitState::HalfOpen => true,
        }
    }

    /// Record a successful call
    pub fn record_success(&self) {
        let state = *self.state.read().unwrap_or_else(|e| e.into_inner());

        match state {
            CircuitState::Closed => {
                // Reset failure count on success
                self.failure_count.store(0, Ordering::SeqCst);
            }
            CircuitState::HalfOpen => {
                let count = self.success_count.fetch_add(1, Ordering::SeqCst) + 1;
                if count >= self.config.success_threshold {
                    self.transition_to_closed();
                }
            }
            CircuitState::Open => {
                // Shouldn't happen, but ignore
            }
        }
    }

    /// Record a failed call
    pub fn record_failure(&self) {
        let state = *self.state.read().unwrap_or_else(|e| e.into_inner());

        match state {
            CircuitState::Closed => {
                let now = Instant::now();
                let last_failure = self.last_failure_time.load(Ordering::SeqCst);

                // Reset count if outside failure window
                if last_failure > 0 {
                    let elapsed = Duration::from_millis(
                        now.elapsed().as_millis() as u64 - last_failure,
                    );
                    if elapsed > self.config.failure_window {
                        self.failure_count.store(0, Ordering::SeqCst);
                    }
                }

                self.last_failure_time
                    .store(now.elapsed().as_millis() as u64, Ordering::SeqCst);

                let count = self.failure_count.fetch_add(1, Ordering::SeqCst) + 1;
                if count >= self.config.failure_threshold {
                    self.transition_to_open();
                }
            }
            CircuitState::HalfOpen => {
                // Single failure in half-open state opens the circuit again
                self.transition_to_open();
            }
            CircuitState::Open => {
                // Already open, ignore
            }
        }
    }

    /// Get the current state
    pub fn state(&self) -> CircuitState {
        *self.state.read().unwrap_or_else(|e| e.into_inner())
    }

    /// Get failure count
    pub fn failure_count(&self) -> u32 {
        self.failure_count.load(Ordering::SeqCst)
    }

    /// Reset the circuit breaker to closed state
    pub fn reset(&self) {
        self.transition_to_closed();
    }

    fn transition_to_open(&self) {
        if let Ok(mut state) = self.state.write() {
            *state = CircuitState::Open;
        }
        if let Ok(mut opened_at) = self.opened_at.write() {
            *opened_at = Some(Instant::now());
        }
        self.success_count.store(0, Ordering::SeqCst);
        tracing::warn!("Circuit breaker opened");
    }

    fn transition_to_half_open(&self) {
        if let Ok(mut state) = self.state.write() {
            *state = CircuitState::HalfOpen;
        }
        self.success_count.store(0, Ordering::SeqCst);
        tracing::info!("Circuit breaker half-open");
    }

    fn transition_to_closed(&self) {
        if let Ok(mut state) = self.state.write() {
            *state = CircuitState::Closed;
        }
        if let Ok(mut opened_at) = self.opened_at.write() {
            *opened_at = None;
        }
        self.failure_count.store(0, Ordering::SeqCst);
        self.success_count.store(0, Ordering::SeqCst);
        tracing::info!("Circuit breaker closed");
    }
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new()
    }
}

/// Execute a fallible operation with circuit breaker protection
pub async fn with_circuit_breaker<F, T, E>(
    circuit_breaker: &CircuitBreaker,
    operation: F,
) -> Result<T, CircuitBreakerError<E>>
where
    F: std::future::Future<Output = Result<T, E>>,
{
    if !circuit_breaker.allow_request() {
        return Err(CircuitBreakerError::CircuitOpen);
    }

    match operation.await {
        Ok(result) => {
            circuit_breaker.record_success();
            Ok(result)
        }
        Err(e) => {
            circuit_breaker.record_failure();
            Err(CircuitBreakerError::OperationFailed(e))
        }
    }
}

/// Error type for circuit breaker operations
#[derive(Debug)]
pub enum CircuitBreakerError<E> {
    /// The circuit is open, request rejected
    CircuitOpen,
    /// The underlying operation failed
    OperationFailed(E),
}

impl<E: std::fmt::Display> std::fmt::Display for CircuitBreakerError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CircuitBreakerError::CircuitOpen => write!(f, "Circuit breaker is open"),
            CircuitBreakerError::OperationFailed(e) => write!(f, "Operation failed: {}", e),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for CircuitBreakerError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CircuitBreakerError::CircuitOpen => None,
            CircuitBreakerError::OperationFailed(e) => Some(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker_initial_state() {
        let cb = CircuitBreaker::new();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.allow_request());
    }

    #[test]
    fn test_circuit_breaker_opens_after_failures() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            ..Default::default()
        };
        let cb = CircuitBreaker::with_config(config);

        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.allow_request());
    }

    #[test]
    fn test_circuit_breaker_success_resets_failures() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            ..Default::default()
        };
        let cb = CircuitBreaker::with_config(config);

        cb.record_failure();
        cb.record_failure();
        cb.record_success();
        assert_eq!(cb.failure_count(), 0);
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_circuit_breaker_reset() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            ..Default::default()
        };
        let cb = CircuitBreaker::with_config(config);

        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        cb.reset();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.allow_request());
    }
}
