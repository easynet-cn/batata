// MCP Server Operation Service - Config-backed CRUD for MCP servers
// Replaces in-memory McpServerRegistry with persistent storage

use std::sync::Arc;

use chrono::Utc;
use tracing::info;
use uuid::Uuid;

use batata_persistence::PersistenceService;

use super::constants::*;
use super::mcp_index::{McpServerIndex, McpServerIndexData};
use crate::api::ai::model::*;

/// Config-backed MCP server operation service
pub struct McpServerOperationService {
    persistence: Arc<dyn PersistenceService>,
    index: Arc<McpServerIndex>,
}

impl McpServerOperationService {
    pub fn new(
        persistence: Arc<dyn PersistenceService>,
        index: Arc<McpServerIndex>,
    ) -> Self {
        Self {
            persistence,
            index,
        }
    }

    /// Create a new MCP server, returning its generated ID
    pub async fn create_mcp_server(
        &self,
        namespace: &str,
        registration: &McpServerRegistration,
    ) -> anyhow::Result<String> {
        let name = &registration.name;
        let version = &registration.version;

        // Check for duplicate
        if self.index.get_by_name(namespace, name).is_some() {
            anyhow::bail!(
                "MCP server '{}' already exists in namespace '{}'",
                name,
                namespace
            );
        }

        let id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let now_str = now.to_rfc3339();

        // Build version info (index entry)
        let version_info = McpServerVersionInfo {
            id: id.clone(),
            name: name.clone(),
            protocol: default_mcp_protocol(),
            description: registration.description.clone(),
            capabilities: registration.capabilities.clone(),
            latest_published_version: version.clone(),
            version_details: vec![VersionDetail {
                version: version.clone(),
                release_date: now_str.clone(),
                is_latest: true,
            }],
        };

        // Build per-version spec
        let storage_info = McpServerStorageInfo {
            id: id.clone(),
            name: name.clone(),
            protocol: default_mcp_protocol(),
            enabled: true,
            remote_server_config: None,
            tools_description_ref: mcp_tool_data_id(&id, version),
            version_detail: Some(VersionDetail {
                version: version.clone(),
                release_date: now_str,
                is_latest: true,
            }),
            server_data: Some(registration.clone()),
        };

        // Publish version info config
        let version_content = serde_json::to_string(&version_info)?;
        self.publish_config(
            namespace,
            MCP_SERVER_VERSIONS_GROUP,
            &mcp_version_data_id(&id),
            &version_content,
            &mcp_tags(name),
            &format!("MCP server version info: {}", name),
        )
        .await?;

        // Publish spec config
        let spec_content = serde_json::to_string(&storage_info)?;
        self.publish_config(
            namespace,
            MCP_SERVER_GROUP,
            &mcp_spec_data_id(&id, version),
            &spec_content,
            &mcp_tags(name),
            &format!("MCP server spec: {}:{}", name, version),
        )
        .await?;

        // Publish tools config if tools are present
        if !registration.tools.is_empty() {
            let tools_content = serde_json::to_string(&registration.tools)?;
            self.publish_config(
                namespace,
                MCP_SERVER_TOOL_GROUP,
                &mcp_tool_data_id(&id, version),
                &tools_content,
                &mcp_tags(name),
                &format!("MCP server tools: {}:{}", name, version),
            )
            .await?;
        }

        // Update index cache
        self.index.upsert(McpServerIndexData {
            id: id.clone(),
            name: name.clone(),
            namespace: namespace.to_string(),
            protocol: "mcp".to_string(),
            description: registration.description.clone(),
            latest_published_version: version.clone(),
            version_count: 1,
            create_time: now.timestamp_millis(),
            modify_time: now.timestamp_millis(),
        });

        info!(
            server_name = %name,
            server_id = %id,
            namespace = %namespace,
            version = %version,
            "MCP server created (config-backed)"
        );

        Ok(id)
    }

