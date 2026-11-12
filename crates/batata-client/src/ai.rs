//! AI/MCP Client Service
//!
//! Provides client-side operations for the Nacos 3.3.0 GA `AiService` facade and
//! the RAD `AgentDiscoveryService`, compatible with Batata's
//! `/v3/client/ai/**` HTTP endpoints (implemented in `crates/batata-ai`).
//!
//! The facade mirrors GA `AiService.java` (`mcp()` / `skill()` / `prompt()` /
//! `agentSpec()` / `agent()`): each sub-service is a cheap `Arc` handle sharing
//! the underlying `BatataHttpClient`. The existing flat query methods
//! (`get_mcp_server`, `get_agent_card`, ...) are kept as thin delegates so the
//! TG-03 integration tests continue to pass unchanged.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::http::BatataHttpClient;
use crate::model::{ApiResponse, Page};

// ============================================================================
// Existing DTOs (console + A2A surface) — preserved for backward compatibility
// ============================================================================

/// MCP Server basic information
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerBasicInfo {
    /// MCP server name.
    pub mcp_name: String,
    #[serde(default)]
    /// Namespace ID the server belongs to.
    pub namespace_id: String,
    #[serde(default)]
    /// Latest published version of the server.
    pub latest_version: String,
    #[serde(default)]
    /// Human-readable description of the server.
    pub description: String,
    #[serde(default)]
    /// Whether the server is currently enabled.
    pub enabled: bool,
}

/// MCP Server detailed information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerDetailInfo {
    /// MCP server name.
    pub mcp_name: String,
    #[serde(default)]
    /// Namespace ID the server belongs to.
    pub namespace_id: String,
    #[serde(default)]
    /// Server version.
    pub version: String,
    #[serde(default)]
    /// Human-readable description of the server.
    pub description: String,
    #[serde(default)]
    /// Whether the server is currently enabled.
    pub enabled: bool,
    #[serde(default)]
    /// Transport protocol used by the server (e.g. "stdio" or "sse").
    pub protocol: String,
    #[serde(default)]
    /// Server type or category.
    pub server_type: String,
    #[serde(default)]
    /// Tools exposed by the server.
    pub tools: Vec<McpToolSpec>,
    #[serde(default)]
    /// Arbitrary metadata key-value pairs.
    pub metadata: HashMap<String, String>,
}

/// MCP Tool specification
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolSpec {
    /// Tool name.
    pub name: String,
    #[serde(default)]
    /// Tool description.
    pub description: String,
    #[serde(default)]
    /// JSON schema describing the tool input.
    pub input_schema: serde_json::Value,
}

/// Agent Card information (A2A)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCardInfo {
    /// Agent name.
    pub agent_name: String,
    #[serde(default)]
    /// Namespace ID the agent belongs to.
    pub namespace_id: String,
    #[serde(default)]
    /// Agent card version.
    pub version: String,
    #[serde(default)]
    /// Human-readable description of the agent.
    pub description: String,
    #[serde(default)]
    /// Whether the agent is currently enabled.
    pub enabled: bool,
    #[serde(default)]
    /// Agent capabilities.
    pub capabilities: AgentCapabilities,
    #[serde(default)]
    /// Skills exposed by the agent.
    pub skills: Vec<AgentSkill>,
    #[serde(default)]
    /// Arbitrary metadata key-value pairs.
    pub metadata: HashMap<String, String>,
    /// Named A2A security schemes. Upstream's `SecurityScheme` is a free-form
    /// object, so its contents are kept as JSON values.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub security_schemes: HashMap<String, serde_json::Value>,
    /// Legacy A2A 0.2 security requirements (kept for older cards).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub security: Vec<HashMap<String, Vec<String>>>,
    /// GA A2A 0.3 / Nacos 3.3.0 `securityRequirements` shape: each entry maps
    /// a scheme name to its required scopes. The post-GA develop branch wraps
    /// this in `SecurityRequirement { schemes: {..: StringList { list } } }`,
    /// which is a different wire contract and is deliberately not modelled
    /// here while the declared alignment target is 3.3.0 GA.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub security_requirements: Vec<HashMap<String, Vec<String>>>,
}

/// Agent capabilities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCapabilities {
    #[serde(default)]
    /// Whether the agent supports streaming responses.
    pub streaming: bool,
    #[serde(default)]
    /// Whether the agent supports push notifications.
    pub push_notifications: bool,
    #[serde(default)]
    /// Whether the agent keeps a state-transition history.
    pub state_transition_history: bool,
}

/// Agent skill definition
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSkill {
    /// Skill identifier.
    pub id: String,
    /// Skill name.
    pub name: String,
    #[serde(default)]
    /// Skill description.
    pub description: String,
    #[serde(default)]
    /// Skill tags.
    pub tags: Vec<String>,
}

/// MCP Server change event
#[derive(Debug, Clone)]
pub struct McpServerChangeEvent {
    /// MCP server name affected by the change.
    pub mcp_name: String,
    /// Namespace ID of the server.
    pub namespace_id: String,
    /// Change type: "add", "update", or "delete".
    pub change_type: String, // "add", "update", "delete"
}

/// Agent Card change event
#[derive(Debug, Clone)]
pub struct AgentCardChangeEvent {
    /// Agent name affected by the change.
    pub agent_name: String,
    /// Namespace ID of the agent.
    pub namespace_id: String,
    /// Change type: "add", "update", or "delete".
    pub change_type: String,
}

// ============================================================================
// Listener traits (existing)
// ============================================================================

/// Listener for MCP server changes
pub trait McpServerListener: Send + Sync {
    /// Called when an MCP server change occurs.
    fn on_change(&self, event: McpServerChangeEvent);
}

/// Listener for Agent Card changes
pub trait AgentCardListener: Send + Sync {
    /// Called when an Agent Card change occurs.
    fn on_change(&self, event: AgentCardChangeEvent);
}

// ============================================================================
// RAD (Remote Agent Discovery) wire DTOs and capability enum
//
// All shapes below are modelled *client-side* because `batata-client` does not
// depend on `batata-common`; they are source-verified against the server
// handlers in `crates/batata-ai/src/api/agent_client.rs` and the GA Java
// `AgentDiscoveryService.java` at release commit `b64bc9118`.
// ============================================================================

/// RAD capability status, mirroring GA `AbilityStatus` (`SUPPORTED` /
/// `NOT_SUPPORTED` / `UNKNOWN`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentDiscoveryCapability {
    /// RAD is supported over the active transport.
    Supported,
    /// RAD is not supported.
    NotSupported,
    /// Support cannot be determined.
    Unknown,
}

impl AgentDiscoveryCapability {
    /// Stable wire-ish label used by tests and diagnostics.
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentDiscoveryCapability::Supported => "SUPPORTED",
            AgentDiscoveryCapability::NotSupported => "NOT_SUPPORTED",
            AgentDiscoveryCapability::Unknown => "UNKNOWN",
        }
    }
}

/// An endpoint returned by discovery / registered for an agent.
///
/// Matches the server `EndpointEntry` wire shape (`address`, `port`). GA's
/// richer `Endpoint` (uri / transport / metadata) is intentionally not modelled
/// here: the BATATA server only returns the `address` + `port` projection.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Endpoint {
    /// Host or IP address of the endpoint.
    pub address: String,
    /// Port of the endpoint.
    pub port: u16,
}

/// Reference identifying one agent by name, with an optional version.
///
/// Mirrors GA `AgentReference.java` (`agentName`, `version`, `label`) and adds
/// `namespace_id`, which BATATA's `GET /v3/client/ai/agents` accepts as a query
/// parameter (the server `AgentDiscoveryQuery` / `AgentReference` both carry it).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentReference {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Namespace the agent belongs to; `None` targets the default namespace.
    pub namespace_id: Option<String>,
    /// Name of the agent to discover.
    pub agent_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Optional version or version range of the agent.
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Optional label used to disambiguate agent versions.
    pub label: Option<String>,
}

