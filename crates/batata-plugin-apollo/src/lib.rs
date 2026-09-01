//! Apollo protocol-adapter plugin for batata, exposing Apollo-compatible config service APIs.
#![warn(missing_docs)]

/// Re-exports the shared bincode serialization helpers.
pub mod bincode {
    pub use batata_common::bincode::{deserialize, serialize};
}
/// Apollo plugin registration and configuration.
pub mod plugin;
/// Database migrations for the Apollo plugin.
pub mod migration;
/// SeaORM entities for the Apollo plugin.
pub mod entity;
/// Data models and DTOs for the Apollo plugin.
pub mod model;
/// Persistence layer (embedded and SQL backends).
pub mod persistence;
/// HTTP API DTOs for the Apollo plugin.
pub mod api;
/// HTTP middleware for the Apollo plugin.
pub mod middleware;
/// Business services for the Apollo plugin.
pub mod service;
/// HTTP route registration for the Apollo plugin.
pub mod route;
/// Authentication and authorization for the Apollo plugin.
pub mod auth;
/// Raft integration for the Apollo plugin.
pub mod raft;

pub use plugin::ApolloPlugin;
pub use model::config::ApolloPluginConfig;
