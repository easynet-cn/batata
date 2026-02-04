//! Control Plugin Service Implementation
//!
//! Provides:
//! - Control Plugin SPI (PLG-001)
//! - TPS Rate Limiting (PLG-002)
//! - Connection Limiting (PLG-003)

use async_trait::async_trait;
use dashmap::DashMap;
use regex::Regex;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use super::model::*;
use super::rule_store::{MemoryRuleStore, RuleStore};
use crate::Plugin;

/// Token bucket for TPS rate limiting (PLG-002)
#[derive(Debug)]
pub struct TokenBucket {
    /// Current token count (scaled by 1000 for precision)
    tokens: AtomicU64,
    /// Maximum tokens (scaled by 1000)
    max_tokens: u64,
    /// Tokens per second (scaled by 1000)
    tokens_per_second: u64,
    /// Last refill time
    last_refill: RwLock<Instant>,
}

impl TokenBucket {
    pub fn new(max_tps: u32, burst_size: u32) -> Self {
        let max_tokens = (burst_size as u64) * 1000;
        Self {
            tokens: AtomicU64::new(max_tokens),
            max_tokens,
            tokens_per_second: (max_tps as u64) * 1000,
            last_refill: RwLock::new(Instant::now()),
        }
    }

    pub async fn try_acquire(&self, count: u32) -> bool {
        self.refill().await;

        let required = (count as u64) * 1000;
        let mut current = self.tokens.load(Ordering::Relaxed);

        loop {
            if current < required {
                return false;
            }

            match self.tokens.compare_exchange_weak(
                current,
                current - required,
                Ordering::SeqCst,
                Ordering::Relaxed,
            ) {
                Ok(_) => return true,
                Err(new_current) => current = new_current,
            }
        }
    }

    async fn refill(&self) {
        let now = Instant::now();
        let mut last = self.last_refill.write().await;

        let elapsed = now.duration_since(*last);
        if elapsed.as_millis() < 1 {
            return;
        }

        let tokens_to_add = (elapsed.as_micros() as u64 * self.tokens_per_second) / 1_000_000;
        if tokens_to_add > 0 {
            let current = self.tokens.load(Ordering::Relaxed);
            let new_tokens = std::cmp::min(current + tokens_to_add, self.max_tokens);
            self.tokens.store(new_tokens, Ordering::Relaxed);
            *last = now;
        }
    }

    pub fn remaining(&self) -> u32 {
        (self.tokens.load(Ordering::Relaxed) / 1000) as u32
    }
}

/// Sliding window rate limiter
#[derive(Debug)]
pub struct SlidingWindowLimiter {
    /// Window size in seconds
    window_seconds: u64,
    /// Maximum requests per window
    max_requests: u32,
    /// Request timestamps (ring buffer of windows)
    windows: DashMap<u64, AtomicU32>,
}

impl SlidingWindowLimiter {
    pub fn new(max_requests: u32, window_seconds: u64) -> Self {
        Self {
            window_seconds,
            max_requests,
            windows: DashMap::new(),
        }
    }

    pub fn try_acquire(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let current_window = now / self.window_seconds;
        let prev_window = current_window.saturating_sub(1);

        // Get counts from current and previous window
        let current_count = self
            .windows
            .get(&current_window)
            .map(|v| v.load(Ordering::Relaxed))
            .unwrap_or(0);

        let prev_count = self
            .windows
            .get(&prev_window)
            .map(|v| v.load(Ordering::Relaxed))
            .unwrap_or(0);

        // Calculate weighted count (sliding window approximation)
        let elapsed_in_window = now % self.window_seconds;
        let weight = 1.0 - (elapsed_in_window as f64 / self.window_seconds as f64);
        let weighted_count = current_count + (prev_count as f64 * weight) as u32;

        if weighted_count >= self.max_requests {
            return false;
        }

        // Increment current window
        self.windows
            .entry(current_window)
            .or_insert_with(|| AtomicU32::new(0))
            .fetch_add(1, Ordering::Relaxed);

        // Cleanup old windows
        self.windows
            .retain(|k, _| *k >= prev_window.saturating_sub(1));

        true
    }

