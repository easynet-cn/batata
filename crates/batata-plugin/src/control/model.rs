//! Control Plugin Data Models
//!
//! Defines the data structures for rate limiting and connection control rules.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Rule match type for identifying targets
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum RuleMatchType {
    /// Match by exact value
    #[default]
    Exact,
    /// Match by prefix
    Prefix,
    /// Match by regex pattern
    Regex,
    /// Match all (wildcard)
    All,
}

/// Target type for control rules
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum RuleTargetType {
    /// Target by client IP
    #[default]
    Ip,
    /// Target by client ID
    ClientId,
    /// Target by namespace
    Namespace,
    /// Target by service name
    Service,
    /// Target by API path
    ApiPath,
    /// Target by user
    User,
    /// Target by gRPC method
    GrpcMethod,
}

/// Rate limit algorithm type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum RateLimitAlgorithm {
    /// Token bucket algorithm (default)
    #[default]
    TokenBucket,
    /// Sliding window algorithm
    SlidingWindow,
    /// Fixed window algorithm
    FixedWindow,
    /// Leaky bucket algorithm
    LeakyBucket,
}

/// Rate limit rule configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitRule {
    /// Unique rule ID
    pub id: String,
    /// Rule name for display
    pub name: String,
    /// Rule description
    #[serde(default)]
    pub description: String,
    /// Whether the rule is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Rule priority (higher = more important)
    #[serde(default)]
    pub priority: i32,
    /// Target type for matching
    #[serde(default)]
    pub target_type: RuleTargetType,
    /// Match type for the target
    #[serde(default)]
    pub match_type: RuleMatchType,
    /// Target value (IP, client ID, etc.)
    pub target_value: String,
    /// Rate limit algorithm
    #[serde(default)]
    pub algorithm: RateLimitAlgorithm,
    /// Maximum requests per second (TPS)
    pub max_tps: u32,
    /// Maximum burst size (for token bucket)
    #[serde(default = "default_burst")]
    pub burst_size: u32,
    /// Time window in seconds
    #[serde(default = "default_window")]
    pub window_seconds: u64,
    /// Action when limit exceeded
    #[serde(default)]
    pub exceed_action: ExceedAction,
    /// Custom labels for grouping/filtering
    #[serde(default)]
    pub labels: HashMap<String, String>,
    /// Creation timestamp
    #[serde(default)]
    pub created_at: i64,
    /// Last update timestamp
    #[serde(default)]
    pub updated_at: i64,
}

fn default_true() -> bool {
    true
}

fn default_burst() -> u32 {
    100
}

fn default_window() -> u64 {
    1
}

impl Default for RateLimitRule {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            description: String::new(),
            enabled: true,
            priority: 0,
            target_type: RuleTargetType::Ip,
            match_type: RuleMatchType::All,
            target_value: String::new(),
            algorithm: RateLimitAlgorithm::TokenBucket,
            max_tps: 100,
            burst_size: 100,
            window_seconds: 1,
            exceed_action: ExceedAction::default(),
            labels: HashMap::new(),
            created_at: 0,
            updated_at: 0,
        }
    }
}

/// Connection limit rule configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionLimitRule {
    /// Unique rule ID
    pub id: String,
    /// Rule name for display
    pub name: String,
    /// Rule description
    #[serde(default)]
    pub description: String,
    /// Whether the rule is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Rule priority (higher = more important)
    #[serde(default)]
    pub priority: i32,
    /// Target type for matching
    #[serde(default)]
    pub target_type: RuleTargetType,
    /// Match type for the target
    #[serde(default)]
    pub match_type: RuleMatchType,
    /// Target value (IP, client ID, etc.)
    pub target_value: String,
    /// Maximum concurrent connections
    pub max_connections: u32,
    /// Maximum connections per IP
    #[serde(default = "default_connections_per_ip")]
    pub max_connections_per_ip: u32,
    /// Maximum connections per client ID
    #[serde(default = "default_connections_per_client")]
    pub max_connections_per_client: u32,
    /// Action when limit exceeded
    #[serde(default)]
    pub exceed_action: ExceedAction,
    /// Queue size for waiting connections (0 = no queue)
    #[serde(default)]
    pub queue_size: u32,
    /// Queue timeout in milliseconds
    #[serde(default = "default_queue_timeout")]
    pub queue_timeout_ms: u64,
    /// Custom labels for grouping/filtering
    #[serde(default)]
    pub labels: HashMap<String, String>,
    /// Creation timestamp
    #[serde(default)]
    pub created_at: i64,
    /// Last update timestamp
    #[serde(default)]
    pub updated_at: i64,
}

fn default_connections_per_ip() -> u32 {
    100
}

fn default_connections_per_client() -> u32 {
    10
}

fn default_queue_timeout() -> u64 {
    5000
}

impl Default for ConnectionLimitRule {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            description: String::new(),
            enabled: true,
            priority: 0,
            target_type: RuleTargetType::Ip,
            match_type: RuleMatchType::All,
            target_value: String::new(),
            max_connections: 10000,
            max_connections_per_ip: 100,
            max_connections_per_client: 10,
            exceed_action: ExceedAction::default(),
            queue_size: 0,
            queue_timeout_ms: 5000,
            labels: HashMap::new(),
            created_at: 0,
            updated_at: 0,
        }
    }
}

