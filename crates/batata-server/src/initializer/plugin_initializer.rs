//! Plugin initializer
//!
//! This module provides service initializers for plugin-related services.

use std::sync::Arc;

use async_trait::async_trait;

use crate::context::AppContext;
use crate::initializer::traits::{InitResult, ServiceInitializer};

/// Plugin initializer
pub struct PluginInitializer;

impl PluginInitializer {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ServiceInitializer for PluginInitializer {
    async fn initialize(
        &self,
        ctx: &AppContext,
    ) -> InitResult<Arc<dyn std::any::Any + Send + Sync>> {
        // Build plugin context
        let mut plugin_ctx = batata_plugin::PluginContext::new();
        let is_cluster = ctx.is_cluster();
        
        plugin_ctx.insert("is_cluster", Arc::new(is_cluster));
        
        if let Some(ref raft) = ctx.persistence.raft_node {
            plugin_ctx.insert("raft_node", raft.clone());
        }
        
        if let Some(ref cm) = ctx.persistence.cluster_manager {
            plugin_ctx.insert("cluster_manager", Arc::new(cm.clone()));
        }
        
        // Note: Distro protocol would be added here if available
        
        tracing::info!(
            "Plugin context created (is_cluster: {})",
            is_cluster
        );
        
        Ok(Arc::new(plugin_ctx) as Arc<dyn std::any::Any + Send + Sync>)
    }
    
    fn priority(&self) -> u8 {
        100 // Plugins initialize after core services
    }
    
    fn name(&self) -> &'static str {
        "PluginContext"
    }
}

impl Default for PluginInitializer {
    fn default() -> Self {
        Self::new()
    }
}
