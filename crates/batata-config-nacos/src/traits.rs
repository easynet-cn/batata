//! Nacos config service traits

use async_trait::async_trait;

use batata_foundation::types::PagedResult;

use crate::error::NacosConfigError;
use crate::model::{
    ConfigChangeEvent, GrayConfig, ImportPolicy, ImportResult, ListenItem, NacosConfig,
    NacosConfigQuery,
};

/// Core Nacos config service
///
/// Handles the full Nacos config lifecycle: CRUD, gray release,
/// listener notification, and import/export.
#[async_trait]
pub trait NacosConfigService: Send + Sync {
    // ========================================================================
    // CRUD Operations
    // ========================================================================

    /// Get a configuration
    async fn get_config(
        &self,
        namespace: &str,
        group: &str,
        data_id: &str,
    ) -> Result<Option<NacosConfig>, NacosConfigError>;

    /// Publish (create or update) a configuration
    async fn publish_config(&self, config: NacosConfig) -> Result<bool, NacosConfigError>;

    /// Delete a configuration
    async fn delete_config(
        &self,
        namespace: &str,
        group: &str,
        data_id: &str,
    ) -> Result<bool, NacosConfigError>;

    /// Search configurations with pagination
    async fn search_configs(
        &self,
        query: &NacosConfigQuery,
    ) -> Result<PagedResult<NacosConfig>, NacosConfigError>;

    /// Batch delete configurations
    async fn batch_delete(
        &self,
        keys: &[(String, String, String)], // (namespace, group, data_id)
    ) -> Result<u32, NacosConfigError>;

    // ========================================================================
    // Gray Release
    // ========================================================================

    /// Get a gray (canary) config if the client matches any rule
    async fn get_gray_config(
        &self,
        namespace: &str,
        group: &str,
        data_id: &str,
        client_ip: &str,
        labels: &std::collections::HashMap<String, String>,
    ) -> Result<Option<NacosConfig>, NacosConfigError>;

    /// Publish a gray release configuration
    async fn publish_gray_config(
        &self,
        namespace: &str,
        group: &str,
        data_id: &str,
        gray: GrayConfig,
    ) -> Result<bool, NacosConfigError>;

    /// Delete a gray release configuration
    async fn delete_gray_config(
        &self,
        namespace: &str,
        group: &str,
        data_id: &str,
        gray_name: &str,
    ) -> Result<bool, NacosConfigError>;

    /// List all gray rules for a config
    async fn list_gray_configs(
        &self,
        namespace: &str,
        group: &str,
        data_id: &str,
    ) -> Result<Vec<GrayConfig>, NacosConfigError>;

    // ========================================================================
    // Listener (Long-polling)
    // ========================================================================

    /// Check if any listened configs have changed (long-polling)
    ///
    /// Compares client MD5 with server MD5. Returns only changed items.
    /// If no changes, blocks up to `timeout_ms` waiting for notifications.
    async fn listen(
        &self,
        items: &[ListenItem],
        timeout_ms: u64,
    ) -> Result<Vec<ListenItem>, NacosConfigError>;

    // ========================================================================
    // Import/Export
    // ========================================================================

    /// Import configurations from a structured format
    async fn import(
        &self,
        namespace: &str,
        configs: Vec<NacosConfig>,
        policy: ImportPolicy,
    ) -> Result<ImportResult, NacosConfigError>;

    /// Export configurations
    async fn export(
        &self,
        namespace: &str,
        group: Option<&str>,
        data_ids: Option<&[String]>,
    ) -> Result<Vec<NacosConfig>, NacosConfigError>;
}

/// Config change event listener
///
/// Used for pushing changes to gRPC subscribers and long-polling waiters.
#[async_trait]
pub trait NacosConfigChangeListener: Send + Sync {
    /// Called when a config changes
    async fn on_config_changed(&self, event: ConfigChangeEvent);
}

/// Config cache for MD5-based change detection
///
/// Maintains an in-memory cache of config MD5 hashes for fast comparison
/// during long-polling listener requests.
#[async_trait]
pub trait NacosConfigCache: Send + Sync {
    /// Get cached MD5 for a config
    fn get_md5(&self, namespace: &str, group: &str, data_id: &str) -> Option<String>;

    /// Update cached MD5
    fn update_md5(&self, namespace: &str, group: &str, data_id: &str, md5: &str);

    /// Remove from cache
    fn remove(&self, namespace: &str, group: &str, data_id: &str);

    /// Get all cached keys
    fn all_keys(&self) -> Vec<(String, String, String)>;
}
