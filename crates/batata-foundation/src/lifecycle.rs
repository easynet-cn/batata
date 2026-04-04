//! Component lifecycle management
//!
//! Provides traits for components that need explicit initialization and shutdown.

use async_trait::async_trait;

use crate::FoundationError;

/// Lifecycle-managed component
///
/// Components that need async initialization (e.g., connecting to a database,
/// joining a cluster) or graceful shutdown (e.g., flushing buffers, leaving
/// a cluster) should implement this trait.
#[async_trait]
pub trait Lifecycle: Send + Sync {
    /// Initialize the component
    ///
    /// Called once during application startup, after dependency injection
    /// but before the component starts handling requests.
    async fn init(&self) -> Result<(), FoundationError>;

    /// Shut down the component gracefully
    ///
    /// Called during application shutdown. Implementations should flush
    /// pending data, close connections, and release resources.
    async fn shutdown(&self) -> Result<(), FoundationError>;

    /// Check if the component is healthy and ready to serve
    async fn health_check(&self) -> Result<ComponentHealth, FoundationError>;
}

/// Health status of a component
#[derive(Debug, Clone)]
pub struct ComponentHealth {
    /// Component name
    pub name: String,
    /// Whether the component is healthy
    pub healthy: bool,
    /// Optional details about the health status
    pub details: Option<String>,
}

impl ComponentHealth {
    pub fn healthy(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            healthy: true,
            details: None,
        }
    }

    pub fn unhealthy(name: impl Into<String>, details: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            healthy: false,
            details: Some(details.into()),
        }
    }
}
