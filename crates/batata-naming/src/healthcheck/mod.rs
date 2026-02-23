//! Nacos-compatible health check module
//!
//! This module provides health checking functionality that matches Nacos implementation
//! including TCP, HTTP health checks, heartbeat monitoring, and
//! automatic instance expiration.

pub mod checker;
pub mod config;
pub mod configurer;
pub mod factory;
pub mod heartbeat;
pub mod processor;
pub mod reactor;
pub mod task;

use std::sync::Arc;

// Re-export common types
pub use checker::{HealthChecker, HealthCheckResult};
pub use config::{HealthCheckConfig, HttpHealthParams, TcpHealthParams};
pub use configurer::{
    HealthCheckerConfig, HttpHealthCheckerConfig, TcpHealthCheckerConfig,
};
pub use factory::HealthCheckerFactory;
pub use heartbeat::{ExpiredInstanceChecker, UnhealthyInstanceChecker};
pub use processor::{
    HealthCheckProcessor, HealthCheckType,
    HttpHealthCheckProcessor, NoneHealthCheckProcessor, TcpHealthCheckProcessor,
};
pub use reactor::HealthCheckReactor;
pub use task::HealthCheckTask;

/// Health check manager that coordinates all health check components
///
/// This struct manages the health check reactor, unhealthy instance checker,
/// and expired instance checker together.
pub struct HealthCheckManager {
    reactor: HealthCheckReactor,
    unhealthy_checker: UnhealthyInstanceChecker,
    expired_checker: ExpiredInstanceChecker,
}

impl HealthCheckManager {
    /// Create a new health check manager
    pub fn new(
        naming_service: Arc<crate::service::NamingService>,
        config: Arc<HealthCheckConfig>,
        expire_enabled: bool,
    ) -> Self {
        let unhealthy_checker = UnhealthyInstanceChecker::new(naming_service.clone(), config.clone());
        let expired_checker = ExpiredInstanceChecker::new(
            naming_service.clone(),
            config.clone(),
            expire_enabled,
            unhealthy_checker.heartbeat_map.clone(),
        );
        let reactor = HealthCheckReactor::new(naming_service, config);

        Self {
            reactor,
            unhealthy_checker,
            expired_checker,
        }
    }

    /// Get reference to the reactor
    pub fn reactor(&self) -> &HealthCheckReactor {
        &self.reactor
    }

    /// Get reference to the unhealthy instance checker
    pub fn unhealthy_checker(&self) -> &UnhealthyInstanceChecker {
        &self.unhealthy_checker
    }

    /// Get reference to the expired instance checker
    pub fn expired_checker(&self) -> &ExpiredInstanceChecker {
        &self.expired_checker
    }

    /// Shutdown all health check components
    pub async fn shutdown(&self) {
        self.unhealthy_checker.stop();
        self.expired_checker.stop();
        self.reactor.shutdown().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_manager_creation() {
        let naming_service = Arc::new(crate::service::NamingService::new());
        let config = Arc::new(HealthCheckConfig::default());
        let manager = HealthCheckManager::new(naming_service, config, true);

        assert_eq!(manager.reactor().get_active_task_count(), 0);
        assert_eq!(manager.unhealthy_checker().get_tracked_count(), 0);
        assert_eq!(manager.expired_checker().get_tracked_count(), 0);
    }
}
