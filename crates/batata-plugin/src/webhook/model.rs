//! Webhook Plugin Data Models

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Webhook event types
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebhookEventType {
    /// Configuration created
    ConfigCreated,
    /// Configuration updated
    ConfigUpdated,
    /// Configuration deleted
    ConfigDeleted,
    /// Configuration published (gray release)
    ConfigPublished,
    /// Service instance registered
    ServiceRegistered,
    /// Service instance deregistered
    ServiceDeregistered,
    /// Service instance updated
    ServiceUpdated,
    /// Service health changed
    ServiceHealthChanged,
    /// Namespace created
    NamespaceCreated,
    /// Namespace deleted
    NamespaceDeleted,
    /// User created
    UserCreated,
    /// User deleted
    UserDeleted,
    /// Custom event type
    Custom(String),
}

impl std::fmt::Display for WebhookEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WebhookEventType::ConfigCreated => write!(f, "config_created"),
            WebhookEventType::ConfigUpdated => write!(f, "config_updated"),
            WebhookEventType::ConfigDeleted => write!(f, "config_deleted"),
            WebhookEventType::ConfigPublished => write!(f, "config_published"),
            WebhookEventType::ServiceRegistered => write!(f, "service_registered"),
            WebhookEventType::ServiceDeregistered => write!(f, "service_deregistered"),
            WebhookEventType::ServiceUpdated => write!(f, "service_updated"),
            WebhookEventType::ServiceHealthChanged => write!(f, "service_health_changed"),
            WebhookEventType::NamespaceCreated => write!(f, "namespace_created"),
            WebhookEventType::NamespaceDeleted => write!(f, "namespace_deleted"),
            WebhookEventType::UserCreated => write!(f, "user_created"),
            WebhookEventType::UserDeleted => write!(f, "user_deleted"),
            WebhookEventType::Custom(name) => write!(f, "custom:{}", name),
        }
    }
}

/// Webhook event payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEvent {
    /// Unique event ID
    pub id: String,
    /// Event type
    pub event_type: WebhookEventType,
    /// Event timestamp (Unix timestamp in milliseconds)
    pub timestamp: i64,
    /// Source of the event
    pub source: String,
    /// Namespace
    #[serde(default)]
    pub namespace: String,
    /// Group (for config events)
    #[serde(default)]
    pub group: String,
    /// Resource name (config data ID, service name, etc.)
    #[serde(default)]
    pub resource: String,
    /// Event data payload
    #[serde(default)]
    pub data: HashMap<String, serde_json::Value>,
    /// Additional metadata
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl WebhookEvent {
    pub fn new(event_type: WebhookEventType) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            event_type,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0),
            source: "batata".to_string(),
            namespace: String::new(),
            group: String::new(),
            resource: String::new(),
            data: HashMap::new(),
            metadata: HashMap::new(),
        }
    }

    pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = namespace.into();
        self
    }

    pub fn with_group(mut self, group: impl Into<String>) -> Self {
        self.group = group.into();
        self
    }

    pub fn with_resource(mut self, resource: impl Into<String>) -> Self {
        self.resource = resource.into();
        self
    }

    pub fn with_data(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.data.insert(key.into(), value);
        self
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Webhook configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookConfig {
    /// Unique webhook ID
    pub id: String,
    /// Webhook name
    pub name: String,
    /// Webhook description
    #[serde(default)]
    pub description: String,
    /// Whether the webhook is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Target URL to send events to
    pub url: String,
    /// HTTP method (POST, PUT)
    #[serde(default = "default_method")]
    pub method: String,
    /// Event types to subscribe to
    pub event_types: Vec<WebhookEventType>,
    /// Namespace filter (empty = all namespaces)
    #[serde(default)]
    pub namespace_filter: Vec<String>,
    /// Group filter (empty = all groups)
    #[serde(default)]
    pub group_filter: Vec<String>,
    /// Custom headers to include in requests
    #[serde(default)]
    pub headers: HashMap<String, String>,
    /// Secret for signing webhook payloads
    #[serde(default)]
    pub secret: String,
    /// Retry configuration
    #[serde(default)]
    pub retry: WebhookRetryConfig,
    /// Request timeout in milliseconds
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    /// Content type
    #[serde(default = "default_content_type")]
    pub content_type: String,
    /// Custom payload template (if empty, default JSON is used)
    #[serde(default)]
    pub payload_template: String,
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

fn default_method() -> String {
    "POST".to_string()
}

fn default_timeout() -> u64 {
    5000
}

fn default_content_type() -> String {
    "application/json".to_string()
}

impl Default for WebhookConfig {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            description: String::new(),
            enabled: true,
            url: String::new(),
            method: "POST".to_string(),
            event_types: Vec::new(),
            namespace_filter: Vec::new(),
            group_filter: Vec::new(),
            headers: HashMap::new(),
            secret: String::new(),
            retry: WebhookRetryConfig::default(),
            timeout_ms: 5000,
            content_type: "application/json".to_string(),
            payload_template: String::new(),
            created_at: 0,
            updated_at: 0,
        }
    }
}

