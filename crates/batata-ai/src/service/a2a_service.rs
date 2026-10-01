// A2A Agent Operation Service — AI-resource-backed CRUD for A2A agents
//
// Storage mirrors upstream Nacos: governance metadata lives in `ai_resource`
// (type = `agent`) and each published version is stored as JSON in
// `ai_resource_version.storage`.
//
// This service previously kept agents as config entries (groups `agent` and
// `agent-version`), which is what Nacos did before 3.2.0. That legacy path has
// been replaced rather than kept behind a compatibility mode: Batata is
// unreleased and therefore has no stored data to migrate.

use std::sync::Arc;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use uuid::Uuid;

use batata_persistence::model::{AiResourceInfo, AiResourceListFilter, AiResourceVersionInfo};
use batata_persistence::PersistenceService;

use crate::model::*;
use crate::repository::{meta_status, resource_type, scope, version_status};
use crate::service::endpoint_service::AiEndpointService;
use crate::service::version_lifecycle;

/// Origin recorded for locally registered agents.
const AGENT_DEFAULT_FROM: &str = "local";

/// Extra fields kept on the `ai_resource` row that have no dedicated column.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct A2aResourceExt {
    /// Stable agent id (UUID) assigned at registration.
    id: String,
    /// How the agent was registered.
    registration_type: String,
}

/// AI-resource-backed A2A agent operation service.
pub struct A2aServerOperationService {
    persistence: Arc<dyn PersistenceService>,
    visibility_manager: Arc<batata_visibility::VisibilityPluginManager>,
    /// Endpoint backend used to read live runtime endpoints. Absent when the
    /// server runs without a Naming-backed endpoint service.
    endpoint_service: Option<Arc<AiEndpointService>>,
}

impl A2aServerOperationService {
    /// Creates a new `A2aServerOperationService` backed by the given persistence.
    pub fn new(persistence: Arc<dyn PersistenceService>) -> Self {
        Self::with_visibility(persistence, None, false)
    }

