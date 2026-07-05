//! Persistence builder
//!
//! This module provides a builder for initializing the persistence layer.

use std::sync::Arc;

use crate::context::DeploymentMode;
use crate::model::common::Configuration;
use crate::startup::persistence::PersistenceContext;

/// Persistence builder
///
/// Initializes the persistence layer based on deployment mode.
pub struct PersistenceBuilder;

impl PersistenceBuilder {
    /// Initialize persistence based on configuration
    ///
    /// In console-remote mode, returns an empty context.
    /// Otherwise, initializes the configured persistence backend.
    pub async fn initialize(
        config: &Arc<Configuration>,
        plugin_cf_names: &[String],
        deployment_mode: DeploymentMode,
    ) -> Result<PersistenceContext, Box<dyn std::error::Error>> {
        if deployment_mode == DeploymentMode::Console {
            tracing::info!("Console remote mode - skipping persistence init");
            return Ok(PersistenceContext::empty());
        }
        
        tracing::info!("Initializing persistence layer...");
        let ctx = crate::startup::persistence::init_persistence(
            config.as_ref(),
            plugin_cf_names,
        )
        .await?;
        
        Ok(ctx)
    }
}

impl Default for PersistenceBuilder {
    fn default() -> Self {
        Self
    }
}

#[cfg(test)]
mod tests {
    // PersistenceBuilder requires a full runtime environment,
    // so we test the basic types and structures here
    
    use super::*;
    
    #[test]
    fn test_persistence_builder_default() {
        let builder = PersistenceBuilder::default();
        // Just verify it can be created
        drop(builder);
    }
    
    #[test]
    fn test_deployment_mode_affects_persistence() {
        // Console mode should not need persistence
        let console_mode = DeploymentMode::Console;
        let merged_mode = DeploymentMode::Merged;
        
        assert_ne!(console_mode, merged_mode);
    }
}
