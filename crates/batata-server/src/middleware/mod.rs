// HTTP middleware implementations
// This module contains middleware for authentication, logging, and request processing

pub mod auth; // Authentication and authorization middleware (server-specific, uses AppState)

// Re-export rate limiting and tracing middleware from server-common
pub use batata_server_common::middleware::rate_limit;
pub use batata_server_common::middleware::tracing;
