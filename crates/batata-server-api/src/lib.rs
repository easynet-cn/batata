//! Batata Server API - server-only API definitions
//!
//! This crate contains API definitions that are used only by the Batata
//! server and its components:
//! - `raft`: Raft consensus protocol gRPC service definitions (generated from proto).
//! - `distro`: Distro cluster synchronization protocol models.
//! - `validation`: Input validation utilities for API requests.
//!
//! It depends on `batata-api` for the shared protocol contract (gRPC stubs,
//! config/naming/remote models) so that server modules can reuse those types.

// Raft consensus protocol gRPC service definitions (generated from proto)
pub mod raft;
// Distro protocol API models
pub mod distro;
// Input validation utilities
pub mod validation;
