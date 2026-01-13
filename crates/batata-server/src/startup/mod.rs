//! Application startup utilities module.
//!
//! This module contains shared initialization code for both the root binary
//! and the batata-server crate.

mod grpc;
mod http;
mod telemetry;

pub use grpc::{GrpcServers, start_grpc_servers};
pub use http::{ConsulServices, console_server, main_server};
pub use telemetry::{
    OtelConfig, OtelGuard, get_subscriber, init_subscriber, init_tracing_with_otel,
    shutdown_tracer_provider,
};
