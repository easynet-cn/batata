// Rate limiting middleware for API protection
// Uses token bucket algorithm with configurable limits per IP

use std::future::{Future, Ready, ready};
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use actix_web::{
    Error, HttpResponse,
    body::EitherBody,
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    http::StatusCode,
};
use dashmap::DashMap;
use serde::Serialize;

/// Rate limiter configuration
#[derive(Clone)]
pub struct RateLimitConfig {
    /// Maximum requests per window
    pub max_requests: u32,
    /// Time window duration
    pub window_duration: Duration,
    /// Whether rate limiting is enabled
    pub enabled: bool,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_requests: 100,
            window_duration: Duration::from_secs(60),
            enabled: true,
        }
    }
}

/// Token bucket for rate limiting
struct TokenBucket {
    tokens: u32,
    last_refill: Instant,
    max_tokens: u32,
    refill_interval: Duration,
}

impl TokenBucket {
    fn new(max_tokens: u32, refill_interval: Duration) -> Self {
        Self {
            tokens: max_tokens,
            last_refill: Instant::now(),
            max_tokens,
            refill_interval,
        }
    }

    fn try_consume(&mut self) -> bool {
        self.refill();
        if self.tokens > 0 {
            self.tokens -= 1;
            true
        } else {
            false
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill);
        if elapsed >= self.refill_interval {
            self.tokens = self.max_tokens;
            self.last_refill = now;
        }
    }

    fn remaining(&self) -> u32 {
        self.tokens
    }
}

/// Rate limiter state shared across requests
pub struct RateLimiterState {
    buckets: DashMap<String, TokenBucket>,
    config: RateLimitConfig,
}

impl RateLimiterState {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            buckets: DashMap::new(),
            config,
        }
    }

    fn check_rate_limit(&self, key: &str) -> (bool, u32) {
        if !self.config.enabled {
            return (true, self.config.max_requests);
        }

        let mut bucket = self.buckets.entry(key.to_string()).or_insert_with(|| {
            TokenBucket::new(self.config.max_requests, self.config.window_duration)
        });

        let allowed = bucket.try_consume();
        let remaining = bucket.remaining();
        (allowed, remaining)
    }

    /// Clean up old entries periodically
    pub fn cleanup(&self) {
        let now = Instant::now();
        self.buckets.retain(|_, bucket| {
            now.duration_since(bucket.last_refill) < self.config.window_duration * 2
        });
    }
}

/// Rate limiting middleware factory
pub struct RateLimiter {
    state: Arc<RateLimiterState>,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            state: Arc::new(RateLimiterState::new(config)),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(RateLimitConfig::default())
    }
}

impl<S, B> Transform<S, ServiceRequest> for RateLimiter
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Transform = RateLimiterMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RateLimiterMiddleware {
            service,
            state: self.state.clone(),
        }))
    }
}

pub struct RateLimiterMiddleware<S> {
    service: S,
    state: Arc<RateLimiterState>,
}

#[derive(Serialize)]
struct RateLimitError {
    code: i32,
    message: String,
    retry_after: u64,
}

impl<S, B> Service<ServiceRequest> for RateLimiterMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(
        &self,
        ctx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(ctx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        // Get client IP for rate limiting key
        let client_ip = req
            .connection_info()
            .realip_remote_addr()
            .unwrap_or("unknown")
            .to_string();

        let (allowed, remaining) = self.state.check_rate_limit(&client_ip);

        if !allowed {
            let response = HttpResponse::build(StatusCode::TOO_MANY_REQUESTS)
                .insert_header((
                    "X-RateLimit-Limit",
                    self.state.config.max_requests.to_string(),
                ))
                .insert_header(("X-RateLimit-Remaining", "0"))
                .insert_header((
                    "Retry-After",
                    self.state.config.window_duration.as_secs().to_string(),
                ))
                .json(RateLimitError {
                    code: 429,
                    message: "Too many requests. Please try again later.".to_string(),
                    retry_after: self.state.config.window_duration.as_secs(),
                });

            return Box::pin(async move { Ok(req.into_response(response).map_into_right_body()) });
        }

        let fut = self.service.call(req);
        let max_requests = self.state.config.max_requests;

        Box::pin(async move {
            let mut res = fut.await?;

            // Add rate limit headers to response
            res.headers_mut().insert(
                actix_web::http::header::HeaderName::from_static("x-ratelimit-limit"),
                actix_web::http::header::HeaderValue::from_str(&max_requests.to_string())
                    .unwrap_or_else(|_| actix_web::http::header::HeaderValue::from_static("0")),
            );
            res.headers_mut().insert(
                actix_web::http::header::HeaderName::from_static("x-ratelimit-remaining"),
                actix_web::http::header::HeaderValue::from_str(&remaining.to_string())
                    .unwrap_or_else(|_| actix_web::http::header::HeaderValue::from_static("0")),
            );

            Ok(res.map_into_left_body())
        })
    }
}

