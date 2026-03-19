// HTTP middleware implementations
// This module contains middleware for authentication, logging, and request processing

pub mod auth; // Authentication and authorization middleware
pub mod distro_filter; // Distro AP mode request routing for naming writes
pub mod rate_limit; // Rate limiting middleware for API protection
pub mod tps_control; // Per-endpoint TPS rate limiting via control plugin
pub mod tracing; // Distributed tracing middleware for OpenTelemetry
pub mod traffic_revise; // Traffic filter for server startup lifecycle
