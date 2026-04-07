//! Batata Plugin - Plugin SPI definitions
//!
//! This crate provides:
//! - Plugin traits and lifecycle management
//! - Control Plugin: Rate limiting and connection control (PLG-001 to PLG-004)
//! - Webhook Plugin: Event notifications (PLG-101 to PLG-104)
//! - CMDB Plugin: Label sync and entity mapping (PLG-201 to PLG-203)
//! - Extension points for custom plugins

pub mod cmdb;
pub mod control;
pub mod spi;
pub mod webhook;

pub use control::{
    ConnectionLimitResult, ConnectionLimitRule, ConnectionLimiter, ControlContext, ControlPlugin,
    ControlPluginConfig, ControlStats, DefaultControlPlugin, ExceedAction, RateLimitAlgorithm,
    RateLimitResult, RateLimitRule, RuleMatchType, RuleStorageType, RuleStore, RuleTargetType,
    TokenBucket,
};

pub use webhook::{
    DefaultWebhookPlugin, WebhookConfig, WebhookEvent, WebhookEventType, WebhookPlugin,
    WebhookResult, WebhookRetryConfig,
};

pub use cmdb::{
    CmdbConfig, CmdbEntity, CmdbEntityType, CmdbLabel, CmdbPlugin, CmdbSyncResult,
    DefaultCmdbPlugin, LabelMapping,
};

/// Plugin trait (re-exported from batata-common)
pub use batata_common::Plugin;

/// Protocol adapter plugin trait (re-exported from spi)
pub use spi::ProtocolAdapterPlugin;

/// Plugin initialization context (re-exported from spi)
pub use spi::PluginContext;

/// Plugin naming store trait and error type (re-exported from spi)
pub use spi::{PluginNamingStore, PluginNamingStoreError};

/// Plugin state provider trait (re-exported from spi)
pub use spi::PluginStateProvider;

/// Health check result handler trait (re-exported from spi)
pub use spi::HealthCheckResultHandler;

/// Plugin registry for managing multiple plugins
pub struct PluginRegistry {
    plugins: Vec<Box<dyn Plugin>>,
    protocol_adapters: Vec<Box<dyn ProtocolAdapterPlugin>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
            protocol_adapters: Vec::new(),
        }
    }

    /// Register a plugin
    pub fn register(&mut self, plugin: Box<dyn Plugin>) {
        self.plugins.push(plugin);
    }

    /// Register a protocol adapter plugin
    pub fn register_protocol_adapter(&mut self, adapter: Box<dyn ProtocolAdapterPlugin>) {
        self.protocol_adapters.push(adapter);
    }

    /// Get a protocol adapter by protocol name
    pub fn get_protocol_adapter(&self, protocol: &str) -> Option<&dyn ProtocolAdapterPlugin> {
        self.protocol_adapters
            .iter()
            .find(|a| a.protocol() == protocol && a.is_enabled())
            .map(|a| a.as_ref())
    }

    /// List all protocol adapter names
    pub fn list_protocol_adapters(&self) -> Vec<&str> {
        self.protocol_adapters.iter().map(|a| a.name()).collect()
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