    pub fn remaining(&self) -> u32 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let current_window = now / self.window_seconds;
        let current_count = self
            .windows
            .get(&current_window)
            .map(|v| v.load(Ordering::Relaxed))
            .unwrap_or(0);

        self.max_requests.saturating_sub(current_count)
    }
}

/// Connection limiter for PLG-003
#[derive(Debug)]
pub struct ConnectionLimiter {
    /// Current connection count
    current: AtomicU32,
    /// Maximum connections
    max_connections: u32,
    /// Connections per IP
    per_ip: DashMap<String, AtomicU32>,
    /// Max connections per IP
    max_per_ip: u32,
    /// Connections per client
    per_client: DashMap<String, AtomicU32>,
    /// Max connections per client
    max_per_client: u32,
}

impl ConnectionLimiter {
    pub fn new(max: u32, max_per_ip: u32, max_per_client: u32) -> Self {
        Self {
            current: AtomicU32::new(0),
            max_connections: max,
            per_ip: DashMap::new(),
            max_per_ip,
            per_client: DashMap::new(),
            max_per_client,
        }
    }

    /// Try to acquire a connection slot
    pub fn try_acquire(&self, ip: Option<&str>, client_id: Option<&str>) -> ConnectionLimitResult {
        // Check global limit
        let current = self.current.load(Ordering::Relaxed);
        if current >= self.max_connections {
            return ConnectionLimitResult {
                allowed: false,
                current,
                limit: self.max_connections,
                action: ExceedAction::Reject,
                ..Default::default()
            };
        }

        // Check per-IP limit
        if let Some(ip) = ip {
            let ip_count = self
                .per_ip
                .entry(ip.to_string())
                .or_insert_with(|| AtomicU32::new(0))
                .load(Ordering::Relaxed);

            if ip_count >= self.max_per_ip {
                return ConnectionLimitResult {
                    allowed: false,
                    current: ip_count,
                    limit: self.max_per_ip,
                    action: ExceedAction::Reject,
                    ..Default::default()
                };
            }
        }

        // Check per-client limit
        if let Some(client_id) = client_id {
            let client_count = self
                .per_client
                .entry(client_id.to_string())
                .or_insert_with(|| AtomicU32::new(0))
                .load(Ordering::Relaxed);

            if client_count >= self.max_per_client {
                return ConnectionLimitResult {
                    allowed: false,
                    current: client_count,
                    limit: self.max_per_client,
                    action: ExceedAction::Reject,
                    ..Default::default()
                };
            }
        }

        // Acquire the connection
        self.current.fetch_add(1, Ordering::Relaxed);

        if let Some(ip) = ip {
            self.per_ip
                .entry(ip.to_string())
                .or_insert_with(|| AtomicU32::new(0))
                .fetch_add(1, Ordering::Relaxed);
        }

        if let Some(client_id) = client_id {
            self.per_client
                .entry(client_id.to_string())
                .or_insert_with(|| AtomicU32::new(0))
                .fetch_add(1, Ordering::Relaxed);
        }

        ConnectionLimitResult {
            allowed: true,
            current: self.current.load(Ordering::Relaxed),
            limit: self.max_connections,
            action: ExceedAction::Reject,
            ..Default::default()
        }
    }

    /// Release a connection slot
    pub fn release(&self, ip: Option<&str>, client_id: Option<&str>) {
        self.current.fetch_sub(1, Ordering::Relaxed);

        if let Some(ip) = ip {
            if let Some(counter) = self.per_ip.get(ip) {
                let prev = counter.fetch_sub(1, Ordering::Relaxed);
                if prev <= 1 {
                    drop(counter);
                    self.per_ip.remove(ip);
                }
            }
        }

        if let Some(client_id) = client_id {
            if let Some(counter) = self.per_client.get(client_id) {
                let prev = counter.fetch_sub(1, Ordering::Relaxed);
                if prev <= 1 {
                    drop(counter);
                    self.per_client.remove(client_id);
                }
            }
        }
    }

    pub fn current_count(&self) -> u32 {
        self.current.load(Ordering::Relaxed)
    }
}

