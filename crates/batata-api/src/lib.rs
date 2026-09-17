#![warn(missing_docs)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]
#![allow(clippy::unnecessary_sort_by)]
#![allow(clippy::assertions_on_constants)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::result_large_err)]
#![allow(clippy::empty_line_after_doc_comments)]
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
