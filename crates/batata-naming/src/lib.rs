//! Batata Naming - Service discovery
//!
//! This crate provides:
//! - Service registration
//! - Service discovery
//! - Health checking (TCP/HTTP)
//! - Load balancing
//! - Service selector evaluation

pub mod health_checker;
pub mod model;
pub mod selector;
pub mod service;

// Re-export commonly used types
pub use health_checker::{InstanceHealthCheckConfig, InstanceHealthChecker, InstanceHealthStatus};
pub use model::{Instance, Service, ServiceInfo, ServiceQuery};
pub use selector::{LabelOperator, LabelRequirement, SelectorBuilder, ServiceSelector};
pub use service::{
    ClusterConfig, FuzzyWatchPattern, NamingService, ProtectionInfo, ServiceMetadata,
};
