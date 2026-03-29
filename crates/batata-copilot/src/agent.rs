//! Copilot Agent Manager — manages config lifecycle and LLM interactions

use tokio::sync::RwLock;
use tracing::{debug, info};

use crate::config::{CopilotConfig, CopilotConfigStorage};

/// Central manager for copilot agent configuration and lifecycle
pub struct CopilotAgentManager {
    config: RwLock<CopilotConfig>,
    storage: Option<CopilotConfigStorage>,
}

impl CopilotAgentManager {
    /// Create with persistent storage
    pub fn new(storage: CopilotConfigStorage) -> Self {
        Self {
            config: RwLock::new(CopilotConfig::default()),
            storage: Some(storage),
        }
    }

    /// Create without storage (defaults only)
    pub fn without_storage() -> Self {
        Self {
            config: RwLock::new(CopilotConfig::default()),
            storage: None,
        }
    }

    /// Initialize: load config from storage
    pub async fn init(&self) {
        self.refresh_config().await;
        let config = self.config.read().await;
        if config.is_configured() {
            info!(
                model = %config.model,
                "Copilot initialized with LLM provider"
            );
        } else {
            info!("Copilot initialized (no API key configured)");
        }
    }

    /// Refresh config from storage
    pub async fn refresh_config(&self) {
        if let Some(ref storage) = self.storage
            && let Some(stored_config) = storage.get_config().await
        {
            let mut config = self.config.write().await;
            // Merge: stored values override defaults, but preserve env var priority
            config.enabled = stored_config.enabled;
            if stored_config.api_key.is_some() {
                config.api_key = stored_config.api_key;
            }
            if !stored_config.model.is_empty() {
                config.model = stored_config.model;
            }
            if stored_config.base_url.is_some() {
                config.base_url = stored_config.base_url;
            }
            if stored_config.studio_url.is_some() {
                config.studio_url = stored_config.studio_url;
            }
            debug!("Refreshed copilot config from storage");
        }
    }

    /// Get current config snapshot
    pub async fn get_config(&self) -> CopilotConfig {
        self.config.read().await.clone()
    }

    /// Update and persist config
    pub async fn update_config(&self, update: CopilotConfig) -> anyhow::Result<()> {
        {
            let mut config = self.config.write().await;
            if update.api_key.is_some() {
                config.api_key = update.api_key;
            }
            if !update.model.is_empty() {
                config.model = update.model;
            }
            config.base_url = update.base_url.or(config.base_url.take());
            config.studio_url = update.studio_url.or(config.studio_url.take());
            if !update.studio_project.is_empty() {
                config.studio_project = update.studio_project;
            }
            config.enabled = update.enabled;
        }

        // Persist
        if let Some(ref storage) = self.storage {
            let config = self.config.read().await;
            storage.save_config(&config).await?;
        }

        info!("Updated copilot configuration");
        Ok(())
    }

    /// Check if copilot is ready for LLM calls
    pub async fn is_ready(&self) -> bool {
        let config = self.config.read().await;
        config.is_configured() || config.effective_api_key().is_some()
    }
}
