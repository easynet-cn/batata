//! AI module model types (MCP and A2A/Agent)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============== MCP Models ==============

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
}