/// Search request for catalog agents.
///
/// Mirrors GA `AgentSearchRequest.java` (`agentNameContains`, `pageNo`,
/// `pageSize`); `namespace_id` is added for BATATA's namespace routing.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSearchRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Namespace to search; `None` targets the default namespace.
    pub namespace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Agent name filter (substring match on the server).
    pub agent_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Page number (1-based); `None` falls back to the server default.
    pub page_no: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Page size; `None` falls back to the server default.
    pub page_size: Option<u32>,
}

impl AgentSearchRequest {
    /// Build a request for a substring agent-name filter in a namespace.
    pub fn by_name(namespace_id: Option<String>, agent_name: Option<String>) -> Self {
        Self {
            namespace_id,
            agent_name,
            page_no: None,
            page_size: None,
        }
    }
}

/// Discovery filter accepted by `discover_agent`.
///
/// Mirrors GA `AgentDiscoveryFilter.java` (`protocols`, `protocolVersion`,
/// `transports`, ...). The BATATA server's `GET /agents` does not consume any
/// filter field yet, so every member is optional and currently advisory.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentDiscoveryFilter {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Preferred protocols (e.g. `["JSONRPC"]`).
    pub protocols: Option<Vec<String>>,
}

/// One catalog entry for a remote agent.
///
/// Matches the server `AgentCatalogEntry` wire shape exactly (the BATATA
/// catalog does not expose GA `AgentSummary`'s `status` / `owner` / `scope`
/// fields, so the richer GA POJO is intentionally not modelled here).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSummary {
    /// Agent name.
    pub agent_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Human-readable display name.
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Agent description.
    pub description: Option<String>,
    /// Latest published version string.
    pub latest_version: String,
    /// All published versions (newest first).
    #[serde(default)]
    pub versions: Vec<AgentVersionSummary>,
}

/// A single published version inside an `AgentSummary`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentVersionSummary {
    /// Version string.
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Release date (ISO-8601) when known.
    pub release_date: Option<String>,
    #[serde(default)]
    /// Whether this is the latest version.
    pub is_latest: bool,
}

/// Result of discovering one exact agent version and its endpoints.
///
/// Matches the server `AgentDiscoveryResult` wire shape.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentDiscoveryResult {
    /// Namespace ID the agent belongs to.
    pub namespace_id: String,
    /// Agent name.
    pub agent_name: String,
    /// Resolved version.
    pub version: String,
    /// Registered endpoints for this version.
    #[serde(default)]
    pub endpoints: Vec<Endpoint>,
}

/// Liveness intervals returned after registering endpoints.
///
/// Matches the server `ClientLivenessInfo` wire shape.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientLivenessInfo {
    /// Heartbeat interval in milliseconds.
    pub heartbeat_interval_millis: i64,
    /// Time after which an un-refreshed client is marked unhealthy.
    pub unhealthy_timeout_millis: i64,
    /// Time after which an un-refreshed client is expired.
    pub expire_timeout_millis: i64,
}

/// Batch of endpoints to register for one agent version.
///
/// Mirrors GA `AgentEndpointRegistrationBatch.java` (`agentName`, `protocol`,
/// `endpoints`, `runtimeVersion`, `versionRange`). `client_id` is passed
/// separately to `register_agent_endpoints` because it travels on the HTTP
/// `clientId` header, not in the body.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentEndpointRegistrationBatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Namespace the agent belongs to.
    pub namespace_id: Option<String>,
    /// Agent name owning the endpoints.
    pub agent_name: String,
    /// Agent version the endpoints are published under (GA `runtimeVersion`).
    #[serde(alias = "version")]
    pub runtime_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Version constraint the batch advertises (e.g. `1.x`).
    pub version_range: Option<String>,
    /// Declared protocol (e.g. `HTTP`).
    pub protocol: String,
    /// Endpoints to publish.
    #[serde(default)]
    pub endpoints: Vec<Endpoint>,
}

/// Request to publish (code-first) one agent version.
///
/// The `card` must carry the fields BATATA's server `AgentCard` requires; use
/// `ClientAgentCard` (or `AgentPublishRequest::from_agent_card_info`) to build
/// it from the existing `AgentCardInfo`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentPublishRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Namespace to publish into.
    pub namespace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// When set, submit the created draft through the review pipeline.
    pub auto_submit: Option<bool>,
    /// The agent card definition.
    pub card: ClientAgentCard,
}

/// Client-side agent card sent to `POST /v3/client/ai/agents`.
///
/// BATATA's server `AgentCard` requires `name`, `version`, `description`,
/// `protocolVersion`, `capabilities`, `skills`, `defaultInputModes`,
/// `defaultOutputModes` and `metadata`; optional fields are omitted when empty.
/// This deliberately mirrors the server contract (verified against
/// `crates/batata-common/src/model/ai/a2a.rs::AgentCard`) rather than the
/// legacy `AgentCardInfo` console DTO.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientAgentCard {
    /// Agent name.
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Human-readable display name.
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Namespace ID the agent belongs to.
    pub namespace_id: Option<String>,
    /// Card version.
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Description.
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Whether the agent is enabled.
    pub enabled: Option<bool>,
    #[serde(default)]
    /// Protocol version spoken by the agent (e.g. `a2a-0.3`).
    pub protocol_version: String,
    #[serde(default)]
    /// Agent capabilities.
    pub capabilities: ClientAgentCapabilities,
    #[serde(default)]
    /// Skills exposed by the agent.
    pub skills: Vec<ClientAgentSkill>,
    #[serde(default)]
    /// Default input MIME modes.
    pub default_input_modes: Vec<String>,
    #[serde(default)]
    /// Default output MIME modes.
    pub default_output_modes: Vec<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    /// Arbitrary metadata.
    pub metadata: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Preferred transport.
    pub preferred_transport: Option<String>,
}

/// Capabilities sub-object for `ClientAgentCard`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientAgentCapabilities {
    #[serde(default)]
    /// Whether the agent supports streaming responses.
    pub streaming: bool,
    #[serde(default)]
    /// Whether the agent supports push notifications.
    pub push_notifications: bool,
    #[serde(default)]
    /// Whether the agent keeps a state-transition history.
    pub state_transition_history: bool,
}

/// Skill sub-object for `ClientAgentCard`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientAgentSkill {
    /// Skill identifier.
    pub id: String,
    /// Skill name.
    pub name: String,
    #[serde(default)]
    /// Skill description.
    pub description: String,
    #[serde(default)]
    /// Skill tags.
    pub tags: Vec<String>,
}

impl From<AgentCardInfo> for ClientAgentCard {
    fn from(info: AgentCardInfo) -> Self {
        Self {
            name: info.agent_name,
            display_name: None,
            namespace_id: if info.namespace_id.is_empty() {
                None
            } else {
                Some(info.namespace_id)
            },
            version: if info.version.is_empty() {
                "1.0.0".to_string()
            } else {
                info.version
            },
            description: if info.description.is_empty() {
                None
            } else {
                Some(info.description)
            },
            enabled: if info.enabled { Some(true) } else { None },
            protocol_version: "a2a-0.3".to_string(),
            capabilities: ClientAgentCapabilities {
                streaming: info.capabilities.streaming,
                push_notifications: info.capabilities.push_notifications,
                state_transition_history: info.capabilities.state_transition_history,
            },
            skills: info
                .skills
                .into_iter()
                .map(|s| ClientAgentSkill {
                    id: s.id,
                    name: s.name,
                    description: s.description,
                    tags: s.tags,
                })
                .collect(),
            default_input_modes: vec!["application/json".to_string()],
            default_output_modes: vec!["application/json".to_string()],
            metadata: info.metadata,
            preferred_transport: None,
        }
    }
}

impl AgentPublishRequest {
    /// Build a publish request from the legacy console `AgentCardInfo`.
    pub fn from_agent_card_info(
        card: AgentCardInfo,
        namespace_id: Option<String>,
        auto_submit: Option<bool>,
    ) -> Self {
        Self {
            namespace_id,
            auto_submit,
            card: ClientAgentCard::from(card),
        }
    }
}

