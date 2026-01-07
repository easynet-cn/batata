//! Batata API - gRPC and HTTP API definitions
//!
//! This crate provides:
//! - Common API models and constants
//! - gRPC service definitions (generated from proto)
//! - HTTP API request/response models

pub mod grpc;
pub mod model;
pub mod raft;
pub mod remote;

// Re-export commonly used types
pub use model::*;
