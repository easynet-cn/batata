// AI service module - config-backed persistent services for MCP and A2A

pub mod a2a_service;
pub mod agentspec_service;
pub mod constants;
pub mod endpoint_service;
pub mod mcp_client;
pub mod mcp_index;
pub mod mcp_service;
pub mod pipeline_service;
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