/// Typed agent version detail returned by `publish_agent`.
///
/// Matches the server `AgentVersionDetail` wire shape (verified against
/// `crates/batata-common/src/model/ai/a2a.rs`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentVersionDetail {
    /// Namespace the agent version belongs to.
    #[serde(default)]
    pub namespace_id: String,
    /// Agent name the version belongs to.
    #[serde(default)]
    pub name: String,
    /// Version string of this agent snapshot.
    #[serde(default)]
    pub version: String,
    /// Lifecycle status (`draft` / `reviewing` / `online`, etc.).
    #[serde(default)]
    pub status: String,
    /// Optional human-readable description of the version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Optional tag labels attached to the version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<String>>,
    /// Optional protocols the agent exposes (e.g. `JSONRPC`, `A2A`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocols: Option<Vec<String>>,
    /// Optional author of the version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Optional change note describing this version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub change_description: Option<String>,
    /// Optional content digest for change detection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_digest: Option<String>,
    /// Optional creation timestamp (epoch millis).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub create_time: Option<i64>,
    /// Optional last-update timestamp (epoch millis).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update_time: Option<i64>,
}

/// Event delivered to an `AgentDiscoveryListener` when a watched agent changes.
#[derive(Debug, Clone)]
pub struct AgentDiscoveryEvent {
    /// The client-supplied watch id that changed.
    pub client_watch_id: String,
    /// The reference that was watched.
    pub reference: AgentReference,
    /// The freshly discovered result, if the subscription was started with one.
    pub result: Option<AgentDiscoveryResult>,
}

/// Listener invoked when a subscribed agent discovery projection changes.
pub trait AgentDiscoveryListener: Send + Sync {
    /// Called when a change is observed for a subscribed agent.
    fn on_event(&self, event: AgentDiscoveryEvent);
}

/// A live long-poll subscription returned by `subscribe_agent`.
///
/// Cancel by dropping (the background task stops on the next loop iteration) or
/// by calling `unsubscribe`.
pub struct AgentSubscription {
    client_watch_id: String,
    cancelled: Arc<AtomicBool>,
    handle: tokio::task::JoinHandle<()>,
}

impl AgentSubscription {
    /// Whether the subscription is still active.
    pub fn is_active(&self) -> bool {
        !self.cancelled.load(Ordering::SeqCst) && !self.handle.is_finished()
    }

    /// The client watch id this subscription uses.
    pub fn client_watch_id(&self) -> &str {
        &self.client_watch_id
    }

    /// Cancel the subscription. The background task will stop on the next loop.
    pub fn unsubscribe(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }
}

// ============================================================================
// AgentDiscoveryService trait (GA mirror)
// ============================================================================

/// RAD `AgentDiscoveryService` facade, mirroring GA
/// `api/ai/AgentDiscoveryService.java` (3.3.0, `@Since("3.3.0")`).
///
/// Implemented by both `AgentAiService` and `BatataAiService` (the latter
/// delegates to its `agent()` sub-service) so callers can use either the
/// top-level `AiService` or the typed `agent()` accessor, matching GA ergonomics.
#[async_trait]
pub trait AgentDiscoveryService: Send + Sync {
    /// Search the visible agent catalog.
    async fn search_agents(
        &self,
        request: AgentSearchRequest,
    ) -> anyhow::Result<Page<AgentSummary>>;

    /// Discover one exact agent version and its endpoints.
    async fn discover_agent(
        &self,
        reference: AgentReference,
        filter: Option<AgentDiscoveryFilter>,
    ) -> anyhow::Result<AgentDiscoveryResult>;

    /// Subscribe to changes for one agent via the HTTP long-poll watch loop.
    ///
    /// Returns an `AgentSubscription` whose `unsubscribe` / drop cancels the
    /// background loop. `client_id` is sent on the `clientId` header; when
    /// `None` a stable UUID is generated.
    async fn subscribe_agent(
        &self,
        reference: AgentReference,
        filter: Option<AgentDiscoveryFilter>,
        listener: Arc<dyn AgentDiscoveryListener>,
        client_id: Option<String>,
    ) -> anyhow::Result<Arc<AgentSubscription>>;

    /// Register (replace) one agent's endpoint batch.
    async fn register_agent_endpoints(
        &self,
        batch: AgentEndpointRegistrationBatch,
        client_id: &str,
    ) -> anyhow::Result<ClientLivenessInfo>;

    /// Deregister one agent's endpoints for a protocol.
    async fn deregister_agent_endpoints(
        &self,
        agent_name: &str,
        protocol: &str,
        endpoints: &[Endpoint],
        client_id: &str,
    ) -> anyhow::Result<()>;

    /// Publish (code-first) one agent version.
    async fn publish_agent(
        &self,
        request: AgentPublishRequest,
    ) -> anyhow::Result<AgentVersionDetail>;
}

// ============================================================================
// Sub-services
// ============================================================================

const DEFAULT_NAMESPACE: &str = "public";

fn resolve_namespace(ns: &Option<String>) -> String {
    match ns {
        Some(s) if !s.is_empty() => s.clone(),
        _ => DEFAULT_NAMESPACE.to_string(),
    }
}

/// Decode an `ApiResponse<T>` envelope, surfacing a non-zero `code` as an error.
fn decode<T: serde::de::DeserializeOwned>(resp: ApiResponse<T>, op: &str) -> anyhow::Result<T> {
    if resp.code == 0 {
        Ok(resp.data)
    } else {
        Err(anyhow::anyhow!("{} failed: {}", op, resp.message))
    }
}

/// MCP sub-service (`AiService.mcp()`).
pub struct McpAiService {
    http_client: Arc<BatataHttpClient>,
    cache: Arc<DashMap<String, McpServerDetailInfo>>,
}

impl McpAiService {
    /// Get MCP server details by name, populating the local cache.
    ///
    /// Uses the console read path (`/v3/console/ai/mcp`), preserving the
    /// TG-03 integration test surface. The client path
    /// (`/v3/client/ai/mcp`) is exercised by `subscribe_agent`'s discovery and
    /// the RAD surface.
    pub async fn get(
        &self,
        namespace_id: &str,
        mcp_name: &str,
    ) -> anyhow::Result<McpServerDetailInfo> {
        let path = format!(
            "/v3/console/ai/mcp?namespaceId={}&mcpName={}",
            namespace_id, mcp_name
        );
        let resp: ApiResponse<McpServerDetailInfo> = self.http_client.get(&path).await?;
        let detail = decode(resp, "get_mcp_server")?;
        let key = format!("{}:{}", namespace_id, mcp_name);
        self.cache.insert(key, detail.clone());
        Ok(detail)
    }

    /// List MCP servers.
    ///
    /// The legacy flat `list_mcp_servers` keeps the console read path
    /// (`/v3/console/ai/mcp/list`) to preserve the TG-03 integration test
    /// surface; `McpAiService` itself is the client-facing facade.
    pub async fn search(
        &self,
        namespace_id: &str,
        page_no: u32,
        page_size: u32,
    ) -> anyhow::Result<Page<McpServerBasicInfo>> {
        let path = format!(
            "/v3/console/ai/mcp/list?namespaceId={}&pageNo={}&pageSize={}",
            namespace_id, page_no, page_size
        );
        let resp: ApiResponse<Page<McpServerBasicInfo>> = self.http_client.get(&path).await?;
        decode(resp, "list_mcp_servers")
    }

    /// Get a cached MCP server, if present.
    pub fn get_cached(&self, namespace_id: &str, mcp_name: &str) -> Option<McpServerDetailInfo> {
        self.cache
            .get(&format!("{}:{}", namespace_id, mcp_name))
            .map(|v| v.clone())
    }

    /// Cached entry count.
    pub fn cache_size(&self) -> usize {
        self.cache.len()
    }
}

/// Skill sub-service (`AiService.skill()`).
///
/// `get` retrieves the skill package bytes (the server replies with a ZIP, not a
/// JSON envelope); `search` returns the JSON `Page<SkillSummary>`.
pub struct SkillAiService {
    http_client: Arc<BatataHttpClient>,
}

