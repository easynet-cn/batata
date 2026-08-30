//! Plugin traits: Plugin, ControlPlugin, CmdbPlugin.

use crate::model::plugin::cmdb::{
    CmdbEntity, CmdbEntityType, CmdbStats, CmdbSyncResult, LabelMapping,
};
use crate::model::plugin::control::{
    ConnectionLimitResult, ControlContext, ControlStats, RateLimitResult,
};

/// Plugin trait for extensibility
#[async_trait::async_trait]
pub trait Plugin: Send + Sync {
    /// Get the plugin name
    fn name(&self) -> &str;

    /// Initialize the plugin
    async fn init(&self) -> anyhow::Result<()>;

    /// Shutdown the plugin
    async fn shutdown(&self) -> anyhow::Result<()>;
}

/// Control Plugin SPI for rate limiting and connection control
#[async_trait::async_trait]
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

/// CMDB Plugin SPI for label sync and entity mapping
#[async_trait::async_trait]
pub trait CmdbPlugin: Plugin {
    /// The `register_entity` method.
    async fn register_entity(&self, entity: CmdbEntity) -> anyhow::Result<String>;
    /// The `update_entity` method.
    async fn update_entity(&self, entity: CmdbEntity) -> anyhow::Result<()>;
    /// The `delete_entity` method.
    async fn delete_entity(&self, id: &str) -> anyhow::Result<bool>;
    /// The `get_entity` method.
    async fn get_entity(&self, id: &str) -> anyhow::Result<Option<CmdbEntity>>;
    /// The `list_entities` method.
    async fn list_entities(
        &self,
        entity_type: Option<CmdbEntityType>,
    ) -> anyhow::Result<Vec<CmdbEntity>>;
    /// The `search_by_labels` method.
    async fn search_by_labels(
        &self,
        labels: &std::collections::HashMap<String, String>,
    ) -> anyhow::Result<Vec<CmdbEntity>>;
    /// The `sync_labels` method.
    async fn sync_labels(
        &self,
        entity_id: &str,
        labels: &std::collections::HashMap<String, String>,
    ) -> anyhow::Result<std::collections::HashMap<String, String>>;
    /// The `map_entity` method.
    async fn map_entity(&self, entity: &CmdbEntity) -> anyhow::Result<serde_json::Value>;
    /// The `full_sync` method.
    async fn full_sync(&self) -> anyhow::Result<CmdbSyncResult>;
    /// The `get_label_mappings` method.
    async fn get_label_mappings(&self) -> anyhow::Result<Vec<LabelMapping>>;
    /// The `add_label_mapping` method.
    async fn add_label_mapping(&self, mapping: LabelMapping) -> anyhow::Result<String>;
    /// The `remove_label_mapping` method.
    async fn remove_label_mapping(&self, id: &str) -> anyhow::Result<bool>;
    /// The `get_stats` method.
    async fn get_stats(&self) -> CmdbStats;
}
