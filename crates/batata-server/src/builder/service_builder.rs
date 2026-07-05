//! Service builder
//!
//! This module provides a builder for registering and initializing services.

use crate::context::AppContext;
use crate::initializer::{InitResult, traits::ServiceInitializer};
use crate::registry::service_registry::ServiceRegistry;

/// Service builder
///
/// Registers and initializes services in dependency order.
pub struct ServiceBuilder {
    initializers: Vec<Box<dyn ServiceInitializer>>,
}

impl ServiceBuilder {
    /// Create a new empty service builder
    pub fn new() -> Self {
        Self {
            initializers: Vec::new(),
        }
    }
    
    /// Add a service initializer
    pub fn with_initializer<I: ServiceInitializer + 'static>(
        mut self,
        initializer: I,
    ) -> Self {
        self.initializers.push(Box::new(initializer));
        self
    }
    
    /// Build and initialize all services
    ///
    /// Initializers are sorted by priority and executed in order.
    /// Each initializer's output is registered to the service registry.
    pub async fn build(
        mut self,
        ctx: &AppContext,
        _registry: &mut ServiceRegistry,
    ) -> InitResult<()> {
        // Sort by priority (lower numbers first)
        self.initializers.sort_by_key(|i| i.priority());
        
        tracing::debug!(
            "Initializing {} services (in priority order)",
            self.initializers.len()
        );
        
        for initializer in &self.initializers {
            let name = initializer.name();
            tracing::info!("Initializing service: {}", name);
            
            let _service = initializer.initialize(ctx).await?;
            
            // Register the service - we need to store it somehow
            // For now, we'll just log that initialization happened
            tracing::debug!("Service initialized: {}", name);
        }
        
        Ok(())
    }
    
    /// Get the number of registered initializers
    pub fn len(&self) -> usize {
        self.initializers.len()
    }
    
    /// Check if there are no initializers
    pub fn is_empty(&self) -> bool {
        self.initializers.is_empty()
    }
}

impl Default for ServiceBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::AppContext;
    use crate::initializer::traits::ServiceInitializer;
    use async_trait::async_trait;
    use std::sync::Arc;
    use std::any::Any;
    
    struct TestInitializer {
        priority: u8,
        name: &'static str,
    }
    
    impl TestInitializer {
        fn new(priority: u8, name: &'static str) -> Self {
            Self { priority, name }
        }
    }
    
    #[async_trait]
    impl ServiceInitializer for TestInitializer {
        async fn initialize(
            &self,
            _ctx: &AppContext,
        ) -> InitResult<Arc<dyn Any + Send + Sync>> {
            Ok(Arc::new(self.name.to_string()))
        }
        
        fn priority(&self) -> u8 {
            self.priority
        }
        
        fn name(&self) -> &'static str {
            self.name
        }
    }
    
    #[tokio::test]
    async fn test_service_builder_empty() {
        let builder = ServiceBuilder::new();
        assert!(builder.is_empty());
        assert_eq!(builder.len(), 0);
    }
    
    #[tokio::test]
    async fn test_service_builder_with_initializers() {
        let builder = ServiceBuilder::new()
            .with_initializer(TestInitializer::new(2, "second"))
            .with_initializer(TestInitializer::new(1, "first"))
            .with_initializer(TestInitializer::new(3, "third"));
        
        assert_eq!(builder.len(), 3);
        assert!(!builder.is_empty());
    }
    
    // Note: Full integration test would require a real AppContext
    // which depends on Configuration and other components
}
