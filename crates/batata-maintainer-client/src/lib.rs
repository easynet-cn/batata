//! Admin/maintainer HTTP client for Batata.
//!
//! Provides the [`MaintainerClient`] facade and the SDK trait contracts
//! [`CoreMaintainerService`], [`ConfigMaintainerService`], and
//! [`NamingMaintainerService`] for administering a Batata (Nacos-compatible)
//! server: server state, cluster, namespace, configuration, naming, AI MCP,
//! AI agent, and plugin management.

#![warn(missing_docs)]

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
