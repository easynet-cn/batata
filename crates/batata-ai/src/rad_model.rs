//! Server-side MCP / A2A (RAD) gRPC models.
//!
//! These request/response models are consumed only by Batata's server-side AI
//! handlers. They live here rather than in the published `batata-api` contract
//! crate so the SDK crates stay independent of `batata-common`.

use std::collections::HashMap;

use prost_types::Any;
use serde::{Deserialize, Serialize};

use batata_api::{
    grpc::Payload,
    remote::model::{Request, RequestTrait, Response, ResponseTrait},
};
use batata_common::model::ai::a2a::{AgentCallInterface, AgentProviderInfo, AgentVersionDetail};

// =============================================================================
// AI-MCP: McpServerEndpoint, QueryMcpServer, ReleaseMcpServer
// =============================================================================

/// Request to register/deregister an MCP server endpoint
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerEndpointRequest {
    #[serde(flatten)]
    /// The `request` field.
    pub request: Request,
    /// The `module` field.
    pub module: String,
    /// The `namespace_id` field.
    pub namespace_id: String,
    #[serde(default)]
    /// The `mcp_id` field.
    pub mcp_id: String,
    /// The `mcp_name` field.
    pub mcp_name: String,
    /// The `address` field.
    pub address: String,
    /// The `port` field.
    pub port: u16,
    /// The `version` field.
    pub version: String,
    #[serde(rename = "type")]
    /// The `operation_type` field.
    pub operation_type: String,
}

batata_api::impl_request_trait!(McpServerEndpointRequest, request);

impl From<&Payload> for McpServerEndpointRequest {
    fn from(value: &Payload) -> Self {
        McpServerEndpointRequest::from_payload(value)
    }
}

/// Response to MCP server endpoint register/deregister
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerEndpointResponse {
    #[serde(flatten)]
    /// The `response` field.
    pub response: Response,
    #[serde(rename = "type")]
    /// The `operation_type` field.
    pub operation_type: String,
}

impl McpServerEndpointResponse {
    /// Creates a new instance.
    pub fn new() -> Self {
        Self {
            response: Response::new(),
            operation_type: String::new(),
        }
    }
}

batata_api::impl_response_trait!(McpServerEndpointResponse);

impl From<McpServerEndpointResponse> for Any {
    fn from(val: McpServerEndpointResponse) -> Self {
        val.to_any()
    }
}

/// Request to query MCP server details
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryMcpServerRequest {
    #[serde(flatten)]
    /// The `request` field.
    pub request: Request,
    /// The `module` field.
    pub module: String,
    /// The `namespace_id` field.
    pub namespace_id: String,
    /// The `mcp_name` field.
    pub mcp_name: String,
    /// The `version` field.
    pub version: String,
}

batata_api::impl_request_trait!(QueryMcpServerRequest, request);

impl From<&Payload> for QueryMcpServerRequest {
    fn from(value: &Payload) -> Self {
        QueryMcpServerRequest::from_payload(value)
    }
}

/// Response containing MCP server details
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryMcpServerResponse {
    #[serde(flatten)]
    /// The `response` field.
    pub response: Response,
    /// The `mcp_server_detail_info` field.
    pub mcp_server_detail_info: serde_json::Value,
}

impl QueryMcpServerResponse {
    /// Creates a new instance.
    pub fn new() -> Self {
        Self {
            response: Response::new(),
            mcp_server_detail_info: serde_json::Value::Null,
        }
    }
}

batata_api::impl_response_trait!(QueryMcpServerResponse);

impl From<QueryMcpServerResponse> for Any {
    fn from(val: QueryMcpServerResponse) -> Self {
        val.to_any()
    }
}

