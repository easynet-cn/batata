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
}

impl A2aServerOperationService {
    /// Creates a new `A2aServerOperationService` backed by the given persistence.
    pub fn new(persistence: Arc<dyn PersistenceService>) -> Self {
        Self { persistence }
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
    pub async fn get_agent_card(
        &self,
        namespace: &str,
        agent_name: &str,
        version: Option<&str>,
    ) -> anyhow::Result<Option<RegisteredAgent>> {
        let resource = match self
            .persistence
            .ai_resource_find(namespace, agent_name, resource_type::AGENT)
            .await?
        {
            Some(r) => r,
            None => return Ok(None),
        };

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
    pub async fn list_agents(
        &self,
        namespace: &str,
        agent_name: Option<&str>,
        search_type: &str,
        page_no: u32,
        page_size: u32,
    ) -> anyhow::Result<batata_api::model::Page<AgentCardVersionInfo>> {
        let page_no = page_no.max(1) as u64;
        let page_size_u64 = page_size as u64;

        let accurate = search_type == "accurate";
        let name_filter = agent_name.filter(|n| !n.is_empty());
        let filter = AiResourceListFilter::new().with_name_filter(name_filter, accurate);

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
        let resource = self
            .persistence
            .ai_resource_find(namespace, agent_name, resource_type::AGENT)
            .await?;

        Ok(match resource {
            Some(r) => Self::parse_version_info(&r).version_details,
            None => Vec::new(),
        })
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
    ) -> anyhow::Result<Option<RegisteredAgent>> {
        self.get_agent_card(namespace, agent_name, version).await
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
    ) -> anyhow::Result<batata_api::model::Page<AgentCardVersionInfo>> {
        self.list_agents(namespace, agent_name, search_type, page_no, page_size)
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
