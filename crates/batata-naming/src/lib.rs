//! Batata Naming - Service discovery
//!
//! This crate provides:
//! - Service registration
//! - Service discovery
//! - Health checking
//! - Load balancing
//! - Persistent instance support

pub mod model;
pub mod persistent;
pub mod service;

// Re-export commonly used types
pub use model::{Instance, Service, ServiceInfo, ServiceQuery};
pub use persistent::PersistentNamingService;
pub use service::{ClusterConfig, FuzzyWatchPattern, NamingService, ServiceMetadata};
