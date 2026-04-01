//! MCP (Model Content Protocol) Server Models

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::{
    HealthStatus, VersionDetail, default_namespace, default_page, default_page_size, default_true,
    default_version,
};

// =============================================================================
// MCP (Model Content Protocol) Server Models
// =============================================================================

/// MCP Server registration request
///
/// Aligned with Nacos McpServerBasicInfo: all fields are nullable in Java (no required fields).
/// The name may be filled from the form-level `mcpName` parameter when not present in
/// the serverSpecification JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerRegistration {
    /// Server name (unique identifier) -- may be empty, filled from form mcpName
    #[serde(default)]
    pub name: String,

    /// Server display name
    #[serde(default)]
    pub display_name: String,

    /// Server description
    #[serde(default)]
    pub description: String,

    /// Namespace for the server
    #[serde(default = "default_namespace")]
    pub namespace: String,

    /// Server version
    #[serde(default = "default_version")]
    pub version: String,

    /// Server endpoint URL (may be empty for stdio/local servers)
    #[serde(default)]
    pub endpoint: String,

    /// Server type (e.g., "stdio", "http", "sse")
    #[serde(default = "default_server_type")]
    pub server_type: McpServerType,

    /// Transport type
    #[serde(default)]
    pub transport: McpTransport,

    /// Server capabilities
    #[serde(default)]
    pub capabilities: McpCapabilities,

    /// Tools provided by the server
    #[serde(default)]
    pub tools: Vec<McpTool>,

    /// Resources provided by the server
    #[serde(default)]
    pub resources: Vec<McpResource>,

    /// Prompts provided by the server
    #[serde(default)]
    pub prompts: Vec<McpPrompt>,

    /// Server metadata
    #[serde(default)]
    pub metadata: HashMap<String, String>,

    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,

    /// Whether to auto-fetch tools from the server
    #[serde(default)]
    pub auto_fetch_tools: bool,

    /// Health check configuration
    #[serde(default)]
    pub health_check: Option<HealthCheck>,
}

fn default_server_type() -> McpServerType {
    McpServerType::Http
}

/// MCP Server type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum McpServerType {
    /// Standard I/O server (command line)
    Stdio,
    /// HTTP server
    #[default]
    Http,
    /// Server-Sent Events
    Sse,
    /// WebSocket server
    WebSocket,
}

/// MCP Transport configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpTransport {
    /// Transport type (e.g., "stdio", "http", "sse")
    #[serde(default)]
    pub transport_type: String,

    /// Command for stdio transport
    pub command: Option<String>,

    /// Arguments for stdio transport
    #[serde(default)]
    pub args: Vec<String>,

    /// Environment variables for stdio transport
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// HTTP URL for HTTP transport
    pub url: Option<String>,

    /// HTTP headers
    #[serde(default)]
    pub headers: HashMap<String, String>,

    /// Timeout in milliseconds
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

fn default_timeout() -> u64 {
    30000
}

/// MCP Server capabilities
/// MCP Server capability enum -- aligned with Nacos McpCapability.
/// Marks what the MCP server supports (tools, prompts, resources).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum McpCapability {
    Tool,
    Prompt,
    Resource,
}

/// Legacy McpCapabilities struct -- kept as type alias for backward compatibility.
/// New code should use `Vec<McpCapability>` directly.
pub type McpCapabilities = Vec<McpCapability>;

/// MCP Tool definition
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpTool {
    /// Tool name
    pub name: String,

    /// Tool description
    #[serde(default)]
    pub description: String,

    /// Input schema (JSON Schema)
    #[serde(default)]
    pub input_schema: serde_json::Value,
}

/// MCP Resource definition
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpResource {
    /// Resource URI
    pub uri: String,

    /// Resource name
    pub name: String,

    /// Resource description
    #[serde(default)]
    pub description: String,

    /// MIME type
    #[serde(default)]
    pub mime_type: String,
}

/// MCP Prompt definition
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpPrompt {
    /// Prompt name
    pub name: String,

    /// Prompt description
    #[serde(default)]
    pub description: String,

    /// Prompt arguments
    #[serde(default)]
    pub arguments: Vec<McpPromptArgument>,
}

/// MCP Prompt argument
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpPromptArgument {
    /// Argument name
    pub name: String,

    /// Argument description
    #[serde(default)]
    pub description: String,

    /// Whether the argument is required
    #[serde(default)]
    pub required: bool,
}

/// Health check configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthCheck {
    /// Health check type
    pub check_type: HealthCheckType,

    /// Health check interval in seconds
    #[serde(default = "default_health_interval")]
    pub interval_seconds: u64,

    /// Health check timeout in seconds
    #[serde(default = "default_health_timeout")]
    pub timeout_seconds: u64,

    /// Health check path (for HTTP)
    #[serde(default)]
    pub path: String,
}

