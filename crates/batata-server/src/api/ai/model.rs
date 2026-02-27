//! AI Capabilities Data Models
//!
//! Data models for MCP (Model Content Protocol) server registration
//! and A2A (Agent-to-Agent) communication.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// =============================================================================
// MCP (Model Content Protocol) Server Models
// =============================================================================

/// MCP Server registration request
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerRegistration {
    /// Server name (unique identifier)
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

    /// Server endpoint URL
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

fn default_namespace() -> String {
    "default".to_string()
}

fn default_version() -> String {
    "1.0.0".to_string()
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
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpCapabilities {
    /// Whether server supports tools
    #[serde(default)]
    pub tools: bool,

    /// Whether server supports resources
    #[serde(default)]
    pub resources: bool,

    /// Whether server supports prompts
    #[serde(default)]
    pub prompts: bool,

    /// Whether server supports logging
    #[serde(default)]
    pub logging: bool,

    /// Whether server supports sampling
    #[serde(default)]
    pub sampling: bool,

    /// Experimental capabilities
    #[serde(default)]
    pub experimental: HashMap<String, serde_json::Value>,
}

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

/// Health status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    /// Status unknown
    #[default]
    Unknown,
    /// Server is healthy
    Healthy,
    /// Server is unhealthy
    Unhealthy,
    /// Server is degraded
    Degraded,
}

/// MCP Server query parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerQuery {
    /// Filter by namespace
    pub namespace: Option<String>,

    /// Filter by name pattern (supports wildcards)
    pub name_pattern: Option<String>,

    /// Filter by tags (any match)
    pub tags: Option<Vec<String>>,

    /// Filter by server type
    pub server_type: Option<McpServerType>,

    /// Filter by health status
    pub health_status: Option<HealthStatus>,

    /// Filter by capability
    pub has_tools: Option<bool>,

    /// Filter by capability
    pub has_resources: Option<bool>,

    /// Filter by capability
    pub has_prompts: Option<bool>,

    /// Page number (1-indexed)
    #[serde(default = "default_page")]
    pub page: u32,

    /// Page size
    #[serde(default = "default_page_size")]
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

fn default_page() -> u32 {
    1
}

fn default_page_size() -> u32 {
    20
}

/// MCP Server list response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerListResponse {
    /// List of servers
    pub servers: Vec<McpServer>,

    /// Total count
    pub total: u64,

    /// Page number
    pub page: u32,

    /// Page size
    pub page_size: u32,
}

/// Nacos-compatible MCP detail query params
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpDetailQuery {
    /// Namespace ID (defaults to "public")
    pub namespace_id: Option<String>,
    /// MCP server ID
    pub mcp_id: Option<String>,
    /// MCP server name
    pub mcp_name: Option<String>,
    /// Version
    pub version: Option<String>,
}

/// Nacos-compatible MCP list query params
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpListQuery {
    /// Namespace ID
    pub namespace_id: Option<String>,
    /// MCP server name
    pub mcp_name: Option<String>,
    /// Search type: "accurate" or "blur"
    pub search: Option<String>,
    /// Page number (1-indexed)
    pub page_no: Option<u32>,
    /// Page size
    pub page_size: Option<u32>,
}

/// Nacos-compatible MCP delete query params
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpDeleteQuery {
    /// Namespace ID
    pub namespace_id: Option<String>,
    /// MCP server ID
    pub mcp_id: Option<String>,
    /// MCP server name
    pub mcp_name: Option<String>,
    /// Version
    pub version: Option<String>,
}

/// Nacos-compatible Agent detail query params
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentDetailQuery {
    /// Namespace ID
    pub namespace_id: Option<String>,
    /// Agent name
    pub agent_name: Option<String>,
    /// Version
    pub version: Option<String>,
}

/// Nacos-compatible Agent list query params
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentListQuery {
    /// Namespace ID
    pub namespace_id: Option<String>,
    /// Agent name
    pub agent_name: Option<String>,
    /// Search type: "accurate" or "blur"
    pub search: Option<String>,
    /// Page number (1-indexed)
    pub page_no: Option<u32>,
    /// Page size
    pub page_size: Option<u32>,
}

