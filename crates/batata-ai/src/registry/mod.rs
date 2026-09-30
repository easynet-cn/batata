//! In-memory registries for MCP servers and A2A agents

pub mod a2a;
pub mod mcp;
pub mod mcp_registry;

/// Error returned by the in-memory registries for operations that require the
/// versioned, `ai_resource`-backed services.
pub(crate) const VERSIONING_UNSUPPORTED: &str =
    "versioning is not supported by the in-memory registry; \
     it requires the ai_resource-backed service";
