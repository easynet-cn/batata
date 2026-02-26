//! MCP (Model Content Protocol) Server Registry API
//!
//! This module provides HTTP endpoints for MCP server registration,
//! discovery, and management.

use std::collections::HashMap;
use std::sync::Arc;

use actix_web::{HttpResponse, delete, get, post, put, web};
use chrono::Utc;
use dashmap::DashMap;
use tracing::{debug, info, warn};
use uuid::Uuid;

use super::model::*;
use crate::model::response::RestResult;

/// MCP Server Registry
///
/// In-memory registry for MCP servers. In production, this would be
/// backed by the database.
pub struct McpServerRegistry {
    /// Servers indexed by ID
    servers: DashMap<String, McpServer>,
    /// Index by namespace -> name -> ID
    name_index: DashMap<String, DashMap<String, String>>,
}

impl McpServerRegistry {
    /// Create a new registry
    pub fn new() -> Self {
        Self {
            servers: DashMap::new(),
            name_index: DashMap::new(),
        }
    }

    /// Register a new server
    pub fn register(&self, registration: McpServerRegistration) -> Result<McpServer, String> {
        let namespace = &registration.namespace;
        let name = &registration.name;

        // Check if server already exists
        let ns_index = self.name_index.entry(namespace.clone()).or_default();

        if ns_index.contains_key(name) {
            return Err(format!(
                "Server '{}' already exists in namespace '{}'",
                name, namespace
            ));
        }

        // Generate server
        let now = Utc::now().timestamp_millis();
        let server = McpServer {
            id: Uuid::new_v4().to_string(),
            name: registration.name.clone(),
            display_name: if registration.display_name.is_empty() {
                registration.name.clone()
            } else {
                registration.display_name
            },
            description: registration.description,
            namespace: registration.namespace.clone(),
            version: registration.version,
            endpoint: registration.endpoint,
            server_type: registration.server_type,
            transport: registration.transport,
            capabilities: registration.capabilities,
            tools: registration.tools,
            resources: registration.resources,
            prompts: registration.prompts,
            metadata: registration.metadata,
            tags: registration.tags,
            health_status: HealthStatus::Unknown,
            registered_at: now,
            last_health_check: None,
            updated_at: now,
        };

        // Store server
        let id = server.id.clone();
        self.servers.insert(id.clone(), server.clone());
        ns_index.insert(name.clone(), id);

        info!(
            server_name = %name,
            namespace = %namespace,
            server_id = %server.id,
            "MCP server registered"
        );

        Ok(server)
    }

    /// Update an existing server
    pub fn update(
        &self,
        namespace: &str,
        name: &str,
        registration: McpServerRegistration,
    ) -> Result<McpServer, String> {
        let ns_index = self
            .name_index
            .get(namespace)
            .ok_or_else(|| format!("Namespace '{}' not found", namespace))?;

        let id = ns_index
            .get(name)
            .ok_or_else(|| format!("Server '{}' not found in namespace '{}'", name, namespace))?
            .clone();

        let mut server = self
            .servers
            .get_mut(&id)
            .ok_or_else(|| "Server not found".to_string())?;

        // Update fields
        server.display_name = if registration.display_name.is_empty() {
            registration.name.clone()
        } else {
            registration.display_name
        };
        server.description = registration.description;
        server.version = registration.version;
        server.endpoint = registration.endpoint;
        server.server_type = registration.server_type;
        server.transport = registration.transport;
        server.capabilities = registration.capabilities;
        server.tools = registration.tools;
        server.resources = registration.resources;
        server.prompts = registration.prompts;
        server.metadata = registration.metadata;
        server.tags = registration.tags;
        server.updated_at = Utc::now().timestamp_millis();

        info!(
            server_name = %name,
            namespace = %namespace,
            "MCP server updated"
        );

        Ok(server.clone())
    }

    /// Deregister a server
    pub fn deregister(&self, namespace: &str, name: &str) -> Result<(), String> {
        let ns_index = self
            .name_index
            .get(namespace)
            .ok_or_else(|| format!("Namespace '{}' not found", namespace))?;

        let id = ns_index
            .get(name)
            .ok_or_else(|| format!("Server '{}' not found in namespace '{}'", name, namespace))?
            .clone();

        // Remove from indices
        ns_index.remove(name);
        self.servers.remove(&id);

        info!(
            server_name = %name,
            namespace = %namespace,
            "MCP server deregistered"
        );

        Ok(())
    }