/// Control Plugin SPI (PLG-001)
#[async_trait]
pub trait ControlPlugin: Plugin {
    /// Check rate limit for a request
    async fn check_rate_limit(&self, ctx: &ControlContext) -> RateLimitResult;

    /// Check connection limit
    async fn check_connection_limit(&self, ctx: &ControlContext) -> ConnectionLimitResult;

    /// Release a connection (call when connection closes)
    async fn release_connection(&self, ctx: &ControlContext);

    /// Get control statistics
    async fn get_stats(&self) -> ControlStats;

    /// Reload rules from storage
    async fn reload_rules(&self) -> anyhow::Result<()>;
}

/// Default control plugin implementation
pub struct DefaultControlPlugin {
    config: ControlPluginConfig,
    rule_store: Arc<dyn RuleStore>,
    /// Rate limiters keyed by rule ID
    rate_limiters: DashMap<String, Arc<TokenBucket>>,
    /// Sliding window limiters keyed by rule ID
    #[allow(dead_code)]
    sliding_limiters: DashMap<String, Arc<SlidingWindowLimiter>>,
    /// Connection limiter
    connection_limiter: ConnectionLimiter,
    /// Default rate limiter
    default_limiter: TokenBucket,
    /// Cached rate rules
    cached_rate_rules: RwLock<Vec<RateLimitRule>>,
    /// Cached connection rules
    #[allow(dead_code)]
    cached_connection_rules: RwLock<Vec<ConnectionLimitRule>>,
    /// Statistics
    #[allow(dead_code)]
    stats: ControlStats,
    #[allow(dead_code)]
    stats_lock: RwLock<()>,
    total_requests: AtomicU64,
    allowed_requests: AtomicU64,
    rate_limited: AtomicU64,
    connection_limited: AtomicU64,
    peak_connections: AtomicU32,
}

impl DefaultControlPlugin {
    pub fn new(config: ControlPluginConfig) -> Self {
        let rule_store: Arc<dyn RuleStore> = Arc::new(MemoryRuleStore::new());

        Self {
            connection_limiter: ConnectionLimiter::new(config.default_max_connections, 100, 10),
            default_limiter: TokenBucket::new(config.default_tps, config.default_tps),
            config,
            rule_store,
            rate_limiters: DashMap::new(),
            sliding_limiters: DashMap::new(),
            cached_rate_rules: RwLock::new(Vec::new()),
            cached_connection_rules: RwLock::new(Vec::new()),
            stats: ControlStats::default(),
            stats_lock: RwLock::new(()),
            total_requests: AtomicU64::new(0),
            allowed_requests: AtomicU64::new(0),
            rate_limited: AtomicU64::new(0),
            connection_limited: AtomicU64::new(0),
            peak_connections: AtomicU32::new(0),
        }
    }

    pub fn with_rule_store(mut self, store: Arc<dyn RuleStore>) -> Self {
        self.rule_store = store;
        self
    }

    fn match_rule_target(
        &self,
        rule_value: &str,
        target_value: &str,
        match_type: &RuleMatchType,
    ) -> bool {
        match match_type {
            RuleMatchType::All => true,
            RuleMatchType::Exact => rule_value == target_value,
            RuleMatchType::Prefix => target_value.starts_with(rule_value),
            RuleMatchType::Regex => Regex::new(rule_value)
                .map(|re| re.is_match(target_value))
                .unwrap_or(false),
        }
    }