/// Request to release (publish) an MCP server
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseMcpServerRequest {
    #[serde(flatten)]
    /// The `request` field.
    pub request: Request,
    /// The `module` field.
    pub module: String,
    /// The `namespace_id` field.
    pub namespace_id: String,
    /// The `mcp_name` field.
    pub mcp_name: String,
    /// The `server_specification` field.
    pub server_specification: serde_json::Value,
    #[serde(default)]
    /// The `tool_specification` field.
    pub tool_specification: serde_json::Value,
    #[serde(default)]
    /// The `endpoint_specification` field.
    pub endpoint_specification: serde_json::Value,
}

batata_api::impl_request_trait!(ReleaseMcpServerRequest, request);

impl From<&Payload> for ReleaseMcpServerRequest {
    fn from(value: &Payload) -> Self {
        ReleaseMcpServerRequest::from_payload(value)
    }
}

/// Response for releasing (publishing) an MCP server
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseMcpServerResponse {
    #[serde(flatten)]
    /// The `response` field.
    pub response: Response,
    /// The `mcp_id` field.
    pub mcp_id: String,
}

impl ReleaseMcpServerResponse {
    /// Creates a new instance.
    pub fn new() -> Self {
        Self {
            response: Response::new(),
            mcp_id: String::new(),
        }
    }
}

batata_api::impl_response_trait!(ReleaseMcpServerResponse);

impl From<ReleaseMcpServerResponse> for Any {
    fn from(val: ReleaseMcpServerResponse) -> Self {
        val.to_any()
    }
}

// =============================================================================
// AI-A2A: AgentEndpoint, QueryAgentCard, ReleaseAgentCard
// =============================================================================

/// Agent endpoint information for gRPC registration
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentEndpoint {
    /// The `address` field.
    pub address: String,
    /// The `port` field.
    pub port: u16,
    /// The `version` field.
    pub version: String,
    /// The `transport` field.
    pub transport: String,
    /// The `path` field.
    pub path: String,
    /// The `support_tls` field.
    pub support_tls: bool,
}

/// Request to register/deregister an agent endpoint
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentEndpointRequest {
    #[serde(flatten)]
    /// The `request` field.
    pub request: Request,
    /// The `module` field.
    pub module: String,
    /// The `namespace_id` field.
    pub namespace_id: String,
    /// The `agent_name` field.
    pub agent_name: String,
    /// The `endpoint` field.
    pub endpoint: Option<AgentEndpoint>,
    #[serde(rename = "type")]
    /// The `operation_type` field.
    pub operation_type: String,
}

batata_api::impl_request_trait!(AgentEndpointRequest, request);

impl From<&Payload> for AgentEndpointRequest {
    fn from(value: &Payload) -> Self {
        AgentEndpointRequest::from_payload(value)
    }
}

/// Response for agent endpoint register/deregister
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentEndpointResponse {
    #[serde(flatten)]
    /// The `response` field.
    pub response: Response,
    #[serde(rename = "type")]
    /// The `operation_type` field.
    pub operation_type: String,
}

impl AgentEndpointResponse {
    /// Creates a new instance.
    pub fn new() -> Self {
        Self {
            response: Response::new(),
            operation_type: String::new(),
        }
    }
}

batata_api::impl_response_trait!(AgentEndpointResponse);

impl From<AgentEndpointResponse> for Any {
    fn from(val: AgentEndpointResponse) -> Self {
        val.to_any()
    }
}

/// Request to query agent card details
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryAgentCardRequest {
    #[serde(flatten)]
    /// The `request` field.
    pub request: Request,
    /// The `module` field.
    pub module: String,
    /// The `namespace_id` field.
    pub namespace_id: String,
    /// The `agent_name` field.
    pub agent_name: String,
    /// The `version` field.
    pub version: String,
    #[serde(default)]
    /// The `registration_type` field.
    pub registration_type: String,
}

batata_api::impl_request_trait!(QueryAgentCardRequest, request);

impl From<&Payload> for QueryAgentCardRequest {
    fn from(value: &Payload) -> Self {
        QueryAgentCardRequest::from_payload(value)
    }
}

