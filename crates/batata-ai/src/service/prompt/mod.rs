//! Prompt management service — config-based storage (Nacos 3.2 compatible)
//!
//! Prompts are stored as JSON configs in the config service with group `nacos-ai-prompt`.
//! Each prompt has multiple configs:
//! - `{key}.json` — latest version mirror
//! - `{key}.{version}.json` — version-specific content
//! - `{key}.descriptor.json` — metadata (description, bizTags)
//! - `{key}.label-version-mapping.json` — label→version mappings

use std::sync::Arc;

use batata_persistence::PersistenceService;
use tracing::{debug, warn};

use crate::model::prompt::*;

/// Prompt operation service (admin + client)
pub struct PromptOperationService {
    persistence: Arc<dyn PersistenceService>,
}

impl PromptOperationService {
    pub fn new(persistence: Arc<dyn PersistenceService>) -> Self {
        Self { persistence }
    }

    // ========================================================================
    // Internal helpers — config read/write
    // ========================================================================

    async fn read_config(&self, data_id: &str, namespace_id: &str) -> Option<String> {
        match self
            .persistence
            .config_find_one(data_id, PROMPT_GROUP, namespace_id)
            .await
        {
            Ok(Some(config)) => Some(config.content),
            Ok(None) => None,
            Err(e) => {
                warn!("Failed to read prompt config {}: {}", data_id, e);
                None
            }
        }
    }

    async fn write_config(
        &self,
        data_id: &str,
        namespace_id: &str,
        content: &str,
        src_user: &str,
        src_ip: &str,
    ) -> anyhow::Result<bool> {
        self.persistence
            .config_create_or_update(
                data_id,
                PROMPT_GROUP,
                namespace_id,
                content,
                "",       // app_name
                src_user, // src_user
                src_ip,   // src_ip
                "",       // config_tags
                "",       // desc
                "",       // use
                "",       // effect
                "json",   // type
                "",       // schema
                "",       // encrypted_data_key
                None,     // cas_md5
            )
            .await
    }

    async fn delete_config(
        &self,
        data_id: &str,
        namespace_id: &str,
        src_user: &str,
    ) -> anyhow::Result<bool> {
        self.persistence
            .config_delete(data_id, PROMPT_GROUP, namespace_id, "", "", src_user)
            .await
    }

    async fn load_mapping(
        &self,
        namespace_id: &str,
        prompt_key: &str,
    ) -> Option<PromptLabelVersionMapping> {
        let data_id = build_label_version_mapping_data_id(prompt_key);
        let content = self.read_config(&data_id, namespace_id).await?;
        serde_json::from_str(&content).ok()
    }

    async fn save_mapping(
        &self,
        namespace_id: &str,
        mapping: &PromptLabelVersionMapping,
        src_user: &str,
        src_ip: &str,
    ) -> anyhow::Result<bool> {
        let data_id = build_label_version_mapping_data_id(&mapping.prompt_key);
        let content = serde_json::to_string(mapping)?;
        self.write_config(&data_id, namespace_id, &content, src_user, src_ip)
            .await
    }

    async fn load_descriptor(
        &self,
        namespace_id: &str,
        prompt_key: &str,
    ) -> Option<PromptDescriptor> {
        let data_id = build_descriptor_data_id(prompt_key);
        let content = self.read_config(&data_id, namespace_id).await?;
        serde_json::from_str(&content).ok()
    }

    async fn save_descriptor(
        &self,
        namespace_id: &str,
        descriptor: &PromptDescriptor,
        src_user: &str,
        src_ip: &str,
    ) -> anyhow::Result<bool> {
        let data_id = build_descriptor_data_id(&descriptor.prompt_key);
        let content = serde_json::to_string(descriptor)?;
        self.write_config(&data_id, namespace_id, &content, src_user, src_ip)
            .await
    }

    async fn load_version_info(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        version: &str,
    ) -> Option<PromptVersionInfo> {
        let data_id = build_version_data_id(prompt_key, version);
        let content = self.read_config(&data_id, namespace_id).await?;
        serde_json::from_str(&content).ok()
    }

    // ========================================================================
    // Admin operations
    // ========================================================================