    /// Creates a new `A2aServerOperationService` wired to the visibility plugin.
    pub fn with_visibility(
        persistence: Arc<dyn PersistenceService>,
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
            visibility_manager,
            endpoint_service: None,
        }
    }

    /// Wires the endpoint service, without which runtime endpoint reads are
    /// impossible rather than merely empty.
    pub fn with_endpoint_service(mut self, endpoint_service: Arc<AiEndpointService>) -> Self {
        self.endpoint_service = Some(endpoint_service);
        self
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

    /// Build an `AiResourceListFilter` from visibility query advice.
    ///
    /// The filter is applied by the persistence query, so it shrinks the
    /// reported total rather than just the current page.
    async fn build_list_filter<'a>(
        &self,
        user: Option<&'a str>,
        name_filter: Option<&'a str>,
        accurate: bool,
    ) -> AiResourceListFilter<'a> {
        let identity = user.unwrap_or("");
        let advisor = self
            .visibility_manager
            .advise_with_default(
                identity,
                batata_visibility::ACTION_READ,
                "admin",
                &batata_visibility::VisibilityQueryContext {
                    namespace_id: String::new(),
                    resource_type: resource_type::AGENT.to_string(),
                },
            )
            .await;

        let mut filter = AiResourceListFilter::new().with_name_filter(name_filter, accurate);
        match advisor.base_predicate {
            batata_visibility::BaseVisibilityPredicate::All => {}
            batata_visibility::BaseVisibilityPredicate::Public => {
                filter = filter.with_scope(Some(batata_visibility::SCOPE_PUBLIC));
            }
            batata_visibility::BaseVisibilityPredicate::Owner => {
                if !identity.is_empty() {
                    filter = filter.with_owner(Some(identity), false);
                }
            }
            batata_visibility::BaseVisibilityPredicate::PublicAndOwner => {
                if !identity.is_empty() {
                    filter = filter.with_owner(Some(identity), true);
                } else {
                    filter = filter.with_scope(Some(batata_visibility::SCOPE_PUBLIC));
                }
            }
        }
        filter
    }

    // =========================================================================
    // Helpers
    // =========================================================================

    /// Parse the version index stored in `ai_resource.version_info`.
    fn parse_version_info(resource: &AiResourceInfo) -> AgentCardVersionInfo {
        match resource.version_info {
            Some(ref json) => match serde_json::from_str::<AgentCardVersionInfo>(json) {
                Ok(vi) => vi,
                Err(e) => {
                    warn!(
                        resource = %resource.name,
                        error = %e,
                        "Failed to parse agent version info"
                    );
                    Self::empty_version_info(resource)
                }
            },
            None => Self::empty_version_info(resource),
        }
    }

    /// Build a placeholder version index for a resource with no stored index.
    fn empty_version_info(resource: &AiResourceInfo) -> AgentCardVersionInfo {
        AgentCardVersionInfo {
            id: String::new(),
            name: resource.name.clone(),
            latest_published_version: String::new(),
            registration_type: String::new(),
            version_details: Vec::new(),
        }
    }

    /// Serialize the resource extension fields.
    fn ext_json(id: &str, registration_type: &str) -> anyhow::Result<String> {
        Ok(serde_json::to_string(&A2aResourceExt {
            id: id.to_string(),
            registration_type: registration_type.to_string(),
        })?)
    }

    /// Read back the resource extension fields.
    fn parse_ext(resource: &AiResourceInfo) -> Option<A2aResourceExt> {
        resource
            .ext
            .as_ref()
            .and_then(|json| serde_json::from_str::<A2aResourceExt>(json).ok())
    }

    /// Build the per-version detail payload stored in `ai_resource_version.storage`.
    fn build_detail(id: &str, card: &AgentCard, registration_type: &str) -> AgentCardDetailInfo {
        AgentCardDetailInfo {
            id: id.to_string(),
            name: card.name.clone(),
            version: card.version.clone(),
            registration_type: registration_type.to_string(),
            description: card.description.clone(),
            url: card.url.clone(),
            capabilities: card.capabilities.clone(),
            skills: card.skills.clone(),
            provider: String::new(),
            agent_card: Some(card.clone()),
        }
    }

    /// Advance the meta_version and write the version index back.
    async fn save_version_info(
        &self,
        namespace: &str,
        name: &str,
        resource: &AiResourceInfo,
        version_info: &AgentCardVersionInfo,
    ) -> anyhow::Result<()> {
        self.persistence
            .ai_resource_update_version_info_cas(
                namespace,
                name,
                resource_type::AGENT,
                resource.meta_version,
                &serde_json::to_string(version_info)?,
                resource.meta_version + 1,
            )
            .await?;
        Ok(())
    }

    // =========================================================================
    // Public operations
    // =========================================================================

    /// Register a new agent
    pub async fn register_agent(
        &self,
        card: &AgentCard,
        namespace: &str,
        registration_type: &str,
    ) -> anyhow::Result<String> {
        let name = &card.name;
        let version = &card.version;

        let existing = self
            .persistence
            .ai_resource_find(namespace, name, resource_type::AGENT)
            .await?;

        if existing.is_some() {
            anyhow::bail!(
                "Agent '{}' already exists in namespace '{}'",
                name,
                namespace
            );
        }

        let id = Uuid::new_v4().to_string();
        let now = Utc::now().naive_utc().to_string();

        let version_info = AgentCardVersionInfo {
            id: id.clone(),
            name: name.clone(),
            latest_published_version: version.clone(),
            registration_type: registration_type.to_string(),
            version_details: vec![VersionDetail {
                version: version.clone(),
                release_date: Utc::now().to_rfc3339(),
                is_latest: true,
            }],
        };

        let detail_info = Self::build_detail(&id, card, registration_type);

        let info = AiResourceInfo {
            id: 0,
            name: name.clone(),
            resource_type: resource_type::AGENT.to_string(),
            description: Some(card.description.clone()),
            status: Some(meta_status::ENABLE.to_string()),
            namespace_id: namespace.to_string(),
            biz_tags: None,
            ext: Some(Self::ext_json(&id, registration_type)?),
            from: AGENT_DEFAULT_FROM.to_string(),
            version_info: Some(serde_json::to_string(&version_info)?),
            meta_version: 1,
            scope: scope::PRIVATE.to_string(),
            owner: String::new(),
            download_count: 0,
            gmt_create: Some(now.clone()),
            gmt_modified: Some(now.clone()),
        };
        self.persistence.ai_resource_insert(&info).await?;

        self.persistence
            .ai_resource_version_insert(&AiResourceVersionInfo {
                id: 0,
                resource_type: resource_type::AGENT.to_string(),
                author: None,
                name: name.clone(),
                description: Some(card.description.clone()),
                status: version_status::ONLINE.to_string(),
                version: version.clone(),
                namespace_id: namespace.to_string(),
                storage: Some(serde_json::to_string(&detail_info)?),
                publish_pipeline_info: None,
                download_count: 0,
                gmt_create: Some(now.clone()),
                gmt_modified: Some(now),
            })
            .await?;

        info!(
            agent_name = %name,
            agent_id = %id,
            namespace = %namespace,
            "A2A agent registered (ai_resource-backed)"
        );

        Ok(id)
    }

    /// Get agent card by name and optional version
    ///
    /// `user` is the caller identity; an agent the caller may not read is
    /// reported as absent rather than leaking its contents.
    pub async fn get_agent_card(
        &self,
        namespace: &str,
        agent_name: &str,
        version: Option<&str>,
        user: Option<&str>,
    ) -> anyhow::Result<Option<RegisteredAgent>> {
        let resource = match self
            .persistence
            .ai_resource_find(namespace, agent_name, resource_type::AGENT)
            .await?
        {
            Some(r) => r,
            None => return Ok(None),
        };

        self.check_visibility(user, batata_visibility::ACTION_READ, &resource)
            .await?;

        let version_info = Self::parse_version_info(&resource);
        let target_version = version
            .filter(|v| !v.is_empty())
            .unwrap_or(&version_info.latest_published_version)
            .to_string();

        let stored = match self
            .persistence
            .ai_resource_version_find(
                namespace,
                agent_name,
                resource_type::AGENT,
                &target_version,
            )
            .await?
        {
            Some(v) => v,
            None => return Ok(None),
        };

        let detail: AgentCardDetailInfo = match stored.storage {
            Some(ref json) => serde_json::from_str(json)?,
            None => return Ok(None),
        };

        let now = Utc::now().timestamp_millis();

        let card = match detail.agent_card {
            Some(card) => card,
            None => AgentCard {
                name: detail.name.clone(),
                display_name: detail.name.clone(),
                description: detail.description.clone(),
                version: detail.version.clone(),
                url: detail.url.clone(),
                protocol_version: "1.0".to_string(),
                capabilities: detail.capabilities.clone(),
                skills: detail.skills.clone(),
                default_input_modes: vec![],
                default_output_modes: vec![],
                preferred_transport: None,
                provider: None,
                documentation_url: None,
                icon_url: None,
                supports_authenticated_extended_card: None,
                metadata: Default::default(),
                tags: vec![],
                ..Default::default()
            },
        };

        Ok(Some(RegisteredAgent {
            id: detail.id,
            card,
            namespace: namespace.to_string(),
            health_status: HealthStatus::Unknown,
            registered_at: now,
            last_health_check: None,
            updated_at: now,
        }))
    }

    /// Update an existing agent
    pub async fn update_agent_card(
        &self,
        card: &AgentCard,
        namespace: &str,
        registration_type: &str,
    ) -> anyhow::Result<()> {
        let name = &card.name;
        let version = &card.version;

        let resource = self
            .persistence
            .ai_resource_find(namespace, name, resource_type::AGENT)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!("Agent '{}' not found in namespace '{}'", name, namespace)
            })?;

        let mut version_info = Self::parse_version_info(&resource);
        let now = Utc::now();
        let release_date = now.to_rfc3339();

        if let Some(idx) = version_info
            .version_details
            .iter()
            .position(|v| v.version == *version)
        {
            version_info.version_details[idx].release_date = release_date;
            version_info.version_details[idx].is_latest = true;
        } else {
            for v in &mut version_info.version_details {
                v.is_latest = false;
            }
            version_info.version_details.push(VersionDetail {
                version: version.clone(),
                release_date,
                is_latest: true,
            });
        }

        version_info.latest_published_version = version.clone();
        self.save_version_info(namespace, name, &resource, &version_info)
            .await?;

        // The stored index owns the id; fall back to the extension copy if the
        // index could not be parsed.
        let id = if version_info.id.is_empty() {
            Self::parse_ext(&resource)
                .map(|e| e.id)
                .unwrap_or_default()
        } else {
            version_info.id.clone()
        };
        let detail_info = Self::build_detail(&id, card, registration_type);
        let storage_json = serde_json::to_string(&detail_info)?;

        let exists = self
            .persistence
            .ai_resource_version_find(namespace, name, resource_type::AGENT, version)
            .await?
            .is_some();

        if exists {
            self.persistence
                .ai_resource_version_update_storage(
                    namespace,
                    name,
                    resource_type::AGENT,
                    version,
                    &storage_json,
                    Some(card.description.as_str()),
                )
                .await?;
        } else {
            let now_str = now.naive_utc().to_string();
            self.persistence
                .ai_resource_version_insert(&AiResourceVersionInfo {
                    id: 0,
                    resource_type: resource_type::AGENT.to_string(),
                    author: None,
                    name: name.clone(),
                    description: Some(card.description.clone()),
                    status: version_status::ONLINE.to_string(),
                    version: version.clone(),
                    namespace_id: namespace.to_string(),
                    storage: Some(storage_json),
                    publish_pipeline_info: None,
                    download_count: 0,
                    gmt_create: Some(now_str.clone()),
                    gmt_modified: Some(now_str),
                })
                .await?;
        }

        info!(
            agent_name = %name,
            namespace = %namespace,
            version = %version,
            "A2A agent updated (ai_resource-backed)"
        );

        Ok(())
    }

    /// Delete an agent (all versions or a specific version)
    pub async fn delete_agent(
        &self,
        namespace: &str,
        agent_name: &str,
        version: Option<&str>,
    ) -> anyhow::Result<()> {
        let resource = match self
            .persistence
            .ai_resource_find(namespace, agent_name, resource_type::AGENT)
            .await?
        {
            Some(r) => r,
            None => {
                info!(
                    agent_name = %agent_name,
                    namespace = %namespace,
                    "A2A agent not found, nothing to delete"
                );
                return Ok(());
            }
        };

        match version.filter(|v| !v.is_empty()) {
            Some(version) => {
                self.persistence
                    .ai_resource_version_delete(namespace, agent_name, resource_type::AGENT, version)
                    .await?;

                let mut version_info = Self::parse_version_info(&resource);
                version_info
                    .version_details
                    .retain(|v| v.version != version);

                if version_info.version_details.is_empty() {
                    self.persistence
                        .ai_resource_delete(namespace, agent_name, resource_type::AGENT)
                        .await?;
                } else {
                    if let Some(last) = version_info.version_details.last_mut() {
                        last.is_latest = true;
                        version_info.latest_published_version = last.version.clone();
                    }
                    self.save_version_info(namespace, agent_name, &resource, &version_info)
                        .await?;
                }
            }
            None => {
                self.persistence
                    .ai_resource_version_delete_all(namespace, agent_name, resource_type::AGENT)
                    .await?;
                self.persistence
                    .ai_resource_delete(namespace, agent_name, resource_type::AGENT)
                    .await?;
            }
        }

        info!(
            agent_name = %agent_name,
            namespace = %namespace,
            "A2A agent deleted (ai_resource-backed)"
        );

        Ok(())
    }

    /// List agents with pagination and search
    ///
    /// Returns `Page<AgentCardVersionInfo>` to match Nacos A2aServerOperationService.listAgents()
    /// which returns `Page<AgentCardVersionInfo>` (not full agent details).
    /// List agents visible to `user`.
    pub async fn list_agents(
        &self,
        namespace: &str,
        agent_name: Option<&str>,
        search_type: &str,
        page_no: u32,
        page_size: u32,
        user: Option<&str>,
    ) -> anyhow::Result<batata_api::model::Page<AgentCardVersionInfo>> {
        let page_no = page_no.max(1) as u64;
        let page_size_u64 = page_size as u64;

        let accurate = search_type == "accurate";
        let name_filter = agent_name.filter(|n| !n.is_empty());
        let filter = self
            .build_list_filter(user, name_filter, accurate)
            .await;

        let page = self
            .persistence
            .ai_resource_list(
                namespace,
                resource_type::AGENT,
                &filter,
                page_no,
                page_size_u64,
            )
            .await?;

        let version_infos: Vec<AgentCardVersionInfo> = page
            .page_items
            .iter()
            .map(Self::parse_version_info)
            .collect();

        Ok(batata_api::model::Page::new(
            page.total_count,
            page_no,
            page_size_u64,
            version_infos,
        ))
    }

    /// List versions for a specific agent
    pub async fn list_versions(
        &self,
        namespace: &str,
        agent_name: &str,
    ) -> anyhow::Result<Vec<VersionDetail>> {
        // Derived from the version rows rather than the denormalised index in
        // `version_info`: a draft created through `/draft` only ever lands in
        // the version table, so reading the index would hide it.
        let resource = match self
            .persistence
            .ai_resource_find(namespace, agent_name, resource_type::AGENT)
            .await?
        {
            Some(resource) => resource,
            None => return Ok(Vec::new()),
        };
        let latest = Self::parse_version_info(&resource).latest_published_version;
        let rows = self
            .persistence
            .ai_resource_version_list(namespace, agent_name, resource_type::AGENT)
            .await?;

        let mut versions: Vec<VersionDetail> = rows
            .iter()
            .map(|row| VersionDetail {
                version: row.version.clone(),
                release_date: String::new(),
                is_latest: !latest.is_empty() && latest == row.version,
            })
            .collect();
        versions.sort_by(|a, b| {
            batata_common::model::ai::skill::compare_versions(&a.version, &b.version).reverse()
        });

        Ok(versions)
    }

    // ========================================================================
    // Version lifecycle
    //
    // The state machine is shared with every other AI resource type
    // (`version_lifecycle`); only the resource type differs.
    // ========================================================================

    /// Load one version as the detail the lifecycle endpoints answer with.
    async fn version_row(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<AgentVersionDetail> {
        let row = version_lifecycle::find_version(
            self.persistence.as_ref(),
            namespace,
            name,
            resource_type::AGENT,
            version,
        )
        .await?;
        Ok(AgentVersionDetail {
            namespace_id: row.namespace_id,
            name: row.name,
            version: row.version,
            status: row.status,
            description: row.description,
        })
    }

    /// Submit a draft version for review (draft → reviewing).
    pub async fn submit_agent_version(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<AgentVersionDetail> {
        version_lifecycle::submit(
            self.persistence.as_ref(),
            namespace,
            name,
            resource_type::AGENT,
            version,
        )
        .await?;
        self.version_row(namespace, name, version).await
    }

    /// Publish a version that passed review (reviewing / reviewed → online).
    pub async fn publish_agent_version(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<AgentVersionDetail> {
        version_lifecycle::publish(
            self.persistence.as_ref(),
            namespace,
            name,
            resource_type::AGENT,
            version,
        )
        .await?;
        self.version_row(namespace, name, version).await
    }

    /// Publish a version bypassing the review gate.
    pub async fn force_publish_agent_version(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<AgentVersionDetail> {
        version_lifecycle::force_publish(
            self.persistence.as_ref(),
            namespace,
            name,
            resource_type::AGENT,
            version,
        )
        .await?;
        self.version_row(namespace, name, version).await
    }

    /// Move a version back to draft so it can be edited again.
    pub async fn redraft_agent_version(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<AgentVersionDetail> {
        version_lifecycle::redraft(
            self.persistence.as_ref(),
            namespace,
            name,
            resource_type::AGENT,
            version,
        )
        .await?;
        self.version_row(namespace, name, version).await
    }

    /// Bring an offline version back online.
    pub async fn online_agent_version(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<AgentVersionDetail> {
        version_lifecycle::online(
            self.persistence.as_ref(),
            namespace,
            name,
            resource_type::AGENT,
            version,
        )
        .await?;
        self.version_row(namespace, name, version).await
    }

    /// Take an online version offline.
    pub async fn offline_agent_version(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<AgentVersionDetail> {
        version_lifecycle::offline(
            self.persistence.as_ref(),
            namespace,
            name,
            resource_type::AGENT,
            version,
        )
        .await?;
        self.version_row(namespace, name, version).await
    }

    /// Replace the custom version labels, preserving the server-managed
    /// `latest` label.
    pub async fn update_agent_labels(
        &self,
        namespace: &str,
        name: &str,
        labels: std::collections::HashMap<String, String>,
    ) -> anyhow::Result<std::collections::HashMap<String, String>> {
        version_lifecycle::update_labels(
            self.persistence.as_ref(),
            namespace,
            name,
            resource_type::AGENT,
            labels,
        )
        .await
    }

    /// Change the visibility scope (`PUBLIC` / `PRIVATE`).
    pub async fn update_agent_scope(
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
        version_lifecycle::set_scope(
            self.persistence.as_ref(),
            namespace,
            name,
            resource_type::AGENT,
            new_scope,
        )
        .await
    }

    // ========================================================================
    // Drafts
    // ========================================================================

    /// The resource-level version index plus the extension that carries the
    /// agent id, for a resource that must already exist.
    async fn existing_resource(
        &self,
        namespace: &str,
        name: &str,
    ) -> anyhow::Result<(AiResourceInfo, ResourceVersionInfo, String, String)> {
        let resource = version_lifecycle::find_resource(
            self.persistence.as_ref(),
            namespace,
            name,
            resource_type::AGENT,
        )
        .await?;
        let version_info = version_lifecycle::parse_version_info(&resource);
        let ext = Self::parse_ext(&resource).unwrap_or(A2aResourceExt {
            id: String::new(),
            registration_type: String::new(),
        });
        Ok((resource, version_info, ext.id, ext.registration_type))
    }

    /// Create a draft version for an agent that already exists.
    ///
    /// Mirrors `create_mcp_server_draft`: at most one draft is being edited at
    /// a time, and `overwrite` is required to replace it.
    pub async fn create_agent_draft(
        &self,
        namespace: &str,
        card: &AgentCard,
        overwrite: bool,
    ) -> anyhow::Result<AgentVersionDetail> {
        let name = &card.name;
        let version = &card.version;

        let (resource, mut version_info, id, registration_type) =
            self.existing_resource(namespace, name).await?;

        if let Some(ref editing) = version_info.editing_version {
            if !overwrite {
                anyhow::bail!(
                    "Agent '{}' already has an editing version '{}', set overwrite=true",
                    name,
                    editing
                );
            }
            self.persistence
                .ai_resource_version_delete(namespace, name, resource_type::AGENT, editing)
                .await?;
        }

        let detail = Self::build_detail(&id, card, &registration_type);
        let now = Utc::now().naive_utc().to_string();
        self.persistence
            .ai_resource_version_insert(&AiResourceVersionInfo {
                id: 0,
                resource_type: resource_type::AGENT.to_string(),
                author: None,
                name: name.clone(),
                description: Some(card.description.clone()),
                status: version_status::DRAFT.to_string(),
                version: version.clone(),
                namespace_id: namespace.to_string(),
                storage: Some(serde_json::to_string(&detail)?),
                publish_pipeline_info: None,
                download_count: 0,
                gmt_create: Some(now.clone()),
                gmt_modified: Some(now),
            })
            .await?;

        version_info.editing_version = Some(version.clone());
        version_lifecycle::save_version_info(
            self.persistence.as_ref(),
            namespace,
            name,
            resource_type::AGENT,
            &resource,
            &version_info,
        )
        .await?;

        self.version_row(namespace, name, version).await
    }

    /// Update the draft version currently being edited.
    pub async fn update_agent_draft(
        &self,
        namespace: &str,
        card: &AgentCard,
    ) -> anyhow::Result<AgentVersionDetail> {
        let name = &card.name;
        let version = &card.version;

        let (_resource, version_info, id, registration_type) =
            self.existing_resource(namespace, name).await?;

        let editing = version_info.editing_version.clone().ok_or_else(|| {
            anyhow::anyhow!("Agent '{}' has no editing version", name)
        })?;
        if editing != *version {
            anyhow::bail!(
                "Agent '{}' editing version is '{}', not '{}'",
                name,
                editing,
                version
            );
        }

        let detail = Self::build_detail(&id, card, &registration_type);
        self.persistence
            .ai_resource_version_update_storage(
                namespace,
                name,
                resource_type::AGENT,
                version,
                &serde_json::to_string(&detail)?,
                Some(card.description.as_str()),
            )
            .await?;

        self.version_row(namespace, name, version).await
    }

    /// Delete a draft version.
    pub async fn delete_agent_draft(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<()> {
        let (resource, mut version_info, _, _) = self.existing_resource(namespace, name).await?;

        self.persistence
            .ai_resource_version_delete(namespace, name, resource_type::AGENT, version)
            .await?;

        if version_info.editing_version.as_deref() == Some(version) {
            version_info.editing_version = None;
            version_lifecycle::save_version_info(
                self.persistence.as_ref(),
                namespace,
                name,
                resource_type::AGENT,
                &resource,
                &version_info,
            )
            .await?;
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl super::traits::A2aAgentService for A2aServerOperationService {
    async fn register_agent(
        &self,
        card: &AgentCard,
        namespace: &str,
        registration_type: &str,
    ) -> anyhow::Result<String> {
        self.register_agent(card, namespace, registration_type)
            .await
    }

    async fn get_agent_card(
        &self,
        namespace: &str,
        agent_name: &str,
        version: Option<&str>,
        user: Option<&str>,
    ) -> anyhow::Result<Option<RegisteredAgent>> {
        self.get_agent_card(namespace, agent_name, version, user)
            .await
    }

    async fn update_agent_card(
        &self,
        card: &AgentCard,
        namespace: &str,
        registration_type: &str,
    ) -> anyhow::Result<()> {
        self.update_agent_card(card, namespace, registration_type)
            .await
    }

    async fn delete_agent(
        &self,
        namespace: &str,
        agent_name: &str,
        version: Option<&str>,
    ) -> anyhow::Result<()> {
        self.delete_agent(namespace, agent_name, version).await
    }

    async fn list_agents(
        &self,
        namespace: &str,
        agent_name: Option<&str>,
        search_type: &str,
        page_no: u32,
        page_size: u32,
        user: Option<&str>,
    ) -> anyhow::Result<batata_api::model::Page<AgentCardVersionInfo>> {
        self.list_agents(namespace, agent_name, search_type, page_no, page_size, user)
            .await
    }

    async fn list_versions(
        &self,
        namespace: &str,
        agent_name: &str,
    ) -> anyhow::Result<Vec<VersionDetail>> {
        self.list_versions(namespace, agent_name).await
    }

    async fn find_by_skill(&self, _skill: &str) -> anyhow::Result<Vec<RegisteredAgent>> {
        // Skill-based search is not implemented for the AI-resource-backed
        // service yet.
        Ok(vec![])
    }

    async fn batch_register(
        &self,
        request: batata_common::model::ai::a2a::BatchAgentRegistrationRequest,
    ) -> anyhow::Result<batata_common::model::ai::a2a::BatchRegistrationResponse> {
        let mut success_count = 0u32;
        let mut errors = Vec::new();
        for agent_req in request.agents {
            let name = agent_req.card.name.clone();
            let namespace = &agent_req.namespace;
            match self
                .register_agent(&agent_req.card, namespace, "direct")
                .await
            {
                Ok(_) => success_count += 1,
                Err(e) => errors.push(batata_common::model::ai::a2a::RegistrationError {
                    name,
                    error: e.to_string(),
                }),
            }
        }
        let failed_count = errors.len() as u32;
        Ok(batata_common::model::ai::a2a::BatchRegistrationResponse {
            success_count,
            failed_count,
            errors,
        })
    }

    async fn create_agent_draft(
        &self,
        namespace: &str,
        card: &AgentCard,
        overwrite: bool,
    ) -> anyhow::Result<AgentVersionDetail> {
        self.create_agent_draft(namespace, card, overwrite).await
    }

    async fn update_agent_draft(
        &self,
        namespace: &str,
        card: &AgentCard,
    ) -> anyhow::Result<AgentVersionDetail> {
        self.update_agent_draft(namespace, card).await
    }

    async fn delete_agent_draft(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<()> {
        self.delete_agent_draft(namespace, name, version).await
    }

    async fn submit_agent_version(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<AgentVersionDetail> {
        self.submit_agent_version(namespace, name, version).await
    }

    async fn publish_agent_version(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<AgentVersionDetail> {
        self.publish_agent_version(namespace, name, version).await
    }

    async fn force_publish_agent_version(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<AgentVersionDetail> {
        self.force_publish_agent_version(namespace, name, version)
            .await
    }

    async fn redraft_agent_version(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<AgentVersionDetail> {
        self.redraft_agent_version(namespace, name, version).await
    }

    async fn online_agent_version(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<AgentVersionDetail> {
        self.online_agent_version(namespace, name, version).await
    }

    async fn offline_agent_version(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
    ) -> anyhow::Result<AgentVersionDetail> {
        self.offline_agent_version(namespace, name, version).await
    }

    async fn update_agent_labels(
        &self,
        namespace: &str,
        name: &str,
        labels: std::collections::HashMap<String, String>,
    ) -> anyhow::Result<std::collections::HashMap<String, String>> {
        self.update_agent_labels(namespace, name, labels).await
    }

    async fn update_agent_scope(
        &self,
        namespace: &str,
        name: &str,
        new_scope: &str,
    ) -> anyhow::Result<()> {
        self.update_agent_scope(namespace, name, new_scope).await
    }

    async fn stats(&self) -> anyhow::Result<batata_common::model::ai::a2a::AgentRegistryStats> {
        // The AI-resource-backed service does not track registry-wide counters.
        Ok(batata_common::model::ai::a2a::AgentRegistryStats {
            total_agents: 0,
            healthy_agents: 0,
            unhealthy_agents: 0,
            by_namespace: std::collections::HashMap::new(),
            by_skill: std::collections::HashMap::new(),
        })
    }

    async fn get_runtime_endpoints(
        &self,
        namespace: &str,
        agent_name: &str,
        protocol: &str,
        version: &str,
    ) -> anyhow::Result<batata_common::model::ai::a2a::ConsoleRuntimeEndpointView> {
        let endpoint_service = self.endpoint_service.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "runtime endpoints are unavailable: no endpoint service is configured"
            )
        })?;

        let endpoints = endpoint_service.get_agent_endpoints(namespace, agent_name, version);

        Ok(batata_common::model::ai::a2a::ConsoleRuntimeEndpointView {
            runtime_endpoint_snapshot: batata_common::model::ai::a2a::RuntimeEndpointSnapshot {
                namespace_id: namespace.to_string(),
                agent_name: agent_name.to_string(),
                version: version.to_string(),
                call_interface: batata_common::model::ai::a2a::AgentCallInterface {
                    protocol: protocol.to_string(),
                    endpoints: endpoints
                        .iter()
                        .map(|e| batata_common::model::ai::a2a::RuntimeEndpoint {
                            address: e.address.clone(),
                            port: e.port,
                            healthy: e.healthy,
                        })
                        .collect(),
                },
            },
            naming_service_ref: batata_common::model::ai::a2a::NamingServiceRef {
                namespace_id: namespace.to_string(),
                group_name: crate::service::constants::AGENT_ENDPOINT_GROUP.to_string(),
                service_name: crate::service::constants::a2a_service_name(agent_name, version),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_card_version_info_serialization() {
        let info = AgentCardVersionInfo {
            id: "test-id".to_string(),
            name: "test-agent".to_string(),
            latest_published_version: "1.0.0".to_string(),
            registration_type: "manual".to_string(),
            version_details: vec![VersionDetail {
                version: "1.0.0".to_string(),
                release_date: "2024-01-01T00:00:00Z".to_string(),
                is_latest: true,
            }],
        };

        let json = serde_json::to_string(&info).unwrap();
        let parsed: AgentCardVersionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "test-id");
        assert_eq!(parsed.version_details.len(), 1);
    }

    #[test]
    fn test_agent_card_detail_info_serialization() {
        let info = AgentCardDetailInfo {
            id: "test-id".to_string(),
            name: "test-agent".to_string(),
            version: "1.0.0".to_string(),
            registration_type: "manual".to_string(),
            description: "Test agent".to_string(),
            url: "http://localhost:8080".to_string(),
            capabilities: AgentCapabilities::default(),
            skills: vec![],
            provider: String::new(),
            agent_card: None,
        };

        let json = serde_json::to_string(&info).unwrap();
        let parsed: AgentCardDetailInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "test-id");
    }
}
