//! Encryption Manager with hot reload support
//!
//! Manages the encryption plugin and supports hot reloading of encryption configuration.

use std::sync::Arc;
use std::time::Duration;

use parking_lot::RwLock;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use batata_config::service::{
    ConfigEncryptionService, ConfigEncryptionServiceBuilder, EncryptionPattern,
};

use crate::model::Configuration;

/// Encryption manager that wraps the encryption service with hot reload support
pub struct EncryptionManager {
    /// The current encryption service (wrapped in RwLock for hot reload)
    service: Arc<RwLock<ConfigEncryptionService>>,
    /// Configuration reference
    config: Arc<Configuration>,
    /// Shutdown signal sender
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl EncryptionManager {
    /// Create a new encryption manager from configuration
    pub fn new(config: Arc<Configuration>) -> Self {
        let service = Self::create_service_from_config(&config);
        Self {
            service: Arc::new(RwLock::new(service)),
            config,
            shutdown_tx: None,
        }
    }

    /// Create encryption service from configuration
    fn create_service_from_config(config: &Configuration) -> ConfigEncryptionService {
        if !config.encryption_enabled() {
            info!("Configuration encryption is disabled");
            return ConfigEncryptionService::disabled();
        }

        let encryption_key = match config.encryption_key() {
            Some(key) => key,
            None => {
                warn!("Encryption is enabled but no key is configured, disabling encryption");
                return ConfigEncryptionService::disabled();
            }
        };

        // Build the service with default cipher- prefix pattern
        match ConfigEncryptionServiceBuilder::new()
            .encryption_key(&encryption_key)
            .build()
        {
            Ok(service) => {
                info!(
                    "Configuration encryption enabled with {} patterns",
                    service.patterns().len()
                );
                service
            }
            Err(e) => {
                error!("Failed to create encryption service: {}", e);
                ConfigEncryptionService::disabled()
            }
        }
    }

    /// Get the encryption service for use
    pub fn service(&self) -> Arc<RwLock<ConfigEncryptionService>> {
        self.service.clone()
    }

    /// Reload the encryption configuration
    pub fn reload(&self) {
        let new_service = Self::create_service_from_config(&self.config);
        let mut service = self.service.write();
        *service = new_service;
        info!("Encryption configuration reloaded");
    }

    /// Check if encryption is currently enabled
    pub fn is_enabled(&self) -> bool {
        self.service.read().is_enabled()
    }

    /// Encrypt content if needed (delegates to inner service)
    pub async fn encrypt_if_needed(&self, data_id: &str, content: &str) -> (String, String) {
        let service = self.service.read();
        service.encrypt_if_needed(data_id, content).await
    }

    /// Decrypt content if needed (delegates to inner service)
    pub async fn decrypt_if_needed(
        &self,
        data_id: &str,
        content: &str,
        encrypted_data_key: &str,
    ) -> String {
        let service = self.service.read();
        service
            .decrypt_if_needed(data_id, content, encrypted_data_key)
            .await
    }

    /// Start hot reload background task
    pub fn start_hot_reload_task(mut self: Arc<Self>) -> Option<tokio::task::JoinHandle<()>> {
        let interval_ms = self.config.encryption_reload_interval_ms();
        if interval_ms == 0 {
            info!("Encryption hot reload is disabled");
            return None;
        }

        let (tx, mut rx) = mpsc::channel::<()>(1);

        // Store shutdown sender - need to get mutable access
        if let Some(this) = Arc::get_mut(&mut self) {
            this.shutdown_tx = Some(tx);
        }

        let manager = self.clone();
        let interval = Duration::from_millis(interval_ms);

        let handle = tokio::spawn(async move {
            info!(
                "Starting encryption hot reload task with interval {:?}",
                interval
            );
            let mut interval_timer = tokio::time::interval(interval);
            interval_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                tokio::select! {
                    _ = interval_timer.tick() => {
                        manager.reload();
                    }
                    _ = rx.recv() => {
                        info!("Encryption hot reload task shutting down");
                        break;
                    }
                }
            }
        });

        Some(handle)
    }

    /// Stop the hot reload task
    pub async fn stop_hot_reload(&self) {
        if let Some(tx) = &self.shutdown_tx {
            let _ = tx.send(()).await;
        }
    }
}

/// Helper functions for creating encryption patterns from configuration
pub fn parse_encryption_patterns(
    patterns_config: &[EncryptionPatternConfig],
) -> Vec<EncryptionPattern> {
    patterns_config
        .iter()
        .filter_map(|p| match p.pattern_type.as_str() {
            "prefix" => Some(EncryptionPattern::Prefix(p.value.clone())),
            "suffix" => Some(EncryptionPattern::Suffix(p.value.clone())),
            "contains" => Some(EncryptionPattern::Contains(p.value.clone())),
            "exact" => Some(EncryptionPattern::Exact(p.value.clone())),
            "regex" => Some(EncryptionPattern::Regex(p.value.clone())),
            _ => {
                warn!("Unknown encryption pattern type: {}", p.pattern_type);
                None
            }
        })
        .collect()
}

/// Configuration structure for encryption patterns
#[derive(Clone, Debug)]
pub struct EncryptionPatternConfig {
    pub pattern_type: String,
    pub value: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_patterns() {
        let configs = vec![
            EncryptionPatternConfig {
                pattern_type: "prefix".to_string(),
                value: "cipher-".to_string(),
            },
            EncryptionPatternConfig {
                pattern_type: "suffix".to_string(),
                value: "-secret".to_string(),
            },
        ];

        let patterns = parse_encryption_patterns(&configs);
        assert_eq!(patterns.len(), 2);
    }

    #[test]
    fn test_parse_invalid_pattern() {
        let configs = vec![EncryptionPatternConfig {
            pattern_type: "invalid".to_string(),
            value: "test".to_string(),
        }];

        let patterns = parse_encryption_patterns(&configs);
        assert_eq!(patterns.len(), 0);
    }
}
