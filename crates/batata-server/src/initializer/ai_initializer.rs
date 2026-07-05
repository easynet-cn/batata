//! AI services initializer
//!
//! This module provides service initializers for AI-related services.

use std::sync::Arc;

use async_trait::async_trait;

use crate::context::AppContext;
use crate::initializer::traits::{InitResult, ServiceInitializer};

/// AI services initializer
pub struct AiInitializer;

impl AiInitializer {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ServiceInitializer for AiInitializer {
    async fn initialize(
        &self,
        ctx: &AppContext,
    ) -> InitResult<Arc<dyn std::any::Any + Send + Sync>> {
        // Determine which AI services to initialize based on persistence availability
        let persistence = ctx.persistence.persistence.clone();
        let naming_service: Option<Arc<dyn batata_api::naming::NamingServiceProvider>> = None;
        
        let ai_services = if let Some(ref persist) = persistence {
            if let Some(ref ns) = naming_service {
                tracing::info!("AI services using config-backed persistence");
                crate::startup::AIServices::with_persistence(
                    persist.clone(),
                    ns.clone(),
                )
            } else {
                tracing::info!("AI services using in-memory storage (no naming service)");
                crate::startup::AIServices::new()
            }
        } else {
            tracing::info!("AI services using in-memory storage (no persistence)");
            crate::startup::AIServices::new()
        };
        
        Ok(Arc::new(ai_services) as Arc<dyn std::any::Any + Send + Sync>)
    }
    
    fn priority(&self) -> u8 {
        50 // AI services are lower priority
    }
    
    fn name(&self) -> &'static str {
        "AIServices"
    }
}

impl Default for AiInitializer {
    fn default() -> Self {
        Self::new()
    }
}
