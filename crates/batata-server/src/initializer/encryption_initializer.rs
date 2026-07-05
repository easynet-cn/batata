//! Encryption initializer
//!
//! This module provides service initializers for encryption services.

use std::sync::Arc;

use async_trait::async_trait;

use crate::context::AppContext;
use crate::initializer::traits::{InitResult, ServiceInitializer};
use batata_config::service::encryption::{ConfigEncryptionService, EncryptionPattern};

/// Encryption initializer
pub struct EncryptionInitializer;

impl EncryptionInitializer {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ServiceInitializer for EncryptionInitializer {
    async fn initialize(
        &self,
        ctx: &AppContext,
    ) -> InitResult<Arc<dyn std::any::Any + Send + Sync>> {
        // Register the always-available "none" backend
        let registry = batata_plugin::global_encryption_registry();
        registry.register(Arc::new(batata_plugin::NoopEncryptionPlugin::new()));
        
        // Register AES-GCM plugin if encryption key is configured
        if ctx.config.encryption_enabled() {
            if let Some(ref key) = ctx.config.encryption_key() {
                if !key.is_empty() {
                    match batata_common::crypto::AesGcmEncryptionPlugin::new(key) {
                        Ok(plugin) => {
                            registry.register(Arc::new(plugin));
                            tracing::info!("AES-GCM encryption plugin registered");
                        }
                        Err(e) => {
                            tracing::warn!("Failed to build AES-GCM encryption plugin: {}", e);
                        }
                    }
                }
            }
        }
        
        // Get the configured plugin
        let plugin_name = ctx.config.encryption_plugin_type();
        let default_patterns = vec![EncryptionPattern::Prefix("cipher-".to_string())];
        
        let encryption_service = if let Some(plugin) = registry.find(&plugin_name) {
            if plugin.is_enabled() {
                tracing::info!("Config encryption enabled via plugin '{}'", plugin.name());
            }
            Arc::new(ConfigEncryptionService::from_plugin(
                plugin,
                default_patterns,
            ))
        } else {
            tracing::warn!(
                "Encryption plugin '{}' not found, using disabled service",
                plugin_name
            );
            Arc::new(ConfigEncryptionService::disabled())
        };
        
        Ok(encryption_service as Arc<dyn std::any::Any + Send + Sync>)
    }
    
    fn priority(&self) -> u8 {
        40 // After persistence, before servers
    }
    
    fn name(&self) -> &'static str {
        "ConfigEncryptionService"
    }
}

impl Default for EncryptionInitializer {
    fn default() -> Self {
        Self::new()
    }
}
