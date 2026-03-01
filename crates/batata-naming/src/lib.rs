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
pub mod model;
pub mod persistence;
pub mod selector;
pub mod service;

// Re-export commonly used types
pub use healthcheck::{
    CheckOrigin, CheckStatus, CheckType, ExpiredInstanceChecker, HealthCheckConfig,
    HealthCheckManager, HealthCheckProcessor, HealthCheckReactor, HealthCheckResult,
    HealthCheckTask, HealthCheckType, HttpHealthCheckProcessor, HttpHealthParams,
    InstanceCheckConfig, InstanceCheckRegistry, InstanceCheckStatus, TcpHealthCheckProcessor,
    TcpHealthParams, UnhealthyInstanceChecker,
};
pub use model::{Instance, Service, ServiceInfo, ServiceQuery};
pub use selector::{LabelOperator, LabelRequirement, SelectorBuilder, ServiceSelector};
pub use service::{
    ClusterConfig, ClusterStatistics, FuzzyWatchPattern, NamingService, ProtectionInfo,
    ServiceMetadata,
};

// Implement ConnectionCleanupHandler for NamingService
use batata_core::handler::rpc::ConnectionCleanupHandler;

#[tonic::async_trait]
impl ConnectionCleanupHandler for NamingService {
    fn deregister_all_by_connection(&self, connection_id: &str) -> Vec<String> {
        self.deregister_all_by_connection(connection_id)
    }

    fn remove_subscriber(&self, connection_id: &str) {
        self.remove_subscriber(connection_id)
    }
}
