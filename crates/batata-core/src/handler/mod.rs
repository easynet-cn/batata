//! gRPC handler infrastructure and core handlers
//!
//! This module provides the PayloadHandler trait, handler registry,
//! RPC services, and core gRPC message handlers.

#[macro_use]
pub mod macros;
pub mod auth_cache;
/// Cluster module re-exports.
pub mod cluster;
pub mod distro;
/// Generic request handler implementations.
pub mod generic;
/// Distributed locking services and handlers.
pub mod lock;
pub mod param_check;
/// RPC request handlers.
pub mod rpc;
