//! Admin/maintainer HTTP client for Batata.
//!
//! Provides the [`MaintainerClient`] facade and the SDK trait contracts
//! [`CoreMaintainerService`], [`ConfigMaintainerService`], and
//! [`NamingMaintainerService`] for administering a Batata (Nacos-compatible)
//! server: server state, cluster, namespace, configuration, naming, AI MCP,
//! AI agent, and plugin management.

#![warn(missing_docs)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]
#![allow(clippy::unnecessary_sort_by)]
#![allow(clippy::assertions_on_constants)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::result_large_err)]
#![allow(clippy::empty_line_after_doc_comments)]

pub mod client;
pub mod config;
pub mod constants;
pub mod error;
pub mod model;
pub mod traits;

pub use client::MaintainerClient;
pub use config::MaintainerClientConfig;
pub use error::MaintainerError;
pub use traits::{ConfigMaintainerService, CoreMaintainerService, NamingMaintainerService};
