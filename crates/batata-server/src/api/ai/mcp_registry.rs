//! MCP Registry API (Official MCP Registry OpenAPI)
//!
//! This module implements the official MCP Registry OpenAPI spec
//! (`GET /v0/servers`, `GET /v0/servers/{id}`), exposing registered MCP servers
//! in the standardized format from `https://static.modelcontextprotocol.io/`.
//!
//! This is served on a separate port (default 9080) and enabled via
//! `nacos.ai.mcp.registry.enabled` or the `serverWithMcp` deployment type.

use std::sync::Arc;

use actix_web::{HttpResponse, get, web};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::mcp::McpServerRegistry;
use super::model::{McpServer, McpServerType};

// =============================================================================
// MCP Registry OpenAPI Response Models
// =============================================================================

const MCP_SERVER_SCHEMA: &str =
    "https://static.modelcontextprotocol.io/schemas/2025-07-09/server.schema.json";

/// Top-level response for `GET /v0/servers`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpRegistryServerList {
    pub servers: Vec<McpRegistryServerDetail>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<ListMetadata>,
}

/// Pagination metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
}

/// Detail for a single MCP server (matches official schema)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpRegistryServerDetail {
    #[serde(rename = "$schema", skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<Repository>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub packages: Option<Vec<Package>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remotes: Option<Vec<Remote>>,
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<OfficialMeta>,
}

/// Remote connection info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Remote {
    #[serde(rename = "type")]
    pub remote_type: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<std::collections::HashMap<String, String>>,
}

/// Repository info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

/// Package info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registry_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identifier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// Official metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfficialMeta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_latest: Option<bool>,
}

/// Error response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpErrorResponse {
    pub error: String,
}

// =============================================================================
// Query Parameters
// =============================================================================

/// Query parameters for `GET /v0/servers`
#[derive(Debug, Clone, Deserialize)]
pub struct ListServersQuery {
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}

/// Query parameters for `GET /v0/servers/{id}`
#[derive(Debug, Clone, Deserialize)]
pub struct GetServerQuery {
    pub version: Option<String>,
}

// =============================================================================
// Conversion
// =============================================================================

fn millis_to_rfc3339(millis: i64) -> Option<String> {
    DateTime::<Utc>::from_timestamp_millis(millis).map(|dt| dt.to_rfc3339())
}

fn mcp_server_type_to_remote_type(server_type: McpServerType) -> &'static str {
    match server_type {
        McpServerType::Http => "http",
        McpServerType::Sse => "sse",
        McpServerType::Stdio => "stdio",
        McpServerType::WebSocket => "http",
    }
}

fn convert_to_registry_detail(server: &McpServer) -> McpRegistryServerDetail {
    let remote_type = mcp_server_type_to_remote_type(server.server_type);

    let remotes = vec![Remote {
        remote_type: remote_type.to_string(),
        url: server.endpoint.clone(),
        headers: if server.transport.headers.is_empty() {
            None
        } else {
            Some(server.transport.headers.clone())
        },
    }];

    McpRegistryServerDetail {
        schema: Some(MCP_SERVER_SCHEMA.to_string()),
        name: server.name.clone(),
        description: if server.description.is_empty() {
            None
        } else {
            Some(server.description.clone())
        },
        status: Some("active".to_string()),
        version: Some(server.version.clone()),
        repository: None,
        packages: None,
        remotes: Some(remotes),
        meta: Some(OfficialMeta {
            created_at: millis_to_rfc3339(server.registered_at),
            updated_at: millis_to_rfc3339(server.updated_at),
            server_id: Some(server.id.clone()),
            version_id: Some(format!("{}:{}", server.id, server.version)),
            is_latest: Some(true),
        }),
    }
}

// =============================================================================
// Handlers
// =============================================================================

/// List all registered MCP servers
///
/// `GET /v0/servers`
#[get("/v0/servers")]
async fn list_servers(
    query: web::Query<ListServersQuery>,
    registry: web::Data<Arc<McpServerRegistry>>,
) -> HttpResponse {
    let offset: usize = query
        .cursor
        .as_deref()
        .and_then(|c| c.parse().ok())
        .unwrap_or(0);

    let limit = query.limit.unwrap_or(30).min(100) as usize;

    // Get all servers sorted by name
    let all_servers = registry.list(&super::model::McpServerQuery::default());
    let total = all_servers.servers.len();

    let page: Vec<McpRegistryServerDetail> = all_servers
        .servers
        .iter()
        .skip(offset)
        .take(limit)
        .map(convert_to_registry_detail)
        .collect();

    let next_cursor = if offset + limit < total {
        Some((offset + limit).to_string())
    } else {
        None
    };

    HttpResponse::Ok().json(McpRegistryServerList {
        servers: page,
        metadata: Some(ListMetadata {
            next_cursor,
            count: Some(total as u32),
        }),
    })
}

