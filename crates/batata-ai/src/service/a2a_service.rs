// A2A Agent Operation Service - Config-backed CRUD for A2A agents
// Replaces in-memory AgentRegistry with persistent storage

use std::sync::Arc;

use chrono::Utc;
use tracing::{info, warn};
use uuid::Uuid;

use batata_persistence::PersistenceService;

use super::constants::*;
use crate::model::*;

/// Config-backed A2A agent operation service
pub struct A2aServerOperationService {
    persistence: Arc<dyn PersistenceService>,
}

impl A2aServerOperationService {
    pub fn new(persistence: Arc<dyn PersistenceService>) -> Self {
        Self { persistence }
    }

    /// Register a new agent
    pub async fn register_agent(
        &self,
        card: &AgentCard,
        namespace: &str,
        registration_type: &str,
    ) -> anyhow::Result<String> {
        let name = &card.name;
        let version = &card.version;

        // Check for duplicate
        let encoded_name = encode_agent_name(name);
        let existing = self
            .query_config(namespace, AGENT_GROUP, &encoded_name)
            .await?;

        if existing.is_some() {
            anyhow::bail!(
                "Agent '{}' already exists in namespace '{}'",
                name,
                namespace
            );
        }

        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        // Build version info (index entry)
        let version_info = AgentCardVersionInfo {
            id: id.clone(),
            name: name.clone(),
            latest_published_version: version.clone(),
            registration_type: registration_type.to_string(),
            version_details: vec![VersionDetail {
                version: version.clone(),
                release_date: now.clone(),
                is_latest: true,
            }],
        };

        // Build per-version detail
        let detail_info = AgentCardDetailInfo {
            id: id.clone(),
            name: name.clone(),
            version: version.clone(),
            registration_type: registration_type.to_string(),
            description: card.description.clone(),
            url: card.endpoint.clone(),
            capabilities: card.capabilities.clone(),
            skills: card.skills.clone(),
            provider: String::new(),
            agent_card: Some(card.clone()),
        };

        // Publish version info config
        let version_content = serde_json::to_string(&version_info)?;
        self.publish_config(
            namespace,
            AGENT_GROUP,
            &encoded_name,
            &version_content,
            &a2a_tags(name),
            &format!("A2A agent version info: {}", name),
        )
        .await?;

        // Publish detail config
        let detail_data_id = format!("{}-{}", encoded_name, version);
        let detail_content = serde_json::to_string(&detail_info)?;
        self.publish_config(
            namespace,
            AGENT_VERSION_GROUP,
            &detail_data_id,
            &detail_content,
            &a2a_tags(name),
            &format!("A2A agent detail: {}:{}", name, version),
        )
        .await?;

        info!(
            agent_name = %name,
            agent_id = %id,
            namespace = %namespace,
            "A2A agent registered (config-backed)"
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
        let encoded_name = encode_agent_name(agent_name);

        // Fetch version info
        let version_config = self
            .query_config(namespace, AGENT_GROUP, &encoded_name)
            .await?;

        let version_config = match version_config {
            Some(c) => c,
            None => return Ok(None),
        };

        let version_info: AgentCardVersionInfo = serde_json::from_str(&version_config)?;

        // Resolve version
        let target_version = version
            .filter(|v| !v.is_empty())
            .unwrap_or(&version_info.latest_published_version);

        // Fetch detail
        let detail_data_id = format!("{}-{}", encoded_name, target_version);
        let detail_config = self
            .query_config(namespace, AGENT_VERSION_GROUP, &detail_data_id)
            .await?;

        let detail_config = match detail_config {
            Some(c) => c,
            None => return Ok(None),
        };

        let detail_info: AgentCardDetailInfo = serde_json::from_str(&detail_config)?;

        // Build RegisteredAgent from storage data
        let now = Utc::now().timestamp_millis();

        let agent = if let Some(ref card) = detail_info.agent_card {
            RegisteredAgent {
                id: detail_info.id.clone(),
                card: card.clone(),
                namespace: namespace.to_string(),
                health_status: HealthStatus::Unknown,
                registered_at: now,
                last_health_check: None,
                updated_at: now,
            }
        } else {
            let card = AgentCard {
                name: detail_info.name.clone(),
                display_name: detail_info.name.clone(),
                description: detail_info.description.clone(),
                version: detail_info.version.clone(),
                endpoint: detail_info.url.clone(),
                protocol_version: "1.0".to_string(),
                capabilities: detail_info.capabilities.clone(),
                skills: detail_info.skills.clone(),
                input_modes: vec![],
                output_modes: vec![],
                authentication: None,
                rate_limits: None,
                metadata: Default::default(),
                tags: vec![],
            };
            RegisteredAgent {
                id: detail_info.id.clone(),
                card,
                namespace: namespace.to_string(),
                health_status: HealthStatus::Unknown,
                registered_at: now,
                last_health_check: None,
                updated_at: now,
            }
        };

        Ok(Some(agent))
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
        let encoded_name = encode_agent_name(name);

        // Fetch existing version info
        let existing = self
            .query_config(namespace, AGENT_GROUP, &encoded_name)
            .await?;

        let mut version_info: AgentCardVersionInfo = match existing {
            Some(content) => serde_json::from_str(&content)?,
            None => {
                anyhow::bail!("Agent '{}' not found in namespace '{}'", name, namespace);
            }
        };

        let now = Utc::now().to_rfc3339();

        // Update or add version
        let existing_idx = version_info
            .version_details
            .iter()
            .position(|v| v.version == *version);

        if let Some(idx) = existing_idx {
            version_info.version_details[idx].release_date = now.clone();
            version_info.version_details[idx].is_latest = true;
        } else {
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

        // Publish updated version info
        let version_content = serde_json::to_string(&version_info)?;
        self.publish_config(
            namespace,
            AGENT_GROUP,
            &encoded_name,
            &version_content,
            &a2a_tags(name),
            &format!("A2A agent version info updated: {}", name),
        )
        .await?;

        // Build and publish updated detail
        let detail_info = AgentCardDetailInfo {
            id: version_info.id.clone(),
            name: name.clone(),
            version: version.clone(),
            registration_type: registration_type.to_string(),
            description: card.description.clone(),
            url: card.endpoint.clone(),
            capabilities: card.capabilities.clone(),
            skills: card.skills.clone(),
            provider: String::new(),
            agent_card: Some(card.clone()),
        };

        let detail_data_id = format!("{}-{}", encoded_name, version);
        let detail_content = serde_json::to_string(&detail_info)?;
        self.publish_config(
            namespace,
            AGENT_VERSION_GROUP,
            &detail_data_id,
            &detail_content,
            &a2a_tags(name),
            &format!("A2A agent detail updated: {}:{}", name, version),
        )
        .await?;

        info!(
            agent_name = %name,
            namespace = %namespace,
            version = %version,
            "A2A agent updated (config-backed)"
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
        let encoded_name = encode_agent_name(agent_name);

        if let Some(version) = version.filter(|v| !v.is_empty()) {
            // Delete specific version detail
            let detail_data_id = format!("{}-{}", encoded_name, version);
            self.delete_config(namespace, AGENT_VERSION_GROUP, &detail_data_id)
                .await?;

            // Update version info
            if let Some(content) = self
                .query_config(namespace, AGENT_GROUP, &encoded_name)
                .await?
            {
                let mut version_info: AgentCardVersionInfo = serde_json::from_str(&content)?;
                version_info
                    .version_details
                    .retain(|v| v.version != version);

                if version_info.version_details.is_empty() {
                    self.delete_config(namespace, AGENT_GROUP, &encoded_name)
                        .await?;
                } else {
                    if let Some(last) = version_info.version_details.last_mut() {
                        last.is_latest = true;
                        version_info.latest_published_version = last.version.clone();
                    }
                    let content = serde_json::to_string(&version_info)?;
                    self.publish_config(
                        namespace,
                        AGENT_GROUP,
                        &encoded_name,
                        &content,
                        &a2a_tags(agent_name),
                        &format!("A2A agent version removed: {}:{}", agent_name, version),
                    )
                    .await?;
                }
            }
        } else {
            // Delete all versions
            if let Some(content) = self
                .query_config(namespace, AGENT_GROUP, &encoded_name)
                .await?
            {
                let version_info: AgentCardVersionInfo = serde_json::from_str(&content)?;
                for vd in &version_info.version_details {
                    let detail_data_id = format!("{}-{}", encoded_name, vd.version);
                    self.delete_config(namespace, AGENT_VERSION_GROUP, &detail_data_id)
                        .await
                        .ok();
                }
            }

            // Delete version info
            self.delete_config(namespace, AGENT_GROUP, &encoded_name)
                .await?;
        }

        info!(
            agent_name = %agent_name,
            namespace = %namespace,
            "A2A agent deleted (config-backed)"
        );

        Ok(())
    }

    /// List agents with pagination and search
    pub async fn list_agents(
        &self,
        namespace: &str,
        agent_name: Option<&str>,
        search_type: &str,
        page_no: u32,
        page_size: u32,
    ) -> anyhow::Result<AgentListResponse> {
        let page_no = page_no.max(1) as u64;
        let page_size_u64 = page_size as u64;

        // Search by tags to find agent entries
        let data_id_filter = match (agent_name, search_type) {
            (Some(name), "accurate") if !name.is_empty() => encode_agent_name(name),
            (Some(name), _) if !name.is_empty() => format!("*{}*", name),
            _ => String::new(),
        };

        let page = self
            .persistence
            .config_search_page(
                page_no,
                page_size_u64,
                namespace,
                &data_id_filter,
                AGENT_GROUP,
                AI_APP_NAME,
                vec![],
                vec![AI_CONFIG_TYPE.to_string()],
                "",
            )
            .await?;

        let mut agents = Vec::new();
        for item in &page.page_items {
            match serde_json::from_str::<AgentCardVersionInfo>(&item.content) {
                Ok(version_info) => {
                    // Fetch the latest version detail
                    let encoded = encode_agent_name(&version_info.name);
                    let detail_data_id =
                        format!("{}-{}", encoded, version_info.latest_published_version);

                    let detail = self
                        .query_config(&item.tenant, AGENT_VERSION_GROUP, &detail_data_id)
                        .await
                        .ok()
                        .flatten();

                    let agent = if let Some(detail_content) = detail {
                        if let Ok(detail_info) =
                            serde_json::from_str::<AgentCardDetailInfo>(&detail_content)
                        {
                            if let Some(card) = detail_info.agent_card {
                                RegisteredAgent {
                                    id: version_info.id.clone(),
                                    card,
                                    namespace: item.tenant.clone(),
                                    health_status: HealthStatus::Unknown,
                                    registered_at: item.created_time,
                                    last_health_check: None,
                                    updated_at: item.modified_time,
                                }
                            } else {
                                build_agent_stub(
                                    &version_info,
                                    &item.tenant,
                                    item.created_time,
                                    item.modified_time,
                                )
                            }
                        } else {
                            build_agent_stub(
                                &version_info,
                                &item.tenant,
                                item.created_time,
                                item.modified_time,
                            )
                        }
                    } else {
                        build_agent_stub(
                            &version_info,
                            &item.tenant,
                            item.created_time,
                            item.modified_time,
                        )
                    };

                    agents.push(agent);
                }
                Err(e) => {
                    warn!(
                        data_id = %item.data_id,
                        error = %e,
                        "Failed to parse agent version info"
                    );
                }
            }
        }

        Ok(AgentListResponse {
            agents,
            total: page.total_count,
            page: page_no as u32,
            page_size,
        })
    }

    /// List versions for a specific agent
    pub async fn list_versions(
        &self,
        namespace: &str,
        agent_name: &str,
    ) -> anyhow::Result<Vec<VersionDetail>> {
        let encoded_name = encode_agent_name(agent_name);

        let config = self
            .query_config(namespace, AGENT_GROUP, &encoded_name)
            .await?;

        match config {
            Some(content) => {
                let version_info: AgentCardVersionInfo = serde_json::from_str(&content)?;
                Ok(version_info.version_details)
            }
            None => Ok(vec![]),
        }
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
                "",
                "",
                AI_CONFIG_TYPE,
                "",
                "",
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

/// Build a stub RegisteredAgent from version info (when detail is not available)
fn build_agent_stub(
    version_info: &AgentCardVersionInfo,
    namespace: &str,
    created_time: i64,
    modified_time: i64,
) -> RegisteredAgent {
    RegisteredAgent {
        id: version_info.id.clone(),
        card: AgentCard {
            name: version_info.name.clone(),
            display_name: version_info.name.clone(),
            description: String::new(),
            version: version_info.latest_published_version.clone(),
            endpoint: String::new(),
            protocol_version: "1.0".to_string(),
            capabilities: AgentCapabilities::default(),
            skills: vec![],
            input_modes: vec![],
            output_modes: vec![],
            authentication: None,
            rate_limits: None,
            metadata: Default::default(),
            tags: vec![],
        },
        namespace: namespace.to_string(),
        health_status: HealthStatus::Unknown,
        registered_at: created_time,
        last_health_check: None,
        updated_at: modified_time,
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
