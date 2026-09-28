//! Constants for the AI module's NamingService usage.
//!
//! AI resources (MCP servers, A2A agents) are stored in `ai_resource` /
//! `ai_resource_version`. Their **runtime endpoints** are still resolved
//! through NamingService, so the group and metadata keys below remain in use
//! (see `service/endpoint_service.rs`).
//!
//! The config-backed storage constants that used to live here (config groups,
//! data-id suffixes, tag keys) were removed once every AI domain moved onto
//! `ai_resource`.

// =============================================================================
// NamingService Groups (used as service group for endpoint registration)
// =============================================================================

/// Naming group for MCP server endpoints
pub const MCP_ENDPOINT_GROUP: &str = "mcp-endpoints";

/// Naming group for A2A agent endpoints
pub const AGENT_ENDPOINT_GROUP: &str = "agent-endpoints";

// =============================================================================
// NamingService Metadata Keys
// =============================================================================

/// Metadata key marking a naming service entry as an AI/MCP managed service
pub const METADATA_AI_MCP_SERVICE: &str = "__nacos.ai.mcp.service__";

/// Metadata key marking a naming service entry as an AI/A2A managed service
pub const METADATA_AI_A2A_SERVICE: &str = "__nacos.ai.a2a.service__";

// =============================================================================
// Helper Functions
// =============================================================================

/// Suffix for MCP server tool description references.
///
/// `tools_description_ref` is stored on the version payload; it is not a
/// `config_info` data ID any more, but the format is kept for wire
/// compatibility.
pub const MCP_TOOL_SUFFIX: &str = "-mcp-tools.json";

/// Build the tool description reference for an MCP server version.
pub fn mcp_tool_data_id(id: &str, version: &str) -> String {
    format!("{}-{}{}", id, version, MCP_TOOL_SUFFIX)
}

/// Build the naming service name for an MCP endpoint
pub fn mcp_service_name(name: &str, version: &str) -> String {
    format!("{}::{}", name, version)
}

/// Build the naming service name for an A2A agent endpoint
pub fn a2a_service_name(name: &str, version: &str) -> String {
    format!("{}::{}", name, version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_tool_data_id() {
        assert_eq!(
            mcp_tool_data_id("abc123", "1.0.0"),
            "abc123-1.0.0-mcp-tools.json"
        );
    }

    #[test]
    fn test_mcp_service_name() {
        assert_eq!(mcp_service_name("my-mcp", "1.0.0"), "my-mcp::1.0.0");
    }
}
