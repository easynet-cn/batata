//! Batata Config - Configuration management service
//!
//! This crate provides:
//! - Config CRUD operations
//! - Config listening/pushing
//! - Gray release (beta configs)
//! - Import/Export functionality
//! - History management

pub mod model;
pub mod service;

// Re-export commonly used types
pub use model::*;
