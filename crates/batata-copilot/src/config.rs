//! Copilot configuration and persistent storage

use std::sync::Arc;

use batata_persistence::PersistenceService;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

const COPILOT_CONFIG_DATA_ID: &str = "copilot-config.json";
const COPILOT_CONFIG_GROUP: &str = "nacos-copilot";

/// Copilot configuration — persisted to Batata Config
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopilotConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub studio_url: Option<String>,
    #[serde(default = "default_studio_project")]
    pub studio_project: String,
    #[serde(default = "default_namespace")]
    pub default_namespace: String,
}

impl Default for CopilotConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            api_key: None,
            model: default_model(),
            base_url: None,
            studio_url: None,
            studio_project: default_studio_project(),
            default_namespace: default_namespace(),
        }
    }
}

impl CopilotConfig {
    /// Check if copilot is properly configured (has API key)
    pub fn is_configured(&self) -> bool {
        self.enabled && self.api_key.as_ref().is_some_and(|k| !k.is_empty())
    }

    /// Resolve the effective API key (env var takes precedence)
    pub fn effective_api_key(&self) -> Option<String> {
        // Environment variable takes priority
        if let Ok(key) = std::env::var("NACOS_COPILOT_API_KEY")
            && !key.is_empty()
        {
            return Some(key);
        }
        if let Ok(key) = std::env::var("DASHSCOPE_API_KEY")
            && !key.is_empty()
        {
            return Some(key);
        }
        self.api_key.clone()
    }

    /// Resolve the effective base URL for the LLM API
    pub fn effective_base_url(&self) -> String {
        self.base_url
            .clone()
            .or_else(|| std::env::var("NACOS_COPILOT_BASE_URL").ok())
            .unwrap_or_else(|| "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string())
    }
}

fn default_true() -> bool {
    true
}
fn default_model() -> String {
    "qwen-turbo".to_string()
}
fn default_studio_project() -> String {
    "BatataCopilot".to_string()
}
fn default_namespace() -> String {
    "public".to_string()
}

/// Persistent storage for copilot configuration via PersistenceService (config_info table)
pub struct CopilotConfigStorage {
    persistence: Arc<dyn PersistenceService>,
}

impl CopilotConfigStorage {
    pub fn new(persistence: Arc<dyn PersistenceService>) -> Self {
        Self { persistence }
    }

    /// Load configuration from persistent storage
    pub async fn get_config(&self) -> Option<CopilotConfig> {
        match self
            .persistence
            .config_find_one(COPILOT_CONFIG_DATA_ID, COPILOT_CONFIG_GROUP, "")
            .await
        {
            Ok(Some(config)) => match serde_json::from_str(&config.content) {
                Ok(cfg) => Some(cfg),
                Err(e) => {
                    warn!("Failed to parse copilot config: {}", e);
                    None
                }
            },
            Ok(None) => None,
            Err(e) => {
                warn!("Failed to read copilot config: {}", e);
                None
            }
        }
    }

    /// Save configuration to persistent storage
    pub async fn save_config(&self, config: &CopilotConfig) -> anyhow::Result<()> {
        let content = serde_json::to_string_pretty(config)?;
        self.persistence
            .config_create_or_update(
                COPILOT_CONFIG_DATA_ID,
                COPILOT_CONFIG_GROUP,
                "",        // namespace
                &content,  // content
                "",        // app_name
                "copilot", // src_user
                "",        // src_ip
                "",        // config_tags
                "Copilot configuration",
                "", // use
                "", // effect
                "json",
                "",   // schema
                "",   // encrypted_data_key
                None, // cas_md5
            )
            .await?;
        debug!("Saved copilot configuration");
        Ok(())
    }
}
