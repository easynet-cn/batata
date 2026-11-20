//! Istio MCP (Mesh Configuration Protocol) Support
//!
//! This module provides MCP server implementation for Istio integration,
//! allowing Istio control plane to discover services from Batata.

pub mod ads;
pub mod encode;
pub mod grpc;
pub mod server;
pub mod types;
pub mod virtual_service;

pub use ads::BatataIstioMcpAdsService;
pub use encode::{
    MCP_RESOURCE_TYPE_URL, MESH_CONFIG_GVK_TYPE_URL, SERVICE_ENTRY_COLLECTION,
    SERVICE_ENTRY_GVK_TYPE_URL, SERVICE_ENTRY_TYPE_URL, encode_ads_resources, encode_resource,
    encode_resources, encode_service_entry,
};
pub use grpc::{BatataMcpService, McpSnapshot, start_istio_mcp_grpc_server_on_listener};
pub use server::{McpServer, McpServerConfig};
pub use types::*;
pub use virtual_service::{
    DESTINATION_RULE_TAG, IstioConfigSourceEntry, ROUTE_CONFIG_SUFFIX, VIRTUAL_SERVICE_API_VERSION,
    VIRTUAL_SERVICE_KIND, VIRTUAL_SERVICE_TAG, build_istio_mcp_snapshot, is_istio_config_tag,
    parse_virtual_service_config, virtual_service_to_route_config,
};
