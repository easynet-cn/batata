//! Batata Auth - Authentication and authorization
//!
//! This crate provides:
//! - JWT token handling
//! - RBAC permission model
//! - User, Role, Permission services
//! - Auth middleware

pub mod model;
pub mod service;

// Re-export commonly used types
pub use model::*;
