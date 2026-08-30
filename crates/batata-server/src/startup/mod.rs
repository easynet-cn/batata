//! Application startup utilities module.
//!
//! This module contains shared initialization code for both the root binary
//! and the batata-server crate.

/// `cluster` module.
pub mod cluster;
mod dns;
mod grpc;
mod http;
/// `logging` module.
pub mod logging;
/// `persistence` module.
pub mod persistence;
mod shutdown;
mod telemetry;
mod xds;

/// Re-exported item.
pub use dns::{DnsConfig, DnsServer};
/// Re-exported item.
pub use grpc::{GrpcServers, start_grpc_servers};
#[cfg(feature = "consul")]
/// Re-exported item.
pub use http::plugin_http_server;
/// Re-exported item.
pub use http::{AIServices, console_server, main_server, mcp_registry_server};
/// Re-exported item.
pub use logging::{LogRotation, LoggingConfig, LoggingGuard, init_file_logging, init_logging};
/// Re-exported item.
pub use shutdown::{GracefulShutdown, ShutdownSignal, run_with_shutdown, wait_for_shutdown_signal};
/// Re-exported item.
pub use telemetry::{
    OtelConfig, OtelGuard, get_subscriber, init_subscriber, init_tracing_with_otel,
    shutdown_tracer_provider,
};
/// Re-exported item.
pub use xds::{XdsServerHandle, start_xds_service};
