//! Batata Naming - Service discovery
//!
//! This crate provides:
//! - Service registration
//! - Service discovery
//! - Nacos-compatible health checking (TCP/HTTP with heartbeat monitoring)
//! - Load balancing
//! - Service selector evaluation
//! - Metadata persistence

pub mod api;
pub mod handler;
pub mod healthcheck;
pub mod manager;
pub mod model;
pub mod persistence;
pub mod selector;
pub mod service;
pub mod switch_domain;

// Re-export commonly used types
pub use healthcheck::{
    CheckStatus, CheckType, ExpiredInstanceChecker, HealthCheckConfig, HealthCheckManager,
    HealthCheckProcessor, HealthCheckReactor, HealthCheckResult, HealthCheckTask, HealthCheckType,
    HttpHealthCheckProcessor, HttpHealthParams, InstanceCheckConfig, InstanceCheckRegistry,
    InstanceCheckStatus, TcpHealthCheckProcessor, TcpHealthParams, UnhealthyInstanceChecker,
};
pub use manager::NamingManager;
pub use model::{Instance, Service, ServiceInfo, ServiceQuery};
pub use selector::{LabelOperator, LabelRequirement, SelectorBuilder, ServiceSelector};
pub use service::{
    ClusterConfig, ClusterStatistics, FuzzyWatchPattern, NamingService, ProtectionInfo,
    ServiceMetadata,
};

// Implement ConnectionCleanupHandler for NamingService
use batata_core::handler::rpc::ConnectionCleanupHandler;
use batata_core::service::distro::{DistroDataType, DistroProtocol};
use std::sync::Arc;

#[tonic::async_trait]
impl ConnectionCleanupHandler for NamingService {
    fn deregister_all_by_connection(&self, connection_id: &str) -> Vec<String> {
        self.deregister_all_by_connection(connection_id)
    }

    fn remove_subscriber(&self, connection_id: &str) {
        self.remove_subscriber(connection_id)
    }
    // No distro broadcast — used when the protocol is disabled (standalone)
    // or when a distro-aware wrapper is composed at the startup layer.
}

/// Connection cleanup wrapper that also pushes ephemeral-instance removal
/// to the cluster via Distro after the local deregister completes.
///
/// Used only in cluster mode. Standalone deployments pass a bare
/// `NamingService` as the cleanup handler because there are no peers
/// to notify.
pub struct DistroAwareCleanup {
    naming: Arc<NamingService>,
    distro: Arc<DistroProtocol>,
}

impl DistroAwareCleanup {
    pub fn new(naming: Arc<NamingService>, distro: Arc<DistroProtocol>) -> Self {
        Self { naming, distro }
    }
}

#[tonic::async_trait]
impl ConnectionCleanupHandler for DistroAwareCleanup {
    fn deregister_all_by_connection(&self, connection_id: &str) -> Vec<String> {
        self.naming.deregister_all_by_connection(connection_id)
    }

    fn remove_subscriber(&self, connection_id: &str) {
        self.naming.remove_subscriber(connection_id)
    }

    async fn broadcast_disconnect(&self, affected_service_keys: &[String]) {
        // Fan out one sync per affected service. These are fire-and-forget
        // at the Distro layer (the protocol enqueues a task and returns),
        // so this is cheap even for large fan-outs.
        for key in affected_service_keys {
            self.distro
                .sync_data(DistroDataType::NamingInstance, key)
                .await;
        }
    }
}
