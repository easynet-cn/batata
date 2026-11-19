//! Istio MCP (Mesh Configuration Protocol) Support
//!
//! This module provides MCP server implementation for Istio integration,
//! allowing Istio control plane to discover services from Batata.

pub mod encode;
pub mod grpc;
pub mod server;
pub mod types;

pub use encode::{
    SERVICE_ENTRY_COLLECTION, SERVICE_ENTRY_TYPE_URL, encode_resource, encode_resources,
    encode_service_entry,
};
pub use grpc::{BatataMcpService, McpSnapshot, start_istio_mcp_grpc_server_on_listener};
pub use server::{McpServer, McpServerConfig};
pub use types::*;