    /// Get a server by namespace and name
    pub fn get(&self, namespace: &str, name: &str) -> Option<McpServer> {
        let ns_index = self.name_index.get(namespace)?;
        let id = ns_index.get(name)?;
        self.servers.get(&*id).map(|s| s.clone())
    }

    /// Get a server by ID
    pub fn get_by_id(&self, id: &str) -> Option<McpServer> {
        self.servers.get(id).map(|s| s.clone())
    }

    /// List servers with optional filtering
    pub fn list(&self, query: &McpServerQuery) -> McpServerListResponse {
        let mut servers: Vec<McpServer> = self
            .servers
            .iter()
            .map(|e| e.value().clone())
            .filter(|s| {
                // Filter by namespace
                if let Some(ref ns) = query.namespace
                    && &s.namespace != ns
                {
                    return false;
                }

                // Filter by name pattern
                if let Some(ref pattern) = query.name_pattern
                    && !matches_pattern(&s.name, pattern)
                {
                    return false;
                }

                // Filter by server type
                if let Some(st) = query.server_type
                    && s.server_type != st
                {
                    return false;
                }

                // Filter by health status
                if let Some(hs) = query.health_status
                    && s.health_status != hs
                {
                    return false;
                }

                // Filter by tags
                if let Some(ref tags) = query.tags
                    && !tags.iter().any(|t| s.tags.contains(t))
                {
                    return false;
                }

                // Filter by capabilities
                if let Some(has_tools) = query.has_tools
                    && s.capabilities.tools != has_tools
                {
                    return false;
                }

                if let Some(has_resources) = query.has_resources
                    && s.capabilities.resources != has_resources
                {
                    return false;
                }

                if let Some(has_prompts) = query.has_prompts
                    && s.capabilities.prompts != has_prompts
                {
                    return false;
                }

                true
            })
            .collect();

        // Sort by name
        servers.sort_by(|a, b| a.name.cmp(&b.name));

        let total = servers.len() as u64;

        // Pagination (page is 1-indexed, ensure page >= 1)
        let page = query.page.max(1);
        let start = ((page - 1) * query.page_size) as usize;
        let end = (start + query.page_size as usize).min(servers.len());

        let servers = if start < servers.len() {
            servers[start..end].to_vec()
        } else {
            vec![]
        };

        McpServerListResponse {
            servers,
            total,
            page: query.page,
            page_size: query.page_size,
        }
    }

    /// Import servers from JSON config (claude_desktop_config.json format)
    pub fn import(&self, request: McpServerImportRequest) -> BatchRegistrationResponse {
        let mut success_count = 0u32;
        let mut errors = Vec::new();

        for (name, config) in request.mcp_servers {
            // Build registration from config
            let registration = McpServerRegistration {
                name: name.clone(),
                display_name: name.clone(),
                description: String::new(),
                namespace: request.namespace.clone(),
                version: "1.0.0".to_string(),
                endpoint: config.url.clone().unwrap_or_else(|| {
                    format!(
                        "stdio://{}",
                        config.command.as_ref().unwrap_or(&"unknown".to_string())
                    )
                }),
                server_type: if config.command.is_some() {
                    McpServerType::Stdio
                } else {
                    McpServerType::Http
                },
                transport: McpTransport {
                    transport_type: if config.command.is_some() {
                        "stdio".to_string()
                    } else {
                        "http".to_string()
                    },
                    command: config.command,
                    args: config.args,
                    env: config.env,
                    url: config.url,
                    ..Default::default()
                },
                capabilities: McpCapabilities::default(),
                tools: vec![],
                resources: vec![],
                prompts: vec![],
                metadata: HashMap::new(),
                tags: vec!["imported".to_string()],
                auto_fetch_tools: true,
                health_check: None,
            };

            // Check if exists and handle overwrite
            if self.get(&request.namespace, &name).is_some() {
                if request.overwrite {
                    match self.update(&request.namespace, &name, registration) {
                        Ok(_) => success_count += 1,
                        Err(e) => errors.push(RegistrationError { name, error: e }),
                    }
                } else {
                    errors.push(RegistrationError {
                        name,
                        error: "Server already exists".to_string(),
                    });
                }
            } else {
                match self.register(registration) {
                    Ok(_) => success_count += 1,
                    Err(e) => errors.push(RegistrationError { name, error: e }),
                }
            }
        }

        BatchRegistrationResponse {
            success_count,
            failed_count: errors.len() as u32,
            errors,
        }
    }