/// Get a specific MCP server by ID
///
/// `GET /v0/servers/{id}`
#[get("/v0/servers/{id}")]
async fn get_server(
    path: web::Path<String>,
    _query: web::Query<GetServerQuery>,
    registry: web::Data<Arc<McpServerRegistry>>,
) -> HttpResponse {
    let id = path.into_inner();

    match registry.get_by_id(&id) {
        Some(server) => HttpResponse::Ok().json(convert_to_registry_detail(&server)),
        None => HttpResponse::NotFound().json(McpErrorResponse {
            error: format!("Server '{}' not found", id),
        }),
    }
}

// =============================================================================
// Route Configuration
// =============================================================================

/// Configure MCP Registry routes
pub fn configure_mcp_registry(cfg: &mut web::ServiceConfig) {
    cfg.service(list_servers).service(get_server);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::ai::model::{McpCapabilities, McpTransport};
    use std::collections::HashMap;

    fn make_test_server() -> McpServer {
        McpServer {
            id: "test-id-123".to_string(),
            name: "test-mcp-server".to_string(),
            display_name: "Test MCP Server".to_string(),
            description: "A test server".to_string(),
            namespace: "default".to_string(),
            version: "1.0.0".to_string(),
            endpoint: "http://localhost:9090".to_string(),
            server_type: McpServerType::Http,
            transport: McpTransport::default(),
            capabilities: McpCapabilities::default(),
            tools: vec![],
            resources: vec![],
            prompts: vec![],
            metadata: HashMap::new(),
            tags: vec![],
            health_status: crate::api::ai::model::HealthStatus::Unknown,
            registered_at: 1700000000000,
            last_health_check: None,
            updated_at: 1700000000000,
        }
    }

    #[test]
    fn test_convert_to_registry_detail() {
        let server = make_test_server();
        let detail = convert_to_registry_detail(&server);

        assert_eq!(detail.name, "test-mcp-server");
        assert_eq!(detail.schema.unwrap(), MCP_SERVER_SCHEMA);
        assert_eq!(detail.status.unwrap(), "active");
        assert_eq!(detail.version.unwrap(), "1.0.0");
        assert_eq!(detail.description.unwrap(), "A test server");

        let remotes = detail.remotes.unwrap();
        assert_eq!(remotes.len(), 1);
        assert_eq!(remotes[0].remote_type, "http");
        assert_eq!(remotes[0].url, "http://localhost:9090");
        assert!(remotes[0].headers.is_none());

        let meta = detail.meta.unwrap();
        assert_eq!(meta.server_id.unwrap(), "test-id-123");
        assert!(meta.created_at.is_some());
        assert!(meta.updated_at.is_some());
        assert!(meta.is_latest.unwrap());
    }

    #[test]
    fn test_convert_empty_description() {
        let mut server = make_test_server();
        server.description = String::new();
        let detail = convert_to_registry_detail(&server);
        assert!(detail.description.is_none());
    }

    #[test]
    fn test_convert_server_types() {
        assert_eq!(mcp_server_type_to_remote_type(McpServerType::Http), "http");
        assert_eq!(mcp_server_type_to_remote_type(McpServerType::Sse), "sse");
        assert_eq!(
            mcp_server_type_to_remote_type(McpServerType::Stdio),
            "stdio"
        );
        assert_eq!(
            mcp_server_type_to_remote_type(McpServerType::WebSocket),
            "http"
        );
    }

    #[test]
    fn test_millis_to_rfc3339() {
        let result = millis_to_rfc3339(1700000000000);
        assert!(result.is_some());
        assert!(result.unwrap().contains("2023"));
    }

    #[test]
    fn test_transport_headers_included() {
        let mut server = make_test_server();
        server
            .transport
            .headers
            .insert("Authorization".to_string(), "Bearer token".to_string());
        let detail = convert_to_registry_detail(&server);
        let remotes = detail.remotes.unwrap();
        assert!(remotes[0].headers.is_some());
        let headers = remotes[0].headers.as_ref().unwrap();
        assert_eq!(headers.get("Authorization").unwrap(), "Bearer token");
    }
}
