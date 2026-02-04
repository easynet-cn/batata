//! Application startup utilities module.
//!
//! This module contains shared initialization code for both the root binary
//! and the batata-server crate.

mod dns;
mod grpc;
mod http;
mod shutdown;
mod telemetry;
mod xds;

pub use dns::{DnsConfig, DnsServer};
pub use grpc::{GrpcServers, start_grpc_servers};
pub use http::{ApolloServices, ConsulServices, console_server, main_server};
pub use shutdown::{GracefulShutdown, ShutdownSignal, run_with_shutdown, wait_for_shutdown_signal};
pub use telemetry::{
    OtelConfig, OtelGuard, get_subscriber, init_subscriber, init_tracing_with_otel,
    shutdown_tracer_provider,
};
pub use xds::{XdsServerHandle, start_xds_service};