/// Nacos-compatible Agent delete query params
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentDeleteQuery {
    /// Namespace ID
    pub namespace_id: Option<String>,
    /// Agent name
    pub agent_name: Option<String>,
    /// Version
    pub version: Option<String>,
}

/// Agent version list query params
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentVersionListQuery {
    /// Namespace ID
    pub namespace_id: Option<String>,
    /// Agent name
    pub agent_name: Option<String>,
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
// A2A (Agent-to-Agent) Models
// =============================================================================

/// AgentCard - represents an AI agent's capabilities and identity
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCard {
    /// Agent name (unique identifier)
    pub name: String,

    /// Agent display name
    #[serde(default)]
    pub display_name: String,

    /// Agent description
    #[serde(default)]
    pub description: String,

    /// Agent version
    #[serde(default = "default_version")]
    pub version: String,

    /// Agent endpoint URL
    pub endpoint: String,

    /// Agent protocol version
    #[serde(default = "default_protocol_version")]
    pub protocol_version: String,

    /// Agent capabilities
    #[serde(default)]
    pub capabilities: AgentCapabilities,

    /// Agent skills
    #[serde(default)]
    pub skills: Vec<AgentSkill>,

    /// Supported input modes
    #[serde(default)]
    pub input_modes: Vec<InputMode>,

    /// Supported output modes
    #[serde(default)]
    pub output_modes: Vec<OutputMode>,

    /// Authentication configuration
    #[serde(default)]
    pub authentication: Option<AgentAuthentication>,

    /// Rate limiting configuration
    #[serde(default)]
    pub rate_limits: Option<RateLimits>,

    /// Agent metadata
    #[serde(default)]
    pub metadata: HashMap<String, String>,

    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,
}

fn default_protocol_version() -> String {
    "1.0".to_string()
}

/// Agent capabilities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCapabilities {
    /// Whether agent supports streaming
    #[serde(default)]
    pub streaming: bool,

    /// Whether agent supports multi-turn conversations
    #[serde(default)]
    pub multi_turn: bool,

    /// Whether agent supports tool use
    #[serde(default)]
    pub tool_use: bool,

    /// Whether agent supports file attachments
    #[serde(default)]
    pub file_attachments: bool,

    /// Whether agent supports images
    #[serde(default)]
    pub images: bool,

    /// Whether agent supports audio
    #[serde(default)]
    pub audio: bool,

    /// Whether agent supports video
    #[serde(default)]
    pub video: bool,

    /// Maximum context length
    #[serde(default)]
    pub max_context_length: Option<u64>,

    /// Maximum output tokens
    #[serde(default)]
    pub max_output_tokens: Option<u64>,
}

/// Agent skill
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSkill {
    /// Skill name
    pub name: String,

    /// Skill description
    #[serde(default)]
    pub description: String,

    /// Skill proficiency level (0-100)
    #[serde(default)]
    pub proficiency: u8,

    /// Example inputs
    #[serde(default)]
    pub examples: Vec<String>,
}

/// Input mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InputMode {
    Text,
    Image,
    Audio,
    Video,
    File,
}

/// Output mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputMode {
    Text,
    Image,
    Audio,
    Video,
    File,
    Json,
}

/// Agent authentication configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentAuthentication {
    /// Authentication type
    pub auth_type: AuthType,

    /// OAuth configuration
    #[serde(default)]
    pub oauth: Option<OAuthConfig>,
}

/// Authentication type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthType {
    /// No authentication required
    #[default]
    None,
    /// API key authentication
    ApiKey,
    /// Bearer token authentication
    Bearer,
    /// OAuth authentication
    OAuth,
}

/// OAuth configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthConfig {
    /// Authorization URL
    pub authorization_url: String,

    /// Token URL
    pub token_url: String,

    /// Scopes
    #[serde(default)]
    pub scopes: Vec<String>,
}

