// MCP Server Operation Service — AI-resource-backed CRUD for MCP servers
//
// Storage mirrors upstream Nacos: governance metadata lives in `ai_resource`
// (type = `mcp`) and each published version is stored as JSON in
// `ai_resource_version.storage`.
//
// This service previously kept MCP servers as three config entries
// (`mcp-server-versions`, `mcp-server`, `mcp-tools`), which is what Nacos did
// before 3.2.0. The separate tools entry was write-only — tools are always read
// back through `server_data` — so tools now live inside the version storage.
//
// The in-memory `McpServerIndex` is kept as a cache: it backs the synchronous
// list APIs and id/name resolution, and is rebuilt from `ai_resource` by
// `McpServerIndex::refresh`.

use std::sync::Arc;

use chrono::Utc;
use tracing::info;
use uuid::Uuid;

use batata_persistence::model::{AiResourceInfo, AiResourceVersionInfo};
use batata_persistence::PersistenceService;

use super::constants::*;
use super::mcp_index::{McpServerIndex, McpServerIndexData};
use crate::model::*;
use crate::repository::{meta_status, resource_type, scope, version_status};

/// Origin recorded for locally registered MCP servers.
const MCP_DEFAULT_FROM: &str = "local";

/// AI-resource-backed MCP server operation service
pub struct McpServerOperationService {
    persistence: Arc<dyn PersistenceService>,
    index: Arc<McpServerIndex>,
}

impl McpServerOperationService {
    /// Creates a new `McpServerOperationService` with the given persistence and index.
    pub fn new(persistence: Arc<dyn PersistenceService>, index: Arc<McpServerIndex>) -> Self {
        Self { persistence, index }
    }

    // =========================================================================
    // Helpers
    // =========================================================================

    /// Parse the shared version index stored in `ai_resource.version_info`.
    fn parse_resource_version(resource: &AiResourceInfo) -> ResourceVersionInfo {
        match resource.version_info {
            Some(ref json) => serde_json::from_str::<ResourceVersionInfo>(json).unwrap_or_default(),
            None => ResourceVersionInfo::default(),
        }
    }

    /// Serialize the MCP-specific fields kept in `ai_resource.ext`.
    fn ext_json(mcp_id: &str) -> anyhow::Result<String> {
        Ok(serde_json::to_string(&McpResourceExt {
            schema_version: Some(1),
            mcp_id: mcp_id.to_string(),
        })?)
    }

    /// Read the MCP-specific fields from `ai_resource.ext`.
    fn parse_ext(resource: &AiResourceInfo) -> Option<McpResourceExt> {
        resource
            .ext
            .as_ref()
            .and_then(|json| serde_json::from_str::<McpResourceExt>(json).ok())
    }

    /// Recompute `online_cnt` and the `latest` label from the stored version
    /// rows, then persist them with an optimistic lock.
    ///
    /// Mirrors upstream `chooseLatest(onlineVersions, preferredLatest,
    /// currentLatest)`: the version being published wins, otherwise the
    /// existing label is kept while it is still online, otherwise the highest
    /// remaining online version is used.
    async fn refresh_version_meta(
        &self,
        namespace: &str,
        name: &str,
        resource: &AiResourceInfo,
        preferred: Option<&str>,
    ) -> anyhow::Result<()> {
        let rows = self
            .persistence
            .ai_resource_version_list(namespace, name, resource_type::MCP)
            .await?;

        let mut online: Vec<String> = rows
            .iter()
            .filter(|r| r.status == version_status::ONLINE)
            .map(|r| r.version.clone())
            .collect();
        online.sort();

        let mut rv = Self::parse_resource_version(resource);
        rv.online_cnt = online.len() as i64;

        let current = rv.latest_version().cloned();
        let next = match preferred {
            Some(v) if online.iter().any(|o| o == v) => Some(v.to_string()),
            _ => match current {
                Some(ref v) if online.contains(v) => Some(v.clone()),
                _ => online.last().cloned(),
            },
        };

        match next {
            Some(v) => rv.set_latest(&v),
            None => rv.clear_latest(),
        }

        self.save_version_info(namespace, name, resource, &rv).await
    }

