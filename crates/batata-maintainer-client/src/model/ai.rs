//! AI module model types (MCP and A2A/Agent)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============== MCP Models ==============

/// One MCP server version as returned by the ADMIN version endpoints.
///
/// Upstream answers with `McpServerVersionDetail`, whose nested specification
/// sections are optional and vary with the version state. The shared fields are
/// typed here; every other wire field is preserved in `extra` so a caller can
/// still reach `serverSpecification`, `toolSpecification`, labels and so on
/// without this crate having to guess the whole upstream DTO.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct McpServerVersionInfo {
    /// Version string.
    pub version: String,
    /// Lifecycle status: `draft`, `reviewing`, `reviewed`, `online` or `offline`.
    pub status: String,
    /// Whether this is the latest published version.
    pub latest: Option<bool>,
    /// Namespace ID the version belongs to.
    pub namespace_id: String,
    /// MCP server name.
    pub mcp_name: String,
    /// Any additional fields returned by the server.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// One protocol-neutral agent version from the ADMIN `/agents` endpoints.
///
/// Nacos 3.3.0 answers with `AgentVersionSummary` / `AgentVersionDetail` while
/// batata's admin layer answers with its own `VersionDetail`. The shared
/// fields are typed here; every other wire field is preserved in `extra` so a
/// caller can still reach labels, specifications and timestamps without this
/// crate having to guess the whole upstream DTO.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentVersionInfo {
    /// Version string.
    pub version: String,
    /// Lifecycle status, e.g. `draft`, `reviewing`, `online` or `offline`.
    pub status: String,
    /// Whether this is the latest published version.
    pub latest: Option<bool>,
    /// Namespace ID the version belongs to.
    pub namespace_id: String,
    /// Agent name.
    pub agent_name: String,
    /// Any additional fields returned by the server.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// MCP server basic information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct McpServerBasicInfo {
    /// The `id` field.
    pub id: String,
    /// The `name` field.
    pub name: String,
    /// The `protocol` field.
    pub protocol: String,
    /// The `front_protocol` field.
    pub front_protocol: String,
    /// The `description` field.
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `repository` field.
    pub repository: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `packages` field.
    pub packages: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `version_detail` field.
    pub version_detail: Option<ServerVersionDetail>,
    /// The `version` field.
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `remote_server_config` field.
    pub remote_server_config: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `local_server_config` field.
    pub local_server_config: Option<HashMap<String, serde_json::Value>>,
    /// The `enabled` field.
    pub enabled: bool,
    /// The `status` field.
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `capabilities` field.
    pub capabilities: Option<Vec<serde_json::Value>>,
}

/// MCP server detailed information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct McpServerDetailInfo {
    #[serde(flatten)]
    /// The `basic_info` field.
    pub basic_info: McpServerBasicInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `backend_endpoints` field.
    pub backend_endpoints: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `frontend_endpoints` field.
    pub frontend_endpoints: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `tool_spec` field.
    pub tool_spec: Option<McpToolSpecification>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `all_versions` field.
    pub all_versions: Option<Vec<ServerVersionDetail>>,
    /// The `namespace_id` field.
    pub namespace_id: String,
}

/// MCP tool specification
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct McpToolSpecification {
    /// The `specification_type` field.
    pub specification_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `encrypt_data` field.
    pub encrypt_data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `tools` field.
    pub tools: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `tools_meta` field.
    pub tools_meta: Option<HashMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `security_schemes` field.
    pub security_schemes: Option<Vec<serde_json::Value>>,
}

/// MCP endpoint specification
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct McpEndpointSpec {
    /// The `type` field.
    pub r#type: String,
    /// The `data` field.
    pub data: HashMap<String, String>,
}

/// Server version detail
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ServerVersionDetail {
    /// The `version` field.
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `created_at` field.
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `updated_at` field.
    pub updated_at: Option<String>,
    /// The `is_latest` field.
    pub is_latest: bool,
}