    fn find_matching_rate_rule<'a>(
        &self,
        ctx: &ControlContext,
        rules: &'a [RateLimitRule],
    ) -> Option<&'a RateLimitRule> {
        rules.iter().find(|rule| {
            if !rule.enabled {
                return false;
            }

            let target_value = match ctx.get_target_value(&rule.target_type) {
                Some(v) => v,
                None => return false,
            };

            self.match_rule_target(&rule.target_value, target_value, &rule.match_type)
        })
    }

    #[allow(dead_code)]
    fn find_matching_connection_rule<'a>(
        &self,
        ctx: &ControlContext,
        rules: &'a [ConnectionLimitRule],
    ) -> Option<&'a ConnectionLimitRule> {
        rules.iter().find(|rule| {
            if !rule.enabled {
                return false;
            }

            let target_value = match ctx.get_target_value(&rule.target_type) {
                Some(v) => v,
                None => return false,
            };

            self.match_rule_target(&rule.target_value, target_value, &rule.match_type)
        })
    }

    fn get_or_create_rate_limiter(&self, rule: &RateLimitRule) -> Arc<TokenBucket> {
        self.rate_limiters
            .entry(rule.id.clone())
            .or_insert_with(|| Arc::new(TokenBucket::new(rule.max_tps, rule.burst_size)))
            .clone()
    }

    fn update_peak_connections(&self) {
        let current = self.connection_limiter.current_count();
        let mut peak = self.peak_connections.load(Ordering::Relaxed);
        while current > peak {
            match self.peak_connections.compare_exchange_weak(
                peak,
                current,
                Ordering::SeqCst,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(new_peak) => peak = new_peak,
            }
        }
    }
}

#[async_trait]
impl Plugin for DefaultControlPlugin {
    fn name(&self) -> &str {
        "control"
    }

    async fn init(&self) -> anyhow::Result<()> {
        self.reload_rules().await?;
        tracing::info!("Control plugin initialized");
        Ok(())
    }

    async fn shutdown(&self) -> anyhow::Result<()> {
        tracing::info!("Control plugin shutdown");
        Ok(())
    }
}

#[async_trait]
impl ControlPlugin for DefaultControlPlugin {
    async fn check_rate_limit(&self, ctx: &ControlContext) -> RateLimitResult {
        if !self.config.enabled {
            return RateLimitResult {
                allowed: true,
                ..Default::default()
            };
        }

        self.total_requests.fetch_add(1, Ordering::Relaxed);

        let rules = self.cached_rate_rules.read().await;

        let result = if let Some(rule) = self.find_matching_rate_rule(ctx, &rules) {
            let limiter = self.get_or_create_rate_limiter(rule);
            let allowed = limiter.try_acquire(1).await;

            if allowed {
                self.allowed_requests.fetch_add(1, Ordering::Relaxed);
            } else {
                self.rate_limited.fetch_add(1, Ordering::Relaxed);
            }

            RateLimitResult {
                allowed,
                remaining: limiter.remaining(),
                limit: rule.max_tps,
                reset_seconds: rule.window_seconds,
                matched_rule: Some(rule.id.clone()),
                action: rule.exceed_action.clone(),
                delay_ms: 0,
            }
        } else {
            // Use default limiter
            let allowed = self.default_limiter.try_acquire(1).await;

            if allowed {
                self.allowed_requests.fetch_add(1, Ordering::Relaxed);
            } else {
                self.rate_limited.fetch_add(1, Ordering::Relaxed);
            }

            RateLimitResult {
                allowed,
                remaining: self.default_limiter.remaining(),
                limit: self.config.default_tps,
                reset_seconds: 1,
                matched_rule: None,
                action: ExceedAction::Reject,
                delay_ms: 0,
            }
        };

        result
    }

    async fn check_connection_limit(&self, ctx: &ControlContext) -> ConnectionLimitResult {
        if !self.config.enabled {
            return ConnectionLimitResult {
                allowed: true,
                ..Default::default()
            };
        }

        let result = self
            .connection_limiter
            .try_acquire(ctx.client_ip.as_deref(), ctx.client_id.as_deref());

        if result.allowed {
            self.update_peak_connections();
        } else {
            self.connection_limited.fetch_add(1, Ordering::Relaxed);
        }

        result
    }

    async fn release_connection(&self, ctx: &ControlContext) {
        self.connection_limiter
            .release(ctx.client_ip.as_deref(), ctx.client_id.as_deref());
    }

    async fn get_stats(&self) -> ControlStats {
        let (rate_count, conn_count) = self.rule_store.get_counts().await.unwrap_or((0, 0));

        ControlStats {
            total_requests: self.total_requests.load(Ordering::Relaxed),
            allowed_requests: self.allowed_requests.load(Ordering::Relaxed),
            rate_limited_requests: self.rate_limited.load(Ordering::Relaxed),
            connection_limited_requests: self.connection_limited.load(Ordering::Relaxed),
            active_connections: self.connection_limiter.current_count(),
            peak_connections: self.peak_connections.load(Ordering::Relaxed),
            active_rate_rules: rate_count as u32,
            active_connection_rules: conn_count as u32,
        }
    }

