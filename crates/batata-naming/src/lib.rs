//! Batata Naming - Service discovery
//!
//! This crate provides:
//! - Service registration
//! - Service discovery
//! - Nacos-compatible health checking (TCP/HTTP with heartbeat monitoring)
//! - Load balancing
//! - Service selector evaluation
//! - Metadata persistence

pub mod health_checker;
pub mod healthcheck;
pub mod model;
pub mod persistence;
pub mod selector;
pub mod service;

// Re-export commonly used types
pub use health_checker::{InstanceHealthCheckConfig, InstanceHealthChecker, InstanceHealthStatus};
pub use healthcheck::{
    ExpiredInstanceChecker, HealthCheckConfig, HealthCheckManager, HealthCheckProcessor,
    HealthCheckResult, HealthCheckReactor, HealthCheckTask, HealthCheckType, HttpHealthParams,
    HttpHealthCheckProcessor, TcpHealthParams, TcpHealthCheckProcessor, UnhealthyInstanceChecker,
};
pub use model::{Instance, Service, ServiceInfo, ServiceQuery};
pub use selector::{LabelOperator, LabelRequirement, SelectorBuilder, ServiceSelector};
pub use service::{
    ClusterConfig, ClusterStatistics, FuzzyWatchPattern, NamingService, ProtectionInfo, ServiceMetadata,
};
