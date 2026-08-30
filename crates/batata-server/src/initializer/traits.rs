//! Core traits for the initializer module
//!
//! These traits define the contract for service initialization across
//! different phases of the application lifecycle.

use std::borrow::Cow;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

/// Validation error for configuration
#[derive(Debug, Clone)]
pub struct ValidationError {
    /// `message` field.
    pub message: String,
    /// `field` field.
    pub field: Option<String>,
}

impl ValidationError {
    /// `new` function.
    ///
    /// # Arguments
    /// - `message`: `message : impl Into < String > . ty`.
    ///
    /// # Returns
    /// `Self`.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            field: None,
        }
    }
    
    /// `with_field` function.
    ///
    /// # Arguments
    /// - `field`: `field : impl Into < String > . ty`.
    ///
    /// # Returns
    /// `Self`.
    pub fn with_field(mut self, field: impl Into<String>) -> Self {
        self.field = Some(field.into());
        self
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(ref field) = self.field {
            write!(f, "{} [field: {}]", self.message, field)
        } else {
            write!(f, "{}", self.message)
        }
    }
}

impl std::error::Error for ValidationError {}

/// Result type for initializer operations
pub type InitResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Service initializer trait - defines how a service is initialized
///
/// Implement this trait to add a new service initialization phase.
/// Initializers are executed in order of priority (lower numbers first).
#[async_trait]
pub trait ServiceInitializer: Send + Sync {
    /// Initialize the service
    async fn initialize(
        &self,
        ctx: &crate::context::AppContext,
    ) -> InitResult<Arc<dyn std::any::Any + Send + Sync>>;

    /// Initialize priority - lower numbers execute first
    fn priority(&self) -> u8 {
        128 // default middle priority
    }

    /// Shutdown the service gracefully (optional)
    async fn shutdown(&self, _service: Arc<dyn std::any::Any + Send + Sync>) -> InitResult<()> {
        Ok(())
    }

    /// Get the service name for logging
    fn name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// Phase initializer trait - for one-time phase setup
#[async_trait]
pub trait PhaseInitializer: Send + Sync {
    /// Phase identifier
    fn phase_id(&self) -> &'static str;
    
    /// Phase priority - execution order
    fn priority(&self) -> u8 {
        128
    }
    
    /// Execute the phase
    async fn execute(&self, ctx: &mut crate::context::AppContext) -> InitResult<()>;
}

// =============================================================================
// Builder traits
// =============================================================================

/// Configuration builder trait
pub trait ConfigBuilderTrait: Send + Sync {
    /// Build and validate configuration
    fn build(&self) -> InitResult<Arc<Configuration>>;
    
    /// Validate the configuration
    fn validate(&self, config: &Configuration) -> Result<(), Vec<ValidationError>>;
}

/// Persistence builder trait
#[async_trait]
pub trait PersistenceBuilderTrait: Send + Sync {
    /// `initialize` function.
    ///
    /// # Arguments
    /// - `config`: `config : & Configuration . ty`.
    /// - `plugin_cf_names`: `plugin_cf_names : & [String] . ty`.
    ///
    /// # Returns
    /// `InitResult < PersistenceContext >`.
    async fn initialize(
        &self,
        config: &Configuration,
        plugin_cf_names: &[String],
    ) -> InitResult<PersistenceContext>;
}

/// Service builder trait
#[async_trait]
pub trait ServiceBuilderTrait: Send + Sync {
    /// `build` function.
    ///
    /// # Arguments
    /// - `ctx`: `ctx : & crate :: context :: AppContext . ty`.
    ///
    /// # Returns
    /// `InitResult < () >`.
    async fn build(&self, ctx: &crate::context::AppContext) -> InitResult<()>;
}

// =============================================================================
// Server lifecycle traits
// =============================================================================

/// Server kind enumeration
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ServerKind {
    /// `HttpMain` variant.
    HttpMain,
    /// `HttpConsole` variant.
    HttpConsole,
    /// `GrpcSdk` variant.
    GrpcSdk,
    /// `GrpcCluster` variant.
    GrpcCluster,
    /// `GrpcRaft` variant.
    GrpcRaft,
    /// `Xds` variant.
    Xds,
    /// `McpRegistry` variant.
    McpRegistry,
    /// `Plugin` variant.
    Plugin(String),
}

impl ServerKind {
    /// `as_str` function.
    ///
    /// # Returns
    /// `std :: borrow :: Cow < 'static , str >`.
    pub fn as_str(&self) -> std::borrow::Cow<'static, str> {
        match self {
            ServerKind::HttpMain => "HttpMain".into(),
            ServerKind::HttpConsole => "HttpConsole".into(),
            ServerKind::GrpcSdk => "GrpcSdk".into(),
            ServerKind::GrpcCluster => "GrpcCluster".into(),
            ServerKind::GrpcRaft => "GrpcRaft".into(),
            ServerKind::Xds => "Xds".into(),
            ServerKind::McpRegistry => "McpRegistry".into(),
            ServerKind::Plugin(name) => name.clone().into(),
        }
    }
}

