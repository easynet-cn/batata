//! Batata Config - Configuration management service
//!
//! This crate provides:
//! - Config CRUD operations
//! - Config listening/pushing
//! - Gray release (beta configs)
//! - Import/Export functionality
//! - History management
//! - HTTP API handlers (V2, V3 admin, V3 client)
//! - Config fuzzy watch manager

pub mod api;
pub mod handler;
pub mod model;
pub mod service;

// Re-export commonly used types
pub use model::*;