/// Response containing agent card details
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryAgentCardResponse {
    #[serde(flatten)]
    /// The `response` field.
    pub response: Response,
    /// The `agent_card_detail_info` field.
    pub agent_card_detail_info: serde_json::Value,
}

impl QueryAgentCardResponse {
    /// Creates a new instance.
    pub fn new() -> Self {
        Self {
            response: Response::new(),
            agent_card_detail_info: serde_json::Value::Null,
        }
    }
}

batata_api::impl_response_trait!(QueryAgentCardResponse);

impl From<QueryAgentCardResponse> for Any {
    fn from(val: QueryAgentCardResponse) -> Self {
        val.to_any()
    }
}

fn default_registration_type() -> String {
    "service".to_string()
}

/// Request to release (publish) an agent card
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseAgentCardRequest {
    #[serde(flatten)]
    /// The `request` field.
    pub request: Request,
    /// The `module` field.
    pub module: String,
    /// The `namespace_id` field.
    pub namespace_id: String,
    /// The `agent_name` field.
    pub agent_name: String,
    /// The `agent_card` field.
    pub agent_card: serde_json::Value,
    /// The `set_as_latest` field.
    pub set_as_latest: bool,
    #[serde(default = "default_registration_type")]
    /// The `registration_type` field.
    pub registration_type: String,
}

batata_api::impl_request_trait!(ReleaseAgentCardRequest, request);

impl From<&Payload> for ReleaseAgentCardRequest {
    fn from(value: &Payload) -> Self {
        ReleaseAgentCardRequest::from_payload(value)
    }
}

/// Response for releasing (publishing) an agent card
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseAgentCardResponse {
    #[serde(flatten)]
    /// The `response` field.
    pub response: Response,
}

impl ReleaseAgentCardResponse {
    /// Creates a new instance.
    pub fn new() -> Self {
        Self {
            response: Response::new(),
        }
    }
}

batata_api::impl_response_trait!(ReleaseAgentCardResponse);

impl From<ReleaseAgentCardResponse> for Any {
    fn from(val: ReleaseAgentCardResponse) -> Self {
        val.to_any()
    }
}

// =============================================================================
// AI-RAD: AgentSearch, AgentDiscovery, AgentEndpointRegister/Deregister
// (Nacos 3.x Remote Agent Discovery protocol via gRPC)
// =============================================================================

// --- Agent Search ---

/// Inner search request (Nacos `AgentSearchRequest`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentSearchRequest {
    /// The `namespace_id` field.
    pub namespace_id: String,
    /// The `agent_name_contains` field.
    pub agent_name_contains: String,
    /// The `tags_all` field.
    pub tags_all: Vec<String>,
    /// The `protocols_any` field.
    pub protocols_any: Vec<String>,
    /// The `page_no` field.
    pub page_no: u32,
    /// The `page_size` field.
    pub page_size: u32,
}

/// gRPC request for agent search (Nacos `AgentSearchRpcRequest`).
///
/// Wire-compatible: `module` = "ai", wraps inner `AgentSearchRequest`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentSearchRpcRequest {
    #[serde(flatten)]
    /// The `request` field.
    pub request: Request,
    /// The `module` field.
    pub module: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `search_request` field.
    pub search_request: Option<AgentSearchRequest>,
}

batata_api::impl_request_trait!(AgentSearchRpcRequest, request);

impl From<&Payload> for AgentSearchRpcRequest {
    fn from(value: &Payload) -> Self {
        AgentSearchRpcRequest::from_payload(value)
    }
}

/// Catalog version entry in search results (Nacos `AgentCatalogVersion`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentCatalogVersion {
    /// The `version` field.
    pub version: String,
    /// The `labels` field.
    pub labels: Vec<String>,
    /// The `protocols` field.
    pub protocols: Vec<String>,
}

