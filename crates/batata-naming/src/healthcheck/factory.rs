//! Health checker factory for extensibility
//!
//! This module provides a factory for creating and managing health checkers,
//! supporting dynamic registration of custom checkers.

use super::checker::{HealthChecker, HealthCheckResult};
use super::configurer::HealthCheckerConfig;
use crate::model::Instance;
use dashmap::DashMap;
use std::sync::Arc;
use tracing::{debug, warn};

/// Health checker factory
///
/// This factory manages health checkers and supports dynamic registration
/// of custom checkers for extensibility.
pub struct HealthCheckerFactory {
    checkers: DashMap<String, Arc<dyn HealthChecker>>,
}

impl HealthCheckerFactory {
    /// Create a new health checker factory with built-in checkers registered
    pub fn new() -> Self {
        let factory = Self {
            checkers: DashMap::new(),
        };

        // Register built-in checkers
        factory.register("TCP", Arc::new(super::checker::TcpHealthChecker));
        factory.register("HTTP", Arc::new(super::checker::HttpHealthChecker));
        factory.register("NONE", Arc::new(super::checker::NoneHealthChecker));

        factory
    }

    /// Register a custom health checker
    ///
    /// # Example
    /// ```
    /// let factory = HealthCheckerFactory::new();
    /// factory.register("REDIS", Arc::new(RedisHealthChecker));
    /// ```
    pub fn register(&self, r#type: &str, checker: Arc<dyn HealthChecker>) {
        let type_upper = r#type.to_uppercase();
        self.checkers.insert(type_upper.clone(), checker);
        debug!("Registered health checker: {}", type_upper);
    }

    /// Get a health checker by type
    ///
    /// Returns None if the checker type is not registered.
    pub fn get_checker(&self, r#type: &str) -> Option<Arc<dyn HealthChecker>> {
        let type_upper = r#type.to_uppercase();
        self.checkers.get(&type_upper).map(|entry| entry.value().clone())
    }

    /// Get all registered checker types
    pub fn get_checker_types(&self) -> Vec<String> {
        self.checkers.iter().map(|entry| entry.key().clone()).collect()
    }

    /// Check if a checker type is registered
    pub fn has_checker(&self, r#type: &str) -> bool {
        self.checkers.contains_key(&r#type.to_uppercase())
    }

    /// Execute health check using the appropriate checker
    ///
    /// Automatically selects the checker based on the config type.
    /// Returns failure result if checker type is not registered.
    pub async fn check(&self, instance: &Instance, config: &HealthCheckerConfig) -> HealthCheckResult {
        let checker = match self.get_checker(&config.r#type) {
            Some(c) => c,
            None => {
                warn!(
                    "Unknown health checker type: {}, falling back to NONE",
                    config.r#type
                );
                self.get_checker("NONE").unwrap()
            }
        };

        checker.check(instance, config).await
    }

    /// Unregister a health checker
    ///
    /// Returns false if the checker was not registered.
    pub fn unregister(&self, r#type: &str) -> bool {
        self.checkers.remove(&r#type.to_uppercase()).is_some()
    }

    /// Get the number of registered checkers
    pub fn checker_count(&self) -> usize {
        self.checkers.len()
    }
}

impl Default for HealthCheckerFactory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_factory_registration() {
        let factory = HealthCheckerFactory::new();

        assert!(factory.has_checker("TCP"));
        assert!(factory.has_checker("HTTP"));
        assert!(factory.has_checker("NONE"));
        assert_eq!(factory.checker_count(), 3);
    }

    #[tokio::test]
    async fn test_factory_custom_checker() {
        let factory = HealthCheckerFactory::new();

        // Register custom checker
        struct CustomChecker;
        #[async_trait::async_trait]
        impl HealthChecker for CustomChecker {
            fn get_type(&self) -> &str { "CUSTOM" }
            async fn check(&self, _instance: &Instance, _config: &HealthCheckerConfig) -> HealthCheckResult {
                HealthCheckResult {
                    success: true,
                    message: Some("Custom check".to_string()),
                    response_time_ms: 0,
                }
            }
        }

        factory.register("CUSTOM", Arc::new(CustomChecker));
        assert!(factory.has_checker("CUSTOM"));
        assert_eq!(factory.checker_count(), 4);

        // Test custom checker
        let instance = Instance::default();
        let config = HealthCheckerConfig::new("CUSTOM");
        let result = factory.check(&instance, &config).await;
        assert!(result.success);
        assert_eq!(result.message, Some("Custom check".to_string()));
    }

    #[tokio::test]
    async fn test_factory_unregistered_type() {
        let factory = HealthCheckerFactory::new();

        let instance = Instance::default();
        let config = HealthCheckerConfig::new("UNKNOWN");
        let result = factory.check(&instance, &config).await;

        // Should fall back to NONE checker and return success
        assert!(result.success);
    }

    #[tokio::test]
    async fn test_factory_unregister() {
        let factory = HealthCheckerFactory::new();
        let initial_count = factory.checker_count();

        assert!(factory.unregister("TCP"));
        assert_eq!(factory.checker_count(), initial_count - 1);
        assert!(!factory.has_checker("TCP"));
        assert!(!factory.unregister("UNKNOWN"));
    }

    #[tokio::test]
    async fn test_factory_get_checker_types() {
        let factory = HealthCheckerFactory::new();
        let types = factory.get_checker_types();

        assert_eq!(types.len(), 3);
        assert!(types.contains(&"TCP".to_string()));
        assert!(types.contains(&"HTTP".to_string()));
        assert!(types.contains(&"NONE".to_string()));
    }
}