    /// Publish a new prompt version.
    #[allow(clippy::too_many_arguments)]
    pub async fn publish_version(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        version: &str,
        template: &str,
        commit_msg: Option<&str>,
        description: Option<&str>,
        biz_tags: Vec<String>,
        variables: Option<Vec<PromptVariable>>,
        src_user: &str,
        src_ip: &str,
    ) -> anyhow::Result<bool> {
        // Validate version format
        if !is_valid_version(version) {
            anyhow::bail!(
                "Invalid version format '{}', must be major.minor.patch",
                version
            );
        }

        if template.is_empty() {
            anyhow::bail!("Template cannot be empty");
        }

        let now = chrono::Utc::now().timestamp_millis();
        let md5 = const_hex::encode(md5::Md5::digest(template.as_bytes()));

        // Load or create mapping
        let mut mapping = self
            .load_mapping(namespace_id, prompt_key)
            .await
            .unwrap_or_else(|| PromptLabelVersionMapping {
                prompt_key: prompt_key.to_string(),
                ..Default::default()
            });

        // Check if version already exists
        if mapping.versions.contains(&version.to_string()) {
            anyhow::bail!(
                "Version '{}' already exists for prompt '{}'",
                version,
                prompt_key
            );
        }

        // Create version info
        let version_info = PromptVersionInfo {
            prompt_key: prompt_key.to_string(),
            version: version.to_string(),
            template: template.to_string(),
            md5: Some(md5),
            commit_msg: commit_msg.map(|s| s.to_string()),
            src_user: Some(src_user.to_string()),
            gmt_modified: Some(now),
            variables,
        };

        // Save version config
        let version_data_id = build_version_data_id(prompt_key, version);
        let version_json = serde_json::to_string(&version_info)?;
        self.write_config(
            &version_data_id,
            namespace_id,
            &version_json,
            src_user,
            src_ip,
        )
        .await?;

        // Update mapping
        mapping.versions.push(version.to_string());
        mapping
            .versions
            .sort_by(|a, b| compare_versions(a, b).reverse());
        mapping.latest_version = Some(version.to_string());
        mapping.gmt_modified = Some(now);
        self.save_mapping(namespace_id, &mapping, src_user, src_ip)
            .await?;

        // Create/update descriptor on first publish or if description provided
        let descriptor = self.load_descriptor(namespace_id, prompt_key).await;
        if descriptor.is_none() || description.is_some() {
            let mut desc = descriptor.unwrap_or_else(|| PromptDescriptor {
                prompt_key: prompt_key.to_string(),
                ..Default::default()
            });
            if let Some(d) = description {
                desc.description = Some(d.to_string());
            }
            if !biz_tags.is_empty() {
                desc.biz_tags = biz_tags;
            }
            desc.gmt_modified = Some(now);
            self.save_descriptor(namespace_id, &desc, src_user, src_ip)
                .await?;
        }

        // Update latest mirror
        let latest_data_id = build_latest_data_id(prompt_key);
        self.write_config(
            &latest_data_id,
            namespace_id,
            &version_json,
            src_user,
            src_ip,
        )
        .await?;

        debug!(
            "Published prompt '{}' version '{}' in namespace '{}'",
            prompt_key, version, namespace_id
        );

        Ok(true)
    }

    /// Get prompt metadata (descriptor + label mapping composed)
    pub async fn get_meta(&self, namespace_id: &str, prompt_key: &str) -> Option<PromptMetaInfo> {
        let mapping = self.load_mapping(namespace_id, prompt_key).await?;
        let descriptor = self.load_descriptor(namespace_id, prompt_key).await?;
        Some(compose_meta_info(&descriptor, &mapping))
    }

    /// Delete a prompt and all its versions/metadata
    pub async fn delete_prompt(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        src_user: &str,
    ) -> anyhow::Result<bool> {
        // Load mapping to get all versions
        let mapping = self.load_mapping(namespace_id, prompt_key).await;

        // Delete all version configs
        if let Some(ref m) = mapping {
            for version in &m.versions {
                let data_id = build_version_data_id(prompt_key, version);
                let _ = self.delete_config(&data_id, namespace_id, src_user).await;
            }
        }

        // Delete descriptor, mapping, latest mirror
        let _ = self
            .delete_config(
                &build_descriptor_data_id(prompt_key),
                namespace_id,
                src_user,
            )
            .await;
        let _ = self
            .delete_config(
                &build_label_version_mapping_data_id(prompt_key),
                namespace_id,
                src_user,
            )
            .await;
        let _ = self
            .delete_config(&build_latest_data_id(prompt_key), namespace_id, src_user)
            .await;

        debug!(
            "Deleted prompt '{}' in namespace '{}'",
            prompt_key, namespace_id
        );
        Ok(true)
    }

    /// Bind a label to a specific version
    pub async fn bind_label(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        label: &str,
        version: &str,
        src_user: &str,
        src_ip: &str,
    ) -> anyhow::Result<bool> {
        let mut mapping = self
            .load_mapping(namespace_id, prompt_key)
            .await
            .ok_or_else(|| anyhow::anyhow!("Prompt '{}' not found", prompt_key))?;

        if !mapping.versions.contains(&version.to_string()) {
            anyhow::bail!(
                "Version '{}' not found for prompt '{}'",
                version,
                prompt_key
            );
        }

        mapping
            .labels
            .insert(label.to_string(), version.to_string());
        mapping.gmt_modified = Some(chrono::Utc::now().timestamp_millis());

        self.save_mapping(namespace_id, &mapping, src_user, src_ip)
            .await?;
        Ok(true)
    }

