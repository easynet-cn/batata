//! Lifecycle module - trait-driven server lifecycle management
//!
//! This module provides lifecycle traits for different server types,
//! enabling consistent start/stop/health management across all servers.

pub mod http_server;
pub mod grpc_server;
pub mod xds_server;
pub mod shutdown_trait;

pub use http_server::{HttpServerConfig, HttpServerKind, HttpServerLifecycle, HttpServerState};
pub use grpc_server::{GrpcServerConfig, GrpcServerKind, GrpcServerLifecycle, GrpcServerState};
pub use xds_server::{XdsServerConfig, XdsServerLifecycle, XdsServerState};
pub use shutdown_trait::*;