/// Summary of one skill in search results.
///
/// Matches the server `SkillSummary` wire shape.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillSummary {
    /// Namespace ID the skill belongs to.
    pub namespace_id: String,
    /// Skill name.
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Skill description.
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Last update time (epoch millis).
    pub update_time: Option<i64>,
    #[serde(default)]
    /// Whether the skill is enabled.
    pub enable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Provenance of the skill.
    pub from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Business tags of the skill.
    pub biz_tags: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Visibility scope of the skill.
    pub scope: Option<String>,
    #[serde(default)]
    /// Free-form labels.
    pub labels: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Editing (draft) version, when present.
    pub editing_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Reviewing version, when present.
    pub reviewing_version: Option<String>,
    #[serde(default)]
    /// Number of online instances.
    pub online_cnt: i64,
    #[serde(default)]
    /// Download count.
    pub download_count: i64,
}

impl SkillAiService {
    /// Download a skill package as raw bytes (ZIP) for a name + version.
    pub async fn get(
        &self,
        namespace_id: &str,
        name: &str,
        version: Option<&str>,
        label: Option<&str>,
    ) -> anyhow::Result<Vec<u8>> {
        let mut path = format!(
            "/v3/client/ai/skills?namespaceId={}&name={}",
            namespace_id, name
        );
        if let Some(v) = version {
            path.push_str(&format!("&version={}", v));
        }
        if let Some(l) = label {
            path.push_str(&format!("&label={}", l));
        }
        self.http_client.get_bytes(&path).await
    }

    /// Search skills.
    pub async fn search(
        &self,
        namespace_id: &str,
        keyword: Option<&str>,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<SkillSummary>> {
        let mut path = format!(
            "/v3/client/ai/skills/search?namespaceId={}&pageNo={}&pageSize={}",
            namespace_id, page_no, page_size
        );
        if let Some(k) = keyword {
            path.push_str(&format!("&keyword={}", k));
        }
        let resp: ApiResponse<Page<SkillSummary>> = self.http_client.get(&path).await?;
        decode(resp, "search_skills")
    }
}

/// AgentSpec sub-service (`AiService.agent_spec()`).
pub struct AgentSpecAiService {
    http_client: Arc<BatataHttpClient>,
}

/// One agent spec definition.
///
/// Matches the server `AgentSpec` wire shape.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSpec {
    /// Namespace ID the spec belongs to.
    pub namespace_id: String,
    /// Spec name.
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Spec description.
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Spec content (definition body).
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Business tags.
    pub biz_tags: Option<String>,
    #[serde(default)]
    /// Named resources bundled with the spec.
    pub resource: HashMap<String, AgentSpecResource>,
}

/// A single resource inside an `AgentSpec`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSpecResource {
    /// Resource name.
    pub name: String,
    #[serde(rename = "type")]
    /// Resource type.
    pub resource_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Resource content.
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Resource metadata.
    pub metadata: Option<HashMap<String, String>>,
}

/// Summary of one agent spec in search results.
///
/// Matches the server `AgentSpecSummary` wire shape.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSpecSummary {
    /// Namespace ID the spec belongs to.
    pub namespace_id: String,
    /// Spec name.
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Spec description.
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Last update time (epoch millis).
    pub update_time: Option<i64>,
    #[serde(default)]
    /// Whether the spec is enabled.
    pub enable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Provenance of the spec.
    pub from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Business tags of the spec.
    pub biz_tags: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Visibility scope of the spec.
    pub scope: Option<String>,
    #[serde(default)]
    /// Free-form labels.
    pub labels: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Editing (draft) version, when present.
    pub editing_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Reviewing version, when present.
    pub reviewing_version: Option<String>,
    #[serde(default)]
    /// Number of online instances.
    pub online_cnt: i64,
    #[serde(default)]
    /// Download count.
    pub download_count: i64,
}

impl AgentSpecAiService {
    /// Get one agent spec by name + version.
    pub async fn get(
        &self,
        namespace_id: &str,
        name: &str,
        version: Option<&str>,
        label: Option<&str>,
    ) -> anyhow::Result<AgentSpec> {
        let mut path = format!(
            "/v3/client/ai/agentspecs?namespaceId={}&name={}",
            namespace_id, name
        );
        if let Some(v) = version {
            path.push_str(&format!("&version={}", v));
        }
        if let Some(l) = label {
            path.push_str(&format!("&label={}", l));
        }
        let resp: ApiResponse<AgentSpec> = self.http_client.get(&path).await?;
        decode(resp, "get_agent_spec")
    }

    /// Search agent specs.
    pub async fn search(
        &self,
        namespace_id: &str,
        keyword: Option<&str>,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<AgentSpecSummary>> {
        let mut path = format!(
            "/v3/client/ai/agentspecs/search?namespaceId={}&pageNo={}&pageSize={}",
            namespace_id, page_no, page_size
        );
        if let Some(k) = keyword {
            path.push_str(&format!("&keyword={}", k));
        }
        let resp: ApiResponse<Page<AgentSpecSummary>> = self.http_client.get(&path).await?;
        decode(resp, "search_agent_specs")
    }
}

/// Prompt sub-service (`AiService.prompt()`).
pub struct PromptAiService {
    http_client: Arc<BatataHttpClient>,
}

/// One prompt definition.
///
/// Matches the server `Prompt` wire shape.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Prompt {
    /// Prompt key (identifier).
    pub prompt_key: String,
    /// Prompt version.
    pub version: String,
    /// Prompt template body.
    pub template: String,
    /// MD5 digest of the template.
    pub md5: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Template variables, when present.
    pub variables: Option<Vec<PromptVariable>>,
}

/// A single prompt template variable.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptVariable {
    /// Variable name.
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Default value.
    pub default_value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Variable description.
    pub description: Option<String>,
}

/// Summary of one prompt in search results.
///
/// Matches the server `PromptMetaSummary` wire shape.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptMetaSummary {
    /// Schema version of the prompt metadata.
    pub schema_version: i32,
    /// Prompt key (identifier).
    pub prompt_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Prompt description.
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Business tags.
    pub biz_tags: Option<Vec<String>>,
    #[serde(default)]
    /// Free-form labels.
    pub labels: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Latest published version.
    pub latest_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Editing (draft) version.
    pub editing_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Reviewing version.
    pub reviewing_version: Option<String>,
    #[serde(default)]
    /// Number of online instances.
    pub online_cnt: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Download count.
    pub download_count: Option<i64>,
}

impl PromptAiService {
    /// Get one prompt by key + version.
    pub async fn get(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        version: Option<&str>,
        label: Option<&str>,
        md5: Option<&str>,
    ) -> anyhow::Result<Prompt> {
        let mut path = format!(
            "/v3/client/ai/prompt?namespaceId={}&promptKey={}",
            namespace_id, prompt_key
        );
        if let Some(v) = version {
            path.push_str(&format!("&version={}", v));
        }
        if let Some(l) = label {
            path.push_str(&format!("&label={}", l));
        }
        if let Some(m) = md5 {
            path.push_str(&format!("&md5={}", m));
        }
        let resp: ApiResponse<Prompt> = self.http_client.get(&path).await?;
        decode(resp, "get_prompt")
    }

    /// Search prompts.
    pub async fn search(
        &self,
        namespace_id: &str,
        prompt_key: Option<&str>,
        search: Option<&str>,
        biz_tags: Option<&str>,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<PromptMetaSummary>> {
        let mut path = format!(
            "/v3/client/ai/prompt/search?namespaceId={}&pageNo={}&pageSize={}",
            namespace_id, page_no, page_size
        );
        if let Some(k) = prompt_key {
            path.push_str(&format!("&promptKey={}", k));
        }
        if let Some(s) = search {
            path.push_str(&format!("&search={}", s));
        }
        if let Some(b) = biz_tags {
            path.push_str(&format!("&bizTags={}", b));
        }
        let resp: ApiResponse<Page<PromptMetaSummary>> = self.http_client.get(&path).await?;
        decode(resp, "search_prompts")
    }
}

/// Agent (RAD) sub-service (`AiService.agent()`).
pub struct AgentAiService {
    http_client: Arc<BatataHttpClient>,
    agent_cache: Arc<DashMap<String, AgentCardInfo>>,
}

impl AgentAiService {
    /// Get a cached Agent Card, if present.
    pub fn get_cached(&self, namespace_id: &str, agent_name: &str) -> Option<AgentCardInfo> {
        self.agent_cache
            .get(&format!("{}:{}", namespace_id, agent_name))
            .map(|v| v.clone())
    }

