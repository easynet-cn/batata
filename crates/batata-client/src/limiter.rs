//! Rate limiter for API requests
//!
//! Provides token bucket and sliding window rate limiting.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// Rate limiter using token bucket algorithm
#[derive(Clone)]
pub struct RateLimiter {
    capacity: u64,
    tokens: Arc<AtomicU64>,
    rate: Duration,
    last_refill: Arc<Mutex<Instant>>,
}

impl RateLimiter {
    /// Create a new rate limiter
    ///
    /// # Arguments
    /// * `capacity` - Maximum number of tokens
    /// * `rate_per_sec` - Tokens added per second
    pub fn new(capacity: u64, rate_per_sec: f64) -> Self {
        let rate = Duration::from_secs_f64(1.0 / rate_per_sec);
        Self {
            capacity,
            tokens: Arc::new(AtomicU64::new(capacity)),
            rate,
            last_refill: Arc::new(Mutex::new(Instant::now())),
        }
    }

    /// Try to acquire a token
    ///
    /// Returns true if a token was acquired, false otherwise
    pub async fn try_acquire(&self) -> bool {
        let mut last_refill = self.last_refill.lock().await;
        let now = Instant::now();
        let elapsed = now.duration_since(*last_refill);

        // Refill tokens based on elapsed time
        if elapsed >= self.rate {
            let tokens_to_add = elapsed.as_secs_f64() / self.rate.as_secs_f64() as f64;
            let current = self.tokens.load(Ordering::Relaxed);
            let new_tokens = (current as f64 + tokens_to_add).floor() as u64;
            self.tokens
                .store(new_tokens.min(self.capacity), Ordering::Relaxed);
            *last_refill = now;
        }

        // Try to acquire a token
        let current = self.tokens.load(Ordering::Relaxed);
        if current > 0 {
            self.tokens.fetch_sub(1, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    /// Acquire a token, blocking if necessary
    pub async fn acquire(&self) {
        while !self.try_acquire().await {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    /// Get current token count
    pub fn current_tokens(&self) -> u64 {
        self.tokens.load(Ordering::Relaxed)
    }

    /// Reset the limiter
    pub async fn reset(&self) {
        let mut last_refill = self.last_refill.lock().await;
        self.tokens.store(self.capacity, Ordering::Relaxed);
        *last_refill = Instant::now();
    }
}

/// Sliding window rate limiter
#[derive(Clone)]
pub struct SlidingWindowLimiter {
    max_requests: u64,
    window: Duration,
    requests: Arc<Mutex<Vec<Instant>>>,
}

impl SlidingWindowLimiter {
    /// Create a new sliding window rate limiter
    ///
    /// # Arguments
    /// * `max_requests` - Maximum number of requests in the window
    /// * `window_duration_secs` - Window duration in seconds
    pub fn new(max_requests: u64, window_duration_secs: u64) -> Self {
        Self {
            max_requests,
            window: Duration::from_secs(window_duration_secs),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Try to allow a request
    pub async fn try_allow(&self) -> bool {
        let mut requests = self.requests.lock().await;
        let now = Instant::now();

        // Remove requests outside the window
        requests.retain(|&req| now.duration_since(req) < self.window);

        // Check if under limit
        if requests.len() < self.max_requests as usize {
            requests.push(now);
            true
        } else {
            false
        }
    }

    /// Get current request count in the window
    pub async fn current_count(&self) -> usize {
        let requests = self.requests.lock().await;
        let now = Instant::now();
        requests
            .iter()
            .filter(|&&req| now.duration_since(req) < self.window)
            .count()
    }

    /// Reset the limiter
    pub async fn reset(&self) {
        let mut requests = self.requests.lock().await;
        requests.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiter_basic() {
        let limiter = RateLimiter::new(10, 100.0); // 10 tokens, 100/sec = 10ms per token

        // Should be able to acquire 10 tokens immediately
        for _ in 0..10 {
            assert!(limiter.try_acquire().await);
        }

        // Next acquire should fail
        assert!(!limiter.try_acquire().await);

        // Wait for token refill
        tokio::time::sleep(Duration::from_millis(20)).await;

        // Should be able to acquire again
        assert!(limiter.try_acquire().await);
    }

    #[tokio::test]
    async fn test_rate_limiter_acquire_blocking() {
        let limiter = RateLimiter::new(1, 10.0); // 1 token, 10/sec

        // Acquire the token
        assert!(limiter.try_acquire().await);

        // Acquire should block and eventually succeed
        let start = Instant::now();
        limiter.acquire().await;
        let elapsed = start.elapsed();

        // Should have waited at least ~100ms
        assert!(elapsed >= Duration::from_millis(90));
    }

    #[tokio::test]
    async fn test_sliding_window_limiter() {
        let limiter = SlidingWindowLimiter::new(5, 1); // 5 requests per second

        // Allow 5 requests
        for _ in 0..5 {
            assert!(limiter.try_allow().await);
        }

        // 6th request should be denied
        assert!(!limiter.try_allow().await);

        // Wait for window to slide
        tokio::time::sleep(Duration::from_secs(1)).await;

        // Should be able to allow again
        assert!(limiter.try_allow().await);
    }

    #[tokio::test]
    async fn test_sliding_window_count() {
        let limiter = SlidingWindowLimiter::new(10, 1); // 10 requests per 1 second

        // Allow 5 requests
        for _ in 0..5 {
            limiter.try_allow().await;
        }

        let count = limiter.current_count().await;
        assert_eq!(count, 5);

        // Wait for window to slide
        tokio::time::sleep(Duration::from_secs(1)).await;

        let count = limiter.current_count().await;
        assert_eq!(count, 0);
    }
}
