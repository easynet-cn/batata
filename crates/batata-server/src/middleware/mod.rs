// HTTP middleware implementations
// This module contains middleware for authentication, logging, and request processing

pub mod auth; // Authentication and authorization middleware
pub mod rate_limit; // Rate limiting middleware for API protection
pub mod tracing; // Distributed tracing middleware for OpenTelemetry