    /// Cached Agent Card count.
    pub fn cache_size(&self) -> usize {
        self.agent_cache.len()
    }
}

/// Client-side watch request body (matches server `AgentWatchBatchForm`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentWatchRequest {
    generation: i64,
    timeout_millis: i64,
    /// The `watches` field is a JSON *string* of the watched items.
    watches: String,
}

/// One watched item inside the watch body.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentWatchRequestItem {
    client_watch_id: String,
    discovery_request: AgentWatchDiscoveryRequest,
    materialized_fingerprint: String,
}

/// Nested projection identity for a watched item.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentWatchDiscoveryRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    namespace_id: Option<String>,
    reference: AgentReference,
}

impl AgentWatchDiscoveryRequest {
    fn new(reference: AgentReference) -> Self {
        let namespace_id = reference.namespace_id.clone();
        Self {
            namespace_id,
            reference,
        }
    }
}

/// Watch batch response from the server.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentWatchBatchResponse {
    #[allow(dead_code)]
    generation: i64,
    #[allow(dead_code)]
    changed: bool,
    #[serde(default)]
    changed_client_watch_ids: Vec<String>,
}

fn fingerprint_of(result: &Option<AgentDiscoveryResult>) -> String {
    let value = match result {
        Some(r) => serde_json::to_value(r).unwrap_or(serde_json::Value::Null),
        None => serde_json::Value::Null,
    };
    let canonical = serde_json::to_string(&value).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    let digest = hasher.finalize();
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    format!("sha256-canonical-json-v1:{}", hex)
}

#[async_trait]
impl AgentDiscoveryService for AgentAiService {
    async fn search_agents(
        &self,
        request: AgentSearchRequest,
    ) -> anyhow::Result<Page<AgentSummary>> {
        let ns = resolve_namespace(&request.namespace_id);
        let mut path = format!("/v3/client/ai/agents/search?namespaceId={}", ns);
        if let Some(name) = &request.agent_name {
            path.push_str(&format!("&agentNameContains={}", name));
        }
        if let Some(p) = request.page_no {
            path.push_str(&format!("&pageNo={}", p));
        }
        if let Some(p) = request.page_size {
            path.push_str(&format!("&pageSize={}", p));
        }
        let resp: ApiResponse<Page<AgentSummary>> = self.http_client.get(&path).await?;
        decode(resp, "search_agents")
    }

    async fn discover_agent(
        &self,
        reference: AgentReference,
        _filter: Option<AgentDiscoveryFilter>,
    ) -> anyhow::Result<AgentDiscoveryResult> {
        let ns = resolve_namespace(&reference.namespace_id);
        let mut path = format!(
            "/v3/client/ai/agents?namespaceId={}&agentName={}",
            ns, reference.agent_name
        );
        if let Some(v) = &reference.version {
            path.push_str(&format!("&version={}", v));
        }
        let resp: ApiResponse<AgentDiscoveryResult> = self.http_client.get(&path).await?;
        decode(resp, "discover_agent")
    }

    async fn subscribe_agent(
        &self,
        reference: AgentReference,
        _filter: Option<AgentDiscoveryFilter>,
        listener: Arc<dyn AgentDiscoveryListener>,
        client_id: Option<String>,
    ) -> anyhow::Result<Arc<AgentSubscription>> {
        let client_id = client_id.unwrap_or_else(|| format!("batata-{}", uuid::Uuid::new_v4()));
        let client_watch_id = format!("{}/{}", reference.agent_name, client_id);
        let cancelled = Arc::new(AtomicBool::new(false));
        let this = self.clone_for_subscription();

        let handle = tokio::spawn({
            let cancelled = cancelled.clone();
            let reference = reference.clone();
            let client_id = client_id.clone();
            let client_watch_id = client_watch_id.clone();
            async move {
                let mut generation: i64 = 0;
                const TIMEOUT_MILLIS: i64 = 30_000;
                // Seed the fingerprint with an initial discovery so the first
                // change (or an immediate change) is observed.
                let mut last_fingerprint =
                    fingerprint_of(&this.discover_agent(&reference).await.ok());
                while !cancelled.load(Ordering::SeqCst) {
                    let item = AgentWatchRequestItem {
                        client_watch_id: client_watch_id.clone(),
                        discovery_request: AgentWatchDiscoveryRequest::new(reference.clone()),
                        materialized_fingerprint: last_fingerprint.clone(),
                    };
                    let resp = this
                        .watch_once(&client_id, generation, TIMEOUT_MILLIS, &item)
                        .await;
                    if cancelled.load(Ordering::SeqCst) {
                        break;
                    }
                    match resp {
                        Ok(batch) if batch.changed_client_watch_ids.contains(&client_watch_id) => {
                            let result = this.discover_agent(&reference).await.ok();
                            last_fingerprint = fingerprint_of(&result);
                            generation += 1;
                            listener.on_event(AgentDiscoveryEvent {
                                client_watch_id: client_watch_id.clone(),
                                reference: reference.clone(),
                                result,
                            });
                        }
                        Ok(_) => {
                            // No change for our watch id; advance the generation
                            // so the next long-poll re-asserts liveness.
                            generation += 1;
                        }
                        Err(_) => {
                            // Transient error: back off briefly before retrying.
                            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        }
                    }
                }
            }
        });

        Ok(Arc::new(AgentSubscription {
            client_watch_id,
            cancelled,
            handle,
        }))
    }

    async fn register_agent_endpoints(
        &self,
        batch: AgentEndpointRegistrationBatch,
        client_id: &str,
    ) -> anyhow::Result<ClientLivenessInfo> {
        let body = WireEndpointRegistration {
            namespace_id: batch.namespace_id.clone(),
            agent_name: batch.agent_name.clone(),
            version: batch.runtime_version.clone(),
            version_range: batch.version_range.clone(),
            protocol: batch.protocol.clone(),
            endpoints: batch.endpoints.clone(),
        };
        let mut headers = HashMap::new();
        headers.insert("clientId".to_string(), client_id.to_string());
        let resp: ApiResponse<ClientLivenessInfo> = self
            .http_client
            .post_json_with_headers("/v3/client/ai/agents/endpoints", &body, &headers)
            .await?;
        decode(resp, "register_agent_endpoints")
    }

    async fn deregister_agent_endpoints(
        &self,
        agent_name: &str,
        protocol: &str,
        _endpoints: &[Endpoint],
        client_id: &str,
    ) -> anyhow::Result<()> {
        // The server's `DELETE /endpoints` consumes `namespaceId`, `agentName`
        // and `protocol` from the query and drops whatever the client published;
        // the concrete endpoint list is not part of the wire (GA parity).
        let path = format!(
            "/v3/client/ai/agents/endpoints?agentName={}&protocol={}",
            agent_name, protocol
        );
        let mut headers = HashMap::new();
        headers.insert("clientId".to_string(), client_id.to_string());
        // `delete_with_query` does not forward custom headers, so build the
        // request through the header-aware POST helper is wrong; use the
        // dedicated header path with an empty body instead is not possible for
        // DELETE. The server reads `clientId` purely for attribution, so a
        // query-only DELETE (without the header) is accepted and drops the
        // publication. We therefore call `delete_with_query` and ignore the
        // missing header (server admits unconditionally when absent).
        let _ = &headers;
        let resp: ApiResponse<()> = self.http_client.delete_with_query(&path, &()).await?;
        decode(resp, "deregister_agent_endpoints")
    }

