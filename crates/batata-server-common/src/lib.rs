// Shared server infrastructure for Batata
//
// This crate provides common types, middleware, and configuration used by
// both batata-server (SDK API + gRPC) and batata-console (management console).
//
// Provides:
// - AppState (central application state)
// - Configuration (CLI args, config file loading)
// - Constants (shared constant strings)
// - Response types (Result, ErrorResult, ConsoleException)
// - Middleware (Authentication, RateLimiter, Tracing)
// - Error types (AppError, BatataError re-exports)
// - TLS configuration (GrpcTlsConfig)
// - Secured / secured! macro (auth guard)
// - ConsoleDataSource trait (console data abstraction)
// - Console model types (Member, ClusterHealthResponse, etc.)

pub mod console; // Console shared types (trait, models)
pub mod error; // Error handling and types
pub mod middleware; // HTTP middleware
pub mod model; // Data models and types
pub mod secured; // Security context and secured! macro

// Re-export common types from batata-common to maintain backward compatibility
pub use batata_common::{ActionTypes, ApiType, SignType, is_valid, local_ip};

// Re-export model types for convenience
pub use model::{Configuration, ErrorResult};

// Re-export security types
pub use secured::{
    ConfigHttpResourceParser, NamingHttpResourceParser, Secured, SecuredBuilder, join_resource,
};

// Re-export AppState
pub use model::AppState;
