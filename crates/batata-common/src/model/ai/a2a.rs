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
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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

    /// Extra endpoints the agent can be reached on, beyond `url`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additional_interfaces: Vec<AgentInterface>,

    /// Transports the agent declares support for.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supported_interfaces: Vec<AgentInterface>,

    /// Security schemes by name. Upstream's `SecurityScheme` is a free-form map,
    /// so the contents are not modelled here.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub security_schemes: HashMap<String, serde_json::Value>,

    /// Security requirements (A2A 0.2 shape).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub security: Vec<HashMap<String, Vec<String>>>,

    /// Security requirements (A2A 0.3 shape), preferred over `security`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub security_requirements: Vec<HashMap<String, Vec<String>>>,

    /// Signatures over the card contents.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signatures: Vec<HashMap<String, serde_json::Value>>,
}

/// What `GET /runtime-endpoints` answers: the live endpoints of one agent
/// version, plus the Naming service they are published under.
///
/// Upstream nests endpoints inside `callInterface.endpointSets`, grouping them
/// by source. Batata publishes every endpoint as one Naming instance, so the
/// grouping carries no information here and the endpoints are returned flat.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleRuntimeEndpointView {
    /// The live endpoint snapshot.
    pub runtime_endpoint_snapshot: RuntimeEndpointSnapshot,
    /// Where the snapshot was read from.
    pub naming_service_ref: NamingServiceRef,
}

/// The live endpoints of one agent version.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeEndpointSnapshot {
    /// Namespace the agent lives in.
    pub namespace_id: String,
    /// Agent name.
    pub agent_name: String,
    /// Agent version.
    pub version: String,
    /// How the agent is called (runtime/flat view; the deferred
    /// `GET /runtime-endpoints` feature publishes every endpoint as one Naming
    /// instance, so the definition-time `endpointSets` grouping is flattened).
    pub call_interface: RuntimeCallInterface,
}

/// The protocol and endpoints of one call interface, as observed at runtime.
///
/// This is the flat, Naming-backed view that batata's `GET /runtime-endpoints`
/// returns. The GA *definition*-time call interface (`AgentCallInterface`) has a
/// richer shape (`protocolVersion`, `descriptorMediaType`, `nativeDescriptor`,
/// `endpointSets`); see the ARD model section below.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCallInterface {
    /// Requested protocol, e.g. `JSONRPC`.
    pub protocol: String,
    /// Endpoints currently registered.
    #[serde(default)]
    pub endpoints: Vec<RuntimeEndpoint>,
}

/// One reachable agent endpoint.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeEndpoint {
    /// Address.
    pub address: String,
    /// Port.
    pub port: u16,
    /// Whether the endpoint is healthy.
    pub healthy: bool,
}

/// The Naming service an agent's endpoints are published under.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamingServiceRef {
    /// Namespace.
    pub namespace_id: String,
    /// Group, e.g. `agent-endpoints`.
    pub group_name: String,
    /// Service name.
    pub service_name: String,
}

/// One way of reaching an agent (aligned with Nacos AgentInterface).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentInterface {
    /// Endpoint URL
    #[serde(default)]
    pub url: String,
    /// Transport, e.g. `JSONRPC` or `HTTP`
    #[serde(default)]
    pub transport: String,
    /// Protocol binding layered on the transport
    #[serde(default)]
    pub protocol_binding: Option<String>,
    /// Protocol version spoken on this interface
    #[serde(default)]
    pub protocol_version: Option<String>,
    /// Tenant the interface belongs to
    #[serde(default)]
    pub tenant: Option<String>,
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

// =============================================================================
// GA ARD (Agent Resource Definition) models — Nacos 3.3.0 GA
//
// These mirror `com.alibaba.nacos.api.ai.model.agent.*` (release `b64bc9118`).
// They are the *definition*-time shape that GA's `AgentAdminController` stores
// and serves, distinct from the A2A `AgentCard` batata registers at runtime.
// `AgentVersionDetail.callInterfaces` is the field GA's first-draft validator
// (`requireInitialDraftContent`) requires to be non-null.
// =============================================================================

/// Where an endpoint set came from: declared in the definition or observed at runtime.
///
/// Serialized exactly like the GA Java enum (`RUNTIME` / `DECLARED`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum EndpointSource {
    /// Endpoints declared in the agent definition.
    #[default]
    Declared,
    /// Endpoints discovered from the runtime registry.
    Runtime,
}

/// One runtime implementation version and the version range it can serve.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeVersionBinding {
    /// Runtime implementation version.
    #[serde(default)]
    pub runtime_version: String,
    /// Agent version range the binding can serve.
    #[serde(default)]
    pub version_range: String,
}

/// One endpoint value, shared by declared and runtime agent views.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Endpoint {
    /// Endpoint URI.
    #[serde(default)]
    pub uri: String,
    /// Transport, e.g. `HTTP` or `GRPC`.
    #[serde(default)]
    pub transport: String,
    /// Priority; lower values win. GA defaults to zero.
    #[serde(default)]
    pub priority: i32,
    /// Weight among same-priority endpoints. GA defaults to one.
    #[serde(default = "default_endpoint_weight")]
    pub weight: f64,
    /// Arbitrary endpoint metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,
    /// Registration health; GA defaults to true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub healthy: Option<bool>,
    /// Optional registration bindings (runtime reads may aggregate several).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bindings: Option<Vec<RuntimeVersionBinding>>,
    /// Registration enablement; GA defaults to true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

fn default_endpoint_weight() -> f64 {
    1.0
}

