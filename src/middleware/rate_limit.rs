// Rate limiting middleware for API protection
// Uses token bucket algorithm with configurable limits per IP

use std::collections::HashMap;
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
use serde::Serialize;
use std::sync::Mutex;

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
    buckets: Mutex<HashMap<String, TokenBucket>>,
    config: RateLimitConfig,
}

impl RateLimiterState {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            buckets: Mutex::new(HashMap::new()),
            config,
        }
    }

    fn check_rate_limit(&self, key: &str) -> (bool, u32) {
        if !self.config.enabled {
            return (true, self.config.max_requests);
        }

        let mut buckets = match self.buckets.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let bucket = buckets.entry(key.to_string()).or_insert_with(|| {
            TokenBucket::new(self.config.max_requests, self.config.window_duration)
        });

        let allowed = bucket.try_consume();
        let remaining = bucket.remaining();
        (allowed, remaining)
    }

    /// Clean up old entries periodically
    pub fn cleanup(&self) {
        let mut buckets = match self.buckets.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let now = Instant::now();
        buckets.retain(|_, bucket| {
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
}
