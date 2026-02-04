//! Batata Plugin - Plugin SPI definitions
//!
//! This crate provides:
//! - Plugin traits and lifecycle management
//! - Control Plugin: Rate limiting and connection control (PLG-001 to PLG-004)
//! - Webhook Plugin: Event notifications (PLG-101 to PLG-104)
//! - CMDB Plugin: Label sync and entity mapping (PLG-201 to PLG-203)
//! - Extension points for custom plugins

use async_trait::async_trait;

pub mod control;
pub mod webhook;
pub mod cmdb;

pub use control::{
    ControlContext, ControlPlugin, ControlPluginConfig, ControlStats,
    ConnectionLimitResult, ConnectionLimitRule, ConnectionLimiter,
    DefaultControlPlugin, ExceedAction, RateLimitAlgorithm, RateLimitResult,
    RateLimitRule, RuleMatchType, RuleStore, RuleStorageType, RuleTargetType,
    TokenBucket,
};

pub use webhook::{
    WebhookConfig, WebhookEvent, WebhookEventType, WebhookPlugin,
    WebhookResult, WebhookRetryConfig, DefaultWebhookPlugin,
};

pub use cmdb::{
    CmdbConfig, CmdbEntity, CmdbEntityType, CmdbLabel, CmdbPlugin,
    CmdbSyncResult, DefaultCmdbPlugin, LabelMapping,
};

/// Plugin trait for extensibility
#[async_trait]
pub trait Plugin: Send + Sync {
    /// Get the plugin name
    fn name(&self) -> &str;

    /// Initialize the plugin
    async fn init(&self) -> anyhow::Result<()>;

    /// Shutdown the plugin
    async fn shutdown(&self) -> anyhow::Result<()>;
}

/// Plugin registry for managing multiple plugins
pub struct PluginRegistry {
    plugins: Vec<Box<dyn Plugin>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    /// Register a plugin
    pub fn register(&mut self, plugin: Box<dyn Plugin>) {
        self.plugins.push(plugin);
    }

    /// Initialize all plugins
    pub async fn init_all(&self) -> anyhow::Result<()> {
        for plugin in &self.plugins {
            plugin.init().await?;
            tracing::info!("Plugin '{}' initialized", plugin.name());
        }
        Ok(())
    }

    /// Shutdown all plugins
    pub async fn shutdown_all(&self) -> anyhow::Result<()> {
        for plugin in self.plugins.iter().rev() {
            plugin.shutdown().await?;
            tracing::info!("Plugin '{}' shutdown", plugin.name());
        }
        Ok(())
    }

    /// Get plugin by name
    pub fn get(&self, name: &str) -> Option<&dyn Plugin> {
        self.plugins
            .iter()
            .find(|p| p.name() == name)
            .map(|p| p.as_ref())
    }

    /// List all plugin names
    pub fn list(&self) -> Vec<&str> {
        self.plugins.iter().map(|p| p.name()).collect()
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}
