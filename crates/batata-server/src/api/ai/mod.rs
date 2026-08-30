//! AI Capabilities API
//!
//! Re-exports from the batata-ai crate.

/// `model` module.
pub mod model {
    /// Re-exported item.
    pub use batata_ai::model::*;
}

/// `mcp` module.
pub mod mcp {
    /// Re-exported item.
    pub use batata_ai::registry::mcp::*;
}

/// `a2a` module.
pub mod a2a {
    /// Re-exported item.
    pub use batata_ai::registry::a2a::*;
}

/// `mcp_registry` module.
pub mod mcp_registry {
    /// Re-exported item.
    pub use batata_ai::registry::mcp_registry::*;
}

// Re-export registry types
/// Re-exported item.
pub use a2a::{AgentCardChangeEvent, AgentChangeType, AgentRegistry, AgentRegistryStats};
/// Re-exported item.
pub use mcp::{McpChangeType, McpRegistryStats, McpServerChangeEvent, McpServerRegistry};

// Re-export config-backed service types
/// Re-exported item.
pub use batata_ai::service::A2aServerOperationService;
/// Re-exported item.
pub use batata_ai::service::McpServerOperationService;

// Re-export trait types for trait object usage
/// Re-exported item.
pub use batata_ai::service::A2aAgentService;
/// Re-exported item.
pub use batata_ai::service::McpServerService;

// Re-export configure functions for route setup
/// Re-exported item.
pub use batata_ai::configure_mcp_registry;