    /// Get MCP server detail by ID or name, optionally with a specific version
    pub async fn get_mcp_server_detail(
        &self,
        namespace: &str,
        id: Option<&str>,
        name: Option<&str>,
        version: Option<&str>,
    ) -> anyhow::Result<Option<McpServer>> {
        // Resolve the server ID
        let resolved_id = if let Some(id) = id {
            id.to_string()
        } else if let Some(name) = name {
            match self.index.get_by_name(namespace, name) {
                Some(data) => data.id,
                None => return Ok(None),
            }
        } else {
            return Ok(None);
        };

        // Fetch version info to resolve version
        let version_data_id = mcp_version_data_id(&resolved_id);
        let version_config = self
            .query_config(namespace, MCP_SERVER_VERSIONS_GROUP, &version_data_id)
            .await?;

        let version_config = match version_config {
            Some(c) => c,
            None => return Ok(None),
        };

        let version_info: McpServerVersionInfo = serde_json::from_str(&version_config)?;

        // Resolve which version to fetch
        let target_version = version
            .filter(|v| !v.is_empty())
            .unwrap_or(&version_info.latest_published_version);

        // Fetch the spec for the target version
        let spec_data_id = mcp_spec_data_id(&resolved_id, target_version);
        let spec_config = self
            .query_config(namespace, MCP_SERVER_GROUP, &spec_data_id)
            .await?;

        let spec_config = match spec_config {
            Some(c) => c,
            None => return Ok(None),
        };

        let storage_info: McpServerStorageInfo = serde_json::from_str(&spec_config)?;

        // Build McpServer from storage data
        let now = Utc::now().timestamp_millis();
        let server = if let Some(ref reg) = storage_info.server_data {
            McpServer {
                id: storage_info.id.clone(),
                name: storage_info.name.clone(),
                display_name: if reg.display_name.is_empty() {
                    reg.name.clone()
                } else {
                    reg.display_name.clone()
                },
                description: reg.description.clone(),
                namespace: namespace.to_string(),
                version: target_version.to_string(),
                endpoint: reg.endpoint.clone(),
                server_type: reg.server_type,
                transport: reg.transport.clone(),
                capabilities: reg.capabilities.clone(),
                tools: reg.tools.clone(),
                resources: reg.resources.clone(),
                prompts: reg.prompts.clone(),
                metadata: reg.metadata.clone(),
                tags: reg.tags.clone(),
                health_status: HealthStatus::Unknown,
                registered_at: now,
                last_health_check: None,
                updated_at: now,
            }
        } else {
            McpServer {
                id: storage_info.id.clone(),
                name: storage_info.name.clone(),
                display_name: storage_info.name.clone(),
                description: version_info.description,
                namespace: namespace.to_string(),
                version: target_version.to_string(),
                endpoint: String::new(),
                server_type: McpServerType::Http,
                transport: McpTransport::default(),
                capabilities: version_info.capabilities,
                tools: vec![],
                resources: vec![],
                prompts: vec![],
                metadata: Default::default(),
                tags: vec![],
                health_status: HealthStatus::Unknown,
                registered_at: now,
                last_health_check: None,
                updated_at: now,
            }
        };

        Ok(Some(server))
    }

    /// Update an existing MCP server
    pub async fn update_mcp_server(
        &self,
        namespace: &str,
        registration: &McpServerRegistration,
    ) -> anyhow::Result<()> {
        let name = &registration.name;
        let version = &registration.version;

        // Resolve ID from name
        let index_data = self
            .index
            .get_by_name(namespace, name)
            .ok_or_else(|| anyhow::anyhow!("MCP server '{}' not found in namespace '{}'", name, namespace))?;

        let id = &index_data.id;
        let now = Utc::now().to_rfc3339();

        // Fetch and update version info
        let version_data_id = mcp_version_data_id(id);
        let existing_version = self
            .query_config(namespace, MCP_SERVER_VERSIONS_GROUP, &version_data_id)
            .await?;

        let mut version_info: McpServerVersionInfo = match existing_version {
            Some(content) => serde_json::from_str(&content)?,
            None => McpServerVersionInfo {
                id: id.clone(),
                name: name.clone(),
                protocol: default_mcp_protocol(),
                description: String::new(),
                capabilities: McpCapabilities::default(),
                latest_published_version: String::new(),
                version_details: vec![],
            },
        };

        // Update or add version detail
        let existing_version_idx = version_info
            .version_details
            .iter()
            .position(|v| v.version == *version);

        if let Some(idx) = existing_version_idx {
            version_info.version_details[idx].release_date = now.clone();
            version_info.version_details[idx].is_latest = true;
        } else {
            // Mark all existing as not latest
            for v in &mut version_info.version_details {
                v.is_latest = false;
            }
            version_info.version_details.push(VersionDetail {
                version: version.clone(),
                release_date: now.clone(),
                is_latest: true,
            });
        }

        version_info.latest_published_version = version.clone();
        version_info.description = registration.description.clone();
        version_info.capabilities = registration.capabilities.clone();

        // Publish updated version info
        let version_content = serde_json::to_string(&version_info)?;
        self.publish_config(
            namespace,
            MCP_SERVER_VERSIONS_GROUP,
            &version_data_id,
            &version_content,
            &mcp_tags(name),
            &format!("MCP server version info updated: {}", name),
        )
        .await?;

        // Build and publish updated spec
        let storage_info = McpServerStorageInfo {
            id: id.clone(),
            name: name.clone(),
            protocol: default_mcp_protocol(),
            enabled: true,
            remote_server_config: None,
            tools_description_ref: mcp_tool_data_id(id, version),
            version_detail: Some(VersionDetail {
                version: version.clone(),
                release_date: now,
                is_latest: true,
            }),
            server_data: Some(registration.clone()),
        };

        let spec_content = serde_json::to_string(&storage_info)?;
        self.publish_config(
            namespace,
            MCP_SERVER_GROUP,
            &mcp_spec_data_id(id, version),
            &spec_content,
            &mcp_tags(name),
            &format!("MCP server spec updated: {}:{}", name, version),
        )
        .await?;

        // Publish tools if present
        if !registration.tools.is_empty() {
            let tools_content = serde_json::to_string(&registration.tools)?;
            self.publish_config(
                namespace,
                MCP_SERVER_TOOL_GROUP,
                &mcp_tool_data_id(id, version),
                &tools_content,
                &mcp_tags(name),
                &format!("MCP server tools updated: {}:{}", name, version),
            )
            .await?;
        }

        // Update index cache
        self.index.upsert(McpServerIndexData {
            id: id.clone(),
            name: name.clone(),
            namespace: namespace.to_string(),
            protocol: "mcp".to_string(),
            description: registration.description.clone(),
            latest_published_version: version.clone(),
            version_count: version_info.version_details.len(),
            create_time: index_data.create_time,
            modify_time: Utc::now().timestamp_millis(),
        });

        info!(
            server_name = %name,
            namespace = %namespace,
            version = %version,
            "MCP server updated (config-backed)"
        );

        Ok(())
    }

