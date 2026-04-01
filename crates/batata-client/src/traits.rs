//! SDK trait contracts matching Nacos Java `ConfigService` and `NamingService` interfaces.
//!
//! These traits define the formal SDK API contract for configuration management
//! and service discovery, with typed request/response objects.

use std::sync::Arc;
use std::time::Duration;

use batata_api::naming::model::Instance;

use crate::config::listener::{ConfigChangeEventListener, ConfigChangeListener};
use crate::error::Result;
use crate::naming::listener::EventListener;

/// Default timeout for SDK operations (3 seconds).
pub const DEFAULT_TIMEOUT_MS: u64 = 3000;

// ============================================================================
// Response Types
// ============================================================================

/// Result of a config query, containing content and metadata.
///
/// Matches Nacos Java `ConfigQueryResult`.
/// Unlike raw `String`, this preserves MD5, config_type, and encrypted_data_key.
#[derive(Clone, Debug, Default)]
pub struct ConfigQueryResult {
    /// The configuration content
    pub content: String,
    /// MD5 hash of the content (useful for CAS operations)
    pub md5: String,
    /// Config type (e.g., "properties", "yaml", "json", "text")
    pub config_type: String,
    /// Encrypted data key if encryption is enabled
    pub encrypted_data_key: String,
}

/// Paginated list result.
///
/// Matches Nacos Java `ListView<T>`.
#[derive(Clone, Debug, Default)]
pub struct ListView<T> {
    /// Total count of items (across all pages)
    pub count: i32,
    /// Items in this page
    pub data: Vec<T>,
}

// ============================================================================
// ConfigService Trait
// ============================================================================

/// SDK contract for configuration management.
///
/// Matches Nacos Java `ConfigService` interface.
/// All methods are async and return typed results.
#[async_trait::async_trait]
pub trait ConfigService: Send + Sync {
    /// Get a config value from the server.
    ///
    /// Returns the raw content string. For metadata (MD5, type), use `get_config_with_result`.
    async fn get_config(&self, data_id: &str, group: &str, timeout: Duration) -> Result<String>;

    /// Get a config value with full metadata.
    ///
    /// Returns `ConfigQueryResult` containing content, MD5, config_type, and encrypted_data_key.
    /// Equivalent to Nacos `getConfigWithResult()`.
    async fn get_config_with_result(
        &self,
        data_id: &str,
        group: &str,
        timeout: Duration,
    ) -> Result<ConfigQueryResult>;

    /// Get config and atomically register a listener.
    ///
    /// Prevents the race condition where a config change happens between
    /// `get_config()` and `add_listener()`.
    async fn get_config_and_sign_listener(
        &self,
        data_id: &str,
        group: &str,
        timeout: Duration,
        listener: Arc<dyn ConfigChangeListener>,
    ) -> Result<String>;

    /// Publish (create or update) a config on the server.
    async fn publish_config(&self, data_id: &str, group: &str, content: &str) -> Result<bool>;

    /// Publish config with a specific type.
    async fn publish_config_with_type(
        &self,
        data_id: &str,
        group: &str,
        content: &str,
        config_type: &str,
    ) -> Result<bool>;

    /// Publish config with CAS (Compare-And-Swap) to avoid concurrent overwrites.
    async fn publish_config_cas(
        &self,
        data_id: &str,
        group: &str,
        content: &str,
        cas_md5: &str,
    ) -> Result<bool>;

    /// Publish config with CAS and type.
    async fn publish_config_cas_with_type(
        &self,
        data_id: &str,
        group: &str,
        content: &str,
        cas_md5: &str,
        config_type: &str,
    ) -> Result<bool>;

    /// Remove a config from the server.
    async fn remove_config(&self, data_id: &str, group: &str) -> Result<bool>;

    /// Add a listener for config changes.
    async fn add_listener(
        &self,
        data_id: &str,
        group: &str,
        listener: Arc<dyn ConfigChangeListener>,
    ) -> Result<()>;

    /// Add a detailed change event listener with field-level diffs.
    async fn add_change_event_listener(
        &self,
        data_id: &str,
        group: &str,
        config_type: &str,
        listener: Arc<dyn ConfigChangeEventListener>,
    ) -> Result<()>;

    /// Remove all listeners for a config.
    async fn remove_listener(&self, data_id: &str, group: &str) -> Result<()>;

    /// Remove a specific listener instance (by pointer equality).
    /// Matches Nacos Java `removeListener(dataId, group, listener)`.
    async fn remove_specific_listener(
        &self,
        data_id: &str,
        group: &str,
        listener: &Arc<dyn crate::config::listener::ConfigChangeListener>,
    ) -> Result<()>;

