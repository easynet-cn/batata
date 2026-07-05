//! Application context - shared state across all components
//!
//! AppContext holds references to all services and configuration
//! needed by different parts of the application.

use std::sync::Arc;

use crate::context::DeploymentMode;
use crate::model::common::Configuration;
use crate::registry::service_registry::ServiceRegistry;
use crate::startup::persistence::PersistenceContext;

/// Application context - the central shared state
///
/// This struct is passed to all initializers and servers, providing
/// access to configuration, services, and other resources.
pub struct AppContext {
    /// Application configuration
    pub config: Arc<Configuration>,
    
    /// Persistence context (cluster manager, raft, database)
    pub persistence: PersistenceContext,
    
    /// Service registry
    pub services: ServiceRegistry,
    
    /// Deployment mode
    pub deployment_mode: DeploymentMode,
}

impl AppContext {
    /// Create a new AppContext
    pub fn new(
        config: Arc<Configuration>,
        persistence: PersistenceContext,
        services: ServiceRegistry,
        deployment_mode: DeploymentMode,
    ) -> Self {
        Self {
            config,
            persistence,
            services,
            deployment_mode,
        }
    }
    
    /// Check if running in standalone mode
    pub fn is_standalone(&self) -> bool {
        self.config.is_standalone()
    }
    
    /// Check if running in console-remote mode
    pub fn is_console_remote(&self) -> bool {
        self.deployment_mode == DeploymentMode::Console
    }
    
    /// Check if cluster mode (not standalone, not console-remote)
    pub fn is_cluster(&self) -> bool {
        !self.is_standalone() && !self.is_console_remote()
    }
}

impl std::fmt::Debug for AppContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppContext")
            .field("config", &"...")
            .field("persistence", &"...")
            .field("services", &self.services)
            .field("deployment_mode", &self.deployment_mode)
            .finish()
    }
}