// ============== A2A/Agent Models ==============

/// Agent card basic information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentCardBasicInfo {
    /// The `protocol_version` field.
    pub protocol_version: String,
    /// The `name` field.
    pub name: String,
    /// The `description` field.
    pub description: String,
    /// The `version` field.
    pub version: String,
    /// The `icon_url` field.
    pub icon_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `capabilities` field.
    pub capabilities: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `skills` field.
    pub skills: Option<Vec<serde_json::Value>>,
}

/// Full agent card information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentCard {
    #[serde(flatten)]
    /// The `basic_info` field.
    pub basic_info: AgentCardBasicInfo,
    /// The `url` field.
    pub url: String,
    /// The `preferred_transport` field.
    pub preferred_transport: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `additional_interfaces` field.
    pub additional_interfaces: Option<Vec<AgentInterface>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `provider` field.
    pub provider: Option<serde_json::Value>,
    /// The `documentation_url` field.
    pub documentation_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `security_schemes` field.
    pub security_schemes: Option<HashMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `security` field.
    pub security: Option<Vec<HashMap<String, Vec<String>>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `default_input_modes` field.
    pub default_input_modes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `default_output_modes` field.
    pub default_output_modes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `supports_authenticated_extended_card` field.
    pub supports_authenticated_extended_card: Option<bool>,
}

/// Agent card detail info (extends AgentCard with additional fields)
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentCardDetailInfo {
    #[serde(flatten)]
    /// The `agent_card` field.
    pub agent_card: AgentCard,
    /// The `registration_type` field.
    pub registration_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `latest_version` field.
    pub latest_version: Option<bool>,
}

/// Agent card version info for list operations
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentCardVersionInfo {
    #[serde(flatten)]
    /// The `basic_info` field.
    pub basic_info: AgentCardBasicInfo,
    /// The `latest_published_version` field.
    pub latest_published_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `version_details` field.
    pub version_details: Option<Vec<AgentVersionDetail>>,
    /// The `registration_type` field.
    pub registration_type: String,
}

/// Agent version detail
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentVersionDetail {
    /// The `version` field.
    pub version: String,
    /// The `created_at` field.
    pub created_at: String,
    /// The `updated_at` field.
    pub updated_at: String,
    /// The `is_latest` field.
    pub is_latest: bool,
}

/// Agent interface information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentInterface {
    /// The `url` field.
    pub url: String,
    /// The `transport` field.
    pub transport: String,
}

// ============== Protocol-neutral Agent write forms (GA 3.3.0) ==============
//
// `AgentMaintainerServiceImpl` sends every POST/PUT through
// `ClientHttpProxy.postForm/putForm`: identity fields live in the urlencoded
// form body, and nested objects (`provider`, `tags`, `extensions`,
// `callInterfaces`, `labels`) are collapsed into a SINGLE JSON string form
// value via `putJsonIfNotNull`. These DTOs model that exact field table.
//
// They intentionally do NOT carry a `#[serde(flatten)] extra` map: a request
// struct is only ever serialized by `serde_urlencoded`, which cannot serialize
// a flatten map. Every response DTO below does carry the tolerant `extra`.

/// `POST /v3/admin/ai/agents/draft` form (GA `createDraft`).
///
/// `namespaceId`, `agentName` and `version` are always sent. The nested
/// `*`-object fields are JSON strings; the remaining optional fields are
/// omitted from the form when `None` (`putIfNotNull`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentDraftCreateForm {
    /// Namespace ID.
    pub namespace_id: String,
    /// Agent name.
    pub agent_name: String,
    /// Draft version string.
    pub version: String,
    /// Human-readable display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Icon URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    /// Provider object, serialized as a single JSON string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// Tags, serialized as a single JSON string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<String>,
    /// Extensions, serialized as a single JSON string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<String>,
    /// Call interfaces, serialized as a single JSON string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call_interfaces: Option<String>,
    /// Author.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Change description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_description: Option<String>,
    /// Version this draft is based on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub based_on_version: Option<String>,
}