    /// Update server health status
    pub fn update_health(&self, id: &str, status: HealthStatus) {
        if let Some(mut server) = self.servers.get_mut(id) {
            server.health_status = status;
            server.last_health_check = Some(Utc::now().timestamp_millis());
        }
    }

    /// Update server tools (from auto-fetch)
    pub fn update_tools(&self, id: &str, tools: Vec<McpTool>) {
        if let Some(mut server) = self.servers.get_mut(id) {
            server.tools = tools;
            server.capabilities.tools = true;
            server.updated_at = Utc::now().timestamp_millis();
        }
    }

    /// Get all namespaces
    pub fn namespaces(&self) -> Vec<String> {
        self.name_index.iter().map(|e| e.key().clone()).collect()
    }

    /// Get a server by query params (Nacos-compatible)
    /// Resolves by mcpId or mcpName within the given namespace
    pub fn get_by_query(&self, query: &McpDetailQuery) -> Option<McpServer> {
        let namespace = query.namespace_id.as_deref().unwrap_or("public");

        // Try by ID first
        if let Some(ref id) = query.mcp_id {
            return self.get_by_id(id);
        }

        // Try by name
        if let Some(ref name) = query.mcp_name {
            return self.get(namespace, name);
        }

        None
    }

    /// List servers with Nacos-compatible search params
    pub fn list_with_search(&self, query: &McpListQuery) -> McpServerListResponse {
        let namespace = query.namespace_id.clone();
        let search_type = query.search.as_deref().unwrap_or("blur");
        let page_no = query.page_no.unwrap_or(1).max(1);
        let page_size = query.page_size.unwrap_or(20);

        let mut servers: Vec<McpServer> = self
            .servers
            .iter()
            .map(|e| e.value().clone())
            .filter(|s| {
                // Filter by namespace
                if let Some(ref ns) = namespace
                    && &s.namespace != ns
                {
                    return false;
                }

                // Filter by name
                if let Some(ref name) = query.mcp_name {
                    if search_type == "accurate" {
                        if &s.name != name {
                            return false;
                        }
                    } else if !s.name.contains(name.as_str()) {
                        return false;
                    }
                }

                true
            })
            .collect();

        servers.sort_by(|a, b| a.name.cmp(&b.name));

        let total = servers.len() as u64;
        let start = ((page_no - 1) * page_size) as usize;
        let end = (start + page_size as usize).min(servers.len());

        let servers = if start < servers.len() {
            servers[start..end].to_vec()
        } else {
            vec![]
        };

        McpServerListResponse {
            servers,
            total,
            page: page_no,
            page_size,
        }
    }

    /// Delete a server by query params (Nacos-compatible)
    pub fn delete_by_query(&self, query: &McpDeleteQuery) -> Result<(), String> {
        let namespace = query.namespace_id.as_deref().unwrap_or("public");

        // Try by name first
        if let Some(ref name) = query.mcp_name {
            return self.deregister(namespace, name);
        }

        // Try by ID
        if let Some(ref id) = query.mcp_id {
            if let Some(server) = self.get_by_id(id) {
                return self.deregister(&server.namespace, &server.name);
            }
            return Err(format!("Server with ID '{}' not found", id));
        }

        Err("Either mcpName or mcpId must be provided".to_string())
    }

    /// Get stats
    pub fn stats(&self) -> McpRegistryStats {
        let mut by_namespace: HashMap<String, u32> = HashMap::new();
        let mut by_type: HashMap<String, u32> = HashMap::new();
        let mut healthy = 0u32;
        let mut unhealthy = 0u32;

        for entry in self.servers.iter() {
            let server = entry.value();
            *by_namespace.entry(server.namespace.clone()).or_insert(0) += 1;
            *by_type
                .entry(format!("{:?}", server.server_type).to_lowercase())
                .or_insert(0) += 1;

            match server.health_status {
                HealthStatus::Healthy => healthy += 1,
                HealthStatus::Unhealthy => unhealthy += 1,
                _ => {}
            }
        }

        McpRegistryStats {
            total_servers: self.servers.len() as u32,
            healthy_servers: healthy,
            unhealthy_servers: unhealthy,
            by_namespace,
            by_type,
        }
    }
}

impl Default for McpServerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple wildcard pattern matching
fn matches_pattern(name: &str, pattern: &str) -> bool {
    if pattern.is_empty() {
        return true;
    }

    if pattern == "*" {
        return true;
    }

    if pattern.starts_with('*') && pattern.ends_with('*') {
        let inner = &pattern[1..pattern.len() - 1];
        return name.contains(inner);
    }

    if let Some(suffix) = pattern.strip_prefix('*') {
        return name.ends_with(suffix);
    }

    if let Some(prefix) = pattern.strip_suffix('*') {
        return name.starts_with(prefix);
    }

    name == pattern
}