/// Catalog entry for one agent in search results (Nacos `AgentCatalogEntry`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentCatalogEntry {
    /// The `agent_name` field.
    pub agent_name: String,
    /// The `display_name` field.
    pub display_name: String,
    /// The `description` field.
    pub description: String,
    /// The `icon_url` field.
    pub icon_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `provider` field.
    pub provider: Option<serde_json::Value>,
    /// The `tags` field.
    pub tags: Vec<String>,
    /// The `latest_version` field.
    pub latest_version: String,
    /// The `versions` field.
    pub versions: Vec<AgentCatalogVersion>,
}

/// Page wrapper for search results (Nacos `Page<AgentCatalogEntry>`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentSearchPage {
    /// The `total_count` field.
    pub total_count: u64,
    /// The `page_number` field.
    pub page_number: u64,
    /// The `pages_available` field.
    pub pages_available: u64,
    /// The `page_items` field.
    pub page_items: Vec<AgentCatalogEntry>,
}

/// gRPC response for agent search (Nacos `AgentSearchResponse`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSearchResponse {
    #[serde(flatten)]
    /// The `response` field.
    pub response: Response,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `page` field.
    pub page: Option<AgentSearchPage>,
}

impl AgentSearchResponse {
    /// Creates a new instance.
    pub fn new() -> Self {
        Self {
            response: Response::new(),
            page: None,
        }
    }
}

batata_api::impl_response_trait!(AgentSearchResponse);

impl From<AgentSearchResponse> for Any {
    fn from(val: AgentSearchResponse) -> Self {
        val.to_any()
    }
}

// --- Agent Discovery ---

/// Reference to a specific agent version (Nacos `AgentReference`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentReference {
    /// The `agent_name` field.
    pub agent_name: String,
    /// The `version` field.
    pub version: String,
    /// The `label` field.
    pub label: String,
}

/// Optional filter for discovery (Nacos `AgentDiscoveryFilter`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentDiscoveryFilter {
    /// The `protocols` field.
    pub protocols: Vec<String>,
    /// The `protocol_version` field.
    pub protocol_version: String,
    /// The `transports` field.
    pub transports: Vec<String>,
    /// The `endpoint_sources` field.
    pub endpoint_sources: Vec<String>,
    /// The `metadata_selector` field.
    pub metadata_selector: HashMap<String, String>,
}

/// Inner discovery request (Nacos `AgentDiscoveryRequest`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentDiscoveryRequest {
    /// The `namespace_id` field.
    pub namespace_id: String,
    /// The `reference` field.
    pub reference: AgentReference,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `filter` field.
    pub filter: Option<AgentDiscoveryFilter>,
}

/// gRPC request for agent discovery (Nacos `AgentDiscoveryRpcRequest`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentDiscoveryRpcRequest {
    #[serde(flatten)]
    /// The `request` field.
    pub request: Request,
    /// The `module` field.
    pub module: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `discovery_request` field.
    pub discovery_request: Option<AgentDiscoveryRequest>,
}

batata_api::impl_request_trait!(AgentDiscoveryRpcRequest, request);

impl From<&Payload> for AgentDiscoveryRpcRequest {
    fn from(value: &Payload) -> Self {
        AgentDiscoveryRpcRequest::from_payload(value)
    }
}

/// A discovered call interface with endpoints (Nacos `AgentDiscoveryCallInterface`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentDiscoveryCallInterface {
    /// The `protocol` field.
    pub protocol: String,
    /// The `protocol_version` field.
    pub protocol_version: String,
    /// The `descriptor_media_type` field.
    pub descriptor_media_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `native_descriptor` field.
    pub native_descriptor: Option<serde_json::Value>,
    /// The `endpoint_sets` field.
    pub endpoint_sets: Vec<EndpointSet>,
}

/// A set of endpoints from a specific source (Nacos `EndpointSet`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct EndpointSet {
    /// The `source` field.
    pub source: String,
    /// The `source_revision` field.
    pub source_revision: String,
    /// The `endpoints` field.
    pub endpoints: Vec<AgentEndpointInfo>,
}