    /// Advance the meta_version and write the version index back.
    async fn save_version_info(
        &self,
        namespace: &str,
        name: &str,
        resource: &AiResourceInfo,
        version_info: &ResourceVersionInfo,
    ) -> anyhow::Result<()> {
        self.persistence
            .ai_resource_update_version_info_cas(
                namespace,
                name,
                resource_type::MCP,
                resource.meta_version,
                &serde_json::to_string(version_info)?,
                resource.meta_version + 1,
            )
            .await?;
        Ok(())
    }

    /// Build the per-version payload stored in `ai_resource_version.storage`.
    fn build_storage_info(
        id: &str,
        name: &str,
        version: &str,
        registration: &McpServerRegistration,
        release_date: String,
    ) -> McpServerStorageInfo {
        McpServerStorageInfo {
            id: id.to_string(),
            name: name.to_string(),
            protocol: default_mcp_protocol(),
            enabled: true,
            remote_server_config: None,
            tools_description_ref: mcp_tool_data_id(id, version),
            version_detail: Some(VersionDetail {
                version: version.to_string(),
                release_date,
                is_latest: true,
            }),
            server_data: Some(registration.clone()),
        }
    }

    /// Resolve (id, name) from the caller-supplied id or name.
    async fn resolve_server(
        &self,
        namespace: &str,
        id: Option<&str>,
        name: Option<&str>,
    ) -> anyhow::Result<Option<(String, String)>> {
        if let Some(id) = id {
            return Ok(self.index.get_by_id(id).map(|d| (id.to_string(), d.name)));
        }
        if let Some(name) = name {
            return Ok(self
                .persistence
                .ai_resource_find(namespace, name, resource_type::MCP)
                .await?
                .map(|r| {
                    let mcp_id = Self::parse_ext(&r).map(|e| e.mcp_id).unwrap_or_default();
                    (mcp_id, name.to_string())
                }));
        }
        Ok(None)
    }

    // =========================================================================
    // Public operations
    // =========================================================================

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
        let now_db = now.naive_utc().to_string();

        // The shared version index carries the online count and the
        // server-managed `latest` label.
        let mut resource_version = ResourceVersionInfo::default();
        resource_version.online_cnt = 1;
        resource_version.set_latest(version);

        let storage_info = Self::build_storage_info(
            &id,
            name,
            version,
            registration,
            now_str,
        );

        let info = AiResourceInfo {
            id: 0,
            name: name.clone(),
            resource_type: resource_type::MCP.to_string(),
            description: Some(registration.description.clone()),
            status: Some(meta_status::ENABLE.to_string()),
            namespace_id: namespace.to_string(),
            biz_tags: None,
            ext: Some(Self::ext_json(&id)?),
            from: MCP_DEFAULT_FROM.to_string(),
            version_info: Some(serde_json::to_string(&resource_version)?),
            meta_version: 1,
            scope: scope::PRIVATE.to_string(),
            owner: String::new(),
            download_count: 0,
            gmt_create: Some(now_db.clone()),
            gmt_modified: Some(now_db.clone()),
        };
        self.persistence.ai_resource_insert(&info).await?;

        self.persistence
            .ai_resource_version_insert(&AiResourceVersionInfo {
                id: 0,
                resource_type: resource_type::MCP.to_string(),
                author: None,
                name: name.clone(),
                description: Some(registration.description.clone()),
                status: version_status::ONLINE.to_string(),
                version: version.clone(),
                namespace_id: namespace.to_string(),
                storage: Some(serde_json::to_string(&storage_info)?),
                publish_pipeline_info: None,
                download_count: 0,
                gmt_create: Some(now_db.clone()),
                gmt_modified: Some(now_db),
            })
            .await?;

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
            "MCP server created (ai_resource-backed)"
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
        let (_resolved_id, resolved_name) =
            match self.resolve_server(namespace, id, name).await? {
                Some(v) => v,
                None => return Ok(None),
            };

        let resource = match self
            .persistence
            .ai_resource_find(namespace, &resolved_name, resource_type::MCP)
            .await?
        {
            Some(r) => r,
            None => return Ok(None),
        };

        let resource_version = Self::parse_resource_version(&resource);
        let latest = resource_version
            .latest_version()
            .cloned()
            .unwrap_or_default();
        let target_version = version
            .filter(|v| !v.is_empty())
            .unwrap_or(&latest)
            .to_string();

