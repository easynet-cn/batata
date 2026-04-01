//! A2A (Agent-to-Agent) Models

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::{
    HealthStatus, VersionDetail, default_namespace, default_page, default_page_size,
    default_version,
};

// =============================================================================
// A2A (Agent-to-Agent) Models
// =============================================================================

/// AgentCard - represents an AI agent's capabilities and identity
///
/// Aligned with Nacos AgentCard (extends AgentCardBasicInfo):
/// - AgentCardBasicInfo: protocolVersion, name, description, version, iconUrl, capabilities, skills
/// - AgentCard: url, preferredTransport, additionalInterfaces, provider, documentationUrl,
///   securitySchemes, security, defaultInputModes, defaultOutputModes, supportsAuthenticatedExtendedCard
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCard {
    /// Agent name (unique identifier)
    #[serde(default)]
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

    /// Agent URL (Nacos uses `url`, not `endpoint`)
    #[serde(default)]
    pub url: String,

    /// Agent protocol version
    #[serde(default = "default_protocol_version")]
    pub protocol_version: String,

    /// Agent capabilities
    #[serde(default)]
    pub capabilities: AgentCapabilities,

    /// Agent skills
    #[serde(default)]
    pub skills: Vec<AgentSkill>,

    /// Supported default input modes
    #[serde(default)]
    pub default_input_modes: Vec<String>,

    /// Supported default output modes
    #[serde(default)]
    pub default_output_modes: Vec<String>,

    /// Preferred transport type
    #[serde(default)]
    pub preferred_transport: Option<String>,

    /// Provider information
    #[serde(default)]
    pub provider: Option<AgentProvider>,

    /// Documentation URL
    #[serde(default)]
    pub documentation_url: Option<String>,

    /// Icon URL
    #[serde(default)]
    pub icon_url: Option<String>,

    /// Whether supports authenticated extended card
    #[serde(default)]
    pub supports_authenticated_extended_card: Option<bool>,

    /// Agent metadata (batata extension)
    #[serde(default)]
    pub metadata: HashMap<String, String>,

    /// Tags for categorization (batata extension)
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Agent provider information (aligned with Nacos AgentProvider)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentProvider {
    /// Organization name
    #[serde(default)]
    pub organization: String,
    /// URL
    #[serde(default)]
    pub url: String,
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

// Note: Nacos A2A uses string-typed mode arrays (defaultInputModes, defaultOutputModes)
// and SecurityScheme maps instead of typed enums. Batata uses the same JSON schema.

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

/// Nacos-compatible Agent detail query params
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentDetailQuery {
    /// Namespace ID
    #[serde(alias = "namespaceId")]
    pub namespace_id: Option<String>,
    /// Agent name
    #[serde(alias = "agentName")]
    pub agent_name: Option<String>,
    /// Version
    pub version: Option<String>,
}

/// Nacos-compatible Agent list query params
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentListQuery {
    /// Namespace ID
    #[serde(alias = "namespaceId")]
    pub namespace_id: Option<String>,
    /// Agent name
    #[serde(alias = "agentName")]
    pub agent_name: Option<String>,
    /// Search type: "accurate" or "blur"
    pub search: Option<String>,
    /// Page number (1-indexed)
    #[serde(alias = "pageNo")]
    pub page_no: Option<u32>,
    /// Page size
    #[serde(alias = "pageSize")]
    pub page_size: Option<u32>,
}

/// Nacos-compatible Agent delete query params
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentDeleteQuery {
    /// Namespace ID
    #[serde(alias = "namespaceId")]
    pub namespace_id: Option<String>,
    /// Agent name
    #[serde(alias = "agentName")]
    pub agent_name: Option<String>,
    /// Version
    pub version: Option<String>,
}

/// Agent version list query params
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentVersionListQuery {
    /// Namespace ID
    #[serde(alias = "namespaceId")]
    pub namespace_id: Option<String>,
    /// Agent name
    #[serde(alias = "agentName")]
    pub agent_name: Option<String>,
}

// =============================================================================
// Config-Backed Storage Models (Nacos 3.x aligned) - A2A
// =============================================================================

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

/// Agent Registry statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRegistryStats {
    pub total_agents: u32,
    pub healthy_agents: u32,
    pub unhealthy_agents: u32,
    pub by_namespace: HashMap<String, u32>,
    pub by_skill: HashMap<String, u32>,
}