fn default_health_interval() -> u64 {
    30
}

fn default_health_timeout() -> u64 {
    5
}

/// Health check type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthCheckType {
    /// No health check
    #[default]
    None,
    /// TCP connection check
    Tcp,
    /// HTTP health check
    Http,
    /// MCP ping check
    Mcp,
}

/// Registered MCP Server
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServer {
    /// Server ID (auto-generated)
    pub id: String,

    /// Server name
    pub name: String,

    /// Server display name
    pub display_name: String,

    /// Server description
    pub description: String,

    /// Namespace
    pub namespace: String,

    /// Server version
    pub version: String,

    /// Server endpoint URL
    pub endpoint: String,

    /// Server type
    pub server_type: McpServerType,

    /// Transport configuration
    pub transport: McpTransport,

    /// Server capabilities
    pub capabilities: McpCapabilities,

    /// Tools provided by the server
    pub tools: Vec<McpTool>,

    /// Resources provided by the server
    pub resources: Vec<McpResource>,

    /// Prompts provided by the server
    pub prompts: Vec<McpPrompt>,

    /// Server metadata
    pub metadata: HashMap<String, String>,

    /// Tags for categorization
    pub tags: Vec<String>,

    /// Health status
    pub health_status: HealthStatus,

    /// Registration timestamp (milliseconds)
    pub registered_at: i64,

    /// Last health check timestamp (milliseconds)
    pub last_health_check: Option<i64>,

    /// Last updated timestamp (milliseconds)
    pub updated_at: i64,
}

/// MCP Server query parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerQuery {
    /// Filter by namespace
    pub namespace: Option<String>,

    /// Filter by name pattern (supports wildcards)
    #[serde(alias = "namePattern")]
    pub name_pattern: Option<String>,

    /// Filter by tags (any match)
    pub tags: Option<Vec<String>>,

    /// Filter by server type
    #[serde(alias = "serverType")]
    pub server_type: Option<McpServerType>,

    /// Filter by health status
    #[serde(alias = "healthStatus")]
    pub health_status: Option<HealthStatus>,

    /// Filter by capability
    #[serde(alias = "hasTools")]
    pub has_tools: Option<bool>,

    /// Filter by capability
    #[serde(alias = "hasResources")]
    pub has_resources: Option<bool>,

    /// Filter by capability
    #[serde(alias = "hasPrompts")]
    pub has_prompts: Option<bool>,

    /// Page number (1-indexed)
    #[serde(default = "default_page")]
    pub page: u32,

    /// Page size
    #[serde(default = "default_page_size", alias = "pageSize")]
    pub page_size: u32,
}

impl Default for McpServerQuery {
    fn default() -> Self {
        Self {
            namespace: None,
            name_pattern: None,
            tags: None,
            server_type: None,
            health_status: None,
            has_tools: None,
            has_resources: None,
            has_prompts: None,
            page: default_page(),
            page_size: default_page_size(),
        }
    }
}

/// MCP Server basic info -- lightweight summary for list responses.
/// Aligned with Nacos `McpServerBasicInfo` Java class.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerBasicInfo {
    #[serde(default)]
    pub namespace_id: String,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub protocol: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_mcp_status")]
    pub status: String,
    #[serde(default)]
    pub capabilities: McpCapabilities,
}

fn default_mcp_status() -> String {
    "ACTIVE".to_string()
}

/// Nacos-compatible MCP detail query params
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpDetailQuery {
    /// Namespace ID (defaults to "public")
    #[serde(alias = "namespaceId")]
    pub namespace_id: Option<String>,
    /// MCP server ID
    #[serde(alias = "mcpId")]
    pub mcp_id: Option<String>,
    /// MCP server name
    #[serde(alias = "mcpName")]
    pub mcp_name: Option<String>,
    /// Version
    pub version: Option<String>,
}

/// Nacos-compatible MCP list query params
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpListQuery {
    /// Namespace ID
    #[serde(alias = "namespaceId")]
    pub namespace_id: Option<String>,
    /// MCP server name
    #[serde(alias = "mcpName")]
    pub mcp_name: Option<String>,
    /// Search type: "accurate" or "blur"
    pub search: Option<String>,
    /// Page number (1-indexed)
    #[serde(alias = "pageNo")]
    pub page_no: Option<u32>,
    /// Page size
    #[serde(alias = "pageSize")]
    pub page_size: Option<u32>,
}

/// Nacos-compatible MCP delete query params
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpDeleteQuery {
    /// Namespace ID
    #[serde(alias = "namespaceId")]
    pub namespace_id: Option<String>,
    /// MCP server ID
    #[serde(alias = "mcpId")]
    pub mcp_id: Option<String>,
    /// MCP server name
    #[serde(alias = "mcpName")]
    pub mcp_name: Option<String>,
    /// Version
    pub version: Option<String>,
}