    /// Get server health status ("UP" or "DOWN").
    async fn get_server_status(&self) -> String;

    /// Shutdown the config service, releasing resources.
    async fn shut_down(&self);
}

// ============================================================================
// NamingService Trait
// ============================================================================

/// SDK contract for service discovery.
///
/// Matches Nacos Java `NamingService` interface.
/// Uses typed `Instance` objects instead of raw ip/port strings.
#[async_trait::async_trait]
pub trait NamingService: Send + Sync {
    /// Register a service instance.
    async fn register_instance(
        &self,
        service_name: &str,
        group_name: &str,
        instance: Instance,
    ) -> Result<()>;

    /// Deregister a service instance.
    async fn deregister_instance(
        &self,
        service_name: &str,
        group_name: &str,
        instance: Instance,
    ) -> Result<()>;

    /// Batch register multiple instances.
    async fn batch_register_instance(
        &self,
        service_name: &str,
        group_name: &str,
        instances: Vec<Instance>,
    ) -> Result<()>;

    /// Batch deregister multiple instances.
    async fn batch_deregister_instance(
        &self,
        service_name: &str,
        group_name: &str,
        instances: Vec<Instance>,
    ) -> Result<()>;

    /// Get all instances for a service.
    async fn get_all_instances(
        &self,
        service_name: &str,
        group_name: &str,
    ) -> Result<Vec<Instance>>;

    /// Get all instances with cluster filter.
    async fn get_all_instances_with_clusters(
        &self,
        service_name: &str,
        group_name: &str,
        clusters: &[String],
    ) -> Result<Vec<Instance>>;

    /// Select instances by health status.
    async fn select_instances(
        &self,
        service_name: &str,
        group_name: &str,
        healthy: bool,
    ) -> Result<Vec<Instance>>;

    /// Select instances by health status and clusters.
    async fn select_instances_with_clusters(
        &self,
        service_name: &str,
        group_name: &str,
        clusters: &[String],
        healthy: bool,
    ) -> Result<Vec<Instance>>;

    /// Select one healthy instance using weighted random selection.
    async fn select_one_healthy_instance(
        &self,
        service_name: &str,
        group_name: &str,
    ) -> Result<Option<Instance>>;

    /// Subscribe to service changes.
    async fn subscribe(
        &self,
        service_name: &str,
        group_name: &str,
        clusters: &str,
        listener: Arc<dyn EventListener>,
    ) -> Result<Vec<Instance>>;

    /// Unsubscribe from service changes.
    async fn unsubscribe(&self, service_name: &str, group_name: &str, clusters: &str)
    -> Result<()>;

    /// List services with pagination.
    async fn get_services_of_server(
        &self,
        page_no: i32,
        page_size: i32,
        group_name: &str,
    ) -> Result<ListView<String>>;

    /// Get server health status ("UP" or "DOWN").
    async fn get_server_status(&self) -> String;

    /// Shutdown the naming service, releasing resources.
    async fn shut_down(&self);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_query_result_default() {
        let result = ConfigQueryResult::default();
        assert!(result.content.is_empty());
        assert!(result.md5.is_empty());
        assert!(result.config_type.is_empty());
        assert!(result.encrypted_data_key.is_empty());
    }

    #[test]
    fn test_config_query_result_with_data() {
        let result = ConfigQueryResult {
            content: "server.port=8080".to_string(),
            md5: "abc123".to_string(),
            config_type: "properties".to_string(),
            encrypted_data_key: String::new(),
        };
        assert_eq!(result.content, "server.port=8080");
        assert_eq!(result.md5, "abc123");
        assert_eq!(result.config_type, "properties");
    }

    #[test]
    fn test_list_view_default() {
        let view = ListView::<String>::default();
        assert_eq!(view.count, 0);
        assert!(view.data.is_empty());
    }

    #[test]
    fn test_list_view_with_data() {
        let view = ListView {
            count: 10,
            data: vec!["svc-1".to_string(), "svc-2".to_string()],
        };
        assert_eq!(view.count, 10);
        assert_eq!(view.data.len(), 2);
        assert_eq!(view.data[0], "svc-1");
    }

    #[test]
    fn test_list_view_clone() {
        let view = ListView {
            count: 5,
            data: vec![1, 2, 3],
        };
        let cloned = view.clone();
        assert_eq!(cloned.count, 5);
        assert_eq!(cloned.data.len(), 3);
    }
}