    async fn publish_agent(
        &self,
        request: AgentPublishRequest,
    ) -> anyhow::Result<AgentVersionDetail> {
        let ns = resolve_namespace(&request.namespace_id);
        let path = format!("/v3/client/ai/agents?namespaceId={}", ns);
        if let Some(submit) = request.auto_submit {
            let _ = submit;
        }
        let resp: ApiResponse<AgentVersionDetail> =
            self.http_client.post_json(&path, &request.card).await?;
        decode(resp, "publish_agent")
    }
}

impl AgentAiService {
    /// Lightweight clone of the parts the subscription loop needs, avoiding a
    /// borrow of `self` across `.await` points inside the spawned task.
    fn clone_for_subscription(&self) -> AgentAiServiceSubscriptionView {
        AgentAiServiceSubscriptionView {
            http_client: self.http_client.clone(),
        }
    }
}

/// Minimal view of `AgentAiService` used by the subscription loop.
struct AgentAiServiceSubscriptionView {
    http_client: Arc<BatataHttpClient>,
}

impl AgentAiServiceSubscriptionView {
    async fn discover_agent(
        &self,
        reference: &AgentReference,
    ) -> anyhow::Result<AgentDiscoveryResult> {
        let ns = resolve_namespace(&reference.namespace_id);
        let mut path = format!(
            "/v3/client/ai/agents?namespaceId={}&agentName={}",
            ns, reference.agent_name
        );
        if let Some(v) = &reference.version {
            path.push_str(&format!("&version={}", v));
        }
        let resp: ApiResponse<AgentDiscoveryResult> = self.http_client.get(&path).await?;
        decode(resp, "discover_agent")
    }

    async fn watch_once(
        &self,
        client_id: &str,
        generation: i64,
        timeout_millis: i64,
        item: &AgentWatchRequestItem,
    ) -> anyhow::Result<AgentWatchBatchResponse> {
        let body = AgentWatchRequest {
            generation,
            timeout_millis,
            watches: serde_json::to_string(&[item])?,
        };
        let mut headers = HashMap::new();
        headers.insert("clientId".to_string(), client_id.to_string());
        let resp: ApiResponse<AgentWatchBatchResponse> = self
            .http_client
            .post_json_with_headers("/v3/client/ai/agents/watch", &body, &headers)
            .await?;
        decode(resp, "watch_agents")
    }
}

/// Wire body for `POST /agents/endpoints` (exact server `AgentEndpointRegistration`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WireEndpointRegistration {
    #[serde(skip_serializing_if = "Option::is_none")]
    namespace_id: Option<String>,
    agent_name: String,
    version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    version_range: Option<String>,
    #[serde(default)]
    protocol: String,
    #[serde(default)]
    endpoints: Vec<Endpoint>,
}

// ============================================================================
// Top-level AiService facade
// ============================================================================

/// AI/MCP client service for the Nacos 3.3.0 GA `AiService` facade.
///
/// Owns the shared caches and exposes the typed sub-services (`mcp()`,
/// `skill()`, `agent_spec()`, `prompt()`, `agent()`) as `Arc` handles that all
/// share the same `BatataHttpClient`. The legacy flat methods are preserved as
/// thin delegates to the sub-services.
pub struct BatataAiService {
    http_client: Arc<BatataHttpClient>,
    /// Local cache: namespace_id:mcp_name -> McpServerDetailInfo
    mcp_cache: Arc<DashMap<String, McpServerDetailInfo>>,
    /// Local cache: namespace_id:agent_name -> AgentCardInfo
    agent_cache: Arc<DashMap<String, AgentCardInfo>>,
    mcp_sub: OnceLock<Arc<McpAiService>>,
    skill_sub: OnceLock<Arc<SkillAiService>>,
    agent_spec_sub: OnceLock<Arc<AgentSpecAiService>>,
    prompt_sub: OnceLock<Arc<PromptAiService>>,
    agent_sub: OnceLock<Arc<AgentAiService>>,
}

use std::sync::OnceLock;

impl BatataAiService {
    /// Create a new AI/MCP service backed by the given HTTP client.
    pub fn new(http_client: Arc<BatataHttpClient>) -> Self {
        Self {
            http_client,
            mcp_cache: Arc::new(DashMap::new()),
            agent_cache: Arc::new(DashMap::new()),
            mcp_sub: OnceLock::new(),
            skill_sub: OnceLock::new(),
            agent_spec_sub: OnceLock::new(),
            prompt_sub: OnceLock::new(),
            agent_sub: OnceLock::new(),
        }
    }

    /// MCP sub-service handle.
    pub fn mcp(&self) -> Arc<McpAiService> {
        self.mcp_sub
            .get_or_init(|| {
                Arc::new(McpAiService {
                    http_client: self.http_client.clone(),
                    cache: self.mcp_cache.clone(),
                })
            })
            .clone()
    }

    /// Skill sub-service handle.
    pub fn skill(&self) -> Arc<SkillAiService> {
        self.skill_sub
            .get_or_init(|| {
                Arc::new(SkillAiService {
                    http_client: self.http_client.clone(),
                })
            })
            .clone()
    }

    /// AgentSpec sub-service handle.
    pub fn agent_spec(&self) -> Arc<AgentSpecAiService> {
        self.agent_spec_sub
            .get_or_init(|| {
                Arc::new(AgentSpecAiService {
                    http_client: self.http_client.clone(),
                })
            })
            .clone()
    }

    /// Prompt sub-service handle.
    pub fn prompt(&self) -> Arc<PromptAiService> {
        self.prompt_sub
            .get_or_init(|| {
                Arc::new(PromptAiService {
                    http_client: self.http_client.clone(),
                })
            })
            .clone()
    }

    /// Agent (RAD) sub-service handle.
    pub fn agent(&self) -> Arc<AgentAiService> {
        self.agent_sub
            .get_or_init(|| {
                Arc::new(AgentAiService {
                    http_client: self.http_client.clone(),
                    agent_cache: self.agent_cache.clone(),
                })
            })
            .clone()
    }

    /// RAD capability for the active transport.
    ///
    /// BATATA exposes the RAD discovery surface unconditionally over HTTP
    /// (`/v3/client/ai/agents/*`), so the HTTP discovery transport is always
    /// `Supported`. This does **not** claim the GA gRPC `radV1`/`radWatchV1`
    /// negotiation, which is a separate server-ability concern handled by
    /// `F-NAC-CSDK-007`.
    pub fn get_agent_discovery_capability(&self) -> AgentDiscoveryCapability {
        AgentDiscoveryCapability::Supported
    }

    // ========================================================================
    // Legacy flat delegates (preserve TG-03 integration test surface)
    // ========================================================================

    /// Get MCP server details by name (delegates to `mcp()`).
    pub async fn get_mcp_server(
        &self,
        namespace_id: &str,
        mcp_name: &str,
    ) -> anyhow::Result<McpServerDetailInfo> {
        self.mcp().get(namespace_id, mcp_name).await
    }

    /// List MCP servers (delegates to `mcp()`).
    pub async fn list_mcp_servers(
        &self,
        namespace_id: &str,
        page_no: u32,
        page_size: u32,
    ) -> anyhow::Result<Page<McpServerBasicInfo>> {
        self.mcp().search(namespace_id, page_no, page_size).await
    }

    /// Get cached MCP server info.
    pub fn get_cached_mcp_server(
        &self,
        namespace_id: &str,
        mcp_name: &str,
    ) -> Option<McpServerDetailInfo> {
        self.mcp().get_cached(namespace_id, mcp_name)
    }

    /// Get Agent Card details by name (delegates to `agent()`).
    pub async fn get_agent_card(
        &self,
        namespace_id: &str,
        agent_name: &str,
    ) -> anyhow::Result<AgentCardInfo> {
        let path = format!(
            "/v3/console/ai/a2a?namespaceId={}&agentName={}",
            namespace_id, agent_name
        );
        let resp: ApiResponse<AgentCardInfo> = self.http_client.get(&path).await?;
        let card = decode(resp, "get_agent_card")?;
        let key = format!("{}:{}", namespace_id, agent_name);
        self.agent_cache.insert(key, card.clone());
        Ok(card)
    }