/// Endpoint info in discovery results (Nacos `Endpoint`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentEndpointInfo {
    /// The `address` field.
    pub address: String,
    /// The `port` field.
    pub port: u16,
    #[serde(skip_serializing_if = "String::is_empty")]
    /// The `transport` field.
    pub transport: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    /// The `path` field.
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `healthy` field.
    pub healthy: Option<bool>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    /// The `metadata` field.
    pub metadata: HashMap<String, String>,
}

/// Discovery result for one agent version (Nacos `AgentDiscoveryResult`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentDiscoveryResult {
    /// The `namespace_id` field.
    pub namespace_id: String,
    /// The `agent_name` field.
    pub agent_name: String,
    /// The `version` field.
    pub version: String,
    /// The `content_digest` field.
    pub content_digest: String,
    /// The `call_interfaces` field.
    pub call_interfaces: Vec<AgentDiscoveryCallInterface>,
}

/// gRPC response for agent discovery (Nacos `AgentDiscoveryResponse`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentDiscoveryResponse {
    #[serde(flatten)]
    /// The `response` field.
    pub response: Response,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `discovery_result` field.
    pub discovery_result: Option<AgentDiscoveryResult>,
}

impl AgentDiscoveryResponse {
    /// Creates a new instance.
    pub fn new() -> Self {
        Self {
            response: Response::new(),
            discovery_result: None,
        }
    }
}

batata_api::impl_response_trait!(AgentDiscoveryResponse);

impl From<AgentDiscoveryResponse> for Any {
    fn from(val: AgentDiscoveryResponse) -> Self {
        val.to_any()
    }
}

// --- Agent Endpoint Register / Deregister ---

/// Batch of endpoint registrations (Nacos `AgentEndpointRegistrationBatch`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentEndpointRegistrationBatch {
    /// The `namespace_id` field.
    pub namespace_id: String,
    /// The `agent_name` field.
    pub agent_name: String,
    /// The `runtime_version` field.
    pub runtime_version: String,
    /// The `version_range` field.
    pub version_range: String,
    /// The `protocol` field.
    pub protocol: String,
    /// The `endpoints` field.
    pub endpoints: Vec<AgentEndpointInfo>,
}

/// gRPC request to register agent endpoints (Nacos `AgentEndpointRegisterRpcRequest`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentEndpointRegisterRpcRequest {
    #[serde(flatten)]
    /// The `request` field.
    pub request: Request,
    /// The `module` field.
    pub module: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `registration_batch` field.
    pub registration_batch: Option<AgentEndpointRegistrationBatch>,
}

batata_api::impl_request_trait!(AgentEndpointRegisterRpcRequest, request);

impl From<&Payload> for AgentEndpointRegisterRpcRequest {
    fn from(value: &Payload) -> Self {
        AgentEndpointRegisterRpcRequest::from_payload(value)
    }
}

/// gRPC request to deregister agent endpoints (Nacos `AgentEndpointDeregisterRpcRequest`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentEndpointDeregisterRpcRequest {
    #[serde(flatten)]
    /// The `request` field.
    pub request: Request,
    /// The `module` field.
    pub module: String,
    /// The `namespace_id` field.
    pub namespace_id: String,
    /// The `agent_name` field.
    pub agent_name: String,
    /// The `protocol` field.
    pub protocol: String,
}

batata_api::impl_request_trait!(AgentEndpointDeregisterRpcRequest, request);

impl From<&Payload> for AgentEndpointDeregisterRpcRequest {
    fn from(value: &Payload) -> Self {
        AgentEndpointDeregisterRpcRequest::from_payload(value)
    }
}

/// gRPC response for endpoint register/deregister (Nacos `AgentEndpointOperationResponse`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentEndpointOperationResponse {
    #[serde(flatten)]
    /// The `response` field.
    pub response: Response,
}

impl AgentEndpointOperationResponse {
    /// Creates a new instance.
    pub fn new() -> Self {
        Self {
            response: Response::new(),
        }
    }
}

