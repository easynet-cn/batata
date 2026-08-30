//! Lifecycle module - trait-driven server lifecycle management
//!
//! This module provides lifecycle traits for different server types,
//! enabling consistent start/stop/health management across all servers.

/// `http_server` module.
pub mod http_server;
/// `grpc_server` module.
pub mod grpc_server;
/// `xds_server` module.
pub mod xds_server;
/// `shutdown_trait` module.
pub mod shutdown_trait;

/// Re-exported item.
pub use http_server::{HttpServerConfig, HttpServerKind, HttpServerLifecycle, HttpServerState};
/// Re-exported item.
pub use grpc_server::{GrpcServerConfig, GrpcServerKind, GrpcServerLifecycle, GrpcServerState};
/// Re-exported item.
pub use xds_server::{XdsServerConfig, XdsServerLifecycle, XdsServerState};
/// Re-exported item.
pub use shutdown_trait::*;
