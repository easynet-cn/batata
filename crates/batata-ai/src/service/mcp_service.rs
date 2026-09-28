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
use tracing::{info, warn};
use uuid::Uuid;

use batata_persistence::PersistenceService;
use batata_persistence::model::{AiResourceInfo, AiResourceVersionInfo};

use super::constants::*;
use super::mcp_index::{McpServerIndex, McpServerIndexData};
use crate::model::*;
use crate::repository::{meta_status, resource_type, scope, version_status};
use batata_common::model::ai::search::AiResourceSearchHit;

/// Origin recorded for locally registered MCP servers.
const MCP_DEFAULT_FROM: &str = "local";

/// AI-resource-backed MCP server operation service
pub struct McpServerOperationService {
    persistence: Arc<dyn PersistenceService>,
    index: Arc<McpServerIndex>,
    visibility_manager: Arc<batata_visibility::VisibilityPluginManager>,
}

impl McpServerOperationService {
    /// Creates a new `McpServerOperationService` with the given persistence and index.
    pub fn new(persistence: Arc<dyn PersistenceService>, index: Arc<McpServerIndex>) -> Self {
        Self::with_visibility(persistence, index, None, false)
    }

    /// Creates a new `McpServerOperationService` wired to the visibility plugin.
    ///
    /// Registers a default visibility service (with the optional auth plugin and
    /// auth enabled flag) if one is not already present, mirroring
    /// `SkillOperationService`.
    pub fn with_visibility(
        persistence: Arc<dyn PersistenceService>,
        index: Arc<McpServerIndex>,
        auth_plugin: Option<Arc<dyn batata_common::AuthPlugin>>,
        auth_enabled: bool,
    ) -> Self {
        let visibility_manager = batata_visibility::VisibilityPluginManager::instance();
        if visibility_manager.default_service().is_none() {
            let mut service = batata_visibility::DefaultVisibilityService::new()
                .with_auth_disabled(!auth_enabled);
            if let Some(plugin) = auth_plugin {
                service = service.with_auth_plugin(plugin);
            }
            visibility_manager.register(Arc::new(service));
        }
        Self {
            persistence,
            index,
            visibility_manager,
        }
    }

    /// Ask the visibility plugin which rows the caller may read.
    async fn read_predicate(
        &self,
        user: Option<&str>,
    ) -> batata_visibility::BaseVisibilityPredicate {
        let identity = user.unwrap_or("");
        let advisor = self
            .visibility_manager
            .advise_with_default(
                identity,
                batata_visibility::ACTION_READ,
                "admin",
                &batata_visibility::VisibilityQueryContext {
                    namespace_id: String::new(),
                    resource_type: resource_type::MCP.to_string(),
                },
            )
            .await;
        advisor.base_predicate
    }

