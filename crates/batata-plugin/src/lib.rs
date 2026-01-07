//! Batata Plugin - Plugin SPI definitions
//!
//! This crate provides:
//! - Plugin traits
//! - Plugin loading mechanism
//! - Extension points

use async_trait::async_trait;

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
