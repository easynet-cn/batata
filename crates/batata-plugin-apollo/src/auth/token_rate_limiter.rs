//! Per-token-prefix rate limiting for Apollo user tokens.
//!
//! Upstream `UserTokenAuthenticationFilter` keeps a Guava `RateLimiter` cache
//! keyed by `tokenPrefix`, with one-hour idle expiry and a 20,000 entry cap.
//! This module reproduces those semantics with a bounded token bucket so no
//! extra runtime dependency is needed. The bucket refills at `rate_limit`
//! permits per second, which is what `RateLimiter.create(permitsPerSecond)`
//! means upstream.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Maximum number of cached buckets (upstream `RATE_LIMITER_CACHE_MAX_SIZE`).
pub const MAX_LIMITERS: usize = 20_000;
/// Idle expiry of a cached bucket (upstream `expireAfterAccess(1, HOURS)`).
pub const IDLE_EXPIRY: Duration = Duration::from_secs(60 * 60);

#[derive(Debug, Clone, Copy)]
struct Bucket {
    permits: f64,
    last_refill: Instant,
}

/// Bounded, process-wide limiter registry keyed by token prefix.
#[derive(Debug, Default)]
pub struct TokenRateLimiter {
    buckets: Mutex<HashMap<String, Bucket>>,
}

impl TokenRateLimiter {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Attempts to consume one permit for `prefix` at `rate_limit` per second.
    ///
    /// A non-positive rate means unlimited and always succeeds. The first
    /// request starts with a full bucket so a freshly issued token may make
    /// one immediate call, matching Guava's initial permit behavior closely
    /// enough for request throttling.
    pub fn try_acquire(&self, prefix: &str, rate_limit: i32) -> bool {
        if rate_limit <= 0 {
            return true;
        }
        let now = Instant::now();
        let rate = f64::from(rate_limit);
        let mut buckets = match self.buckets.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if buckets.len() >= MAX_LIMITERS && !buckets.contains_key(prefix) {
            let cutoff = now.checked_sub(IDLE_EXPIRY).unwrap_or(now);
            buckets.retain(|_, bucket| bucket.last_refill > cutoff);
            if buckets.len() >= MAX_LIMITERS {
                return false;
            }
        }
        let bucket = buckets.entry(prefix.to_string()).or_insert(Bucket {
            permits: rate,
            last_refill: now,
        });
        let elapsed = now
            .saturating_duration_since(bucket.last_refill)
            .as_secs_f64();
        bucket.permits = (bucket.permits + elapsed * rate).min(rate);
        bucket.last_refill = now;
        if bucket.permits >= 1.0 {
            bucket.permits -= 1.0;
            true
        } else {
            false
        }
    }

    /// Removes a token's limiter, used when a token is revoked or rotated.
    pub fn remove(&self, prefix: &str) {
        if let Ok(mut buckets) = self.buckets.lock() {
            buckets.remove(prefix);
        }
    }
}

/// Shared limiter used by the OpenAPI auth middleware and token services.
pub static LIMITER: std::sync::LazyLock<TokenRateLimiter> =
    std::sync::LazyLock::new(TokenRateLimiter::new);

#[cfg(test)]
mod tests {
    use super::TokenRateLimiter;

    #[test]
    fn non_positive_rate_is_unlimited() {
        let limiter = TokenRateLimiter::new();
        for _ in 0..100 {
            assert!(limiter.try_acquire("unlimited", 0));
        }
    }

    #[test]
    fn bounded_rate_rejects_burst() {
        let limiter = TokenRateLimiter::new();
        assert!(limiter.try_acquire("limited", 2));
        assert!(limiter.try_acquire("limited", 2));
        assert!(!limiter.try_acquire("limited", 2));
    }

    #[test]
    fn limiters_are_per_prefix() {
        let limiter = TokenRateLimiter::new();
        assert!(limiter.try_acquire("a", 1));
        assert!(!limiter.try_acquire("a", 1));
        assert!(limiter.try_acquire("b", 1));
        limiter.remove("a");
        assert!(limiter.try_acquire("a", 1));
    }
}
