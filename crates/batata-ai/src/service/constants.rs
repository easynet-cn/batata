// AI/MCP persistent storage constants
// Aligned with Nacos 3.x AI module config-backed storage patterns

// =============================================================================
// Config Groups (used as group_id in config_info table)
// =============================================================================

/// Config group for MCP server version index entries
pub const MCP_SERVER_VERSIONS_GROUP: &str = "mcp-server-versions";

/// Config group for MCP server spec entries (per-version)
pub const MCP_SERVER_GROUP: &str = "mcp-server";

/// Config group for MCP server tool specifications (per-version)
pub const MCP_SERVER_TOOL_GROUP: &str = "mcp-tools";

/// Config group for A2A agent version index entries
pub const AGENT_GROUP: &str = "agent";

/// Config group for A2A agent detail entries (per-version)
pub const AGENT_VERSION_GROUP: &str = "agent-version";

// =============================================================================
// Data ID Suffixes
// =============================================================================

/// Suffix for MCP server version info data IDs: `{id}-mcp-versions.json`
pub const MCP_VERSION_SUFFIX: &str = "-mcp-versions.json";

/// Suffix for MCP server spec data IDs: `{id}-{version}-mcp-server.json`
pub const MCP_SPEC_SUFFIX: &str = "-mcp-server.json";

/// Suffix for MCP server tools data IDs: `{id}-{version}-mcp-tools.json`
pub const MCP_TOOL_SUFFIX: &str = "-mcp-tools.json";

// =============================================================================
// NamingService Groups (used as service group for endpoint registration)
// =============================================================================

/// Naming group for MCP server endpoints
pub const MCP_ENDPOINT_GROUP: &str = "mcp-endpoints";

/// Naming group for A2A agent endpoints
pub const AGENT_ENDPOINT_GROUP: &str = "agent-endpoints";

// =============================================================================
// Tag Keys (stored in config_tags_relation)
// =============================================================================

/// Tag indicating internal AI config entry
pub const TAG_INTERNAL_CONFIG: &str = "batata.internal.config";

/// Tag value for MCP-type internal config
pub const TAG_INTERNAL_CONFIG_MCP: &str = "mcp";

/// Tag value for A2A-type internal config
pub const TAG_INTERNAL_CONFIG_A2A: &str = "a2a";

/// Tag key for MCP server name (used in name-based search)
pub const TAG_MCP_SERVER_NAME: &str = "mcpServerName";

/// Tag key for A2A agent name (used in name-based search)
pub const TAG_AGENT_NAME: &str = "agentName";

// =============================================================================
// NamingService Metadata Keys
// =============================================================================

/// Metadata key marking a naming service entry as an AI/MCP managed service
pub const METADATA_AI_MCP_SERVICE: &str = "__nacos.ai.mcp.service__";

/// Metadata key marking a naming service entry as an AI/A2A managed service
pub const METADATA_AI_A2A_SERVICE: &str = "__nacos.ai.a2a.service__";

// =============================================================================
// App Name (used in config_info.app_name for AI entries)
// =============================================================================

/// App name for AI config entries
pub const AI_APP_NAME: &str = "nacos-ai";

/// Config type for AI config entries (stored as JSON)
pub const AI_CONFIG_TYPE: &str = "json";

/// Source user for system-managed AI config operations
pub const AI_SRC_USER: &str = "nacos-ai-system";

// =============================================================================
// Helper Functions
// =============================================================================

/// Build the data ID for an MCP server version info entry
pub fn mcp_version_data_id(id: &str) -> String {
    format!("{}{}", id, MCP_VERSION_SUFFIX)
}

/// Build the data ID for an MCP server spec entry
pub fn mcp_spec_data_id(id: &str, version: &str) -> String {
    format!("{}-{}{}", id, version, MCP_SPEC_SUFFIX)
}

/// Build the data ID for an MCP server tools entry
pub fn mcp_tool_data_id(id: &str, version: &str) -> String {
    format!("{}-{}{}", id, version, MCP_TOOL_SUFFIX)
}

/// Build tags string for an MCP server config entry
pub fn mcp_tags(name: &str) -> String {
    format!(
        "{}={},{}={}",
        TAG_INTERNAL_CONFIG, TAG_INTERNAL_CONFIG_MCP, TAG_MCP_SERVER_NAME, name
    )
}

/// Build tags string for an A2A agent config entry
pub fn a2a_tags(name: &str) -> String {
    format!(
        "{}={},{}={}",
        TAG_INTERNAL_CONFIG, TAG_INTERNAL_CONFIG_A2A, TAG_AGENT_NAME, name
    )
}

/// Encode agent name for use as data ID (URL-safe)
pub fn encode_agent_name(name: &str) -> String {
    name.replace('/', "_SLASH_")
        .replace(':', "_COLON_")
        .replace(' ', "_SPACE_")
}

/// Decode agent name from data ID
pub fn decode_agent_name(encoded: &str) -> String {
    encoded
        .replace("_SLASH_", "/")
        .replace("_COLON_", ":")
        .replace("_SPACE_", " ")
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
    fn test_mcp_version_data_id() {
        assert_eq!(mcp_version_data_id("abc123"), "abc123-mcp-versions.json");
    }

    #[test]
    fn test_mcp_spec_data_id() {
        assert_eq!(
            mcp_spec_data_id("abc123", "1.0.0"),
            "abc123-1.0.0-mcp-server.json"
        );
    }

    #[test]
    fn test_mcp_tool_data_id() {
        assert_eq!(
            mcp_tool_data_id("abc123", "1.0.0"),
            "abc123-1.0.0-mcp-tools.json"
        );
    }

    #[test]
    fn test_mcp_tags() {
        let tags = mcp_tags("my-server");
        assert!(tags.contains("batata.internal.config=mcp"));
        assert!(tags.contains("mcpServerName=my-server"));
    }

    #[test]
    fn test_encode_decode_agent_name() {
        let name = "my/agent:v1";
        let encoded = encode_agent_name(name);
        assert_eq!(encoded, "my_SLASH_agent_COLON_v1");
        assert_eq!(decode_agent_name(&encoded), name);
    }

    #[test]
    fn test_mcp_service_name() {
        assert_eq!(mcp_service_name("my-mcp", "1.0.0"), "my-mcp::1.0.0");
    }
}