/// MCP Registry statistics
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpRegistryStats {
    pub total_servers: u32,
    pub healthy_servers: u32,
    pub unhealthy_servers: u32,
    pub by_namespace: HashMap<String, u32>,
    pub by_type: HashMap<String, u32>,
}

// =============================================================================
// HTTP Handlers
// =============================================================================

/// Register a new MCP server
#[post("/v1/ai/mcp/servers")]
pub async fn register_server(
    registry: web::Data<Arc<McpServerRegistry>>,
    body: web::Json<McpServerRegistration>,
) -> HttpResponse {
    debug!(server_name = %body.name, "Registering MCP server");

    match registry.register(body.into_inner()) {
        Ok(server) => HttpResponse::Ok().json(RestResult::ok(Some(server))),
        Err(e) => {
            warn!(error = %e, "Failed to register MCP server");
            HttpResponse::BadRequest().json(RestResult::<()>::err(400, &e))
        }
    }
}

/// Update an existing MCP server
#[put("/v1/ai/mcp/servers/{namespace}/{name}")]
pub async fn update_server(
    registry: web::Data<Arc<McpServerRegistry>>,
    path: web::Path<(String, String)>,
    body: web::Json<McpServerRegistration>,
) -> HttpResponse {
    let (namespace, name) = path.into_inner();
    debug!(server_name = %name, namespace = %namespace, "Updating MCP server");

    match registry.update(&namespace, &name, body.into_inner()) {
        Ok(server) => HttpResponse::Ok().json(RestResult::ok(Some(server))),
        Err(e) => {
            warn!(error = %e, "Failed to update MCP server");
            HttpResponse::NotFound().json(RestResult::<()>::err(404, &e))
        }
    }
}

/// Deregister an MCP server
#[delete("/v1/ai/mcp/servers/{namespace}/{name}")]
pub async fn deregister_server(
    registry: web::Data<Arc<McpServerRegistry>>,
    path: web::Path<(String, String)>,
) -> HttpResponse {
    let (namespace, name) = path.into_inner();
    debug!(server_name = %name, namespace = %namespace, "Deregistering MCP server");

    match registry.deregister(&namespace, &name) {
        Ok(()) => HttpResponse::Ok().json(RestResult::ok(Some(true))),
        Err(e) => {
            warn!(error = %e, "Failed to deregister MCP server");
            HttpResponse::NotFound().json(RestResult::<()>::err(404, &e))
        }
    }
}

/// Get an MCP server by namespace and name
#[get("/v1/ai/mcp/servers/{namespace}/{name}")]
pub async fn get_server(
    registry: web::Data<Arc<McpServerRegistry>>,
    path: web::Path<(String, String)>,
) -> HttpResponse {
    let (namespace, name) = path.into_inner();

    match registry.get(&namespace, &name) {
        Some(server) => HttpResponse::Ok().json(RestResult::ok(Some(server))),
        None => HttpResponse::NotFound().json(RestResult::<()>::err(
            404,
            &format!("Server '{}' not found in namespace '{}'", name, namespace),
        )),
    }
}

/// List MCP servers
#[get("/v1/ai/mcp/servers")]
pub async fn list_servers(
    registry: web::Data<Arc<McpServerRegistry>>,
    query: web::Query<McpServerQuery>,
) -> HttpResponse {
    let result = registry.list(&query.into_inner());
    HttpResponse::Ok().json(RestResult::ok(Some(result)))
}

/// Import MCP servers from JSON config
#[post("/v1/ai/mcp/servers/import")]
pub async fn import_servers(
    registry: web::Data<Arc<McpServerRegistry>>,
    body: web::Json<McpServerImportRequest>,
) -> HttpResponse {
    debug!("Importing MCP servers");

    let result = registry.import(body.into_inner());
    HttpResponse::Ok().json(RestResult::ok(Some(result)))
}

/// Get MCP registry statistics
#[get("/v1/ai/mcp/stats")]
pub async fn get_stats(registry: web::Data<Arc<McpServerRegistry>>) -> HttpResponse {
    let stats = registry.stats();
    HttpResponse::Ok().json(RestResult::ok(Some(stats)))
}

