//! Exponential backoff retry utility
//!
//! Provides configurable retry logic with exponential backoff and jitter,
//! compatible with Nacos RetryUtil patterns.

use std::future::Future;
use std::time::Duration;

use rand::Rng;

/// Configuration for retry behavior
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Initial delay before first retry
    pub initial_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Multiplier for each subsequent retry (e.g., 2.0 for doubling)
    pub multiplier: f64,
    /// Whether to add random jitter to delays
    pub jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
            multiplier: 2.0,
            jitter: true,
        }
    }
}

impl RetryConfig {
    pub fn new(max_retries: u32) -> Self {
        Self {
            max_retries,
            ..Default::default()
        }
    }

    pub fn with_initial_delay(mut self, delay: Duration) -> Self {
        self.initial_delay = delay;
        self
    }

    pub fn with_max_delay(mut self, delay: Duration) -> Self {
        self.max_delay = delay;
        self
    }

    pub fn with_multiplier(mut self, multiplier: f64) -> Self {
        self.multiplier = multiplier;
        self
    }

    pub fn without_jitter(mut self) -> Self {
        self.jitter = false;
        self
    }

    /// Calculate delay for a given attempt number (0-based)
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let base = self.initial_delay.as_millis() as f64 * self.multiplier.powi(attempt as i32);
        let capped = base.min(self.max_delay.as_millis() as f64);

        let delay_ms = if self.jitter {
            let jitter = rand::rng().random_range(0.0..=capped * 0.1);
            (capped + jitter) as u64
        } else {
            capped as u64
        };

        Duration::from_millis(delay_ms)
    }
}

/// Execute an async operation with retry logic.
///
/// The operation is retried on failure up to `config.max_retries` times
/// with exponential backoff between attempts.
pub async fn retry_with_backoff<F, Fut, T, E>(config: &RetryConfig, operation: F) -> Result<T, E>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let mut last_error = None;

    for attempt in 0..=config.max_retries {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                if attempt < config.max_retries {
                    let delay = config.delay_for_attempt(attempt);
                    tracing::debug!(
                        "Retry attempt {}/{} failed: {}. Waiting {:?}",
                        attempt + 1,
                        config.max_retries,
                        e,
                        delay
                    );
                    tokio::time::sleep(delay).await;
                }
                last_error = Some(e);
            }
        }
    }

    Err(last_error.unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.initial_delay, Duration::from_millis(500));
        assert_eq!(config.max_delay, Duration::from_secs(30));
        assert_eq!(config.multiplier, 2.0);
        assert!(config.jitter);
    }

    #[test]
    fn test_config_builder() {
        let config = RetryConfig::new(5)
            .with_initial_delay(Duration::from_secs(1))
            .with_max_delay(Duration::from_secs(60))
            .with_multiplier(3.0)
            .without_jitter();

        assert_eq!(config.max_retries, 5);
        assert_eq!(config.initial_delay, Duration::from_secs(1));
        assert_eq!(config.multiplier, 3.0);
        assert!(!config.jitter);
    }

    #[test]
    fn test_delay_calculation_no_jitter() {
        let config = RetryConfig::new(5)
            .with_initial_delay(Duration::from_millis(100))
            .with_multiplier(2.0)
            .without_jitter();

        assert_eq!(config.delay_for_attempt(0), Duration::from_millis(100));
        assert_eq!(config.delay_for_attempt(1), Duration::from_millis(200));
        assert_eq!(config.delay_for_attempt(2), Duration::from_millis(400));
        assert_eq!(config.delay_for_attempt(3), Duration::from_millis(800));
    }

    #[test]
    fn test_delay_capped_at_max() {
        let config = RetryConfig::new(10)
            .with_initial_delay(Duration::from_secs(1))
            .with_max_delay(Duration::from_secs(5))
            .with_multiplier(10.0)
            .without_jitter();

        assert_eq!(config.delay_for_attempt(0), Duration::from_secs(1));
        // 1 * 10^1 = 10s, capped to 5s
        assert_eq!(config.delay_for_attempt(1), Duration::from_secs(5));
        assert_eq!(config.delay_for_attempt(5), Duration::from_secs(5));
    }

    #[tokio::test]
    async fn test_retry_succeeds_first_attempt() {
        let config = RetryConfig::new(3).without_jitter();
        let result: Result<i32, String> = retry_with_backoff(&config, || async { Ok(42) }).await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_retry_succeeds_after_failures() {
        let config = RetryConfig::new(3)
            .with_initial_delay(Duration::from_millis(1))
            .without_jitter();

        let attempt = std::sync::atomic::AtomicU32::new(0);
        let result: Result<i32, String> = retry_with_backoff(&config, || {
            let current = attempt.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            async move {
                if current < 2 {
                    Err("not yet".to_string())
                } else {
                    Ok(42)
                }
            }
        })
        .await;

        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_retry_exhausted() {
        let config = RetryConfig::new(2)
            .with_initial_delay(Duration::from_millis(1))
            .without_jitter();

        let result: Result<i32, String> =
            retry_with_backoff(&config, || async { Err("always fails".to_string()) }).await;

        assert_eq!(result.unwrap_err(), "always fails");
    }
}