        let stored = match self
            .persistence
            .ai_resource_version_find(
                namespace,
                &resolved_name,
                resource_type::MCP,
                &target_version,
            )
            .await?
        {
            Some(v) => v,
            None => return Ok(None),
        };

        let storage_info: McpServerStorageInfo = match stored.storage {
            Some(ref json) => serde_json::from_str(json)?,
            None => return Ok(None),
        };

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
                version: target_version,
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
                description: String::new(),
                namespace: namespace.to_string(),
                version: target_version,
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

        let resource = self
            .persistence
            .ai_resource_find(namespace, name, resource_type::MCP)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "MCP server '{}' not found in namespace '{}'",
                    name,
                    namespace
                )
            })?;

        let now = Utc::now();
        let mcp_id = Self::parse_ext(&resource)
            .map(|e| e.mcp_id)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| resource.name.clone());

        // Build and publish updated spec
        let storage_info = Self::build_storage_info(
            &mcp_id,
            name,
            version,
            registration,
            Utc::now().to_rfc3339(),
        );
        let storage_json = serde_json::to_string(&storage_info)?;

        let exists = self
            .persistence
            .ai_resource_version_find(namespace, name, resource_type::MCP, version)
            .await?
            .is_some();

        if exists {
            self.persistence
                .ai_resource_version_update_storage(
                    namespace,
                    name,
                    resource_type::MCP,
                    version,
                    &storage_json,
                    Some(registration.description.as_str()),
                )
                .await?;
        } else {
            let now_db = now.naive_utc().to_string();
            self.persistence
                .ai_resource_version_insert(&AiResourceVersionInfo {
                    id: 0,
                    resource_type: resource_type::MCP.to_string(),
                    author: None,
                    name: name.clone(),
                    description: Some(registration.description.clone()),
                    status: version_status::ONLINE.to_string(),
                    version: version.clone(),
                    namespace_id: namespace.to_string(),
                    storage: Some(storage_json),
                    publish_pipeline_info: None,
                    download_count: 0,
                    gmt_create: Some(now_db.clone()),
                    gmt_modified: Some(now_db),
                })
                .await?;
        }

        // Recompute the online count and the latest label from what is stored.
        self.refresh_version_meta(namespace, name, &resource, Some(version))
            .await?;
        let version_count = self
            .persistence
            .ai_resource_version_list(namespace, name, resource_type::MCP)
            .await?
            .len();

        // Update index cache
        let index_data = self.index.get_by_name(namespace, name);
        self.index.upsert(McpServerIndexData {
            id: mcp_id.clone(),
            name: name.clone(),
            namespace: namespace.to_string(),
            protocol: "mcp".to_string(),
            description: registration.description.clone(),
            latest_published_version: version.clone(),
            version_count,
            create_time: index_data
                .as_ref()
                .map(|d| d.create_time)
                .unwrap_or_else(|| now.timestamp_millis()),
            modify_time: now.timestamp_millis(),
        });

        info!(
            server_name = %name,
            namespace = %namespace,
            version = %version,
            "MCP server updated (ai_resource-backed)"
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
        let (resolved_id, resolved_name) =
            match self.resolve_server(namespace, id, name).await? {
                Some(v) => v,
                None => anyhow::bail!("Either mcpName or mcpId must be provided"),
            };

        if let Some(version) = version.filter(|v| !v.is_empty()) {
            self.persistence
                .ai_resource_version_delete(namespace, &resolved_name, resource_type::MCP, version)
                .await?;

            if let Some(resource) = self
                .persistence
                .ai_resource_find(namespace, &resolved_name, resource_type::MCP)
                .await?
            {
                let remaining = self
                    .persistence
                    .ai_resource_version_list(namespace, &resolved_name, resource_type::MCP)
                    .await?;

                if remaining.is_empty() {
                    self.persistence
                        .ai_resource_delete(namespace, &resolved_name, resource_type::MCP)
                        .await?;
                    self.index.remove_by_id(&resolved_id);
                } else {
                    // Recompute online count and the latest label from what is
                    // actually stored; no version is preferred here.
                    self.refresh_version_meta(namespace, &resolved_name, &resource, None)
                        .await?;
                }
            }
        } else {
            self.persistence
                .ai_resource_version_delete_all(namespace, &resolved_name, resource_type::MCP)
                .await?;
            self.persistence
                .ai_resource_delete(namespace, &resolved_name, resource_type::MCP)
                .await?;
            self.index.remove_by_id(&resolved_id);
        }

        info!(
            server_name = %resolved_name,
            namespace = %namespace,
            "MCP server deleted (ai_resource-backed)"
        );

        Ok(())
    }

    /// List MCP servers with pagination and search.
    /// Returns `Page<McpServerBasicInfo>` matching Nacos Java API contract.
    pub fn list_mcp_servers(
        &self,
        namespace: &str,
        name: Option<&str>,
        search_type: &str,
        page_no: u32,
        page_size: u32,
    ) -> batata_api::model::Page<McpServerBasicInfo> {
        let page_no = page_no.max(1);
        let offset = ((page_no - 1) * page_size) as usize;
        let limit = page_size as usize;

        let (entries, total) =
            self.index
                .search_by_name(namespace, name, search_type, offset, limit);

        let page_items: Vec<McpServerBasicInfo> = entries
            .into_iter()
            .map(|e| McpServerBasicInfo {
                namespace_id: e.namespace,
                id: e.id,
                name: e.name,
                protocol: e.protocol,
                description: e.description,
                version: e.latest_published_version,
                enabled: true,
                status: "ACTIVE".to_string(),
                capabilities: McpCapabilities::default(),
            })
            .collect();

        batata_api::model::Page::new(total, page_no as u64, page_size as u64, page_items)
    }

    /// Get all servers (for MCP Registry server)
    pub fn list_all_servers(&self) -> Vec<McpServerIndexData> {
        self.index.search_by_name("", None, "blur", 0, usize::MAX).0
    }
}

