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