    /// Unbind a label
    pub async fn unbind_label(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        label: &str,
        src_user: &str,
        src_ip: &str,
    ) -> anyhow::Result<bool> {
        let mut mapping = self
            .load_mapping(namespace_id, prompt_key)
            .await
            .ok_or_else(|| anyhow::anyhow!("Prompt '{}' not found", prompt_key))?;

        mapping.labels.remove(label);
        mapping.gmt_modified = Some(chrono::Utc::now().timestamp_millis());

        self.save_mapping(namespace_id, &mapping, src_user, src_ip)
            .await?;
        Ok(true)
    }

    /// Update prompt metadata (description, bizTags) without changing versions
    pub async fn update_metadata(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        description: Option<&str>,
        biz_tags: Option<Vec<String>>,
        src_user: &str,
        src_ip: &str,
    ) -> anyhow::Result<bool> {
        let mut descriptor = self
            .load_descriptor(namespace_id, prompt_key)
            .await
            .ok_or_else(|| anyhow::anyhow!("Prompt '{}' not found", prompt_key))?;

        if let Some(d) = description {
            descriptor.description = Some(d.to_string());
        }
        if let Some(tags) = biz_tags {
            descriptor.biz_tags = tags;
        }
        descriptor.gmt_modified = Some(chrono::Utc::now().timestamp_millis());

        self.save_descriptor(namespace_id, &descriptor, src_user, src_ip)
            .await?;
        Ok(true)
    }

    /// Query a specific prompt version with version/label/latest resolution
    pub async fn query_detail(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        version: Option<&str>,
        label: Option<&str>,
    ) -> anyhow::Result<Option<PromptVersionInfo>> {
        let mapping = match self.load_mapping(namespace_id, prompt_key).await {
            Some(m) => m,
            None => return Ok(None),
        };

        let resolved = resolve_target_version(&mapping, version, label);
        match resolved {
            Some(v) => Ok(self.load_version_info(namespace_id, prompt_key, &v).await),
            None => {
                if version.is_some() {
                    anyhow::bail!("Version not found for prompt '{}'", prompt_key);
                }
                if label.is_some() {
                    anyhow::bail!("Label not found for prompt '{}'", prompt_key);
                }
                Ok(None)
            }
        }
    }

    // ========================================================================
    // Client operations
    // ========================================================================

    /// Query prompt with MD5-based conditional support.
    /// Returns None if client md5 matches (NOT_MODIFIED).
    pub async fn query_prompt(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        version: Option<&str>,
        label: Option<&str>,
        client_md5: Option<&str>,
    ) -> anyhow::Result<Option<PromptVersionInfo>> {
        let info = self
            .query_detail(namespace_id, prompt_key, version, label)
            .await?;

        if let Some(ref info) = info {
            // If client already has this version (MD5 match), return None (NOT_MODIFIED)
            if let Some(client) = client_md5 {
                if !client.is_empty() {
                    if let Some(ref server_md5) = info.md5 {
                        if client == server_md5 {
                            return Ok(None);
                        }
                    }
                }
            }
        }

        Ok(info)
    }

    /// List prompt versions for a prompt key (sorted by version descending)
    pub async fn list_versions(
        &self,
        namespace_id: &str,
        prompt_key: &str,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<batata_persistence::model::Page<PromptVersionSummary>> {
        let mapping = self
            .load_mapping(namespace_id, prompt_key)
            .await
            .ok_or_else(|| anyhow::anyhow!("Prompt '{}' not found", prompt_key))?;

        // Sort versions descending
        let mut versions = mapping.versions.clone();
        versions.sort_by(|a, b| compare_versions(a, b).reverse());

        // Paginate
        let total = versions.len() as u64;
        let start = ((page_no.saturating_sub(1)) * page_size) as usize;
        let page_versions: Vec<&String> = versions
            .iter()
            .skip(start)
            .take(page_size as usize)
            .collect();

        // Load version summaries
        let mut summaries = Vec::with_capacity(page_versions.len());
        for v in page_versions {
            if let Some(info) = self.load_version_info(namespace_id, prompt_key, v).await {
                summaries.push(PromptVersionSummary {
                    prompt_key: info.prompt_key,
                    version: info.version,
                    commit_msg: info.commit_msg,
                    src_user: info.src_user,
                    gmt_modified: info.gmt_modified,
                });
            }
        }

        Ok(batata_persistence::model::Page {
            total_count: total,
            page_number: page_no,
            pages_available: (total + page_size - 1) / page_size,
            page_items: summaries,
        })
    }
}

use md5::Digest;