/// Rate limiting configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimits {
    /// Requests per minute
    pub requests_per_minute: Option<u32>,

    /// Requests per hour
    pub requests_per_hour: Option<u32>,

    /// Requests per day
    pub requests_per_day: Option<u32>,

    /// Tokens per minute
    pub tokens_per_minute: Option<u64>,
}

/// Registered Agent
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisteredAgent {
    /// Agent ID (auto-generated)
    pub id: String,

    /// Agent card
    #[serde(flatten)]
    pub card: AgentCard,

    /// Namespace
    pub namespace: String,

    /// Health status
    pub health_status: HealthStatus,

    /// Registration timestamp (milliseconds)
    pub registered_at: i64,

    /// Last health check timestamp (milliseconds)
    pub last_health_check: Option<i64>,

    /// Last updated timestamp (milliseconds)
    pub updated_at: i64,
}

/// Agent registration request
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRegistrationRequest {
    /// Agent card
    pub card: AgentCard,

    /// Namespace
    #[serde(default = "default_namespace")]
    pub namespace: String,
}

/// Agent query parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentQuery {
    /// Filter by namespace
    pub namespace: Option<String>,

    /// Filter by name pattern
    pub name_pattern: Option<String>,

    /// Filter by skill
    pub skill: Option<String>,

    /// Filter by tags
    pub tags: Option<Vec<String>>,

    /// Filter by capability (streaming)
    pub streaming: Option<bool>,

    /// Filter by capability (tool_use)
    pub tool_use: Option<bool>,

    /// Filter by health status
    pub health_status: Option<HealthStatus>,

    /// Page number
    #[serde(default = "default_page")]
    pub page: u32,

    /// Page size
    #[serde(default = "default_page_size")]
    pub page_size: u32,
}

impl Default for AgentQuery {
    fn default() -> Self {
        Self {
            namespace: None,
            name_pattern: None,
            skill: None,
            tags: None,
            streaming: None,
            tool_use: None,
            health_status: None,
            page: default_page(),
            page_size: default_page_size(),
        }
    }
}

/// Agent list response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentListResponse {
    /// List of agents
    pub agents: Vec<RegisteredAgent>,

    /// Total count
    pub total: u64,

    /// Page number
    pub page: u32,

    /// Page size
    pub page_size: u32,
}

/// Batch agent registration request
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchAgentRegistrationRequest {
    /// List of agents to register
    pub agents: Vec<AgentRegistrationRequest>,

    /// Whether to overwrite existing agents
    #[serde(default)]
    pub overwrite: bool,
}

/// Batch registration response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchRegistrationResponse {
    /// Number of successfully registered
    pub success_count: u32,

    /// Number of failed registrations
    pub failed_count: u32,

    /// Errors for failed registrations
    pub errors: Vec<RegistrationError>,
}

/// Registration error
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistrationError {
    /// Agent/Server name
    pub name: String,

    /// Error message
    pub error: String,
}

// =============================================================================
// Config-Backed Storage Models (Nacos 3.x aligned)
// =============================================================================

/// Version detail for a single version entry
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionDetail {
    /// Version string (e.g., "1.0.0")
    pub version: String,

    /// Release date (ISO 8601)
    #[serde(default)]
    pub release_date: String,

    /// Whether this is the latest published version
    #[serde(default)]
    pub is_latest: bool,
}

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

fn default_true() -> bool {
    true
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

/// A2A agent version index (stored in config group `agent`)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCardVersionInfo {
    /// Agent ID
    pub id: String,

    /// Agent name
    pub name: String,

    /// Latest published version string
    #[serde(default)]
    pub latest_published_version: String,

    /// Registration type (e.g., "manual", "sdk")
    #[serde(default = "default_registration_type")]
    pub registration_type: String,

    /// All known version details
    #[serde(default)]
    pub version_details: Vec<VersionDetail>,
}

fn default_registration_type() -> String {
    "manual".to_string()
}

