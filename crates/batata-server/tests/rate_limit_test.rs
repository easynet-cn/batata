//! Integration tests for Rate Limiting
//!
//! Tests rate limiting functionality including API and authentication rate limits.

use std::time::Duration;

use batata_server::middleware::rate_limit::{
    AuthRateLimitConfig, AuthRateLimiter, RateLimitConfig,
};

// ============================================================================
// RateLimitConfig Tests
// ============================================================================

#[test]
fn test_rate_limit_config_default() {
    let config = RateLimitConfig::default();

    assert_eq!(config.max_requests, 100);
    assert_eq!(config.window_duration, Duration::from_secs(60));
    assert!(config.enabled);
}

#[test]
fn test_rate_limit_config_custom() {
    let config = RateLimitConfig {
        max_requests: 50,
        window_duration: Duration::from_secs(30),
        enabled: false,
    };

    assert_eq!(config.max_requests, 50);
    assert_eq!(config.window_duration, Duration::from_secs(30));
    assert!(!config.enabled);
}

// ============================================================================
// AuthRateLimitConfig Tests
// ============================================================================

#[test]
fn test_auth_rate_limit_config_default() {
    let config = AuthRateLimitConfig::default();

    assert_eq!(config.max_attempts, 5);
    assert_eq!(config.window_duration, Duration::from_secs(60));
    assert_eq!(config.lockout_duration, Duration::from_secs(300));
    assert!(config.enabled);
}

#[test]
fn test_auth_rate_limit_config_custom() {
    let config = AuthRateLimitConfig {
        max_attempts: 3,
        window_duration: Duration::from_secs(120),
        lockout_duration: Duration::from_secs(600),
        enabled: true,
    };

    assert_eq!(config.max_attempts, 3);
    assert_eq!(config.window_duration, Duration::from_secs(120));
    assert_eq!(config.lockout_duration, Duration::from_secs(600));
}

// ============================================================================
// AuthRateLimiter Tests
// ============================================================================

#[test]
fn test_auth_rate_limiter_basic() {
    let config = AuthRateLimitConfig {
        max_attempts: 3,
        window_duration: Duration::from_secs(60),
        lockout_duration: Duration::from_secs(300),
        enabled: true,
    };
    let limiter = AuthRateLimiter::new(config);

    // First 2 attempts should be allowed with decreasing remaining
    let (allowed, remaining, lockout) = limiter.record_attempt("test-user");
    assert!(allowed);
    assert_eq!(remaining, 2);
    assert_eq!(lockout, 0);

    let (allowed, remaining, lockout) = limiter.record_attempt("test-user");
    assert!(allowed);
    assert_eq!(remaining, 1);
    assert_eq!(lockout, 0);

    // 3rd attempt triggers lockout
    let (allowed, remaining, lockout) = limiter.record_attempt("test-user");
    assert!(!allowed);
    assert_eq!(remaining, 0);
    assert!(lockout > 0);

    // Subsequent attempts should be denied
    let (allowed, _, lockout) = limiter.record_attempt("test-user");
    assert!(!allowed);
    assert!(lockout > 0);
}

#[test]
fn test_auth_rate_limiter_check_vs_record() {
    let config = AuthRateLimitConfig {
        max_attempts: 3,
        window_duration: Duration::from_secs(60),
        lockout_duration: Duration::from_secs(300),
        enabled: true,
    };
    let limiter = AuthRateLimiter::new(config);

    // Check doesn't consume attempts
    let (allowed, remaining, _) = limiter.check_attempt("test-user");
    assert!(allowed);
    assert_eq!(remaining, 3);

    // Still 3 remaining
    let (allowed, remaining, _) = limiter.check_attempt("test-user");
    assert!(allowed);
    assert_eq!(remaining, 3);

    // Record consumes an attempt
    let (allowed, remaining, _) = limiter.record_attempt("test-user");
    assert!(allowed);
    assert_eq!(remaining, 2);

    // Check shows updated remaining
    let (allowed, remaining, _) = limiter.check_attempt("test-user");
    assert!(allowed);
    assert_eq!(remaining, 2);
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

    // Verify remaining is 1
    let (_, remaining, _) = limiter.check_attempt("test-user");
    assert_eq!(remaining, 1);

    // Successful login resets
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
        let (allowed, remaining, lockout) = limiter.record_attempt("test-user");
        assert!(allowed);
        assert_eq!(remaining, 1); // Returns max_attempts when disabled
        assert_eq!(lockout, 0);
    }
}

#[test]
fn test_auth_rate_limiter_different_users() {
    let config = AuthRateLimitConfig {
        max_attempts: 3,
        window_duration: Duration::from_secs(60),
        lockout_duration: Duration::from_secs(300),
        enabled: true,
    };
    let limiter = AuthRateLimiter::new(config);

    // user-1 uses some attempts
    limiter.record_attempt("user-1");
    limiter.record_attempt("user-1");

    // user-1 should have 1 remaining
    let (_, remaining, _) = limiter.check_attempt("user-1");
    assert_eq!(remaining, 1);

    // user-2 should have full attempts (independent limit)
    let (allowed, remaining, _) = limiter.check_attempt("user-2");
    assert!(allowed);
    assert_eq!(remaining, 3);
}

