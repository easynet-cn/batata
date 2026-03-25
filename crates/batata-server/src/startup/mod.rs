//! Application startup utilities module.
//!
//! This module contains shared initialization code for both the root binary
//! and the batata-server crate.

pub mod cluster;
#[cfg(feature = "consul")]
pub mod consul_init;
mod dns;
mod grpc;
mod http;
mod logging;
pub mod persistence;
mod shutdown;
mod telemetry;
#[cfg(feature = "mesh")]
mod xds;

pub use dns::{DnsConfig, DnsServer};
pub use grpc::{GrpcServers, start_grpc_servers};
pub use http::{
    AIServices, ConsulServices, console_server, consul_server, main_server, mcp_registry_server,
};
pub use logging::{LogRotation, LoggingConfig, LoggingGuard, init_file_logging, init_logging};
pub use shutdown::{GracefulShutdown, ShutdownSignal, run_with_shutdown, wait_for_shutdown_signal};
pub use telemetry::{
    OtelConfig, OtelGuard, get_subscriber, init_subscriber, init_tracing_with_otel,
    shutdown_tracer_provider,
};
#[cfg(feature = "mesh")]
pub use xds::{XdsServerHandle, start_xds_service};
