//! Istio MCP (Mesh Configuration Protocol) Support
//!
//! This module provides MCP server implementation for Istio integration,
//! allowing Istio control plane to discover services from Batata.

pub mod server;
pub mod types;

pub use server::{McpServer, McpServerConfig};
pub use types::*;
