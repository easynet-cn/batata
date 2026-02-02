//! Batata API - gRPC and HTTP API definitions
//!
//! This crate provides:
//! - Common API models and constants
//! - gRPC service definitions (generated from proto)
//! - HTTP API request/response models
//! - Input validation utilities

pub mod config;
pub mod distro;
pub mod grpc;
pub mod model;
pub mod naming;
pub mod raft;
pub mod remote;
pub mod validation;

// Re-export commonly used types
pub use model::*;
pub use validation::*;
