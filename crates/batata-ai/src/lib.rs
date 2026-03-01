//! AI capabilities for Batata - MCP (Model Content Protocol) and A2A (Agent-to-Agent)
//!
//! This crate provides:
//! - Data models for MCP servers and A2A agents
//! - In-memory registries (MCP and A2A)
//! - Config-backed persistent services
//! - HTTP API handlers for AI routes
//! - Console API handlers for AI management

pub mod api;
pub mod handler;
pub mod model;
pub mod registry;
pub mod service;

// Re-export key types
pub use registry::a2a::{AgentRegistry, AgentRegistryStats};
pub use registry::mcp::{McpRegistryStats, McpServerRegistry};
pub use registry::mcp_registry::configure_mcp_registry;
pub use service::a2a_service::A2aServerOperationService;
pub use service::endpoint_service::{AiEndpointService, EndpointInfo};
pub use service::mcp_index::{McpServerIndex, McpServerIndexData};
pub use service::mcp_service::McpServerOperationService;

// Re-export configure functions for route setup
pub use api::a2a::configure as configure_a2a;
pub use api::mcp::configure as configure_mcp;