    /// Delete an MCP server (all versions or a specific version)
    pub async fn delete_mcp_server(
        &self,
        namespace: &str,
        name: Option<&str>,
        id: Option<&str>,
        version: Option<&str>,
    ) -> anyhow::Result<()> {
        // Resolve the server
        let (resolved_id, resolved_name) = if let Some(name) = name {
            let data = self
                .index
                .get_by_name(namespace, name)
                .ok_or_else(|| anyhow::anyhow!("MCP server '{}' not found", name))?;
            (data.id, name.to_string())
        } else if let Some(id) = id {
            let data = self
                .index
                .get_by_id(id)
                .ok_or_else(|| anyhow::anyhow!("MCP server ID '{}' not found", id))?;
            (id.to_string(), data.name)
        } else {
            anyhow::bail!("Either mcpName or mcpId must be provided");
        };

        if let Some(version) = version.filter(|v| !v.is_empty()) {
            // Delete specific version
            self.delete_config(namespace, MCP_SERVER_GROUP, &mcp_spec_data_id(&resolved_id, version))
                .await?;
            self.delete_config(namespace, MCP_SERVER_TOOL_GROUP, &mcp_tool_data_id(&resolved_id, version))
                .await
                .ok(); // Tools may not exist

            // Update version info to remove this version
            let version_data_id = mcp_version_data_id(&resolved_id);
            if let Some(content) = self
                .query_config(namespace, MCP_SERVER_VERSIONS_GROUP, &version_data_id)
                .await?
            {
                let mut version_info: McpServerVersionInfo = serde_json::from_str(&content)?;
                version_info.version_details.retain(|v| v.version != version);

                if version_info.version_details.is_empty() {
                    // No more versions, delete the whole server
                    self.delete_config(namespace, MCP_SERVER_VERSIONS_GROUP, &version_data_id)
                        .await?;
                    self.index.remove_by_id(&resolved_id);
                } else {
                    // Update latest
                    if let Some(last) = version_info.version_details.last_mut() {
                        last.is_latest = true;
                        version_info.latest_published_version = last.version.clone();
                    }
                    let content = serde_json::to_string(&version_info)?;
                    self.publish_config(
                        namespace,
                        MCP_SERVER_VERSIONS_GROUP,
                        &version_data_id,
                        &content,
                        &mcp_tags(&resolved_name),
                        &format!("MCP server version removed: {}:{}", resolved_name, version),
                    )
                    .await?;
                }
            }
        } else {
            // Delete all versions - first get version info
            let version_data_id = mcp_version_data_id(&resolved_id);
            if let Some(content) = self
                .query_config(namespace, MCP_SERVER_VERSIONS_GROUP, &version_data_id)
                .await?
            {
                let version_info: McpServerVersionInfo = serde_json::from_str(&content)?;
                for vd in &version_info.version_details {
                    self.delete_config(
                        namespace,
                        MCP_SERVER_GROUP,
                        &mcp_spec_data_id(&resolved_id, &vd.version),
                    )
                    .await
                    .ok();
                    self.delete_config(
                        namespace,
                        MCP_SERVER_TOOL_GROUP,
                        &mcp_tool_data_id(&resolved_id, &vd.version),
                    )
                    .await
                    .ok();
                }
            }

            // Delete version info
            self.delete_config(namespace, MCP_SERVER_VERSIONS_GROUP, &version_data_id)
                .await?;

            // Remove from cache
            self.index.remove_by_id(&resolved_id);
        }

        info!(
            server_name = %resolved_name,
            namespace = %namespace,
            "MCP server deleted (config-backed)"
        );

        Ok(())
    }