/// `PUT /v3/admin/ai/agents/draft` form (GA `updateDraft`).
///
/// GA sends only identity (`namespaceId`, `agentName`, `version`) plus
/// `callInterfaces` (JSON string) and `changeDescription`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentDraftUpdateForm {
    /// Namespace ID.
    pub namespace_id: String,
    /// Agent name.
    pub agent_name: String,
    /// Draft version string.
    pub version: String,
    /// Call interfaces, serialized as a single JSON string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call_interfaces: Option<String>,
    /// Change description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_description: Option<String>,
}

/// `PUT /v3/admin/ai/agents/labels` form (GA `updateLabels`).
///
/// `labels` is the JSON string of the labels map (`putJsonIfNotNull`), not a
/// set of repeated `labels.*` form fields.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentLabelsUpdateForm {
    /// Namespace ID.
    pub namespace_id: String,
    /// Agent name.
    pub agent_name: String,
    /// Labels map, serialized as a single JSON string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<String>,
}

/// `PUT /v3/admin/ai/agents` form (GA `updateAgent`).
///
/// Note there is NO `version` field here: GA mutates the agent's current
/// metadata, not one version. Nested objects are single JSON string values.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentUpdateForm {
    /// Namespace ID.
    pub namespace_id: String,
    /// Agent name.
    pub agent_name: String,
    /// Human-readable display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Icon URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    /// Provider object, serialized as a single JSON string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// Tags, serialized as a single JSON string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<String>,
    /// Extensions, serialized as a single JSON string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<String>,
    /// Lifecycle status.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

// ============== Protocol-neutral Agent response DTOs (tolerant) ==============
//
// GA answers `updateAgent`/`updateLabels` with `AgentSummary` and
// `getRuntimeEndpoints` with `RuntimeEndpointSnapshot`. batata's admin layer
// may answer with a different shape (tracked as `F-NAC-AISRV-001`), so these
// DTOs use `default` on every field and preserve all unknown wire fields in
// `extra` rather than failing to decode.

/// Tolerant `AgentSummary` as returned by GA `updateAgent`/`updateLabels`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentSummary {
    /// Namespace ID.
    pub namespace_id: String,
    /// Agent name.
    pub agent_name: String,
    /// Human-readable display name.
    pub display_name: String,
    /// Description.
    pub description: String,
    /// Lifecycle status.
    pub status: String,
    /// Owner.
    pub owner: String,
    /// Visibility scope.
    pub scope: String,
    /// Nested version info, kept raw because its shape varies.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_info: Option<serde_json::Value>,
    /// Metadata version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta_version: Option<i64>,
    /// Creation time (epoch millis).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_time: Option<i64>,
    /// Last update time (epoch millis).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_time: Option<i64>,
    /// Any additional fields returned by the server.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Tolerant `RuntimeEndpointSnapshot` as returned by GA `getRuntimeEndpoints`.
///
/// Nacos 3.3.0 GA answers the **flat** snapshot under `Result.data`; older
/// batata builds answered with a `ConsoleRuntimeEndpointView` wrapper. Every
/// field defaults and unknown keys are retained, so both shapes decode.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RuntimeEndpointSnapshot {
    /// Namespace ID.
    pub namespace_id: String,
    /// Agent name.
    pub agent_name: String,
    /// Version the snapshot refers to (absent when no version filter was sent).
    pub version: String,
    /// One protocol's call interface with a single RUNTIME endpoint set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call_interface: Option<RuntimeCallInterface>,
    /// Any additional fields returned by the server.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// GA runtime `callInterface` (`AgentCallInterface` with only the runtime
