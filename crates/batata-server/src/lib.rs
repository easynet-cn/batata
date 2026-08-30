#![warn(missing_docs)]
//! Crate root for `batata-server`.
// Main library module for Batata - a service discovery and configuration management system (Nacos-compatible)
// This file re-exports security models and common types from batata-server-common

// Module declarations
/// `api` module.
pub mod api; // API handlers and models
/// `auth` module.
pub mod auth; // Authentication and authorization
/// `builder` module.
pub mod builder; // Trait-driven application building
/// `config` module.
pub mod config; // Configuration management
/// `console` module.
pub mod console; // Console web interface
/// `context` module.
pub mod context; // Application context types
/// `error` module.
pub mod error; // Error handling and types
/// `initializer` module.
pub mod initializer; // Service initialization traits
/// `lifecycle` module.
pub mod lifecycle; // Server lifecycle management
/// `metrics` module.
pub mod metrics; // Metrics and observability
/// `middleware` module.
pub mod middleware; // HTTP middleware
/// `model` module.
pub mod model; // Data models and types
/// `registry` module.
pub mod registry; // Service and server registries
/// `service` module.
pub mod service; // Business services
/// `startup` module.
pub mod startup; // Application startup utilities

// Re-export common types from batata-common to maintain backward compatibility
/// Re-exported item.
pub use batata_common::{ActionTypes, ApiType, SignType, is_valid, local_ip};

// Re-export shared types from batata-server-common
/// Re-exported item.
pub use batata_server_common::model::{Configuration, ErrorResult};

// Re-export security types from batata-server-common
/// Re-exported item.
pub use batata_server_common::{
    ConfigHttpResourceParser, NamingHttpResourceParser, Secured, SecuredBuilder, join_resource,
};

// Re-export the secured! macro (it's #[macro_export] in server-common, so it's available
// as batata_server_common::secured, but we also need it at crate level for backward compat)
/// Re-exported item.
pub use batata_server_common::secured;

// Re-export gRPC handler macros from batata-core (they use #[macro_export])
/// Re-exported item.
pub use batata_core::define_handler;
/// Re-exported item.
pub use batata_core::error_response;
/// Re-exported item.
pub use batata_core::impl_ack_handler;
/// Re-exported item.
pub use batata_core::impl_can_handle;
/// Re-exported item.
pub use batata_core::success_response;