    /// List MCP servers with pagination and search
    pub fn list_mcp_servers(
        &self,
        namespace: &str,
        name: Option<&str>,
        search_type: &str,
        page_no: u32,
        page_size: u32,
    ) -> McpServerListResponse {
        let page_no = page_no.max(1);
        let offset = ((page_no - 1) * page_size) as usize;
        let limit = page_size as usize;

        let (entries, total) = self.index.search_by_name(
            namespace,
            name,
            search_type,
            offset,
            limit,
        );

        // Convert index entries to McpServer stubs (without full detail)
        let servers: Vec<McpServer> = entries
            .into_iter()
            .map(|e| McpServer {
                id: e.id,
                name: e.name.clone(),
                display_name: e.name,
                description: e.description,
                namespace: e.namespace,
                version: e.latest_published_version,
                endpoint: String::new(),
                server_type: McpServerType::Http,
                transport: McpTransport::default(),
                capabilities: McpCapabilities::default(),
                tools: vec![],
                resources: vec![],
                prompts: vec![],
                metadata: Default::default(),
                tags: vec![],
                health_status: HealthStatus::Unknown,
                registered_at: e.create_time,
                last_health_check: None,
                updated_at: e.modify_time,
            })
            .collect();

        McpServerListResponse {
            servers,
            total,
            page: page_no,
            page_size,
        }
    }

    /// Get all servers (for MCP Registry server)
    pub fn list_all_servers(&self) -> Vec<McpServerIndexData> {
        self.index
            .search_by_name("", None, "blur", 0, 10000)
            .0
    }

    // =========================================================================
    // Internal helpers
    // =========================================================================

    async fn publish_config(
        &self,
        namespace: &str,
        group: &str,
        data_id: &str,
        content: &str,
        tags: &str,
        desc: &str,
    ) -> anyhow::Result<()> {
        self.persistence
            .config_create_or_update(
                data_id,
                group,
                namespace,
                content,
                AI_APP_NAME,
                AI_SRC_USER,
                "127.0.0.1",
                tags,
                desc,
                "",            // use
                "",            // effect
                AI_CONFIG_TYPE,
                "",            // schema
                "",            // encrypted_data_key
            )
            .await?;
        Ok(())
    }

    async fn query_config(
        &self,
        namespace: &str,
        group: &str,
        data_id: &str,
    ) -> anyhow::Result<Option<String>> {
        let result = self
            .persistence
            .config_find_one(data_id, group, namespace)
            .await?;

        Ok(result.map(|c| c.content))
    }

    async fn delete_config(
        &self,
        namespace: &str,
        group: &str,
        data_id: &str,
    ) -> anyhow::Result<()> {
        self.persistence
            .config_delete(data_id, group, namespace, "", "127.0.0.1", AI_SRC_USER)
            .await?;
        Ok(())
    }
}

fn default_mcp_protocol() -> String {
    "mcp".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_server_version_info_serialization() {
        let info = McpServerVersionInfo {
            id: "test-id".to_string(),
            name: "test-server".to_string(),
            protocol: "mcp".to_string(),
            description: "Test".to_string(),
            capabilities: McpCapabilities::default(),
            latest_published_version: "1.0.0".to_string(),
            version_details: vec![VersionDetail {
                version: "1.0.0".to_string(),
                release_date: "2024-01-01T00:00:00Z".to_string(),
                is_latest: true,
            }],
        };

        let json = serde_json::to_string(&info).unwrap();
        let parsed: McpServerVersionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "test-id");
        assert_eq!(parsed.version_details.len(), 1);
    }

    #[test]
    fn test_mcp_server_storage_info_serialization() {
        let info = McpServerStorageInfo {
            id: "test-id".to_string(),
            name: "test-server".to_string(),
            protocol: "mcp".to_string(),
            enabled: true,
            remote_server_config: None,
            tools_description_ref: "test-id-1.0.0-mcp-tools.json".to_string(),
            version_detail: None,
            server_data: None,
        };

        let json = serde_json::to_string(&info).unwrap();
        let parsed: McpServerStorageInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "test-id");
        assert!(parsed.enabled);
    }
}