    async fn reload_rules(&self) -> anyhow::Result<()> {
        let rate_rules = self.rule_store.list_rate_rules().await?;
        let connection_rules = self.rule_store.list_connection_rules().await?;

        *self.cached_rate_rules.write().await = rate_rules;
        *self.cached_connection_rules.write().await = connection_rules;

        tracing::debug!("Control rules reloaded");
        Ok(())
    }
}

/// Connection guard that automatically releases connection on drop
pub struct ConnectionGuard {
    plugin: Arc<dyn ControlPlugin>,
    ctx: ControlContext,
}

impl ConnectionGuard {
    pub fn new(plugin: Arc<dyn ControlPlugin>, ctx: ControlContext) -> Self {
        Self { plugin, ctx }
    }
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        let plugin = self.plugin.clone();
        let ctx = self.ctx.clone();
        tokio::spawn(async move {
            plugin.release_connection(&ctx).await;
        });
    }
}

/// Start background rule refresh task
pub fn start_rule_refresh_task(
    plugin: Arc<dyn ControlPlugin>,
    interval_seconds: u64,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(interval_seconds));
        loop {
            interval.tick().await;
            if let Err(e) = plugin.reload_rules().await {
                tracing::error!("Failed to reload control rules: {}", e);
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_token_bucket() {
        let bucket = TokenBucket::new(10, 10);

        // Should allow 10 requests
        for _ in 0..10 {
            assert!(bucket.try_acquire(1).await);
        }

        // 11th should fail
        assert!(!bucket.try_acquire(1).await);
    }

    #[tokio::test]
    async fn test_connection_limiter() {
        let limiter = ConnectionLimiter::new(5, 3, 2);

        // Acquire 5 connections
        for i in 0..5 {
            let result = limiter.try_acquire(Some(&format!("ip-{}", i % 2)), None);
            assert!(result.allowed, "Connection {} should be allowed", i);
        }

        // 6th should fail (global limit)
        let result = limiter.try_acquire(Some("new-ip"), None);
        assert!(!result.allowed);

        // Release one
        limiter.release(Some("ip-0"), None);

        // Now it should work
        let result = limiter.try_acquire(Some("new-ip"), None);
        assert!(result.allowed);
    }

    #[tokio::test]
    async fn test_per_ip_limit() {
        let limiter = ConnectionLimiter::new(100, 2, 100);

        // Same IP, should allow 2
        assert!(limiter.try_acquire(Some("192.168.1.1"), None).allowed);
        assert!(limiter.try_acquire(Some("192.168.1.1"), None).allowed);

        // 3rd from same IP should fail
        assert!(!limiter.try_acquire(Some("192.168.1.1"), None).allowed);

        // Different IP should work
        assert!(limiter.try_acquire(Some("192.168.1.2"), None).allowed);
    }

    #[tokio::test]
    async fn test_default_control_plugin() {
        let config = ControlPluginConfig {
            default_tps: 5,
            ..Default::default()
        };

        let plugin = DefaultControlPlugin::new(config);
        plugin.init().await.unwrap();

        let ctx = ControlContext::new().with_ip("192.168.1.1");

        // Should allow 5 requests
        for _ in 0..5 {
            let result = plugin.check_rate_limit(&ctx).await;
            assert!(result.allowed);
        }

        // 6th should be rate limited
        let result = plugin.check_rate_limit(&ctx).await;
        assert!(!result.allowed);

        let stats = plugin.get_stats().await;
        assert_eq!(stats.total_requests, 6);
        assert_eq!(stats.allowed_requests, 5);
        assert_eq!(stats.rate_limited_requests, 1);
    }

    #[test]
    fn test_sliding_window() {
        let limiter = SlidingWindowLimiter::new(5, 1);

        // Should allow 5 requests
        for _ in 0..5 {
            assert!(limiter.try_acquire());
        }

        // 6th should fail
        assert!(!limiter.try_acquire());
    }
}
