// AI module model types (MCP and A2A/Agent)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============== MCP Models ==============

/// MCP server basic information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct McpServerBasicInfo {
    pub id: String,
    pub name: String,
    pub protocol: String,
    pub front_protocol: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub packages: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_detail: Option<ServerVersionDetail>,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_server_config: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_server_config: Option<HashMap<String, serde_json::Value>>,
    pub enabled: bool,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<Vec<serde_json::Value>>,
}

/// MCP server detailed information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct McpServerDetailInfo {
    #[serde(flatten)]
    pub basic_info: McpServerBasicInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend_endpoints: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frontend_endpoints: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_spec: Option<McpToolSpecification>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub all_versions: Option<Vec<ServerVersionDetail>>,
    pub namespace_id: String,
}

/// MCP tool specification
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct McpToolSpecification {
    pub specification_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encrypt_data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools_meta: Option<HashMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_schemes: Option<Vec<serde_json::Value>>,
}

/// MCP endpoint specification
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct McpEndpointSpec {
    pub r#type: String,
    pub data: HashMap<String, String>,
}

/// Server version detail
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ServerVersionDetail {
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    pub is_latest: bool,
}

// ============== A2A/Agent Models ==============

/// Agent card basic information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentCardBasicInfo {
    pub protocol_version: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub icon_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skills: Option<Vec<serde_json::Value>>,
}

/// Full agent card information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentCard {
    #[serde(flatten)]
    pub basic_info: AgentCardBasicInfo,
    pub url: String,
    pub preferred_transport: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_interfaces: Option<Vec<AgentInterface>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<serde_json::Value>,
    pub documentation_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_schemes: Option<HashMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security: Option<Vec<HashMap<String, Vec<String>>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_input_modes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_output_modes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_authenticated_extended_card: Option<bool>,
}

/// Agent card detail info (extends AgentCard with additional fields)
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentCardDetailInfo {
    #[serde(flatten)]
    pub agent_card: AgentCard,
    pub registration_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_version: Option<bool>,
}

/// Agent card version info for list operations
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentCardVersionInfo {
    #[serde(flatten)]
    pub basic_info: AgentCardBasicInfo,
    pub latest_published_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_details: Option<Vec<AgentVersionDetail>>,
    pub registration_type: String,
}

/// Agent version detail
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentVersionDetail {
    pub version: String,
    pub created_at: String,
    pub updated_at: String,
    pub is_latest: bool,
}

/// Agent interface information
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentInterface {
    pub url: String,
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
