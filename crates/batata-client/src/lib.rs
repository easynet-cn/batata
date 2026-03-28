//! Batata Client - Rust SDK for Nacos clients
//!
//! This crate provides:
//! - HTTP client with authentication, retry, and failover
//! - API client with typed methods for all console endpoints
//! - Model types for API responses
//! - gRPC-based ConfigService and NamingService for SDK communication
//! - Server push handling for config change notifications and service updates
//! - Automatic redo on reconnect

pub mod ai;
pub mod api;
pub mod auth;
pub mod client;
pub mod client_config;
pub mod config;

// Re-export unified entry points at crate root
pub use client::BatataClient;
pub use client_config::{ClientConfig, ProxyConfig};
pub mod error;
pub mod grpc;
pub mod http;
pub mod labels;
pub mod limiter;
pub mod local_config;
pub mod lock;
pub mod metrics;
pub mod model;
pub mod naming;
pub mod redo;
pub mod retry;
pub mod server_list;
pub mod signing;
pub mod transport;

// AI/MCP client re-exports
pub use ai::{AgentCardInfo, BatataAiService, McpServerBasicInfo, McpServerDetailInfo};

// HTTP console client re-exports
pub use api::BatataApiClient;
pub use http::{BatataHttpClient, HttpClientConfig};
pub use model::*;

// gRPC SDK client re-exports
pub use config::BatataConfigService;
pub use error::ClientError;
pub use grpc::{GrpcClient, GrpcClientConfig};
pub use lock::BatataLockService;
pub use naming::BatataNamingService;
pub use redo::RedoService;

// Config change parser
pub use config::change_parser::{
    ConfigChangeEvent, ConfigChangeHandler, ConfigChangeItem, PropertyChangeType,
};

// Fuzzy watch
pub use config::fuzzy_watch::{ConfigFuzzyWatchEvent, ConfigFuzzyWatchService};
pub use naming::fuzzy_watch::{NamingFuzzyWatchEvent, NamingFuzzyWatchService};

// Instance diff
pub use naming::instances_diff::InstancesDiff;

// Client labels
pub use labels::ClientLabelsCollector;

// Request signing
pub use signing::{sign_request, sign_resource};

// Auth providers (ClientAuthService SPI)
pub use auth::{
    AccessKeyAuthProvider, ClientAuthService, IdentityContext, JwtAuthProvider, RequestResource,
    SecurityProxy,
};

// Retry and server list
pub use retry::{RetryConfig, retry_with_backoff};
pub use server_list::ServerListManager;

// Connection health
pub use grpc::health::ConnectionHealthChecker;

// Additional re-exports
pub use limiter::{RateLimiter, SlidingWindowLimiter};
pub use metrics::{MetricsMonitor, SimpleCounter, Timer};
