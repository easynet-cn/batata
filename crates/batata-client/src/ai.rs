//! AI/MCP Client Service
//!
//! Provides client-side operations for MCP (Model Context Protocol) server
//! management and A2A (Agent-to-Agent) operations, compatible with Batata AiService.

use std::collections::HashMap;
use std::sync::Arc;

use dashmap::DashMap;
use serde::{Deserialize, Serialize};

use crate::http::BatataHttpClient;
use crate::model::{ApiResponse, Page};

// ============================================================================
// Data Models
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
// Listener Traits
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
// AI Service
// ============================================================================

/// AI/MCP client service for managing MCP servers and Agent cards.
///
/// Provides HTTP API client for console-level AI operations.
pub struct BatataAiService {
    http_client: Arc<BatataHttpClient>,
    /// Local cache: namespace_id:mcp_name -> McpServerDetailInfo
    mcp_cache: DashMap<String, McpServerDetailInfo>,
    /// Local cache: namespace_id:agent_name -> AgentCardInfo
    agent_cache: DashMap<String, AgentCardInfo>,
}

impl BatataAiService {
    /// Create a new AI/MCP service backed by the given HTTP client.
    pub fn new(http_client: Arc<BatataHttpClient>) -> Self {
        Self {
            http_client,
            mcp_cache: DashMap::new(),
            agent_cache: DashMap::new(),
        }
    }

    // ========================================================================
    // MCP Server Operations
    // ========================================================================

    /// Get MCP server details by name
    pub async fn get_mcp_server(
        &self,
        namespace_id: &str,
        mcp_name: &str,
    ) -> anyhow::Result<McpServerDetailInfo> {
        let path = format!(
            "/v3/console/ai/mcp?namespaceId={}&mcpName={}",
            namespace_id, mcp_name
        );
        let resp: ApiResponse<McpServerDetailInfo> = self.http_client.get(&path).await?;

        if resp.code == 0 {
            let key = format!("{}:{}", namespace_id, mcp_name);
            self.mcp_cache.insert(key, resp.data.clone());
            Ok(resp.data)
        } else {
            Err(anyhow::anyhow!(
                "Failed to get MCP server: {}",
                resp.message
            ))
        }
    }

    /// List MCP servers
    pub async fn list_mcp_servers(
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

        if resp.code == 0 {
            Ok(resp.data)
        } else {
            Err(anyhow::anyhow!("List MCP servers failed: {}", resp.message))
        }
    }

    /// Get cached MCP server info
    pub fn get_cached_mcp_server(
        &self,
        namespace_id: &str,
        mcp_name: &str,
    ) -> Option<McpServerDetailInfo> {
        let key = format!("{}:{}", namespace_id, mcp_name);
        self.mcp_cache.get(&key).map(|v| v.clone())
    }

    // ========================================================================
    // A2A Agent Card Operations
    // ========================================================================

    /// Get Agent Card details by name
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

        if resp.code == 0 {
            let key = format!("{}:{}", namespace_id, agent_name);
            self.agent_cache.insert(key, resp.data.clone());
            Ok(resp.data)
        } else {
            Err(anyhow::anyhow!(
                "Failed to get agent card: {}",
                resp.message
            ))
        }
    }

    /// List Agent Cards
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

        if resp.code == 0 {
            Ok(resp.data)
        } else {
            Err(anyhow::anyhow!("List agent cards failed: {}", resp.message))
        }
    }

    /// Get cached Agent Card info
    pub fn get_cached_agent_card(
        &self,
        namespace_id: &str,
        agent_name: &str,
    ) -> Option<AgentCardInfo> {
        let key = format!("{}:{}", namespace_id, agent_name);
        self.agent_cache.get(&key).map(|v| v.clone())
    }

    // ========================================================================
    // Cache Management
    // ========================================================================

    /// Clear all caches
    pub fn clear_cache(&self) {
        self.mcp_cache.clear();
        self.agent_cache.clear();
    }

    /// Get MCP cache size
    pub fn mcp_cache_size(&self) -> usize {
        self.mcp_cache.len()
    }

    /// Get agent cache size
    pub fn agent_cache_size(&self) -> usize {
        self.agent_cache.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