// ============================================================================
// Authentication Rate Limiter
// Specialized rate limiter for login attempts to prevent brute force attacks
// ============================================================================

/// Authentication rate limiter configuration
#[derive(Clone)]
pub struct AuthRateLimitConfig {
    /// Maximum login attempts per window
    pub max_attempts: u32,
    /// Time window duration for attempt counting
    pub window_duration: Duration,
    /// Lockout duration after exceeding max attempts
    pub lockout_duration: Duration,
    /// Whether auth rate limiting is enabled
    pub enabled: bool,
}

impl Default for AuthRateLimitConfig {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            window_duration: Duration::from_secs(60),
            lockout_duration: Duration::from_secs(300), // 5 minutes lockout
            enabled: true,
        }
    }
}

/// Entry tracking login attempts
struct AuthAttemptEntry {
    attempts: u32,
    first_attempt: Instant,
    locked_until: Option<Instant>,
}

impl AuthAttemptEntry {
    fn new() -> Self {
        Self {
            attempts: 0,
            first_attempt: Instant::now(),
            locked_until: None,
        }
    }

    fn is_locked(&self) -> bool {
        if let Some(locked_until) = self.locked_until {
            Instant::now() < locked_until
        } else {
            false
        }
    }

    fn remaining_lockout_secs(&self) -> u64 {
        if let Some(locked_until) = self.locked_until {
            let now = Instant::now();
            if now < locked_until {
                return (locked_until - now).as_secs();
            }
        }
        0
    }
}

/// Authentication rate limiter for protecting login endpoints
pub struct AuthRateLimiter {
    entries: DashMap<String, AuthAttemptEntry>,
    config: AuthRateLimitConfig,
}