#[test]
fn test_auth_rate_limiter_cleanup() {
    let config = AuthRateLimitConfig {
        max_attempts: 3,
        window_duration: Duration::from_secs(60),
        lockout_duration: Duration::from_secs(300),
        enabled: true,
    };
    let limiter = AuthRateLimiter::new(config);

    // Create some entries
    limiter.record_attempt("user-1");
    limiter.record_attempt("user-2");
    limiter.record_attempt("user-3");

    // Cleanup should not panic
    limiter.cleanup();
}

// ============================================================================
// Concurrent Tests
// ============================================================================

#[test]
fn test_auth_rate_limiter_concurrent_access() {
    use std::sync::Arc;
    use std::thread;

    let config = AuthRateLimitConfig {
        max_attempts: 100,
        window_duration: Duration::from_secs(60),
        lockout_duration: Duration::from_secs(300),
        enabled: true,
    };

    let limiter = Arc::new(AuthRateLimiter::new(config));
    let mut handles = vec![];

    // Multiple threads trying to use the same user's rate limit
    for _ in 0..10 {
        let limiter_clone = limiter.clone();
        handles.push(thread::spawn(move || {
            let mut allowed_count = 0;
            for _ in 0..20 {
                let (allowed, _, _) = limiter_clone.record_attempt("shared-user");
                if allowed {
                    allowed_count += 1;
                }
            }
            allowed_count
        }));
    }

    let total_allowed: usize = handles.into_iter().map(|h| h.join().unwrap()).sum();

    // Only 100 attempts should be allowed total (before lockout)
    // Note: Due to race conditions, it might be slightly less
    assert!(total_allowed <= 100);
    assert!(total_allowed >= 95); // Allow some margin for timing
}

#[test]
fn test_auth_rate_limiter_concurrent_different_users() {
    use std::sync::Arc;
    use std::thread;

    let config = AuthRateLimitConfig {
        max_attempts: 5,
        window_duration: Duration::from_secs(60),
        lockout_duration: Duration::from_secs(300),
        enabled: true,
    };

    let limiter = Arc::new(AuthRateLimiter::new(config));
    let mut handles = vec![];

    // Each thread has its own user
    for i in 0..10 {
        let limiter_clone = limiter.clone();
        handles.push(thread::spawn(move || {
            let user = format!("user-{}", i);
            let mut allowed = 0;
            for _ in 0..10 {
                let (is_allowed, _, _) = limiter_clone.record_attempt(&user);
                if is_allowed {
                    allowed += 1;
                }
            }
            allowed
        }));
    }

    let results: Vec<usize> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    // Each user should get exactly max_attempts - 1 allowed (5th attempt triggers lockout)
    for &allowed in &results {
        assert_eq!(allowed, 4, "Each user should get 4 allowed attempts before lockout (5th triggers)");
    }
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_auth_rate_limiter_empty_key() {
    let config = AuthRateLimitConfig {
        max_attempts: 2,
        window_duration: Duration::from_secs(60),
        lockout_duration: Duration::from_secs(300),
        enabled: true,
    };
    let limiter = AuthRateLimiter::new(config);

    // Empty string is still a valid key
    let (allowed, remaining, _) = limiter.record_attempt("");
    assert!(allowed);
    assert_eq!(remaining, 1);
}

#[test]
fn test_auth_rate_limiter_special_characters_in_key() {
    let config = AuthRateLimitConfig {
        max_attempts: 2,
        window_duration: Duration::from_secs(60),
        lockout_duration: Duration::from_secs(300),
        enabled: true,
    };
    let limiter = AuthRateLimiter::new(config);
    let key = "user@domain.com/path#hash?query=1";

    let (allowed, remaining, _) = limiter.record_attempt(key);
    assert!(allowed);
    assert_eq!(remaining, 1);
}

#[test]
fn test_auth_rate_limiter_unicode_key() {
    let config = AuthRateLimitConfig {
        max_attempts: 2,
        window_duration: Duration::from_secs(60),
        lockout_duration: Duration::from_secs(300),
        enabled: true,
    };
    let limiter = AuthRateLimiter::new(config);
    let key = "用户名_ユーザー_🚀";

    let (allowed, remaining, _) = limiter.record_attempt(key);
    assert!(allowed);
    assert_eq!(remaining, 1);
}

#[test]
fn test_auth_rate_limiter_zero_max_attempts() {
    let config = AuthRateLimitConfig {
        max_attempts: 0,
        window_duration: Duration::from_secs(60),
        lockout_duration: Duration::from_secs(300),
        enabled: true,
    };
    let limiter = AuthRateLimiter::new(config);

    // With 0 max attempts, first attempt should trigger lockout
    let (allowed, _, lockout) = limiter.record_attempt("user");
    assert!(!allowed);
    assert!(lockout > 0);
}