/// fields populated).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RuntimeCallInterface {
    /// Protocol token, echoed from the request.
    pub protocol: String,
    /// Exactly one `RUNTIME` set on the admin endpoint.
    pub endpoint_sets: Vec<RuntimeEndpointSet>,
    /// Unknown fields (including batata's legacy flat `endpoints`).
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// One source's endpoint set.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RuntimeEndpointSet {
    /// `RUNTIME` or `DECLARED`.
    pub source: String,
    /// Optional source revision marker.
    pub source_revision: Option<String>,
    /// Nacos observation time (epoch millis).
    pub last_updated_time: Option<i64>,
    /// Live endpoints.
    pub endpoints: Vec<RuntimeEndpoint>,
}

/// One reachable endpoint (GA `Endpoint`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RuntimeEndpoint {
    /// Endpoint URI.
    pub uri: String,
    /// Transport, e.g. `HTTP+JSON`.
    pub transport: String,
    /// Priority; lower values win.
    pub priority: i32,
    /// Weight among same-priority endpoints (GA default 1.0).
    #[serde(default = "default_runtime_endpoint_weight")]
    pub weight: f64,
    /// Public endpoint metadata.
    pub metadata: HashMap<String, String>,
    /// Registration health.
    pub healthy: Option<bool>,
    /// Registration enablement.
    pub enabled: Option<bool>,
    /// Runtime version bindings.
    pub bindings: Vec<RuntimeVersionBinding>,
}

fn default_runtime_endpoint_weight() -> f64 {
    1.0
}