impl AuthRateLimiter {
    pub fn new(config: AuthRateLimitConfig) -> Self {
        Self {
            entries: DashMap::new(),
            config,
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(AuthRateLimitConfig::default())
    }

    /// Check if a login attempt is allowed for the given key (IP or username)
    /// Returns (allowed, remaining_attempts, lockout_secs)
    pub fn check_attempt(&self, key: &str) -> (bool, u32, u64) {
        if !self.config.enabled {
            return (true, self.config.max_attempts, 0);
        }

        let mut entry = self
            .entries
            .entry(key.to_string())
            .or_insert_with(AuthAttemptEntry::new);

        // Check if currently locked out
        if entry.is_locked() {
            return (false, 0, entry.remaining_lockout_secs());
        }

        // Reset if window has passed
        let now = Instant::now();
        if now.duration_since(entry.first_attempt) >= self.config.window_duration {
            entry.attempts = 0;
            entry.first_attempt = now;
            entry.locked_until = None;
        }

        let remaining = self.config.max_attempts.saturating_sub(entry.attempts);
        (remaining > 0, remaining, 0)
    }

    /// Record a login attempt (call this BEFORE validation)
    /// Returns (allowed, remaining_attempts, lockout_secs)
    pub fn record_attempt(&self, key: &str) -> (bool, u32, u64) {
        if !self.config.enabled {
            return (true, self.config.max_attempts, 0);
        }

        let mut entry = self
            .entries
            .entry(key.to_string())
            .or_insert_with(AuthAttemptEntry::new);

        // Check if currently locked out
        if entry.is_locked() {
            return (false, 0, entry.remaining_lockout_secs());
        }

        // Reset if window has passed
        let now = Instant::now();
        if now.duration_since(entry.first_attempt) >= self.config.window_duration {
            entry.attempts = 0;
            entry.first_attempt = now;
            entry.locked_until = None;
        }

        entry.attempts += 1;

        // Check if we've exceeded max attempts
        if entry.attempts >= self.config.max_attempts {
            entry.locked_until = Some(now + self.config.lockout_duration);
            tracing::warn!(
                "Auth rate limit exceeded for key '{}', locked for {} seconds",
                key,
                self.config.lockout_duration.as_secs()
            );
            return (false, 0, self.config.lockout_duration.as_secs());
        }

        let remaining = self.config.max_attempts.saturating_sub(entry.attempts);
        (true, remaining, 0)
    }

    /// Record a successful login (resets the attempt counter)
    pub fn record_success(&self, key: &str) {
        if !self.config.enabled {
            return;
        }

        self.entries.remove(key);
    }

    /// Clean up old entries
    pub fn cleanup(&self) {
        let now = Instant::now();
        let max_age = self.config.window_duration + self.config.lockout_duration;

        self.entries.retain(|_, entry| {
            // Keep if locked or within window
            entry.is_locked() || now.duration_since(entry.first_attempt) < max_age
        });
    }
}

// Global auth rate limiter instance
use std::sync::LazyLock;

pub static AUTH_RATE_LIMITER: LazyLock<AuthRateLimiter> =
    LazyLock::new(AuthRateLimiter::with_defaults);

/// Check if login attempt is allowed for the given IP/username
pub fn check_auth_rate_limit(key: &str) -> (bool, u32, u64) {
    AUTH_RATE_LIMITER.check_attempt(key)
}

/// Record a login attempt
pub fn record_auth_attempt(key: &str) -> (bool, u32, u64) {
    AUTH_RATE_LIMITER.record_attempt(key)
}

/// Record a successful login
pub fn record_auth_success(key: &str) {
    AUTH_RATE_LIMITER.record_success(key)
}

/// Cleanup interval for rate limiter entries (5 minutes)
const CLEANUP_INTERVAL_SECS: u64 = 300;

/// Start a background task to periodically clean up expired rate limiter entries.
/// This prevents memory leaks from accumulating stale entries.
/// Returns a handle that can be used to abort the cleanup task.
pub fn start_cleanup_task() -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(CLEANUP_INTERVAL_SECS));
        loop {
            interval.tick().await;
            // Clean up auth rate limiter entries
            AUTH_RATE_LIMITER.cleanup();
            tracing::debug!("Rate limiter cleanup completed");
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_bucket() {
        let mut bucket = TokenBucket::new(5, Duration::from_secs(60));

        // Should allow 5 requests
        for _ in 0..5 {
            assert!(bucket.try_consume());
        }

        // 6th request should be denied
        assert!(!bucket.try_consume());
    }

    #[test]
    fn test_rate_limiter_state() {
        let config = RateLimitConfig {
            max_requests: 3,
            window_duration: Duration::from_secs(60),
            enabled: true,
        };
        let state = RateLimiterState::new(config);

        // First 3 requests should be allowed
        for _ in 0..3 {
            let (allowed, _) = state.check_rate_limit("test-ip");
            assert!(allowed);
        }

        // 4th request should be denied
        let (allowed, _) = state.check_rate_limit("test-ip");
        assert!(!allowed);
    }

    #[test]
    fn test_rate_limiter_disabled() {
        let config = RateLimitConfig {
            max_requests: 1,
            window_duration: Duration::from_secs(60),
            enabled: false,
        };
        let state = RateLimiterState::new(config);

        // Should allow unlimited requests when disabled
        for _ in 0..10 {
            let (allowed, _) = state.check_rate_limit("test-ip");
            assert!(allowed);
        }
    }

    #[test]
    fn test_auth_rate_limiter_basic() {
        let config = AuthRateLimitConfig {
            max_attempts: 3,
            window_duration: Duration::from_secs(60),
            lockout_duration: Duration::from_secs(300),
            enabled: true,
        };
        let limiter = AuthRateLimiter::new(config);

        // First 3 attempts should be allowed
        for i in 0..3 {
            let (allowed, remaining, _) = limiter.record_attempt("test-user");
            if i < 2 {
                assert!(allowed, "Attempt {} should be allowed", i + 1);
                assert_eq!(remaining, 2 - i as u32);
            } else {
                // 3rd attempt triggers lockout
                assert!(!allowed, "3rd attempt should trigger lockout");
            }
        }

        // 4th attempt should be denied due to lockout
        let (allowed, remaining, lockout_secs) = limiter.record_attempt("test-user");
        assert!(!allowed);
        assert_eq!(remaining, 0);
        assert!(lockout_secs > 0);
    }

    #[test]
    fn test_auth_rate_limiter_success_resets() {
        let config = AuthRateLimitConfig {
            max_attempts: 3,
            window_duration: Duration::from_secs(60),
            lockout_duration: Duration::from_secs(300),
            enabled: true,
        };
        let limiter = AuthRateLimiter::new(config);

        // Make 2 failed attempts
        limiter.record_attempt("test-user");
        limiter.record_attempt("test-user");

        // Successful login should reset
        limiter.record_success("test-user");

        // Should have full attempts again
        let (allowed, remaining, _) = limiter.check_attempt("test-user");
        assert!(allowed);
        assert_eq!(remaining, 3);
    }

    #[test]
    fn test_auth_rate_limiter_disabled() {
        let config = AuthRateLimitConfig {
            max_attempts: 1,
            window_duration: Duration::from_secs(60),
            lockout_duration: Duration::from_secs(300),
            enabled: false,
        };
        let limiter = AuthRateLimiter::new(config);

        // Should allow unlimited attempts when disabled
        for _ in 0..10 {
            let (allowed, _, _) = limiter.record_attempt("test-user");
            assert!(allowed);
        }
    }
}
