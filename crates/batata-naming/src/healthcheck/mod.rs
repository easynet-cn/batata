//! Batata health check module (Nacos-compatible)
//!
//! This module provides health checking functionality compatible with the Nacos implementation
//! including TCP, HTTP health checks, heartbeat monitoring, and
//! automatic instance expiration.

pub mod checker;
pub mod config;
pub mod configurer;
pub mod deregister_monitor;
pub mod factory;
pub mod heartbeat;
pub mod interceptor;
pub mod processor;
pub mod raft_replicator;
pub mod reactor;
pub mod registry;
pub mod registry_task;
pub mod result_handler;
pub mod task;
pub mod ttl_monitor;
pub mod ttl_processor;

use std::sync::Arc;

use batata_core::service::distro::DistroMapper;

// Re-export common types
pub use checker::{HealthCheckResult, HealthChecker};
pub use config::{HealthCheckConfig, HttpHealthParams, TcpHealthParams};
pub use configurer::{
    HealthCheckerConfig, HttpHealthCheckerConfig, MysqlHealthCheckerConfig, TcpHealthCheckerConfig,
};
pub use factory::HealthCheckerFactory;
pub use heartbeat::{ExpiredInstanceChecker, UnhealthyInstanceChecker};
pub use interceptor::HealthCheckInterceptorChain;
pub use processor::{
    HealthCheckProcessor, HealthCheckType, HttpHealthCheckProcessor, MysqlHealthCheckProcessor,
    NoneHealthCheckProcessor, TcpHealthCheckProcessor,
};
pub use reactor::HealthCheckReactor;
pub use registry::{
    CheckStatus, CheckType, InstanceCheckConfig, InstanceCheckRegistry, InstanceCheckStatus,
};
pub use task::HealthCheckTask;

/// Health check manager that coordinates all health check components
///
/// This struct manages the health check reactor, unhealthy instance checker,
/// expired instance checker, and the unified instance check registry.
/// In cluster mode, uses `DistroMapper`-based responsibility filtering so
/// only the responsible node executes health checks for ephemeral instances.
pub struct HealthCheckManager {
    reactor: HealthCheckReactor,
    unhealthy_checker: UnhealthyInstanceChecker,
    expired_checker: ExpiredInstanceChecker,
    registry: Arc<InstanceCheckRegistry>,
    core_result_handler: Arc<result_handler::CoreResultHandler>,
}

impl HealthCheckManager {
    /// Create a new health check manager.
    ///
    /// If `distro_mapper` and `local_address` are provided, cluster-aware
    /// responsibility filtering is enabled: only the node responsible for an
    /// instance (determined by consistent hashing) will execute health checks.
    /// Otherwise, all checks execute locally (standalone mode).
    pub fn new(
        naming_service: Arc<crate::service::NamingService>,
        config: Arc<HealthCheckConfig>,
        expire_enabled: bool,
        distro_mapper: Option<Arc<DistroMapper>>,
        local_address: Option<String>,
    ) -> Self {
        // Build the interceptor chain for the last-beat checkers.
        //
        // The last-beat checkers (Unhealthy/Expired) run on the *owning* node
        // — the node that received the registration/heartbeat — so they use a
        // heartbeat chain WITHOUT the distro responsibility interceptor. The
        // active health-check reactor (TCP/HTTP/MySQL, persistent instances)
        // keeps the full cluster chain with responsibility, set later in
        // `upgrade_to_cluster`.
        let heartbeat_chain = match (distro_mapper, local_address) {
            (Some(_), Some(_)) => {
                Arc::new(HealthCheckInterceptorChain::cluster_heartbeat(config.clone()))
            }
            _ => Arc::new(HealthCheckInterceptorChain::standalone(config.clone())),
        };

        let unhealthy_checker = UnhealthyInstanceChecker::new(
            naming_service.clone(),
            config.clone(),
            heartbeat_chain.clone(),
        );
        let expired_checker = ExpiredInstanceChecker::new(
            naming_service.clone(),
            config.clone(),
            expire_enabled,
            unhealthy_checker.heartbeat_map.clone(),
            heartbeat_chain,
        );
        let reactor = HealthCheckReactor::new(naming_service.clone(), config);
        let core_handler = Arc::new(result_handler::CoreResultHandler::new(naming_service));
        let registry = Arc::new(InstanceCheckRegistry::new(
            core_handler.clone() as Arc<dyn batata_plugin::HealthCheckResultHandler>
        ));

        Self {
            reactor,
            unhealthy_checker,
            expired_checker,
            registry,
            core_result_handler: core_handler,
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

    /// Get the instance check registry
    pub fn registry(&self) -> &Arc<InstanceCheckRegistry> {
        &self.registry
    }

    /// Upgrade the interceptor chain from standalone to cluster mode.
    ///
    /// Call this after `DistroProtocol` is created (i.e., after gRPC servers start)
    /// but **before** `start_health_checkers()` spawns the background tasks.
    ///
    /// This also sets the `DistroProtocol` on the result handler so that health
    /// state changes are propagated to cluster peers via Distro sync.
    pub fn upgrade_to_cluster(
        &self,
        config: Arc<HealthCheckConfig>,
        distro_mapper: Arc<DistroMapper>,
        local_address: String,
        distro_protocol: Arc<batata_core::service::distro::DistroProtocol>,
    ) {
        let heartbeat_chain = Arc::new(HealthCheckInterceptorChain::cluster_heartbeat(
            config.clone(),
        ));
        let reactor_chain = Arc::new(HealthCheckInterceptorChain::cluster(
            config,
            distro_mapper,
            local_address,
        ));
        self.unhealthy_checker.set_interceptor_chain(heartbeat_chain.clone());
        self.expired_checker.set_interceptor_chain(heartbeat_chain.clone());
        self.reactor.set_interceptor_chain(reactor_chain);

        // Enable Distro sync for health state change propagation
        self.core_result_handler
            .set_distro_protocol(distro_protocol);
    }

    /// Shutdown all health check components
    pub async fn shutdown(&self) {
        self.unhealthy_checker.stop();
        self.expired_checker.stop();
        self.reactor.shutdown().await;
    }
}

impl batata_common::HeartbeatService for HealthCheckManager {
    fn record_heartbeat(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        ip: &str,
        port: i32,
        cluster_name: &str,
        heartbeat_timeout: i64,
        ip_delete_timeout: i64,
        ephemeral: bool,
    ) {
        self.unhealthy_checker.record_heartbeat(
            namespace,
            group_name,
            service_name,
            ip,
            port,
            cluster_name,
            heartbeat_timeout,
            ip_delete_timeout,
            ephemeral,
        );
    }

    fn remove_heartbeat(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        ip: &str,
        port: i32,
        cluster_name: &str,
    ) {
        self.unhealthy_checker.remove_heartbeat(
            namespace,
            group_name,
            service_name,
            ip,
            port,
            cluster_name,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_manager_creation() {
        let naming_service = Arc::new(crate::service::NamingService::new());
        let config = Arc::new(HealthCheckConfig::default());
        let manager = HealthCheckManager::new(naming_service, config, true, None, None);

        assert_eq!(manager.reactor().get_active_task_count(), 0);
        assert_eq!(manager.unhealthy_checker().get_tracked_count(), 0);
        assert_eq!(manager.expired_checker().get_tracked_count(), 0);
    }
}
