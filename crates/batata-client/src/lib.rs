//! Batata Client - Rust SDK for Nacos clients
//!
//! This crate provides:
//! - HTTP client with authentication, retry, and failover
//! - API client with typed methods for all console endpoints
//! - Model types for API responses
//! - gRPC-based ConfigService and NamingService for SDK communication
//! - Server push handling for config change notifications and service updates
//! - Automatic redo on reconnect

pub mod api;
pub mod config;
pub mod error;
pub mod grpc;
pub mod http;
pub mod limiter;
pub mod local_config;
pub mod metrics;
pub mod model;
pub mod naming;
pub mod redo;

// HTTP console client re-exports
pub use api::BatataApiClient;
pub use http::{BatataHttpClient, HttpClientConfig};
pub use model::*;

// gRPC SDK client re-exports
pub use config::BatataConfigService;
pub use error::ClientError;
pub use grpc::{GrpcClient, GrpcClientConfig};
pub use naming::BatataNamingService;
pub use redo::RedoService;

// Additional re-exports
pub use limiter::{RateLimiter, SlidingWindowLimiter};
pub use metrics::{MetricsMonitor, SimpleCounter, Timer};
