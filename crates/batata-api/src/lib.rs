#![warn(missing_docs)]
//! Batata API - gRPC and HTTP API definitions
//!
//! This crate provides:
//! - Common API models and constants
//! - gRPC service definitions (generated from proto)
//! - HTTP API request/response models
//! - Input validation utilities

/// Shared macros for API definitions.
#[macro_use]
pub mod macros;
/// Config API models.
pub mod config;
/// Distro protocol models.
pub mod distro;
/// gRPC service definitions (generated from proto).
pub mod grpc;
/// Common API models and constants.
pub mod model;
/// Naming/service discovery API models.
pub mod naming;
/// Raft protocol models.
pub mod raft;
/// Remote API models.
pub mod remote;
/// Input validation utilities.
pub mod validation;

// Re-export commonly used types
pub use model::*;
pub use validation::*;
