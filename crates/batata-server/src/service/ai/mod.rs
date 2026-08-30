//! Module `service::ai` of the `batata-server` crate.
// AI service module - re-exports from batata-ai crate

/// `constants` module.
pub mod constants {
    /// Re-exported item.
    pub use batata_ai::service::constants::*;
}

/// `a2a_service` module.
pub mod a2a_service {
    /// Re-exported item.
    pub use batata_ai::service::a2a_service::*;
}

/// `endpoint_service` module.
pub mod endpoint_service {
    /// Re-exported item.
    pub use batata_ai::service::endpoint_service::*;
}

/// `mcp_index` module.
pub mod mcp_index {
    /// Re-exported item.
    pub use batata_ai::service::mcp_index::*;
}

/// `mcp_service` module.
pub mod mcp_service {
    /// Re-exported item.
    pub use batata_ai::service::mcp_service::*;
}

/// Re-exported item.
pub use a2a_service::A2aServerOperationService;
/// Re-exported item.
pub use endpoint_service::AiEndpointService;
/// Re-exported item.
pub use mcp_index::McpServerIndex;
/// Re-exported item.
pub use mcp_service::McpServerOperationService;
