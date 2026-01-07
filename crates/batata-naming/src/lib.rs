//! Batata Naming - Service discovery
//!
//! This crate provides:
//! - Service registration
//! - Service discovery
//! - Health checking
//! - Load balancing

pub mod model;
pub mod service;

// Re-export commonly used types
pub use model::{Instance, Service, ServiceInfo, ServiceQuery};
pub use service::NamingService;