batata_api::impl_response_trait!(AgentEndpointOperationResponse);

impl From<AgentEndpointOperationResponse> for Any {
    fn from(val: AgentEndpointOperationResponse) -> Self {
        val.to_any()
    }
}

// =============================================================================
// AI-RAD v1: Publish / Subscribe (Watch) / Unsubscribe (Nacos 3.3.0 GA RAD)
// =============================================================================

/// Code-first agent definition publication body (GA `AgentPublishRequest`).
///
/// Wire-compatible: extends `AbstractAgentMetadata` (`agentName`, `displayName`,
/// `description`, `iconUrl`, `provider`, `tags`, `extensions`) and
/// `AbstractAgentDraftRequest` (`version`, `callInterfaces`, `author`,
/// `changeDescription`, `basedOnVersion`), plus `autoSubmit`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentPublishRequest {
    /// The `agent_name` field.
    pub agent_name: String,
    /// The `display_name` field.
    pub display_name: String,
    /// The `description` field.
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `icon_url` field.
    pub icon_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `provider` field.
    pub provider: Option<AgentProviderInfo>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    /// The `tags` field.
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `extensions` field.
    pub extensions: Option<HashMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `version` field.
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    /// The `call_interfaces` field.
    pub call_interfaces: Vec<AgentCallInterface>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `author` field.
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `change_description` field.
    pub change_description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `based_on_version` field.
    pub based_on_version: Option<String>,
    /// The `auto_submit` field.
    pub auto_submit: bool,
}

/// gRPC request for code-first agent definition publication (GA `AgentPublishRpcRequest`).
///
/// Wire-compatible: `module` = "ai".
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentPublishRpcRequest {
    #[serde(flatten)]
    /// The `request` field.
    pub request: Request,
    /// The `module` field.
    pub module: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `namespace_id` field.
    pub namespace_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `publish_request` field.
    pub publish_request: Option<AgentPublishRequest>,
}

batata_api::impl_request_trait!(AgentPublishRpcRequest, request);

impl From<&Payload> for AgentPublishRpcRequest {
    fn from(value: &Payload) -> Self {
        AgentPublishRpcRequest::from_payload(value)
    }
}

/// gRPC response for code-first agent definition publication (GA `AgentPublishRpcResponse`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentPublishRpcResponse {
    #[serde(flatten)]
    /// The `response` field.
    pub response: Response,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `version_detail` field.
    pub version_detail: Option<AgentVersionDetail>,
}

impl AgentPublishRpcResponse {
    /// Creates a new instance.
    pub fn new() -> Self {
        Self {
            response: Response::new(),
            version_detail: None,
        }
    }
}

batata_api::impl_response_trait!(AgentPublishRpcResponse);

impl From<AgentPublishRpcResponse> for Any {
    fn from(val: AgentPublishRpcResponse) -> Self {
        val.to_any()
    }
}

/// gRPC request to install one connection-owned RAD Watch (GA `AgentSubscribeRpcRequest`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentSubscribeRpcRequest {
    #[serde(flatten)]
    /// The `request` field.
    pub request: Request,
    /// The `module` field.
    pub module: String,
    /// The `client_watch_id` field.
    pub client_watch_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `discovery_request` field.
    pub discovery_request: Option<AgentDiscoveryRequest>,
    /// The `materialized_fingerprint` field.
    pub materialized_fingerprint: String,
}

batata_api::impl_request_trait!(AgentSubscribeRpcRequest, request);

impl From<&Payload> for AgentSubscribeRpcRequest {
    fn from(value: &Payload) -> Self {
        AgentSubscribeRpcRequest::from_payload(value)
    }
}

/// gRPC response installing one connection-owned RAD Watch (GA `AgentSubscribeRpcResponse`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSubscribeRpcResponse {
    #[serde(flatten)]
    /// The `response` field.
    pub response: Response,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `watch_key` field.
    pub watch_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `observed_fingerprint` field.
    pub observed_fingerprint: Option<String>,
    /// The `refresh_required` field.
    pub refresh_required: bool,
}