/// Import tools from MCP query params
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportToolsQuery {
    /// Transport type (e.g., "sse", "http")
    pub transport_type: String,
    /// Base URL
    pub base_url: String,
    /// Endpoint path
    pub endpoint: String,
    /// Authentication token
    pub auth_token: Option<String>,
}

/// MCP import validate request
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpImportValidateRequest {
    /// Content to validate (JSON string)
    pub content: String,
}

/// MCP import validate response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpImportValidateResponse {
    /// Whether the content is valid
    pub valid: bool,
    /// Validation error message
    #[serde(default)]
    pub message: String,
    /// Number of servers found in the import
    pub server_count: u32,
}

/// MCP import execute request
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpImportExecuteRequest {
    /// Content to import (JSON string)
    pub content: String,
    /// Namespace to import into
    #[serde(default = "default_namespace")]
    pub namespace: String,
    /// Whether to overwrite existing servers
    #[serde(default)]
    pub overwrite: bool,
}

/// MCP Server JSON import request
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerImportRequest {
    /// MCP servers to import (claude_desktop_config.json format)
    pub mcp_servers: HashMap<String, McpServerConfig>,

    /// Namespace to import into
    #[serde(default = "default_namespace")]
    pub namespace: String,

    /// Whether to overwrite existing servers
    #[serde(default)]
    pub overwrite: bool,
}

/// MCP Server config (claude_desktop_config.json format)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerConfig {
    /// Command to run
    pub command: Option<String>,

    /// Arguments
    #[serde(default)]
    pub args: Vec<String>,

    /// Environment variables
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// URL for HTTP transport
    pub url: Option<String>,
}

// =============================================================================
// Config-Backed Storage Models (Nacos 3.x aligned) - MCP
// =============================================================================

/// MCP server version index (stored in config group `mcp-server-versions`)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerVersionInfo {
    /// Server ID (UUID)
    pub id: String,

    /// Server name (unique within namespace)
    pub name: String,

    /// Protocol type (e.g., "mcp")
    #[serde(default = "default_mcp_protocol")]
    pub protocol: String,

    /// Server description
    #[serde(default)]
    pub description: String,

    /// Server capabilities summary
    #[serde(default)]
    pub capabilities: McpCapabilities,

    /// Latest published version string
    #[serde(default)]
    pub latest_published_version: String,

    /// All known version details
    #[serde(default)]
    pub version_details: Vec<VersionDetail>,
}

fn default_mcp_protocol() -> String {
    "mcp".to_string()
}

/// MCP server per-version spec (stored in config group `mcp-server`)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerStorageInfo {
    /// Server ID
    pub id: String,

    /// Server name
    pub name: String,

    /// Protocol type
    #[serde(default = "default_mcp_protocol")]
    pub protocol: String,

    /// Whether this version is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Remote server configuration
    #[serde(default)]
    pub remote_server_config: Option<RemoteServerConfig>,

    /// Reference to the tools data ID
    #[serde(default)]
    pub tools_description_ref: String,

    /// Version detail for this specific version
    #[serde(default)]
    pub version_detail: Option<VersionDetail>,

    /// Full server registration data (Batata extension)
    #[serde(default)]
    pub server_data: Option<McpServerRegistration>,
}

/// Remote server configuration for NamingService integration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteServerConfig {
    /// Service reference for NamingService lookup
    #[serde(default)]
    pub service_ref: Option<ServiceRef>,

    /// Front endpoint configuration list
    #[serde(default)]
    pub front_endpoint_config_list: Vec<FrontEndpointConfig>,
}

/// Reference to a naming service entry
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceRef {
    /// Namespace ID
    #[serde(default)]
    pub namespace_id: String,

    /// Group name
    #[serde(default)]
    pub group_name: String,

    /// Service name
    #[serde(default)]
    pub service_name: String,

    /// Transport protocol
    #[serde(default)]
    pub transport_protocol: String,
}

/// Front endpoint configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontEndpointConfig {
    /// Endpoint address
    #[serde(default)]
    pub address: String,

    /// Endpoint port
    #[serde(default)]
    pub port: u16,

    /// Whether TLS is supported
    #[serde(default)]
    pub support_tls: bool,

    /// URL path
    #[serde(default)]
    pub path: String,
}

/// MCP Registry statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpRegistryStats {
    pub total_servers: u32,
    pub healthy_servers: u32,
    pub unhealthy_servers: u32,
    pub by_namespace: HashMap<String, u32>,
    pub by_type: HashMap<String, u32>,
}