impl std::fmt::Display for ServerKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Server health status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerHealth {
    /// `Starting` variant.
    Starting,
    /// `Running` variant.
    Running,
    /// `Draining` variant.
    Draining,
    /// `Stopping` variant.
    Stopping,
    /// `Stopped` variant.
    Stopped,
    /// `Failed` variant.
    Failed,
}

impl ServerHealth {
    /// `as_str` function.
    ///
    /// # Returns
    /// `& 'static str`.
    pub fn as_str(&self) -> &'static str {
        match self {
            ServerHealth::Starting => "STARTING",
            ServerHealth::Running => "RUNNING",
            ServerHealth::Draining => "DRAINING",
            ServerHealth::Stopping => "STOPPING",
            ServerHealth::Stopped => "STOPPED",
            ServerHealth::Failed => "FAILED",
        }
    }
}

impl std::fmt::Display for ServerHealth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Server handle for lifecycle management
#[derive(Clone)]
pub struct ServerHandle {
    /// `kind` field.
    pub kind: ServerKind,
    /// `name` field.
    pub name: String,
    /// `shutdown_tx` field.
    pub shutdown_tx: tokio::sync::watch::Sender<bool>,
}

impl ServerHandle {
    /// `new` function.
    ///
    /// # Arguments
    /// - `kind`: `kind : ServerKind . ty`.
    /// - `name`: `name : impl Into < String > . ty`.
    ///
    /// # Returns
    /// `(Self , tokio :: sync :: watch :: Receiver < bool >)`.
    pub fn new(kind: ServerKind, name: impl Into<String>) -> (Self, tokio::sync::watch::Receiver<bool>) {
        let (tx, rx) = tokio::sync::watch::channel(false);
        (
            Self {
                kind,
                name: name.into(),
                shutdown_tx: tx,
            },
            rx,
        )
    }
    
    /// `shutdown` function.
    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(true);
    }
}

/// Server lifecycle trait - defines the contract for all servers
///
/// Implement this trait to add a new server type (HTTP, gRPC, xDS, etc.)
#[async_trait]
pub trait ServerLifecycle: Send + Sync {
    /// Server type identifier
    fn kind(&self) -> ServerKind;
    
    /// Server display name
    fn name(&self) -> Cow<'static, str>;
    
    /// Start the server
    async fn start(&self, ctx: &crate::context::AppContext) -> InitResult<ServerHandle>;
    
    /// Stop the server gracefully
    async fn stop(&self, handle: &ServerHandle) -> InitResult<()>;
    
    /// Get current health status
    fn health(&self) -> ServerHealth;
}

// =============================================================================
// Shutdown traits
// =============================================================================

/// Graceful shutdownable trait - for components that need orderly shutdown
#[async_trait]
pub trait GracefulShutdownable: Send + Sync {
    /// Shutdown order - lower numbers shutdown first
    fn shutdown_order(&self) -> u8 {
        128
    }
    
    /// Execute shutdown
    async fn shutdown(&self) -> InitResult<()>;
    
    /// Drain in-progress requests
    async fn drain(&self, _timeout: Duration) -> InitResult<()> {
        Ok(())
    }
}

#[async_trait]
impl<T: GracefulShutdownable + ?Sized> GracefulShutdownable for Arc<T> {
    fn shutdown_order(&self) -> u8 {
        (**self).shutdown_order()
    }
    
    async fn shutdown(&self) -> InitResult<()> {
        (**self).shutdown().await
    }
    
    async fn drain(&self, timeout: Duration) -> InitResult<()> {
        (**self).drain(timeout).await
    }
}

// =============================================================================
// Re-exports for convenience
// =============================================================================

use crate::model::common::Configuration;
/// Re-exported item.
pub use crate::startup::persistence::PersistenceContext;
