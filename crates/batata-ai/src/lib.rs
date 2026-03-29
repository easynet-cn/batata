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
pub use registry::a2a::{AgentCardChangeEvent, AgentChangeType, AgentRegistry, AgentRegistryStats};
pub use registry::mcp::{McpChangeType, McpRegistryStats, McpServerChangeEvent, McpServerRegistry};
pub use registry::mcp_registry::configure_mcp_registry;
pub use service::a2a_service::A2aServerOperationService;
pub use service::agentspec_service::AgentSpecOperationService;
pub use service::endpoint_service::{AiEndpointService, EndpointInfo};
pub use service::mcp_index::{McpServerIndex, McpServerIndexData};
pub use service::mcp_service::McpServerOperationService;
pub use service::pipeline_service::PipelineQueryService;
pub use service::prompt::PromptOperationService;
pub use service::skill_service::SkillOperationService;

// Re-export configure functions for route setup
pub use api::a2a::configure as configure_a2a;
pub use api::agentspec::{
    admin_routes as agentspec_admin_routes, client_routes as agentspec_client_routes,
};
pub use api::mcp::configure as configure_mcp;
pub use api::pipeline::admin_routes as pipeline_admin_routes;
pub use api::prompt::{admin_routes as prompt_admin_routes, client_routes as prompt_client_routes};
pub use api::skill::{admin_routes as skill_admin_routes, client_routes as skill_client_routes};
pub use api::skills_registry::registry_routes as skills_registry_routes;