/// Protocol identifier recorded on MCP resources and version payloads.
fn default_mcp_protocol() -> String {
    "mcp".to_string()
}

#[async_trait::async_trait]
impl super::traits::McpServerService for McpServerOperationService {
    async fn create_mcp_server(
        &self,
        namespace: &str,
        registration: &McpServerRegistration,
    ) -> anyhow::Result<String> {
        self.create_mcp_server(namespace, registration).await
    }

    async fn get_mcp_server_detail(
        &self,
        namespace: &str,
        id: Option<&str>,
        name: Option<&str>,
        version: Option<&str>,
    ) -> anyhow::Result<Option<McpServer>> {
        self.get_mcp_server_detail(namespace, id, name, version)
            .await
    }

    async fn update_mcp_server(
        &self,
        namespace: &str,
        registration: &McpServerRegistration,
    ) -> anyhow::Result<()> {
        self.update_mcp_server(namespace, registration).await
    }

    async fn delete_mcp_server(
        &self,
        namespace: &str,
        name: Option<&str>,
        id: Option<&str>,
        version: Option<&str>,
    ) -> anyhow::Result<()> {
        self.delete_mcp_server(namespace, name, id, version).await
    }

    fn list_mcp_servers(
        &self,
        namespace: &str,
        name: Option<&str>,
        search_type: &str,
        page_no: u32,
        page_size: u32,
    ) -> batata_api::model::Page<McpServerBasicInfo> {
        self.list_mcp_servers(namespace, name, search_type, page_no, page_size)
    }

    async fn import_tools_from_mcp(
        &self,
        base_url: &str,
        endpoint: &str,
        auth_token: Option<&str>,
        timeout: std::time::Duration,
    ) -> anyhow::Result<Vec<McpTool>> {
        crate::service::mcp_client::import_tools_from_mcp_sse(
            base_url, endpoint, auth_token, timeout,
        )
        .await
    }

    async fn mcp_stats(&self) -> anyhow::Result<batata_common::model::ai::mcp::McpRegistryStats> {
        // The AI-resource-backed service does not track registry-wide counters.
        Ok(batata_common::model::ai::mcp::McpRegistryStats {
            total_servers: 0,
            healthy_servers: 0,
            unhealthy_servers: 0,
            by_namespace: std::collections::HashMap::new(),
            by_type: std::collections::HashMap::new(),
        })
    }
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