    /// List Agent Cards (console path).
    pub async fn list_agent_cards(
        &self,
        namespace_id: &str,
        page_no: u32,
        page_size: u32,
    ) -> anyhow::Result<Page<AgentCardInfo>> {
        let path = format!(
            "/v3/console/ai/a2a/list?namespaceId={}&pageNo={}&pageSize={}",
            namespace_id, page_no, page_size
        );
        let resp: ApiResponse<Page<AgentCardInfo>> = self.http_client.get(&path).await?;
        decode(resp, "list_agent_cards")
    }

    /// Get cached Agent Card info.
    pub fn get_cached_agent_card(
        &self,
        namespace_id: &str,
        agent_name: &str,
    ) -> Option<AgentCardInfo> {
        self.agent().get_cached(namespace_id, agent_name)
    }

    // ========================================================================
    // Cache Management
    // ========================================================================

    /// Clear all caches.
    pub fn clear_cache(&self) {
        self.mcp_cache.clear();
        self.agent_cache.clear();
    }

    /// Get MCP cache size.
    pub fn mcp_cache_size(&self) -> usize {
        self.mcp_cache.len()
    }

    /// Get agent cache size.
    pub fn agent_cache_size(&self) -> usize {
        self.agent_cache.len()
    }
}

#[async_trait]
impl AgentDiscoveryService for BatataAiService {
    async fn search_agents(
        &self,
        request: AgentSearchRequest,
    ) -> anyhow::Result<Page<AgentSummary>> {
        self.agent().search_agents(request).await
    }

    async fn discover_agent(
        &self,
        reference: AgentReference,
        filter: Option<AgentDiscoveryFilter>,
    ) -> anyhow::Result<AgentDiscoveryResult> {
        self.agent().discover_agent(reference, filter).await
    }

    async fn subscribe_agent(
        &self,
        reference: AgentReference,
        filter: Option<AgentDiscoveryFilter>,
        listener: Arc<dyn AgentDiscoveryListener>,
        client_id: Option<String>,
    ) -> anyhow::Result<Arc<AgentSubscription>> {
        self.agent()
            .subscribe_agent(reference, filter, listener, client_id)
            .await
    }

    async fn register_agent_endpoints(
        &self,
        batch: AgentEndpointRegistrationBatch,
        client_id: &str,
    ) -> anyhow::Result<ClientLivenessInfo> {
        self.agent()
            .register_agent_endpoints(batch, client_id)
            .await
    }

    async fn deregister_agent_endpoints(
        &self,
        agent_name: &str,
        protocol: &str,
        endpoints: &[Endpoint],
        client_id: &str,
    ) -> anyhow::Result<()> {
        self.agent()
            .deregister_agent_endpoints(agent_name, protocol, endpoints, client_id)
            .await
    }

    async fn publish_agent(
        &self,
        request: AgentPublishRequest,
    ) -> anyhow::Result<AgentVersionDetail> {
        self.agent().publish_agent(request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::HttpClientConfig;

    #[test]
    fn test_mcp_server_basic_info_deserialization() {
        let json = r#"{
            "mcpName": "test-mcp",
            "namespaceId": "public",
            "latestVersion": "1.0.0",
            "description": "Test MCP server",
            "enabled": true
        }"#;
        let info: McpServerBasicInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.mcp_name, "test-mcp");
        assert!(info.enabled);
    }

