//! Health check initializer
//!
//! This module provides service initializers for health check services.

use std::sync::Arc;

use async_trait::async_trait;

use crate::context::AppContext;
use crate::initializer::traits::{InitResult, ServiceInitializer};
use batata_naming::healthcheck::HealthCheckConfig;

/// Health check service initializer
pub struct HealthCheckInitializer;

impl HealthCheckInitializer {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ServiceInitializer for HealthCheckInitializer {
    async fn initialize(
        &self,
        ctx: &AppContext,
    ) -> InitResult<Arc<dyn std::any::Any + Send + Sync>> {
        // Get naming service from context
        // Note: In full implementation, this would come from the service registry
        // For now, create a basic health check manager
        
        let health_check_config = Arc::new(HealthCheckConfig {
            heartbeat_interval_secs: ctx.config.naming_heartbeat_check_interval_secs(),
            ttl_monitor_interval_secs: ctx.config.naming_ttl_monitor_interval_secs(),
            deregister_monitor_interval_secs: ctx.config.naming_deregister_monitor_interval_secs(),
            ..HealthCheckConfig::default()
        });
        
        let expire_enabled = ctx.config.expire_instance_enabled();
        let health_check_enabled = health_check_config.is_enabled();
        
        // Note: We need the naming service to create the manager
        // In the full implementation, this would be retrieved from the context
        
        tracing::info!(
            "Health check config: expire_enabled={}, health_check_enabled={}",
            expire_enabled,
            health_check_enabled
        );
        
        // Return a placeholder - actual manager requires naming service
        Ok(Arc::new(health_check_config) as Arc<dyn std::any::Any + Send + Sync>)
    }
    
    fn priority(&self) -> u8 {
        30 // Health check should start after naming service
    }
    
    fn name(&self) -> &'static str {
        "HealthCheckManager"
    }
}

impl Default for HealthCheckInitializer {
    fn default() -> Self {
        Self::new()
    }
}
