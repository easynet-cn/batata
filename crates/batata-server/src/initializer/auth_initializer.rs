//! Authentication initializer
//!
//! This module provides service initializers for authentication services.

use std::sync::Arc;

use async_trait::async_trait;

use crate::context::AppContext;
use crate::initializer::traits::{InitResult, ServiceInitializer};

/// Authentication initializer
pub struct AuthInitializer;

impl AuthInitializer {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ServiceInitializer for AuthInitializer {
    async fn initialize(
        &self,
        ctx: &AppContext,
    ) -> InitResult<Arc<dyn std::any::Any + Send + Sync>> {
        // Initialize auth caches
        batata_auth::service::auth::init_auth_caches(
            batata_auth::service::auth::AuthCacheConfig {
                token_capacity: ctx.config.auth_token_cache_capacity(),
                token_ttl_secs: ctx.config.auth_token_cache_ttl_secs(),
                blacklist_capacity: ctx.config.auth_blacklist_capacity(),
                blacklist_ttl_secs: ctx.config.auth_blacklist_ttl_secs(),
            },
        );
        
        // Initialize gRPC auth cache
        batata_core::service::grpc_auth::init_grpc_auth_cache(
            ctx.config.grpc_auth_cache_capacity(),
            ctx.config.grpc_auth_cache_ttl_secs(),
        );
        
        tracing::info!("Auth caches initialized");
        
        // Note: The actual auth plugin would be created based on configuration
        // and stored in the service registry
        
        Ok(Arc::new(()) as Arc<dyn std::any::Any + Send + Sync>)
    }
    
    fn priority(&self) -> u8 {
        5 // Very high priority - auth should be ready early
    }
    
    fn name(&self) -> &'static str {
        "AuthCaches"
    }
}

impl Default for AuthInitializer {
    fn default() -> Self {
        Self::new()
    }
}