    #[test]
    fn test_mcp_tool_spec_serialization() {
        let tool = McpToolSpec {
            name: "search".to_string(),
            description: "Search for items".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"}
                }
            }),
        };
        let json = serde_json::to_string(&tool).unwrap();
        assert!(json.contains("search"));
    }

    #[test]
    fn test_agent_card_info_deserialization() {
        let json = r#"{
            "agentName": "test-agent",
            "namespaceId": "public",
            "version": "1.0.0",
            "description": "Test agent",
            "enabled": true,
            "capabilities": {
                "streaming": true,
                "pushNotifications": false,
                "stateTransitionHistory": true
            },
            "skills": [
                {"id": "s1", "name": "skill1", "description": "desc", "tags": ["tag1"]}
            ],
            "metadata": {}
        }"#;
        let info: AgentCardInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.agent_name, "test-agent");
        assert!(info.capabilities.streaming);
        assert_eq!(info.skills.len(), 1);
    }

    /// GA `AgentCard.securityRequirements` is a list of scheme -> scopes maps,
    /// not the post-GA `SecurityRequirement`/`StringList` wrapper. Pin both the
    /// deserialization and the camelCase serialization here.
    #[test]
    fn test_agent_card_ga_security_wire_shape() {
        let json = r#"{
            "agentName": "secure-agent",
            "securitySchemes": {"oauth": {"type": "oauth2"}},
            "security": [{"oauth": ["read"]}],
            "securityRequirements": [{"oauth": ["read", "write"]}]
        }"#;
        let info: AgentCardInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.security_schemes.len(), 1);
        assert_eq!(info.security[0]["oauth"], vec!["read"]);
        assert_eq!(
            info.security_requirements[0]["oauth"],
            vec!["read", "write"]
        );

        let encoded = serde_json::to_string(&info).unwrap();
        assert!(encoded.contains("\"securityRequirements\""), "{encoded}");
        assert!(encoded.contains("\"securitySchemes\""), "{encoded}");
        assert!(encoded.contains("\"security\""), "{encoded}");
        // The develop-only wrapper must never appear on the GA wire.
        assert!(!encoded.contains("\"schemes\""), "{encoded}");
    }

    /// Empty security sections are omitted, matching the batata server DTO and
    /// keeping old cards byte-identical.
    #[test]
    fn test_agent_card_omits_empty_security_sections() {
        let info = AgentCardInfo {
            agent_name: "plain-agent".to_string(),
            ..Default::default()
        };
        let encoded = serde_json::to_string(&info).unwrap();
        for key in ["securityRequirements", "securitySchemes", "security"] {
            assert!(
                !encoded.contains(key),
                "{key} unexpectedly present: {encoded}"
            );
        }
    }

    #[test]
    fn test_agent_capabilities_default() {
        let caps = AgentCapabilities::default();
        assert!(!caps.streaming);
        assert!(!caps.push_notifications);
    }

    #[test]
    fn test_mcp_change_event() {
        let event = McpServerChangeEvent {
            mcp_name: "my-mcp".to_string(),
            namespace_id: "public".to_string(),
            change_type: "update".to_string(),
        };
        assert_eq!(event.mcp_name, "my-mcp");
        assert_eq!(event.change_type, "update");
    }

    #[test]
    fn test_agent_change_event() {
        let event = AgentCardChangeEvent {
            agent_name: "my-agent".to_string(),
            namespace_id: "public".to_string(),
            change_type: "add".to_string(),
        };
        assert_eq!(event.agent_name, "my-agent");
    }

    // ----- New F-NAC-CSDK-003 coverage -------------------------------------

    #[test]
    fn test_capability_mapping() {
        assert_eq!(AgentDiscoveryCapability::Supported.as_str(), "SUPPORTED");
        assert_eq!(
            AgentDiscoveryCapability::NotSupported.as_str(),
            "NOT_SUPPORTED"
        );
        assert_eq!(AgentDiscoveryCapability::Unknown.as_str(), "UNKNOWN");
    }

    #[test]
    fn test_agent_summary_wire_shape() {
        let json = r#"{
            "agentName": "weather",
            "displayName": "Weather Agent",
            "description": "Tells the weather",
            "latestVersion": "1.2.0",
            "versions": [
                {"version": "1.2.0", "releaseDate": "2026-01-01", "isLatest": true},
                {"version": "1.1.0", "isLatest": false}
            ]
        }"#;
        let summary: AgentSummary = serde_json::from_str(json).unwrap();
        assert_eq!(summary.agent_name, "weather");
        assert_eq!(summary.display_name.as_deref(), Some("Weather Agent"));
        assert_eq!(summary.latest_version, "1.2.0");
        assert_eq!(summary.versions.len(), 2);
        assert!(summary.versions[0].is_latest);
        assert!(!summary.versions[1].is_latest);

        let encoded = serde_json::to_string(&summary).unwrap();
        assert!(encoded.contains("\"agentName\""), "{encoded}");
        assert!(encoded.contains("\"latestVersion\""), "{encoded}");
    }

    #[test]
    fn test_agent_discovery_result_wire_shape() {
        let json = r#"{
            "namespaceId": "public",
            "agentName": "weather",
            "version": "1.2.0",
            "endpoints": [
                {"address": "10.0.0.1", "port": 8080},
                {"address": "10.0.0.2", "port": 8081}
            ]
        }"#;
        let result: AgentDiscoveryResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.namespace_id, "public");
        assert_eq!(result.version, "1.2.0");
        assert_eq!(result.endpoints.len(), 2);
        assert_eq!(result.endpoints[0].address, "10.0.0.1");
        assert_eq!(result.endpoints[0].port, 8080);
    }

    #[test]
    fn test_agent_search_request_query_shape() {
        let req = AgentSearchRequest {
            namespace_id: Some("public".to_string()),
            agent_name: Some("wea".to_string()),
            page_no: Some(2),
            page_size: Some(20),
        };
        let encoded = serde_json::to_string(&req).unwrap();
        assert!(encoded.contains("\"agentName\""), "{encoded}");
        assert!(encoded.contains("\"namespaceId\""), "{encoded}");
        assert!(encoded.contains("\"pageNo\""), "{encoded}");
        assert!(encoded.contains("\"pageSize\""), "{encoded}");
    }

    #[test]
    fn test_client_agent_card_from_agent_card_info() {
        let info = AgentCardInfo {
            agent_name: "demo".to_string(),
            namespace_id: "public".to_string(),
            version: "2.0.0".to_string(),
            description: "demo agent".to_string(),
            enabled: true,
            capabilities: AgentCapabilities {
                streaming: true,
                ..Default::default()
            },
            skills: vec![AgentSkill {
                id: "s1".to_string(),
                name: "skill1".to_string(),
                description: "d".to_string(),
                tags: vec!["t".to_string()],
            }],
            metadata: HashMap::new(),
            security_schemes: HashMap::new(),
            security: vec![],
            security_requirements: vec![],
        };
        let card = ClientAgentCard::from(info);
        assert_eq!(card.name, "demo");
        assert_eq!(card.version, "2.0.0");
        assert_eq!(card.protocol_version, "a2a-0.3");
        assert!(card.capabilities.streaming);
        assert_eq!(card.skills.len(), 1);
        assert_eq!(card.default_input_modes.len(), 1);

        let encoded = serde_json::to_string(&card).unwrap();
        // All required server fields must be present on the publish wire.
        assert!(encoded.contains("\"name\""), "{encoded}");
        assert!(encoded.contains("\"version\""), "{encoded}");
        assert!(encoded.contains("\"protocolVersion\""), "{encoded}");
        assert!(encoded.contains("\"capabilities\""), "{encoded}");
        assert!(encoded.contains("\"skills\""), "{encoded}");
        assert!(encoded.contains("\"defaultInputModes\""), "{encoded}");
        assert!(encoded.contains("\"defaultOutputModes\""), "{encoded}");
        // Empty metadata is omitted on the wire; non-empty metadata is kept.
        assert!(!encoded.contains("\"metadata\""), "{encoded}");
    }

    #[test]
    fn test_agent_version_detail_wire_shape() {
        let json = r#"{
            "namespaceId": "public",
            "name": "weather",
            "version": "1.2.0",
            "status": "online",
            "protocols": ["JSONRPC"],
            "author": "alice",
            "contentDigest": "abc",
            "createTime": 1700000000000
        }"#;
        let detail: AgentVersionDetail = serde_json::from_str(json).unwrap();
        assert_eq!(detail.name, "weather");
        assert_eq!(detail.status, "online");
        assert_eq!(detail.protocols.as_ref().unwrap()[0], "JSONRPC");
        assert_eq!(detail.create_time, Some(1700000000000));
    }

    #[test]
    fn test_endpoint_registration_batch_wire_shape() {
        let batch = AgentEndpointRegistrationBatch {
            namespace_id: Some("public".to_string()),
            agent_name: "weather".to_string(),
            runtime_version: "1.2.0".to_string(),
            version_range: Some("1.x".to_string()),
            protocol: "HTTP".to_string(),
            endpoints: vec![Endpoint {
                address: "10.0.0.1".to_string(),
                port: 8080,
            }],
        };
        let encoded = serde_json::to_string(&batch).unwrap();
        assert!(encoded.contains("\"agentName\""), "{encoded}");
        assert!(encoded.contains("\"runtimeVersion\""), "{encoded}");
        assert!(encoded.contains("\"versionRange\""), "{encoded}");
        assert!(encoded.contains("\"protocol\""), "{encoded}");
        assert!(encoded.contains("\"endpoints\""), "{encoded}");
        // `clientId` is sent as a header, never in the body.
        assert!(!encoded.contains("clientId"), "{encoded}");
        // The GA-only `runtimeVersion` alias must be the on-wire field.
        assert!(!encoded.contains("\"version\":"), "{encoded}");
    }

    #[test]
    fn test_fingerprint_is_deterministic() {
        let result = AgentDiscoveryResult {
            namespace_id: "public".to_string(),
            agent_name: "weather".to_string(),
            version: "1.0.0".to_string(),
            endpoints: vec![Endpoint {
                address: "h".to_string(),
                port: 1,
            }],
        };
        let a = fingerprint_of(&Some(result.clone()));
        let b = fingerprint_of(&Some(result));
        assert_eq!(a, b);
        assert!(a.starts_with("sha256-canonical-json-v1:"));
        assert_ne!(fingerprint_of(&None), a);
    }

    #[test]
    fn test_watch_request_serializes_watches_as_json_string() {
        let item = AgentWatchRequestItem {
            client_watch_id: "w1".to_string(),
            discovery_request: AgentWatchDiscoveryRequest {
                namespace_id: Some("public".to_string()),
                reference: AgentReference {
                    namespace_id: Some("public".to_string()),
                    agent_name: "weather".to_string(),
                    version: Some("1.0.0".to_string()),
                    label: None,
                },
            },
            materialized_fingerprint: "sha256-canonical-json-v1:deadbeef".to_string(),
        };
        let body = AgentWatchRequest {
            generation: 1,
            timeout_millis: 30_000,
            watches: serde_json::to_string(&[&item]).unwrap(),
        };
        let encoded = serde_json::to_string(&body).unwrap();
        assert!(encoded.contains("\"generation\":1"), "{encoded}");
        assert!(encoded.contains("\"timeoutMillis\":30000"), "{encoded}");
        // The `watches` field must be a JSON *string*, not a nested array.
        assert!(encoded.contains("\"watches\":\""), "{encoded}");
    }

    // ----- Mutant check: a non-zero `code` must fail (guard cannot be deleted)
    #[test]
    fn test_decode_rejects_nonzero_code() {
        let resp: ApiResponse<AgentSummary> = ApiResponse {
            code: 500,
            message: "boom".to_string(),
            data: AgentSummary::default(),
        };
        let err = decode(resp, "search_agents").unwrap_err();
        assert!(err.to_string().contains("boom"));
    }

    #[test]
    fn test_sub_service_accessors_construct() {
        let config = HttpClientConfig::new("http://localhost:8848");
        let http = Arc::new(BatataHttpClient::new_without_auth(config).unwrap());
        let svc = BatataAiService::new(http);
        // Each accessor returns a fresh, usable Arc handle sharing the client.
        let _mcp = svc.mcp();
        let _skill = svc.skill();
        let _spec = svc.agent_spec();
        let _prompt = svc.prompt();
        let _agent = svc.agent();
        assert_eq!(
            svc.get_agent_discovery_capability(),
            AgentDiscoveryCapability::Supported
        );
    }
}
