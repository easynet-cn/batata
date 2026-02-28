// Main library module for Batata - A Nacos-compatible service discovery and configuration management system
// This file re-exports security models and common types from batata-server-common

// Module declarations
#[macro_use]
mod macros; // Internal macros
pub mod api; // API handlers and models
pub mod auth; // Authentication and authorization
pub mod config; // Configuration management
pub mod console; // Console web interface
pub mod error; // Error handling and types
pub mod metrics; // Metrics and observability
pub mod middleware; // HTTP middleware
pub mod model; // Data models and types
pub mod service; // Business services
pub mod startup; // Application startup utilities

// Re-export common types from batata-common to maintain backward compatibility
pub use batata_common::{ActionTypes, ApiType, SignType, is_valid, local_ip};

// Re-export shared types from batata-server-common
pub use batata_server_common::model::{Configuration, ErrorResult};

// Re-export security types from batata-server-common
pub use batata_server_common::{
    ConfigHttpResourceParser, NamingHttpResourceParser, Secured, SecuredBuilder, join_resource,
};

// Re-export the secured! macro (it's #[macro_export] in server-common, so it's available
// as batata_server_common::secured, but we also need it at crate level for backward compat)
pub use batata_server_common::secured;