    /// Validate visibility for a single-resource operation.
    async fn check_visibility(
        &self,
        user: Option<&str>,
        action: &str,
        resource: &AiResourceInfo,
    ) -> anyhow::Result<()> {
        let identity = user.unwrap_or("");
        let vis_resource = batata_visibility::GenericVisibilityResource {
            namespace_id: resource.namespace_id.clone(),
            resource_name: resource.name.clone(),
            resource_type: resource.resource_type.clone(),
            scope: resource.scope.clone(),
            owner: resource.owner.clone(),
        };
        let result = self
            .visibility_manager
            .validate_with_default(identity, action, "admin", &vis_resource)
            .await;
        if !result.is_allowed() {
            anyhow::bail!(
                "Visibility check failed: {}",
                result.reason().unwrap_or("access denied")
            );
        }
        Ok(())
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
        preferred: Option<&str>,
    ) -> anyhow::Result<()> {
        // Re-read the resource: callers commonly update the version index just
        // before this, which advances meta_version. Using the caller's stale
        // snapshot would make the optimistic-lock update below fail silently.
        let resource = match self
            .persistence
            .ai_resource_find(namespace, name, resource_type::MCP)
            .await?
        {
            Some(r) => r,
            None => return Ok(()),
        };
        let resource = &resource;

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

        let storage_info = Self::build_storage_info(&id, name, version, registration, now_str);

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
            // Matches the `ai_resource` row written above; new servers are
            // private until `PUT /scope` changes them.
            scope: scope::PRIVATE.to_string(),
            owner: String::new(),
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
    ///
    /// `user` is the caller identity; a resource the caller may not read is
    /// reported as absent rather than leaking its contents.
    pub async fn get_mcp_server_detail(
        &self,
        namespace: &str,
        id: Option<&str>,
        name: Option<&str>,
        version: Option<&str>,
        user: Option<&str>,
    ) -> anyhow::Result<Option<McpServer>> {
        let (_resolved_id, resolved_name) = match self.resolve_server(namespace, id, name).await? {
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

        self.check_visibility(user, batata_visibility::ACTION_READ, &resource)
            .await?;

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
        self.refresh_version_meta(namespace, name, Some(version))
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
            // Carry the previous scope/owner forward; default to private so a
            // freshly indexed row is never over-permissive.
            scope: index_data
                .as_ref()
                .map(|d| d.scope.clone())
                .unwrap_or_else(|| scope::PRIVATE.to_string()),
            owner: index_data
                .as_ref()
                .map(|d| d.owner.clone())
                .unwrap_or_default(),
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
        let (resolved_id, resolved_name) = match self.resolve_server(namespace, id, name).await? {
            Some(v) => v,
            None => anyhow::bail!("Either mcpName or mcpId must be provided"),
        };

        if let Some(version) = version.filter(|v| !v.is_empty()) {
            self.persistence
                .ai_resource_version_delete(namespace, &resolved_name, resource_type::MCP, version)
                .await?;

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
                self.refresh_version_meta(namespace, &resolved_name, None)
                    .await?;
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
    /// List MCP servers visible to `user`.
    ///
    /// Visibility is applied inside the index search so the reported total
    /// excludes rows the caller may not read.
    pub async fn list_mcp_servers(
        &self,
        namespace: &str,
        name: Option<&str>,
        search_type: &str,
        page_no: u32,
        page_size: u32,
        user: Option<&str>,
    ) -> batata_api::model::Page<McpServerBasicInfo> {
        let page_no = page_no.max(1);
        let offset = ((page_no - 1) * page_size) as usize;
        let limit = page_size as usize;

        let identity = user.unwrap_or("").to_string();
        let predicate = self.read_predicate(user).await;
        let visible = |entry: &McpServerIndexData| match &predicate {
            batata_visibility::BaseVisibilityPredicate::All => true,
            batata_visibility::BaseVisibilityPredicate::Public => {
                entry.scope == batata_visibility::SCOPE_PUBLIC
            }
            batata_visibility::BaseVisibilityPredicate::Owner => {
                !identity.is_empty() && entry.owner == identity
            }
            batata_visibility::BaseVisibilityPredicate::PublicAndOwner => {
                entry.scope == batata_visibility::SCOPE_PUBLIC
                    || (!identity.is_empty() && entry.owner == identity)
            }
        };

        let (entries, total) = self.index.search_by_name(
            namespace,
            name,
            search_type,
            offset,
            limit,
            Some(&visible),
        );

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

    /// Build a version summary from a stored version row.
    fn version_to_summary(
        row: &AiResourceVersionInfo,
        latest: Option<&str>,
    ) -> McpServerVersionSummary {
        McpServerVersionSummary {
            version: row.version.clone(),
            status: row.status.clone(),
            publish_pipeline_info: row.publish_pipeline_info.clone(),
            author: row.author.clone(),
            description: row.description.clone(),
            latest: Some(latest == Some(row.version.as_str())),
            create_time: parse_millis(row.gmt_create.as_ref()),
            update_time: parse_millis(row.gmt_modified.as_ref()),
        }
    }

    /// Assemble the version detail payload returned by the version endpoints.
    fn build_version_detail(
        &self,
        namespace: &str,
        name: &str,
        resource: &AiResourceInfo,
        row: &AiResourceVersionInfo,
        resource_version: &ResourceVersionInfo,
        registration: Option<&McpServerRegistration>,
    ) -> McpServerVersionDetail {
        let latest = resource_version.latest_version().cloned();
        let mcp_id = Self::parse_ext(resource)
            .map(|e| e.mcp_id)
            .unwrap_or_default();

        let (server_spec, tool_spec, resource_spec) = match registration {
            Some(reg) => (
                Some(McpServerBasicInfo {
                    namespace_id: namespace.to_string(),
                    id: mcp_id,
                    name: reg.name.clone(),
                    protocol: default_mcp_protocol(),
                    description: reg.description.clone(),
                    version: row.version.clone(),
                    enabled: true,
                    status: "ACTIVE".to_string(),
                    capabilities: reg.capabilities.clone(),
                }),
                Some(McpToolSpecification {
                    specification_type: Some("normal".to_string()),
                    encrypt_data: None,
                    tools: reg.tools.clone(),
                }),
                Some(McpResourceSpecification {
                    specification_type: Some("normal".to_string()),
                    encrypt_data: None,
                    resources: reg
                        .resources
                        .iter()
                        .map(serde_json::to_value)
                        .map(Result::unwrap_or_default)
                        .collect(),
                }),
            ),
            None => (None, None, None),
        };

        McpServerVersionDetail {
            summary: Self::version_to_summary(row, latest.as_deref()),
            namespace_id: namespace.to_string(),
            mcp_name: name.to_string(),
            server_specification: server_spec,
            tool_specification: tool_spec,
            resource_specification: resource_spec,
            resource_status: resource.status.clone(),
            owner: Some(resource.owner.clone()),
            scope: Some(resource.scope.clone()),
            labels: Some(resource_version.labels.clone()),
            editing_version: resource_version.editing_version.clone(),
            reviewing_version: resource_version.reviewing_version.clone(),
            online_count: Some(resource_version.online_cnt as i32),
            // A version may be modified while it is still a draft.
            writable: row.status == version_status::DRAFT,
        }
    }

    /// List versions of one MCP server with pagination.
    pub async fn list_mcp_server_versions(
        &self,
        namespace: &str,
        name: &str,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<batata_api::model::Page<McpServerVersionSummary>> {
        let resource = self
            .persistence
            .ai_resource_find(namespace, name, resource_type::MCP)
            .await?;
        let latest = resource
            .as_ref()
            .and_then(|r| Self::parse_resource_version(r).latest_version().cloned());

        let mut rows = self
            .persistence
            .ai_resource_version_list(namespace, name, resource_type::MCP)
            .await?;
        rows.sort_by(|a, b| a.version.cmp(&b.version));

        let total_count = rows.len() as u64;
        let page_no = page_no.max(1);
        let start = ((page_no - 1) * page_size) as usize;
        let page_items: Vec<McpServerVersionSummary> = rows
            .into_iter()
            .skip(start)
            .take(page_size as usize)
            .map(|r| Self::version_to_summary(&r, latest.as_deref()))
            .collect();

        Ok(batata_api::model::Page::new(
            total_count,
            page_no,
            page_size,
            page_items,
        ))
    }

    /// Get the detail of one version.
    pub async fn get_mcp_server_version(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<Option<McpServerVersionDetail>> {
        let resource = match self
            .persistence
            .ai_resource_find(namespace, name, resource_type::MCP)
            .await?
        {
            Some(r) => r,
            None => return Ok(None),
        };
        let row = match self
            .persistence
            .ai_resource_version_find(namespace, name, resource_type::MCP, version)
            .await?
        {
            Some(v) => v,
            None => return Ok(None),
        };

        let resource_version = Self::parse_resource_version(&resource);
        let registration = row
            .storage
            .as_ref()
            .and_then(|json| serde_json::from_str::<McpServerStorageInfo>(json).ok())
            .and_then(|info| info.server_data);

        Ok(Some(self.build_version_detail(
            namespace,
            name,
            &resource,
            &row,
            &resource_version,
            registration.as_ref(),
        )))
    }

    /// Create a draft version (status `draft`, tracked as `editingVersion`).
    pub async fn create_mcp_server_draft(
        &self,
        namespace: &str,
        registration: &McpServerRegistration,
        overwrite: bool,
    ) -> anyhow::Result<McpServerVersionDetail> {
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

        let mut resource_version = Self::parse_resource_version(&resource);
        if let Some(ref editing) = resource_version.editing_version {
            if !overwrite {
                anyhow::bail!(
                    "MCP server '{}' already has an editing version '{}', set overwrite=true",
                    name,
                    editing
                );
            }
            self.persistence
                .ai_resource_version_delete(namespace, name, resource_type::MCP, editing)
                .await?;
        }

        let mcp_id = Self::parse_ext(&resource)
            .map(|e| e.mcp_id)
            .unwrap_or_default();
        let storage_info = Self::build_storage_info(
            &mcp_id,
            name,
            version,
            registration,
            Utc::now().to_rfc3339(),
        );
        let now_db = Utc::now().naive_utc().to_string();

        self.persistence
            .ai_resource_version_insert(&AiResourceVersionInfo {
                id: 0,
                resource_type: resource_type::MCP.to_string(),
                author: None,
                name: name.clone(),
                description: Some(registration.description.clone()),
                status: version_status::DRAFT.to_string(),
                version: version.clone(),
                namespace_id: namespace.to_string(),
                storage: Some(serde_json::to_string(&storage_info)?),
                publish_pipeline_info: None,
                download_count: 0,
                gmt_create: Some(now_db.clone()),
                gmt_modified: Some(now_db),
            })
            .await?;

        resource_version.editing_version = Some(version.clone());
        self.save_version_info(namespace, name, &resource, &resource_version)
            .await?;

        self.get_mcp_server_version(namespace, name, version)
            .await?
            .ok_or_else(|| anyhow::anyhow!("draft '{}' was not persisted", version))
    }

    /// Update the draft version currently being edited.
    pub async fn update_mcp_server_draft(
        &self,
        namespace: &str,
        registration: &McpServerRegistration,
    ) -> anyhow::Result<McpServerVersionDetail> {
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

        let resource_version = Self::parse_resource_version(&resource);
        let editing = resource_version
            .editing_version
            .clone()
            .ok_or_else(|| anyhow::anyhow!("MCP server '{}' has no editing version", name))?;
        if editing != *version {
            anyhow::bail!(
                "MCP server '{}' editing version is '{}', not '{}'",
                name,
                editing,
                version
            );
        }

        let mcp_id = Self::parse_ext(&resource)
            .map(|e| e.mcp_id)
            .unwrap_or_default();
        let storage_info = Self::build_storage_info(
            &mcp_id,
            name,
            version,
            registration,
            Utc::now().to_rfc3339(),
        );
        let storage_json = serde_json::to_string(&storage_info)?;

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

        self.get_mcp_server_version(namespace, name, version)
            .await?
            .ok_or_else(|| anyhow::anyhow!("draft '{}' was not persisted", version))
    }

    /// Delete a draft version.
    pub async fn delete_mcp_server_draft(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<()> {
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

        self.persistence
            .ai_resource_version_delete(namespace, name, resource_type::MCP, version)
            .await?;

        let mut resource_version = Self::parse_resource_version(&resource);
        if resource_version.editing_version.as_deref() == Some(version) {
            resource_version.editing_version = None;
            self.save_version_info(namespace, name, &resource, &resource_version)
                .await?;
        }

        Ok(())
    }

    /// Load one version row, failing when it does not exist.
    async fn find_version_row(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<AiResourceVersionInfo> {
        self.persistence
            .ai_resource_version_find(namespace, name, resource_type::MCP, version)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!("Version '{}' of MCP server '{}' not found", version, name)
            })
    }

    /// Move a version to `target` status and reconcile the resource-level
    /// editing / reviewing / latest markers.
    async fn transition_version(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
        target: &str,
    ) -> anyhow::Result<McpServerVersionDetail> {
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

        self.persistence
            .ai_resource_version_update_status(namespace, name, resource_type::MCP, version, target)
            .await?;

        let mut resource_version = Self::parse_resource_version(&resource);
        match target {
            version_status::DRAFT => {
                resource_version.editing_version = Some(version.to_string());
                if resource_version.reviewing_version.as_deref() == Some(version) {
                    resource_version.reviewing_version = None;
                }
                self.save_version_info(namespace, name, &resource, &resource_version)
                    .await?;
            }
            version_status::REVIEWING => {
                resource_version.editing_version = None;
                resource_version.reviewing_version = Some(version.to_string());
                self.save_version_info(namespace, name, &resource, &resource_version)
                    .await?;
            }
            version_status::ONLINE => {
                resource_version.editing_version = None;
                resource_version.reviewing_version = None;
                self.save_version_info(namespace, name, &resource, &resource_version)
                    .await?;
                self.refresh_version_meta(namespace, name, Some(version))
                    .await?;
            }
            version_status::OFFLINE => {
                // The version left the online set, so the latest label may have
                // to move to another version.
                self.refresh_version_meta(namespace, name, None).await?;
            }
            _ => {
                self.save_version_info(namespace, name, &resource, &resource_version)
                    .await?;
            }
        }

        self.get_mcp_server_version(namespace, name, version)
            .await?
            .ok_or_else(|| anyhow::anyhow!("version '{}' was not persisted", version))
    }

    /// Submit a draft for review (draft → reviewing).
    pub async fn submit_mcp_server_version(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<McpServerVersionDetail> {
        let row = self.find_version_row(namespace, name, version).await?;
        if row.status != version_status::DRAFT {
            anyhow::bail!(
                "Version '{}' must be in draft status to submit (current: '{}')",
                version,
                row.status
            );
        }
        self.transition_version(namespace, name, version, version_status::REVIEWING)
            .await
    }

    /// Publish a version that passed review (reviewing / reviewed → online).
    ///
    /// Publishing an already-online version is idempotent.
    pub async fn publish_mcp_server_version(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<McpServerVersionDetail> {
        let row = self.find_version_row(namespace, name, version).await?;
        if row.status != version_status::REVIEWING
            && row.status != version_status::REVIEWED
            && row.status != version_status::ONLINE
        {
            anyhow::bail!(
                "Version '{}' must be in reviewing, reviewed or online status to publish (current: '{}')",
                version,
                row.status
            );
        }
        let detail = self
            .transition_version(namespace, name, version, version_status::ONLINE)
            .await?;
        self.schedule_search_index(namespace, name).await;
        Ok(detail)
    }

    /// Publish a version bypassing the review state check.
    pub async fn force_publish_mcp_server_version(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<McpServerVersionDetail> {
        self.find_version_row(namespace, name, version).await?;
        let detail = self
            .transition_version(namespace, name, version, version_status::ONLINE)
            .await?;
        self.schedule_search_index(namespace, name).await;
        Ok(detail)
    }

    /// Queue a `base_index` task for a resource.
    ///
    /// Indexing is asynchronous upstream, so a scheduling failure must not fail
    /// the publish: the task is durable and the index converges later.
    async fn schedule_search_index(&self, namespace: &str, name: &str) {
        let search = crate::search::service::AiResourceSearchService::new(self.persistence.clone());
        if let Err(e) = search.schedule(namespace, resource_type::MCP, name).await {
            warn!(
                server_name = %name,
                error = %e,
                "Failed to schedule MCP search index rebuild"
            );
        }
    }

    /// Move a version back to draft so it can be edited again.
    pub async fn redraft_mcp_server_version(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<McpServerVersionDetail> {
        self.find_version_row(namespace, name, version).await?;
        self.transition_version(namespace, name, version, version_status::DRAFT)
            .await
    }

    /// Bring an offline version back online.
    pub async fn online_mcp_server_version(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<McpServerVersionDetail> {
        let row = self.find_version_row(namespace, name, version).await?;
        if row.status != version_status::OFFLINE {
            anyhow::bail!(
                "Version '{}' must be offline to bring online (current: '{}')",
                version,
                row.status
            );
        }
        self.transition_version(namespace, name, version, version_status::ONLINE)
            .await
    }

    /// Take an online version offline.
    pub async fn offline_mcp_server_version(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<McpServerVersionDetail> {
        let row = self.find_version_row(namespace, name, version).await?;
        if row.status != version_status::ONLINE {
            anyhow::bail!(
                "Version '{}' must be online to take offline (current: '{}')",
                version,
                row.status
            );
        }
        self.transition_version(namespace, name, version, version_status::OFFLINE)
            .await
    }

    /// Replace the custom version labels, preserving the server-managed
    /// `latest` label.
    ///
    /// Mirrors upstream `AiResourceManager.validateAndUpdateLabels`: every
    /// custom label must point at a version that is currently online.
    pub async fn update_mcp_server_labels(
        &self,
        namespace: &str,
        name: &str,
        labels: std::collections::HashMap<String, String>,
    ) -> anyhow::Result<std::collections::HashMap<String, String>> {
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

        let rows = self
            .persistence
            .ai_resource_version_list(namespace, name, resource_type::MCP)
            .await?;
        let online: std::collections::HashSet<&str> = rows
            .iter()
            .filter(|r| r.status == version_status::ONLINE)
            .map(|r| r.version.as_str())
            .collect();

        for (label, version) in &labels {
            if !online.contains(version.as_str()) {
                anyhow::bail!(
                    "Label '{}' points to version '{}' which is not online",
                    label,
                    version
                );
            }
        }

        let mut resource_version = Self::parse_resource_version(&resource);
        let latest = resource_version.latest_version().cloned();
        resource_version.labels = labels;
        if let Some(l) = latest {
            resource_version.set_latest(&l);
        }
        self.save_version_info(namespace, name, &resource, &resource_version)
            .await?;

        Ok(resource_version.labels.clone())
    }

    /// Enable or disable the MCP server (resource-level status).
    pub async fn update_mcp_server_status(
        &self,
        namespace: &str,
        name: &str,
        enabled: bool,
    ) -> anyhow::Result<()> {
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

        let status = if enabled {
            meta_status::ENABLE
        } else {
            meta_status::DISABLE
        };
        self.persistence
            .ai_resource_update_status(namespace, &resource.name, resource_type::MCP, status)
            .await
    }

    /// Change the visibility scope (`PUBLIC` / `PRIVATE`).
    pub async fn update_mcp_server_scope(
        &self,
        namespace: &str,
        name: &str,
        new_scope: &str,
    ) -> anyhow::Result<()> {
        if new_scope != scope::PUBLIC && new_scope != scope::PRIVATE {
            anyhow::bail!(
                "Invalid scope '{}', must be '{}' or '{}'",
                new_scope,
                scope::PUBLIC,
                scope::PRIVATE
            );
        }

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

        self.persistence
            .ai_resource_update_scope(namespace, &resource.name, resource_type::MCP, new_scope)
            .await?;

        // The list path reads the in-memory index, so the cached scope has to
        // change too — otherwise visibility keeps using the old value until the
        // next full refresh.
        if let Some(mut entry) = self.index.get_by_name(namespace, &resource.name) {
            entry.scope = new_scope.to_string();
            self.index.upsert(entry);
        }

        Ok(())
    }

    /// Get all servers (for MCP Registry server)
    ///
    /// This is the registry feed rather than an admin listing, so no visibility
    /// filter is applied — pass a predicate at the call site if that changes.
    pub fn list_all_servers(&self) -> Vec<McpServerIndexData> {
        self.index
            .search_by_name("", None, "blur", 0, usize::MAX, None)
            .0
    }
}

/// Protocol identifier recorded on MCP resources and version payloads.
fn default_mcp_protocol() -> String {
    "mcp".to_string()
}

/// Convert a stored naive timestamp into epoch millis.
fn parse_millis(value: Option<&String>) -> Option<i64> {
    let raw = value?;
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S%.f") {
        return Some(dt.and_utc().timestamp_millis());
    }
    chrono::NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S")
        .ok()
        .map(|dt| dt.and_utc().timestamp_millis())
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
        user: Option<&str>,
    ) -> anyhow::Result<Option<McpServer>> {
        self.get_mcp_server_detail(namespace, id, name, version, user)
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

    async fn list_mcp_servers(
        &self,
        namespace: &str,
        name: Option<&str>,
        search_type: &str,
        page_no: u32,
        page_size: u32,
        user: Option<&str>,
    ) -> batata_api::model::Page<McpServerBasicInfo> {
        self.list_mcp_servers(namespace, name, search_type, page_no, page_size, user)
            .await
    }

    async fn list_mcp_server_versions(
        &self,
        namespace: &str,
        name: &str,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<batata_api::model::Page<McpServerVersionSummary>> {
        self.list_mcp_server_versions(namespace, name, page_no, page_size)
            .await
    }

    async fn get_mcp_server_version(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<Option<McpServerVersionDetail>> {
        self.get_mcp_server_version(namespace, name, version).await
    }

    async fn create_mcp_server_draft(
        &self,
        namespace: &str,
        registration: &McpServerRegistration,
        overwrite: bool,
    ) -> anyhow::Result<McpServerVersionDetail> {
        self.create_mcp_server_draft(namespace, registration, overwrite)
            .await
    }

    async fn update_mcp_server_draft(
        &self,
        namespace: &str,
        registration: &McpServerRegistration,
    ) -> anyhow::Result<McpServerVersionDetail> {
        self.update_mcp_server_draft(namespace, registration).await
    }

    async fn delete_mcp_server_draft(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<()> {
        self.delete_mcp_server_draft(namespace, name, version).await
    }

    async fn submit_mcp_server_version(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<McpServerVersionDetail> {
        self.submit_mcp_server_version(namespace, name, version).await
    }

    async fn publish_mcp_server_version(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<McpServerVersionDetail> {
        self.publish_mcp_server_version(namespace, name, version).await
    }

    async fn force_publish_mcp_server_version(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<McpServerVersionDetail> {
        self.force_publish_mcp_server_version(namespace, name, version)
            .await
    }

    async fn redraft_mcp_server_version(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<McpServerVersionDetail> {
        self.redraft_mcp_server_version(namespace, name, version).await
    }

    async fn online_mcp_server_version(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<McpServerVersionDetail> {
        self.online_mcp_server_version(namespace, name, version).await
    }

    async fn offline_mcp_server_version(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<McpServerVersionDetail> {
        self.offline_mcp_server_version(namespace, name, version).await
    }

    async fn update_mcp_server_labels(
        &self,
        namespace: &str,
        name: &str,
        labels: std::collections::HashMap<String, String>,
    ) -> anyhow::Result<std::collections::HashMap<String, String>> {
        self.update_mcp_server_labels(namespace, name, labels).await
    }

    async fn update_mcp_server_status(
        &self,
        namespace: &str,
        name: &str,
        enabled: bool,
    ) -> anyhow::Result<()> {
        self.update_mcp_server_status(namespace, name, enabled).await
    }

    async fn update_mcp_server_scope(
        &self,
        namespace: &str,
        name: &str,
        new_scope: &str,
    ) -> anyhow::Result<()> {
        self.update_mcp_server_scope(namespace, name, new_scope).await
    }

    async fn search_mcp_servers(
        &self,
        namespace: &str,
        query: &str,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<batata_api::model::Page<AiResourceSearchHit>> {
        crate::search::query::search(
            self.persistence.as_ref(),
            namespace,
            query,
            &[resource_type::MCP],
            page_no,
            page_size,
        )
        .await
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
