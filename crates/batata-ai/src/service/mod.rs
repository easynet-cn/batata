// AI service module - config-backed persistent services for MCP and A2A

/// Config-backed operation service for A2A (Agent-to-Agent) agents.
pub mod a2a_service;
/// Persistence-backed operation service for AgentSpecs.
pub mod agentspec_service;
/// Shared constants for AI config groups, tags, and app names.
pub mod constants;
/// NamingService-backed registration for MCP/A2A endpoints.
pub mod endpoint_service;
pub mod mcp_client;
/// DashMap-backed L1 cache index for MCP servers.
pub mod mcp_index;
/// Config-backed operation service for MCP servers.
pub mod mcp_service;
pub mod pipeline_service;
pub mod version_lifecycle;
pub mod version_range;
pub mod prompt;
pub mod skill_service;
pub mod traits;

/// Skill ZIP utilities (re-exported from batata-common)
pub mod skill_zip {
    pub use batata_common::model::ai::skill_zip::*;
}

pub use a2a_service::A2aServerOperationService;
pub use agentspec_service::AgentSpecOperationService;
pub use endpoint_service::AiEndpointService;
pub use mcp_index::McpServerIndex;
pub use mcp_service::McpServerOperationService;
pub use skill_service::SkillOperationService;
pub use traits::A2aAgentService;
pub use traits::McpServerService;
