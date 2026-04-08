// Configuration management abstraction layer

use async_trait::async_trait;

use super::types::{ChangeType, ConfigItem, ConfigQuery, PagedResult};

/// Configuration store abstraction trait
/// Implementations: NacosConfigStore, ConsulKVStore
#[async_trait]
pub trait ConfigStore: Send + Sync {
    /// Get a configuration item
    async fn get(
        &self,
        namespace: &str,
        group: &str,
        key: &str,
    ) -> Result<Option<ConfigItem>, ConfigError>;

    /// Set/Create a configuration item
    async fn set(&self, item: ConfigItem) -> Result<ConfigItem, ConfigError>;

    /// Delete a configuration item
    async fn delete(&self, namespace: &str, group: &str, key: &str) -> Result<bool, ConfigError>;

    /// Check if a configuration exists
    async fn exists(&self, namespace: &str, group: &str, key: &str) -> Result<bool, ConfigError> {
        Ok(self.get(namespace, group, key).await?.is_some())
    }

    /// List configurations with query
    async fn list(&self, query: ConfigQuery) -> Result<PagedResult<ConfigItem>, ConfigError>;

    /// Get multiple configurations at once
    async fn get_many(
        &self,
        keys: Vec<(String, String, String)>, // (namespace, group, key)
    ) -> Result<Vec<Option<ConfigItem>>, ConfigError>;
}

/// Configuration watch/subscription
#[async_trait]
pub trait ConfigWatch: Send + Sync {
    /// Watch for configuration changes
    /// Returns a receiver that will receive change events
    async fn watch(
        &self,
        namespace: &str,
        group: &str,
        key: &str,
    ) -> Result<tokio::sync::mpsc::Receiver<ConfigChangeEvent>, ConfigError>;

    /// Watch multiple configurations
    async fn watch_many(
        &self,
        keys: Vec<(String, String, String)>,
    ) -> Result<tokio::sync::mpsc::Receiver<ConfigChangeEvent>, ConfigError>;

    /// Watch by prefix (Consul-style)
    async fn watch_prefix(
        &self,
        namespace: &str,
        prefix: &str,
    ) -> Result<tokio::sync::mpsc::Receiver<ConfigChangeEvent>, ConfigError>;

    /// Unwatch a configuration
    async fn unwatch(&self, namespace: &str, group: &str, key: &str) -> Result<(), ConfigError>;
}

/// Configuration change event
#[derive(Debug, Clone)]
pub struct ConfigChangeEvent {
    pub namespace: String,
    pub group: String,
    pub key: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub change_type: ChangeType,
    pub version: u64,
    pub timestamp: i64,
}

/// Batch operations for configuration
#[async_trait]
pub trait BatchConfigStore: ConfigStore {
    /// Set multiple configurations atomically (transaction)
    async fn set_many(&self, items: Vec<ConfigItem>) -> Result<Vec<ConfigItem>, ConfigError>;

    /// Delete multiple configurations
    async fn delete_many(
        &self,
        keys: Vec<(String, String, String)>,
    ) -> Result<Vec<bool>, ConfigError>;

    /// Import configurations from file/format
    async fn import(
        &self,
        namespace: &str,
        group: &str,
        configs: Vec<ConfigItem>,
        policy: ImportPolicy,
    ) -> Result<ImportResult, ConfigError>;

    /// Export configurations to a structured format
    async fn export(
        &self,
        namespace: &str,
        group: Option<&str>,
    ) -> Result<Vec<ConfigItem>, ConfigError>;
}

/// Import policy when conflicts exist
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportPolicy {
    /// Skip existing configurations
    Skip,
    /// Overwrite existing configurations
    Overwrite,
    /// Abort if any conflict exists
    Abort,
}

/// Import operation result
#[derive(Debug, Clone)]
pub struct ImportResult {
    pub created: u32,
    pub updated: u32,
    pub skipped: u32,
    pub failed: u32,
    pub errors: Vec<String>,
}

/// Configuration error types
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Configuration not found: {0}")]
    NotFound(String),

    #[error("Configuration already exists: {0}")]
    AlreadyExists(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Version conflict: expected {expected}, got {actual}")]
    VersionConflict { expected: u64, actual: u64 },

    #[error("Namespace not found: {0}")]
    NamespaceNotFound(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Internal error: {0}")]
    InternalError(String),
}

impl ConfigError {
    pub fn status_code(&self) -> u16 {
        match self {
            ConfigError::NotFound(_) => 404,
            ConfigError::AlreadyExists(_) => 409,
            ConfigError::InvalidConfig(_) => 400,
            ConfigError::VersionConflict { .. } => 409,
            ConfigError::NamespaceNotFound(_) => 404,
            ConfigError::PermissionDenied(_) => 403,
            ConfigError::RateLimitExceeded => 429,
            ConfigError::StorageError(_) => 500,
            ConfigError::SerializationError(_) => 400,
            ConfigError::InternalError(_) => 500,
        }
    }
}
