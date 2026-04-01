//! AI Capabilities API
//!
//! Re-exports from the batata-ai crate.

pub mod model {
    pub use batata_ai::model::*;
}

pub mod mcp {
    pub use batata_ai::registry::mcp::*;
}

pub mod a2a {
    pub use batata_ai::registry::a2a::*;
}

pub mod mcp_registry {
    pub use batata_ai::registry::mcp_registry::*;
}

// Re-export registry types
pub use a2a::{AgentCardChangeEvent, AgentChangeType, AgentRegistry, AgentRegistryStats};
pub use mcp::{McpChangeType, McpRegistryStats, McpServerChangeEvent, McpServerRegistry};

// Re-export config-backed service types
pub use batata_ai::service::A2aServerOperationService;
pub use batata_ai::service::McpServerOperationService;

// Re-export trait types for trait object usage
pub use batata_ai::service::A2aAgentService;
pub use batata_ai::service::McpServerService;

// Re-export configure functions for route setup
pub use batata_ai::configure_mcp_registry;