/// One runtime implementation version and the range it serves.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RuntimeVersionBinding {
    /// Runtime implementation version.
    pub runtime_version: String,
    /// Agent version range the binding can serve.
    pub version_range: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_server_basic_info_serialization() {
        let info = McpServerBasicInfo {
            id: "mcp-1".to_string(),
            name: "test-mcp".to_string(),
            protocol: "sse".to_string(),
            enabled: true,
            status: "active".to_string(),
            ..Default::default()
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"name\":\"test-mcp\""));
        assert!(json.contains("\"protocol\":\"sse\""));

        let deserialized: McpServerBasicInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "test-mcp");
    }

    #[test]
    fn test_agent_card_serialization() {
        let card = AgentCard {
            basic_info: AgentCardBasicInfo {
                name: "test-agent".to_string(),
                protocol_version: "0.2.0".to_string(),
                version: "1.0.0".to_string(),
                ..Default::default()
            },
            url: "https://example.com/agent".to_string(),
            preferred_transport: "http".to_string(),
            ..Default::default()
        };

        let json = serde_json::to_string(&card).unwrap();
        assert!(json.contains("\"name\":\"test-agent\""));
        assert!(json.contains("\"url\":\"https://example.com/agent\""));

        let deserialized: AgentCard = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.basic_info.name, "test-agent");
    }

    #[test]
    fn test_agent_version_detail_serialization() {
        let detail = AgentVersionDetail {
            version: "1.0.0".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-02T00:00:00Z".to_string(),
            is_latest: true,
        };

        let json = serde_json::to_string(&detail).unwrap();
        assert!(json.contains("\"version\":\"1.0.0\""));
        assert!(json.contains("\"isLatest\":true"));

        let deserialized: AgentVersionDetail = serde_json::from_str(&json).unwrap();
        assert!(deserialized.is_latest);
    }

    #[test]
    fn test_mcp_endpoint_spec_serialization() {
        let mut data = HashMap::new();
        data.insert("address".to_string(), "192.168.1.1".to_string());
        data.insert("port".to_string(), "8080".to_string());

        let spec = McpEndpointSpec {
            r#type: "direct".to_string(),
            data,
        };

        let json = serde_json::to_string(&spec).unwrap();
        assert!(json.contains("\"type\":\"direct\""));

        let deserialized: McpEndpointSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.r#type, "direct");
    }

    /// The GA `createDraft` form must emit the exact 13-field camelCase table
    /// through `serde_urlencoded` — no `snake_case` leaks, no extra keys.
    #[test]
    fn test_agent_draft_create_form_urlencoded_field_table() {
        let form = AgentDraftCreateForm {
            namespace_id: "ns-1".to_string(),
            agent_name: "my-agent".to_string(),
            version: "1.0".to_string(),
            display_name: Some("My Agent".to_string()),
            description: Some("desc".to_string()),
            icon_url: Some("http://icon".to_string()),
            provider: Some("{\"org\":\"acme\"}".to_string()),
            tags: Some("[\"chat\"]".to_string()),
            extensions: Some("{\"x\":1}".to_string()),
            call_interfaces: Some("[]".to_string()),
            author: Some("alice".to_string()),
            change_description: Some("initial".to_string()),
            based_on_version: Some("0.9".to_string()),
        };
        let encoded = serde_urlencoded::to_string(&form).unwrap();
        let parsed: HashMap<String, String> = serde_urlencoded::from_str(&encoded).unwrap();
        let mut keys: Vec<&str> = parsed.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec![
                "agentName",
                "author",
                "basedOnVersion",
                "callInterfaces",
                "changeDescription",
                "description",
                "displayName",
                "extensions",
                "iconUrl",
                "namespaceId",
                "provider",
                "tags",
                "version",
            ]
        );
        assert_eq!(parsed["namespaceId"], "ns-1");
        // JSON strings survive the urlencoded round-trip unchanged.
        assert_eq!(parsed["provider"], "{\"org\":\"acme\"}");
    }

    /// `skip_serializing_if` must drop every `None`, leaving only identity.
    #[test]
    fn test_agent_draft_create_form_omits_none_fields() {
        let form = AgentDraftCreateForm {
            namespace_id: "ns-1".to_string(),
            agent_name: "my-agent".to_string(),
            version: "1.0".to_string(),
            ..Default::default()
        };
        let encoded = serde_urlencoded::to_string(&form).unwrap();
        let parsed: HashMap<String, String> = serde_urlencoded::from_str(&encoded).unwrap();
        assert_eq!(parsed.len(), 3);
        assert!(parsed.contains_key("namespaceId"));
        assert!(parsed.contains_key("agentName"));
        assert!(parsed.contains_key("version"));
    }

    /// `updateDraft` form has its own exact 5-field table.
    #[test]
    fn test_agent_draft_update_form_field_table() {
        let form = AgentDraftUpdateForm {
            namespace_id: "ns-1".to_string(),
            agent_name: "my-agent".to_string(),
            version: "1.0".to_string(),
            call_interfaces: Some("[]".to_string()),
            change_description: Some("edit".to_string()),
        };
        let encoded = serde_urlencoded::to_string(&form).unwrap();
        let parsed: HashMap<String, String> = serde_urlencoded::from_str(&encoded).unwrap();
        assert_eq!(parsed.len(), 5);
        assert_eq!(parsed["changeDescription"], "edit");
        assert_eq!(parsed["callInterfaces"], "[]");
    }

    /// `updateAgent` form must NOT contain a `version` key.
    #[test]
    fn test_agent_update_form_has_no_version() {
        let form = AgentUpdateForm {
            namespace_id: "ns-1".to_string(),
            agent_name: "my-agent".to_string(),
            display_name: Some("My Agent".to_string()),
            description: Some("desc".to_string()),
            icon_url: Some("http://icon".to_string()),
            provider: Some("{}".to_string()),
            tags: Some("[]".to_string()),
            extensions: Some("{}".to_string()),
            status: Some("online".to_string()),
        };
        let encoded = serde_urlencoded::to_string(&form).unwrap();
        let parsed: HashMap<String, String> = serde_urlencoded::from_str(&encoded).unwrap();
        assert_eq!(parsed.len(), 9);
        assert!(!parsed.contains_key("version"));
        assert_eq!(parsed["status"], "online");
    }

    /// `labels` is a SINGLE JSON-string form value, not repeated keys.
    #[test]
    fn test_agent_labels_update_form_single_json_value() {
        let form = AgentLabelsUpdateForm {
            namespace_id: "ns-1".to_string(),
            agent_name: "my-agent".to_string(),
            labels: Some("{\"stable\":\"1.0\"}".to_string()),
        };
        let encoded = serde_urlencoded::to_string(&form).unwrap();
        let parsed: HashMap<String, String> = serde_urlencoded::from_str(&encoded).unwrap();
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed["labels"], "{\"stable\":\"1.0\"}");
    }

    /// Response DTOs decode GA-shaped payloads and keep unknown fields in
    /// `extra` (the batata runtime-endpoints shape is known to differ).
    #[test]
    fn test_agent_summary_tolerant_deserialization() {
        let json = r#"{
            "namespaceId": "ns-1",
            "agentName": "my-agent",
            "status": "online",
            "versionInfo": {"editingVersion": "1.0"},
            "serverOnlyField": 42
        }"#;
        let summary: AgentSummary = serde_json::from_str(json).unwrap();
        assert_eq!(summary.agent_name, "my-agent");
        assert_eq!(summary.status, "online");
        assert!(summary.version_info.is_some());
        assert_eq!(summary.display_name, ""); // defaulted, not required
        assert!(summary.extra.contains_key("serverOnlyField"));
    }

    #[test]
    fn test_runtime_endpoint_snapshot_tolerant_deserialization() {
        let json = r#"{
            "namespaceId": "ns-1",
            "agentName": "my-agent",
            "version": "1.0",
            "callInterface": {"transport": "JSONRPC"},
            "batataSpecific": {"a": 1}
        }"#;
        let snapshot: RuntimeEndpointSnapshot = serde_json::from_str(json).unwrap();
        assert_eq!(snapshot.version, "1.0");
        assert!(snapshot.call_interface.is_some());
        assert!(snapshot.extra.contains_key("batataSpecific"));
    }

    #[test]
    fn test_runtime_endpoint_snapshot_decodes_ga_endpoint_sets() {
        let json = r#"{
            "namespaceId": "public",
            "agentName": "demo",
            "version": "1.0.0",
            "callInterface": {
                "protocol": "a2a",
                "endpointSets": [{
                    "source": "RUNTIME",
                    "lastUpdatedTime": 1763000000000,
                    "endpoints": [{
                        "uri": "http://127.0.0.1:8080/a2a",
                        "transport": "HTTP+JSON",
                        "weight": 1.0,
                        "healthy": true,
                        "enabled": true,
                        "bindings": [{
                            "runtimeVersion": "1.0.6",
                            "versionRange": "[1.0.0,2.0.0)"
                        }]
                    }]
                }]
            }
        }"#;
        let snapshot: RuntimeEndpointSnapshot = serde_json::from_str(json).unwrap();
        assert_eq!(snapshot.namespace_id, "public");
        assert!(
            snapshot.extra.is_empty(),
            "flat GA data has no wrapper keys"
        );
        let call = snapshot.call_interface.unwrap();
        assert_eq!(call.protocol, "a2a");
        assert_eq!(call.endpoint_sets.len(), 1);
        let set = &call.endpoint_sets[0];
        assert_eq!(set.source, "RUNTIME");
        assert_eq!(set.last_updated_time, Some(1763000000000));
        assert_eq!(set.endpoints.len(), 1);
        let endpoint = &set.endpoints[0];
        assert_eq!(endpoint.uri, "http://127.0.0.1:8080/a2a");
        assert_eq!(endpoint.transport, "HTTP+JSON");
        assert_eq!(endpoint.weight, 1.0);
        assert_eq!(endpoint.healthy, Some(true));
        assert_eq!(endpoint.enabled, Some(true));
        assert_eq!(endpoint.bindings[0].runtime_version, "1.0.6");
        assert_eq!(endpoint.bindings[0].version_range, "[1.0.0,2.0.0)");
    }
}
