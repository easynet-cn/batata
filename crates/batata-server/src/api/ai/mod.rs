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
pub use a2a::{AgentRegistry, AgentRegistryStats};
pub use mcp::{McpRegistryStats, McpServerRegistry};

// Re-export configure functions for route setup
pub use batata_ai::configure_a2a;
pub use batata_ai::configure_mcp;
pub use batata_ai::configure_mcp_registry;