/// Retry configuration for webhook delivery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookRetryConfig {
    /// Whether retry is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Maximum number of retry attempts
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    /// Initial retry delay in milliseconds
    #[serde(default = "default_initial_delay")]
    pub initial_delay_ms: u64,
    /// Maximum retry delay in milliseconds
    #[serde(default = "default_max_delay")]
    pub max_delay_ms: u64,
    /// Backoff multiplier
    #[serde(default = "default_multiplier")]
    pub multiplier: f64,
    /// Jitter factor (0.0 to 1.0)
    #[serde(default = "default_jitter")]
    pub jitter: f64,
}

fn default_max_retries() -> u32 {
    3
}

fn default_initial_delay() -> u64 {
    1000
}

fn default_max_delay() -> u64 {
    60000
}

fn default_multiplier() -> f64 {
    2.0
}

fn default_jitter() -> f64 {
    0.1
}

impl Default for WebhookRetryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_retries: 3,
            initial_delay_ms: 1000,
            max_delay_ms: 60000,
            multiplier: 2.0,
            jitter: 0.1,
        }
    }
}

impl WebhookRetryConfig {
    /// Calculate delay for a given retry attempt (0-indexed)
    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        if attempt == 0 || !self.enabled {
            return Duration::ZERO;
        }

        let base_delay = self.initial_delay_ms as f64 * self.multiplier.powi(attempt as i32 - 1);
        let capped_delay = base_delay.min(self.max_delay_ms as f64);

        // Add jitter
        let jitter_range = capped_delay * self.jitter;
        let jitter = (rand::random::<f64>() - 0.5) * 2.0 * jitter_range;
        let final_delay = (capped_delay + jitter).max(0.0) as u64;

        Duration::from_millis(final_delay)
    }
}

/// Result of a webhook delivery attempt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookResult {
    /// Whether delivery was successful
    pub success: bool,
    /// HTTP status code (if request was made)
    pub status_code: Option<u16>,
    /// Response body (truncated if too large)
    pub response_body: Option<String>,
    /// Error message (if failed)
    pub error: Option<String>,
    /// Number of attempts made
    pub attempts: u32,
    /// Total time spent in milliseconds
    pub duration_ms: u64,
    /// Webhook ID
    pub webhook_id: String,
    /// Event ID
    pub event_id: String,
}

impl Default for WebhookResult {
    fn default() -> Self {
        Self {
            success: false,
            status_code: None,
            response_body: None,
            error: None,
            attempts: 0,
            duration_ms: 0,
            webhook_id: String::new(),
            event_id: String::new(),
        }
    }
}

/// Webhook delivery status for tracking
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebhookDeliveryStatus {
    /// Pending delivery
    Pending,
    /// Currently being delivered
    InProgress,
    /// Successfully delivered
    Delivered,
    /// Failed after all retries
    Failed,
    /// Canceled (webhook disabled or deleted)
    Canceled,
}

/// Webhook delivery record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookDelivery {
    /// Unique delivery ID
    pub id: String,
    /// Webhook ID
    pub webhook_id: String,
    /// Event ID
    pub event_id: String,
    /// Event type
    pub event_type: WebhookEventType,
    /// Delivery status
    pub status: WebhookDeliveryStatus,
    /// Number of attempts made
    pub attempts: u32,
    /// Last attempt timestamp
    pub last_attempt: i64,
    /// Next retry timestamp (if pending)
    pub next_retry: Option<i64>,
    /// Last error message
    pub last_error: Option<String>,
    /// Creation timestamp
    pub created_at: i64,
    /// Completion timestamp
    pub completed_at: Option<i64>,
}

/// Webhook statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WebhookStats {
    /// Total webhooks configured
    pub total_webhooks: u32,
    /// Enabled webhooks
    pub enabled_webhooks: u32,
    /// Total events processed
    pub total_events: u64,
    /// Successful deliveries
    pub successful_deliveries: u64,
    /// Failed deliveries
    pub failed_deliveries: u64,
    /// Pending deliveries
    pub pending_deliveries: u32,
    /// Average delivery time in milliseconds
    pub avg_delivery_time_ms: u64,
}