/// Action to take when limit is exceeded
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum ExceedAction {
    /// Reject the request immediately
    #[default]
    Reject,
    /// Queue the request and wait
    Queue,
    /// Allow but log a warning
    Warn,
    /// Delay the request (throttle)
    Delay,
}

/// Result of a rate limit check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitResult {
    /// Whether the request is allowed
    pub allowed: bool,
    /// Remaining requests in current window
    pub remaining: u32,
    /// Maximum requests per window
    pub limit: u32,
    /// Time until reset in seconds
    pub reset_seconds: u64,
    /// Matched rule ID (if any)
    pub matched_rule: Option<String>,
    /// Action to take
    pub action: ExceedAction,
    /// Delay in milliseconds (for throttle action)
    pub delay_ms: u64,
}

impl Default for RateLimitResult {
    fn default() -> Self {
        Self {
            allowed: true,
            remaining: 100,
            limit: 100,
            reset_seconds: 0,
            matched_rule: None,
            action: ExceedAction::Reject,
            delay_ms: 0,
        }
    }
}

/// Result of a connection limit check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionLimitResult {
    /// Whether the connection is allowed
    pub allowed: bool,
    /// Current connection count
    pub current: u32,
    /// Maximum connections
    pub limit: u32,
    /// Matched rule ID (if any)
    pub matched_rule: Option<String>,
    /// Action to take
    pub action: ExceedAction,
    /// Queue position (if queued)
    pub queue_position: u32,
    /// Estimated wait time in milliseconds
    pub wait_ms: u64,
}

impl Default for ConnectionLimitResult {
    fn default() -> Self {
        Self {
            allowed: true,
            current: 0,
            limit: 10000,
            matched_rule: None,
            action: ExceedAction::Reject,
            queue_position: 0,
            wait_ms: 0,
        }
    }
}

/// Control request context for rule matching
#[derive(Debug, Clone, Default)]
pub struct ControlContext {
    /// Client IP address
    pub client_ip: Option<String>,
    /// Client ID (gRPC client identifier)
    pub client_id: Option<String>,
    /// Namespace
    pub namespace: Option<String>,
    /// Service name
    pub service: Option<String>,
    /// API path or gRPC method
    pub path: Option<String>,
    /// User ID (if authenticated)
    pub user: Option<String>,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl ControlContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_ip(mut self, ip: impl Into<String>) -> Self {
        self.client_ip = Some(ip.into());
        self
    }

    pub fn with_client_id(mut self, client_id: impl Into<String>) -> Self {
        self.client_id = Some(client_id.into());
        self
    }

    pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = Some(namespace.into());
        self
    }

    pub fn with_service(mut self, service: impl Into<String>) -> Self {
        self.service = Some(service.into());
        self
    }

    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn with_user(mut self, user: impl Into<String>) -> Self {
        self.user = Some(user.into());
        self
    }

    /// Get the value for a given target type
    pub fn get_target_value(&self, target_type: &RuleTargetType) -> Option<&str> {
        match target_type {
            RuleTargetType::Ip => self.client_ip.as_deref(),
            RuleTargetType::ClientId => self.client_id.as_deref(),
            RuleTargetType::Namespace => self.namespace.as_deref(),
            RuleTargetType::Service => self.service.as_deref(),
            RuleTargetType::ApiPath | RuleTargetType::GrpcMethod => self.path.as_deref(),
            RuleTargetType::User => self.user.as_deref(),
        }
    }
}

/// Control plugin configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlPluginConfig {
    /// Whether the control plugin is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Default TPS limit when no rule matches
    #[serde(default = "default_tps")]
    pub default_tps: u32,
    /// Default connection limit when no rule matches
    #[serde(default = "default_connections")]
    pub default_max_connections: u32,
    /// Rule storage type
    #[serde(default)]
    pub storage_type: RuleStorageType,
    /// Rule refresh interval in seconds
    #[serde(default = "default_refresh")]
    pub refresh_interval_seconds: u64,
    /// Metrics collection enabled
    #[serde(default = "default_true")]
    pub metrics_enabled: bool,
}

fn default_tps() -> u32 {
    1000
}

fn default_connections() -> u32 {
    10000
}

fn default_refresh() -> u64 {
    30
}

impl Default for ControlPluginConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_tps: 1000,
            default_max_connections: 10000,
            storage_type: RuleStorageType::Memory,
            refresh_interval_seconds: 30,
            metrics_enabled: true,
        }
    }
}

/// Rule storage type
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleStorageType {
    /// In-memory storage (default)
    #[default]
    Memory,
    /// Database storage
    Database,
    /// File-based storage
    File,
    /// Raft-based distributed storage
    Raft,
}

/// Control statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ControlStats {
    /// Total requests processed
    pub total_requests: u64,
    /// Requests allowed
    pub allowed_requests: u64,
    /// Requests rejected due to rate limit
    pub rate_limited_requests: u64,
    /// Requests rejected due to connection limit
    pub connection_limited_requests: u64,
    /// Current active connections
    pub active_connections: u32,
    /// Peak connections
    pub peak_connections: u32,
    /// Number of active rate limit rules
    pub active_rate_rules: u32,
    /// Number of active connection rules
    pub active_connection_rules: u32,
}