/// A2A agent per-version detail (stored in config group `agent-version`)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCardDetailInfo {
    /// Agent ID
    pub id: String,

    /// Agent name
    pub name: String,

    /// Version string
    #[serde(default)]
    pub version: String,

    /// Registration type
    #[serde(default = "default_registration_type")]
    pub registration_type: String,

    /// Description
    #[serde(default)]
    pub description: String,

    /// Agent endpoint URL
    #[serde(default)]
    pub url: String,

    /// Agent capabilities
    #[serde(default)]
    pub capabilities: AgentCapabilities,

    /// Agent skills
    #[serde(default)]
    pub skills: Vec<AgentSkill>,

    /// Provider information
    #[serde(default)]
    pub provider: String,

    /// Full agent card data (Batata extension)
    #[serde(default)]
    pub agent_card: Option<AgentCard>,
}

/// MCP server basic info returned from list queries
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerBasicInfo {
    /// Server ID
    pub id: String,

    /// Server name
    pub name: String,

    /// Protocol type
    pub protocol: String,

    /// Description
    pub description: String,

    /// Latest published version
    pub latest_published_version: String,

    /// Number of versions
    pub version_count: usize,

    /// Namespace
    pub namespace: String,

    /// Creation time (milliseconds)
    pub create_time: i64,

    /// Modification time (milliseconds)
    pub modify_time: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_server_registration_serialization() {
        let reg = McpServerRegistration {
            name: "test-server".to_string(),
            display_name: "Test Server".to_string(),
            description: "A test server".to_string(),
            namespace: "default".to_string(),
            version: "1.0.0".to_string(),
            endpoint: "http://localhost:8080".to_string(),
            server_type: McpServerType::Http,
            transport: McpTransport::default(),
            capabilities: McpCapabilities {
                tools: true,
                ..Default::default()
            },
            tools: vec![McpTool {
                name: "test_tool".to_string(),
                description: "A test tool".to_string(),
                input_schema: serde_json::json!({}),
            }],
            resources: vec![],
            prompts: vec![],
            metadata: HashMap::new(),
            tags: vec!["test".to_string()],
            auto_fetch_tools: true,
            health_check: None,
        };

        let json = serde_json::to_string(&reg).unwrap();
        assert!(json.contains("test-server"));
        assert!(json.contains("test_tool"));
    }

    #[test]
    fn test_agent_card_serialization() {
        let card = AgentCard {
            name: "test-agent".to_string(),
            display_name: "Test Agent".to_string(),
            description: "A test agent".to_string(),
            version: "1.0.0".to_string(),
            endpoint: "http://localhost:8080".to_string(),
            protocol_version: "1.0".to_string(),
            capabilities: AgentCapabilities {
                streaming: true,
                tool_use: true,
                ..Default::default()
            },
            skills: vec![AgentSkill {
                name: "coding".to_string(),
                description: "Code generation and review".to_string(),
                proficiency: 90,
                examples: vec!["Write a function".to_string()],
            }],
            input_modes: vec![InputMode::Text, InputMode::Image],
            output_modes: vec![OutputMode::Text, OutputMode::Json],
            authentication: None,
            rate_limits: None,
            metadata: HashMap::new(),
            tags: vec!["test".to_string()],
        };

        let json = serde_json::to_string(&card).unwrap();
        assert!(json.contains("test-agent"));
        assert!(json.contains("coding"));
    }

    #[test]
    fn test_mcp_server_config_parsing() {
        let json = r#"{
            "mcpServers": {
                "filesystem": {
                    "command": "npx",
                    "args": ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
                },
                "github": {
                    "command": "npx",
                    "args": ["-y", "@modelcontextprotocol/server-github"],
                    "env": {
                        "GITHUB_PERSONAL_ACCESS_TOKEN": "token"
                    }
                }
            },
            "namespace": "default"
        }"#;

        let import: McpServerImportRequest = serde_json::from_str(json).unwrap();

        assert!(import.mcp_servers.contains_key("filesystem"));
        assert!(import.mcp_servers.contains_key("github"));
    }
}