impl AgentSubscribeRpcResponse {
    /// Creates a new instance.
    pub fn new() -> Self {
        Self {
            response: Response::new(),
            watch_key: None,
            observed_fingerprint: None,
            refresh_required: false,
        }
    }
}

batata_api::impl_response_trait!(AgentSubscribeRpcResponse);

impl From<AgentSubscribeRpcResponse> for Any {
    fn from(val: AgentSubscribeRpcResponse) -> Self {
        val.to_any()
    }
}

/// gRPC request to remove one connection-owned RAD Watch (GA `AgentUnsubscribeRpcRequest`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentUnsubscribeRpcRequest {
    #[serde(flatten)]
    /// The `request` field.
    pub request: Request,
    /// The `module` field.
    pub module: String,
    /// The `watch_key` field.
    pub watch_key: String,
}

batata_api::impl_request_trait!(AgentUnsubscribeRpcRequest, request);

impl From<&Payload> for AgentUnsubscribeRpcRequest {
    fn from(value: &Payload) -> Self {
        AgentUnsubscribeRpcRequest::from_payload(value)
    }
}

/// gRPC response removing one connection-owned RAD Watch (GA `AgentUnsubscribeRpcResponse`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentUnsubscribeRpcResponse {
    #[serde(flatten)]
    /// The `response` field.
    pub response: Response,
}

impl AgentUnsubscribeRpcResponse {
    /// Creates a new instance.
    pub fn new() -> Self {
        Self {
            response: Response::new(),
        }
    }
}

batata_api::impl_response_trait!(AgentUnsubscribeRpcResponse);

impl From<AgentUnsubscribeRpcResponse> for Any {
    fn from(val: AgentUnsubscribeRpcResponse) -> Self {
        val.to_any()
    }
}

// =============================================================================
// AI-Prompt: QueryPrompt
// =============================================================================

/// Request to query a prompt by key, with optional version/label/MD5 filtering
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct QueryPromptRequest {
    #[serde(flatten)]
    /// The `request` field.
    pub request: Request,
    /// The `namespace_id` field.
    pub namespace_id: String,
    /// The `prompt_key` field.
    pub prompt_key: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    /// The `version` field.
    pub version: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    /// The `label` field.
    pub label: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    /// The `md5` field.
    pub md5: String,
}

batata_api::impl_request_trait!(QueryPromptRequest, request);

impl From<&Payload> for QueryPromptRequest {
    fn from(value: &Payload) -> Self {
        QueryPromptRequest::from_payload(value)
    }
}

/// Response for prompt query — contains the prompt info
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryPromptResponse {
    #[serde(flatten)]
    /// The `response` field.
    pub response: Response,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `prompt_info` field.
    pub prompt_info: Option<serde_json::Value>,
}

impl QueryPromptResponse {
    /// Creates a new instance.
    pub fn new() -> Self {
        Self {
            response: Response::new(),
            prompt_info: None,
        }
    }

    /// Builds the value with the given prompt.
    pub fn with_prompt(prompt: serde_json::Value) -> Self {
        Self {
            response: Response::new(),
            prompt_info: Some(prompt),
        }
    }
}

batata_api::impl_response_trait!(QueryPromptResponse);