/// Snapshot of endpoints from one declared or runtime source.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EndpointSet {
    /// Source of the endpoints.
    #[serde(default)]
    pub source: EndpointSource,
    /// Source revision marker.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_revision: Option<String>,
    /// Nacos observation time (runtime queries only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_updated_time: Option<i64>,
    /// Endpoints in this set.
    #[serde(default)]
    pub endpoints: Vec<Endpoint>,
}

/// Public provider information for an Agent (GA `AgentProvider`).
///
/// GA uses `name` + `url`; the A2A `AgentCard.provider` uses `organization` +
/// `url` and is modelled separately by [`AgentProvider`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentProviderInfo {
    /// Provider name.
    #[serde(default)]
    pub name: String,
    /// Provider URL.
    #[serde(default)]
    pub url: String,
}

/// The definition-time call interface of an agent version (GA `AgentCallInterface`).
///
/// Unlike [`RuntimeCallInterface`], this carries the declared protocol
/// descriptor and the `endpointSets` grouping rather than a flat endpoint list.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCallInterface {
    /// Requested protocol, e.g. `JSONRPC`.
    #[serde(default)]
    pub protocol: String,
    /// Protocol version spoken on this interface.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol_version: Option<String>,
    /// Media type of `nativeDescriptor`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub descriptor_media_type: Option<String>,
    /// Protocol-native descriptor blob (free-form).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_descriptor: Option<serde_json::Value>,
    /// Definition source preference; absent from discovery and runtime views.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint_source_order: Option<Vec<EndpointSource>>,
    /// Declared endpoint sets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint_sets: Option<Vec<EndpointSet>>,
}

/// One agent version as returned by `GET /versions` (GA `AgentVersionSummary`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentVersionSummary {
    /// Version string.
    #[serde(default)]
    pub version: String,
    /// Custom labels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<String>>,
    /// Declared protocols.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocols: Option<Vec<String>>,
    /// Lifecycle status.
    #[serde(default)]
    pub status: String,
    /// Publish pipeline info (opaque).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publish_pipeline_info: Option<String>,
    /// Author username.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Change description for this version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub change_description: Option<String>,
    /// Content digest of the definition.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_digest: Option<String>,
    /// Creation time (epoch millis).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub create_time: Option<i64>,
    /// Last update time (epoch millis).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update_time: Option<i64>,
}

/// Version lifecycle, label mapping and online summaries for an agent
/// (GA `AgentVersionInfo`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentVersionInfo {
    /// Version currently being edited.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editing_version: Option<String>,
    /// Version currently under review.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewing_version: Option<String>,
    /// Online version summaries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub online_versions: Option<Vec<AgentVersionSummary>>,
    /// Label map (e.g. `latest`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<HashMap<String, String>>,
}

/// Agent metadata shared by management and discovery catalog views
/// (GA `AgentSummary`, extends `AbstractAgentMetadata`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSummary {
    /// Agent name.
    #[serde(default)]
    pub agent_name: String,
    /// Display name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Icon URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    /// Provider information.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<AgentProviderInfo>,
    /// Tags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// Opaque extension map.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extensions: Option<HashMap<String, serde_json::Value>>,
    /// Namespace ID.
    #[serde(default)]
    pub namespace_id: String,
    /// Agent-level lifecycle status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// Owner username.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    /// Visibility scope (`PUBLIC` / `PRIVATE`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    /// Version lifecycle facts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_info: Option<AgentVersionInfo>,
    /// Extension optimistic-lock version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta_version: Option<i64>,
    /// Creation time (epoch millis).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub create_time: Option<i64>,
    /// Last update time (epoch millis).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update_time: Option<i64>,
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

/// One agent version as returned by the version lifecycle endpoints.
///
/// Mirrors `McpServerVersionDetail` for MCP: the lifecycle endpoints answer
/// with the version they just moved, so callers can assert the transition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentVersionDetail {
    /// Namespace ID.
    #[serde(default)]
    pub namespace_id: String,

    /// Agent name.
    #[serde(default)]
    pub name: String,

    /// Version string.
    #[serde(default)]
    pub version: String,

    /// Lifecycle status (draft / reviewing / reviewed / online / offline).
    #[serde(default)]
    pub status: String,

    /// Optional description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Definition-time call interfaces (GA `AgentVersionDetail.callInterfaces`).
    ///
    /// GA's first-draft validator (`requireInitialDraftContent`) requires this
    /// to be non-null. Batata stores whatever was supplied; an empty list is
    /// distinct from `None` (absent on the wire).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_interfaces: Option<Vec<AgentCallInterface>>,

    /// Custom labels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<String>>,

    /// Declared protocols.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocols: Option<Vec<String>>,

    /// Author username.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,

    /// Change description for this version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub change_description: Option<String>,

    /// Content digest of the definition.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_digest: Option<String>,

    /// Creation time (epoch millis).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub create_time: Option<i64>,

    /// Last update time (epoch millis).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update_time: Option<i64>,
}

fn default_registration_type() -> String {
    "manual".to_string()
}

/// A2A agent per-version detail (stored in config group `agent-version`)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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

    /// Definition-time call interfaces (GA `AgentVersionDetail.callInterfaces`).
    ///
    /// Stored verbatim from the admin write form so the definition survives a
    /// `GET /version` round-trip. Added after initial release; old rows without
    /// this key deserialize as `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_interfaces: Option<Vec<AgentCallInterface>>,
}

/// Agent Registry statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRegistryStats {
    /// The `total_agents` field.
    pub total_agents: u32,
    /// The `healthy_agents` field.
    pub healthy_agents: u32,
    /// The `unhealthy_agents` field.
    pub unhealthy_agents: u32,
    /// The `by_namespace` field.
    pub by_namespace: HashMap<String, u32>,
    /// The `by_skill` field.
    pub by_skill: HashMap<String, u32>,
}