/// Configure MCP routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(register_server)
        .service(update_server)
        .service(deregister_server)
        .service(get_server)
        .service(list_servers)
        .service(import_servers)
        .service(get_stats);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_registration() -> McpServerRegistration {
        McpServerRegistration {
            name: "test-server".to_string(),
            display_name: "Test Server".to_string(),
            description: "A test server".to_string(),
            namespace: "default".to_string(),
            version: "1.0.0".to_string(),
            endpoint: "http://localhost:8080".to_string(),
            server_type: McpServerType::Http,
            transport: McpTransport::default(),
            capabilities: McpCapabilities {
                tools: true,
                ..Default::default()
            },
            tools: vec![],
            resources: vec![],
            prompts: vec![],
            metadata: HashMap::new(),
            tags: vec!["test".to_string()],
            auto_fetch_tools: false,
            health_check: None,
        }
    }

    #[test]
    fn test_register_server() {
        let registry = McpServerRegistry::new();
        let registration = create_test_registration();

        let server = registry.register(registration).unwrap();
        assert_eq!(server.name, "test-server");
        assert!(!server.id.is_empty());
    }

    #[test]
    fn test_get_server() {
        let registry = McpServerRegistry::new();
        let registration = create_test_registration();

        registry.register(registration).unwrap();

        let server = registry.get("default", "test-server").unwrap();
        assert_eq!(server.name, "test-server");
    }

    #[test]
    fn test_deregister_server() {
        let registry = McpServerRegistry::new();
        let registration = create_test_registration();

        registry.register(registration).unwrap();
        registry.deregister("default", "test-server").unwrap();

        assert!(registry.get("default", "test-server").is_none());
    }

    #[test]
    fn test_list_servers() {
        let registry = McpServerRegistry::new();

        // Register multiple servers
        for i in 0..5 {
            let mut reg = create_test_registration();
            reg.name = format!("server-{}", i);
            registry.register(reg).unwrap();
        }

        let result = registry.list(&McpServerQuery::default());
        assert_eq!(result.total, 5);
        assert_eq!(result.servers.len(), 5);
    }

    #[test]
    fn test_list_with_pagination() {
        let registry = McpServerRegistry::new();

        for i in 0..25 {
            let mut reg = create_test_registration();
            reg.name = format!("server-{:02}", i);
            registry.register(reg).unwrap();
        }

        let result = registry.list(&McpServerQuery {
            page: 2,
            page_size: 10,
            ..Default::default()
        });

        assert_eq!(result.total, 25);
        assert_eq!(result.servers.len(), 10);
        assert_eq!(result.page, 2);
    }

    #[test]
    fn test_list_with_namespace_filter() {
        let registry = McpServerRegistry::new();

        let mut reg1 = create_test_registration();
        reg1.namespace = "ns1".to_string();
        registry.register(reg1).unwrap();

        let mut reg2 = create_test_registration();
        reg2.name = "server-2".to_string();
        reg2.namespace = "ns2".to_string();
        registry.register(reg2).unwrap();

        let result = registry.list(&McpServerQuery {
            namespace: Some("ns1".to_string()),
            ..Default::default()
        });

        assert_eq!(result.total, 1);
    }

    #[test]
    fn test_pattern_matching() {
        assert!(matches_pattern("test-server", "test-*"));
        assert!(matches_pattern("test-server", "*-server"));
        assert!(matches_pattern("test-server", "*-serv*"));
        assert!(matches_pattern("test-server", "*"));
        assert!(matches_pattern("test-server", "test-server"));
        assert!(!matches_pattern("test-server", "other-*"));
    }

    #[test]
    fn test_import_servers() {
        let registry = McpServerRegistry::new();

        let mut servers = HashMap::new();
        servers.insert(
            "filesystem".to_string(),
            McpServerConfig {
                command: Some("npx".to_string()),
                args: vec!["-y".to_string(), "@mcp/server-filesystem".to_string()],
                env: HashMap::new(),
                url: None,
            },
        );
        servers.insert(
            "github".to_string(),
            McpServerConfig {
                command: Some("npx".to_string()),
                args: vec!["-y".to_string(), "@mcp/server-github".to_string()],
                env: {
                    let mut env = HashMap::new();
                    env.insert("GITHUB_TOKEN".to_string(), "token".to_string());
                    env
                },
                url: None,
            },
        );

        let request = McpServerImportRequest {
            mcp_servers: servers,
            namespace: "default".to_string(),
            overwrite: false,
        };

        let result = registry.import(request);
        assert_eq!(result.success_count, 2);
        assert_eq!(result.failed_count, 0);
    }
}
