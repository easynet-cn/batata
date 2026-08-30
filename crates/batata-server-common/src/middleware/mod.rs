// HTTP middleware implementations
// This module contains middleware for authentication, logging, and request processing

/// Provides the `auth` module.
pub mod auth; // Authentication and authorization middleware
pub mod distro_filter; // Distro AP mode request routing for naming writes
/// Provides the `http_metrics` module.
pub mod http_metrics; // HTTP request metrics (count, duration, errors)
/// Provides the `rate_limit` module.
pub mod rate_limit; // Rate limiting middleware for API protection
pub mod tps_control; // Per-endpoint TPS rate limiting via control plugin
pub mod tracing; // Distributed tracing middleware for OpenTelemetry
pub mod traffic_revise; // Traffic filter for server startup lifecycle
