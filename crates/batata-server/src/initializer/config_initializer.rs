//! Configuration initializer
//!
//! This module provides service initializers for configuration-related services.

use std::sync::Arc;

use async_trait::async_trait;

use crate::context::AppContext;
use crate::initializer::traits::{InitResult, ServiceInitializer};

/// Configuration subscriber service initializer
pub struct ConfigSubscriberInitializer;

impl ConfigSubscriberInitializer {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ServiceInitializer for ConfigSubscriberInitializer {
    async fn initialize(
        &self,
        _ctx: &AppContext,
    ) -> InitResult<Arc<dyn std::any::Any + Send + Sync>> {
        let manager = Arc::new(batata_core::ConfigSubscriberManager::new());
        Ok(manager as Arc<dyn std::any::Any + Send + Sync>)
    }
    
    fn priority(&self) -> u8 {
        10 // High priority - config subscriber is fundamental
    }
    
    fn name(&self) -> &'static str {
        "ConfigSubscriberManager"
    }
}

impl Default for ConfigSubscriberInitializer {
    fn default() -> Self {
        Self::new()
    }
}
