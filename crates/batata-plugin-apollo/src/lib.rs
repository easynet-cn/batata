//! Apollo protocol-adapter plugin for batata, exposing Apollo-compatible config service APIs.
#![warn(missing_docs)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]
#![allow(clippy::unnecessary_sort_by)]
#![allow(clippy::assertions_on_constants)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::result_large_err)]
#![allow(clippy::empty_line_after_doc_comments)]

/// Re-exports the shared bincode serialization helpers.
pub mod bincode {
    pub use batata_common::bincode::{deserialize, serialize};
}
/// HTTP API DTOs for the Apollo plugin.
pub mod api;
/// Authentication and authorization for the Apollo plugin.
pub mod auth;
/// SeaORM entities for the Apollo plugin.
pub mod entity;
/// HTTP middleware for the Apollo plugin.
pub mod middleware;
/// Database migrations for the Apollo plugin.
pub mod migration;
/// Data models and DTOs for the Apollo plugin.
pub mod model;
/// Persistence layer (embedded and SQL backends).
pub mod persistence;
/// Apollo plugin registration and configuration.
pub mod plugin;
/// Raft integration for the Apollo plugin.
pub mod raft;
/// HTTP route registration for the Apollo plugin.
pub mod route;
/// Business services for the Apollo plugin.
pub mod service;

pub use model::config::ApolloPluginConfig;
pub use plugin::ApolloPlugin;
