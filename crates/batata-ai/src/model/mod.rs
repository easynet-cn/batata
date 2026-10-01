//! AI Capabilities Data Models
//!
//! Data models for MCP (Model Content Protocol) server registration,
//! A2A (Agent-to-Agent) communication, Prompt management, and Skills.
//!
//! All model types are defined in `batata-common` and re-exported here
//! for backward compatibility.

// Re-export all AI model types from batata-common
pub use batata_common::model::ai::*;

// Re-export submodules so `crate::model::skill`, `crate::model::agentspec`, etc. still work
pub use batata_common::model::ai::a2a;
pub use batata_common::model::ai::agentspec;
pub use batata_common::model::ai::mcp;
pub use batata_common::model::ai::pipeline;
pub use batata_common::model::ai::prompt;
pub use batata_common::model::ai::skill;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

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
            capabilities: vec![McpCapability::Tool],
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
            url: "http://localhost:8080".to_string(),
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
            default_input_modes: vec!["text".to_string(), "image".to_string()],
            default_output_modes: vec!["text".to_string(), "json".to_string()],
            preferred_transport: None,
            provider: None,
            documentation_url: None,
            icon_url: None,
            supports_authenticated_extended_card: None,
            metadata: HashMap::new(),
            tags: vec!["test".to_string()],
            ..Default::default()
        };

        let json = serde_json::to_string(&card).unwrap();
        assert!(json.contains("test-agent"));
        assert!(json.contains("coding"));
        assert!(json.contains("\"url\":\"http://localhost:8080\""));
    }

    /// The A2A interface and security fields must survive a round trip, and
    /// must stay off the wire when empty so existing clients see no change.
    #[test]
    fn test_agent_card_interface_and_security_round_trip() {
        let mut schemes = HashMap::new();
        schemes.insert(
            "apiKey".to_string(),
            serde_json::json!({"type": "apiKey", "name": "x-api-key"}),
        );
        let mut requirement = HashMap::new();
        requirement.insert("apiKey".to_string(), vec![]);

        let card = AgentCard {
            additional_interfaces: vec![AgentInterface {
                url: "https://agent.example.com/a2a".to_string(),
                transport: "HTTP".to_string(),
                protocol_binding: Some("JSONRPC".to_string()),
                protocol_version: Some("1.0".to_string()),
                tenant: None,
            }],
            supported_interfaces: vec![AgentInterface {
                url: "https://agent.example.com/grpc".to_string(),
                transport: "GRPC".to_string(),
                ..Default::default()
            }],
            security_schemes: schemes,
            security: vec![requirement.clone()],
            security_requirements: vec![requirement],
            ..Default::default()
        };

        let json = serde_json::to_string(&card).expect("serialize");
        assert!(json.contains("\"additionalInterfaces\""), "{json}");
        assert!(json.contains("\"supportedInterfaces\""), "{json}");
        assert!(json.contains("\"securitySchemes\""), "{json}");
        assert!(json.contains("\"securityRequirements\""), "{json}");
        // camelCase inside the interface object too.
        assert!(json.contains("\"protocolBinding\""), "{json}");

        let back: AgentCard = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.additional_interfaces.len(), 1);
        assert_eq!(back.additional_interfaces[0].transport, "HTTP");
        assert_eq!(
            back.additional_interfaces[0].protocol_binding.as_deref(),
            Some("JSONRPC")
        );
        assert_eq!(back.supported_interfaces.len(), 1);
        assert_eq!(back.security_schemes.len(), 1);
        assert_eq!(back.security.len(), 1);
        assert_eq!(back.security_requirements.len(), 1);
    }

    /// Empty interface and security sections are omitted, not emitted as `[]`.
    #[test]
    fn test_agent_card_omits_empty_interface_and_security() {
        let json = serde_json::to_string(&AgentCard::default()).expect("serialize");
        for key in [
            "additionalInterfaces",
            "supportedInterfaces",
            "securitySchemes",
            "securityRequirements",
            "signatures",
        ] {
            assert!(!json.contains(key), "{key} must be omitted: {json}");
        }
    }

    #[test]
    fn test_mcp_server_config_parsing() {
        let json = r#"{
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
        }"#;

        // The deprecated `McpServerImportRequest` wrapper is gone; the
        // claude_desktop_config.json payload is parsed directly into a map.
        let servers: std::collections::HashMap<String, McpServerConfig> =
            serde_json::from_str(json).unwrap();

        assert!(servers.contains_key("filesystem"));
        assert!(servers.contains_key("github"));
        assert_eq!(servers["filesystem"].command.as_deref(), Some("npx"));
    }
}