impl From<QueryPromptResponse> for Any {
    fn from(val: QueryPromptResponse) -> Self {
        val.to_any()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rad_request_type_strings_survive_the_module_move() {
        // request_type() is stringify!($type) in the exported batata_api macros;
        // these strings go on the wire as the gRPC request type. Passing a
        // qualified path instead of a bare name would silently change them.
        assert_eq!(
            McpServerEndpointRequest::default().request_type(),
            "McpServerEndpointRequest"
        );
        assert_eq!(
            AgentPublishRpcRequest::default().request_type(),
            "AgentPublishRpcRequest"
        );
        assert_eq!(
            QueryPromptRequest::default().request_type(),
            "QueryPromptRequest"
        );
        assert_eq!(
            AgentSubscribeRpcResponse::default().response_type(),
            "AgentSubscribeRpcResponse"
        );
    }

    #[test]
    fn agent_publish_request_keeps_ga_camel_case_wire_shape() {
        let req = AgentPublishRequest {
            agent_name: "agent-a".to_string(),
            display_name: "Agent A".to_string(),
            provider: Some(AgentProviderInfo {
                name: "acme".to_string(),
                url: "https://acme.example".to_string(),
            }),
            call_interfaces: vec![AgentCallInterface {
                protocol: "JSONRPC".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"agentName\":\"agent-a\""), "{json}");
        assert!(json.contains("\"displayName\":\"Agent A\""), "{json}");
        assert!(json.contains("\"provider\":{\"name\":\"acme\""), "{json}");
        assert!(
            json.contains("\"callInterfaces\":[{\"protocol\":\"JSONRPC\""),
            "{json}"
        );
        assert!(!json.contains("agent_name"), "{json}");
        // Empty collections and absent optionals stay off the wire.
        assert!(!json.contains("\"tags\""), "{json}");
        assert!(!json.contains("\"extensions\""), "{json}");
        assert!(!json.contains("\"author\""), "{json}");

        let parsed: AgentPublishRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.agent_name, "agent-a");
        assert_eq!(parsed.call_interfaces.len(), 1);
        assert_eq!(parsed.provider.as_ref().unwrap().name, "acme");
    }

    #[test]
    fn agent_publish_rpc_response_round_trips_version_detail() {
        let mut detail = AgentVersionDetail::default();
        detail.name = "agent-a".to_string();
        detail.version = "1.0.0".to_string();
        let resp = AgentPublishRpcResponse {
            response: Response::default(),
            version_detail: Some(detail),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"versionDetail\""), "{json}");
        assert!(json.contains("\"version\":\"1.0.0\""), "{json}");
        let parsed: AgentPublishRpcResponse = serde_json::from_str(&json).unwrap();
        let detail = parsed.version_detail.unwrap();
        assert_eq!(detail.name, "agent-a");
        assert_eq!(detail.version, "1.0.0");
    }

    #[test]
    fn test_mcp_server_endpoint_request() {
        let req = McpServerEndpointRequest::default();
        assert_eq!(req.request_type(), "McpServerEndpointRequest");
    }

    #[test]
    fn test_query_mcp_server_request() {
        let req = QueryMcpServerRequest::default();
        assert_eq!(req.request_type(), "QueryMcpServerRequest");
    }

    #[test]
    fn test_release_mcp_server_request() {
        let req = ReleaseMcpServerRequest::default();
        assert_eq!(req.request_type(), "ReleaseMcpServerRequest");
    }

    #[test]
    fn test_agent_endpoint_request() {
        let req = AgentEndpointRequest::default();
        assert_eq!(req.request_type(), "AgentEndpointRequest");
    }

    #[test]
    fn test_query_agent_card_request() {
        let req = QueryAgentCardRequest::default();
        assert_eq!(req.request_type(), "QueryAgentCardRequest");
    }

    #[test]
    fn test_release_agent_card_request() {
        let req = ReleaseAgentCardRequest::default();
        assert_eq!(req.request_type(), "ReleaseAgentCardRequest");
    }

    #[test]
    fn test_agent_endpoint_serialization() {
        let ep = AgentEndpoint {
            address: "127.0.0.1".to_string(),
            port: 8080,
            version: "1.0".to_string(),
            transport: "http".to_string(),
            path: "/agent".to_string(),
            support_tls: true,
        };
        let json = serde_json::to_string(&ep).unwrap();
        let parsed: AgentEndpoint = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.address, "127.0.0.1");
        assert_eq!(parsed.port, 8080);
        assert!(parsed.support_tls);
    }
}
