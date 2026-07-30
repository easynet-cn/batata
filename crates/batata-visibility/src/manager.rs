//! Visibility Plugin Manager — aligned with Nacos VisibilityPluginManager

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use tracing::{debug, info, warn};

use crate::model::{QueryAdvisor, ValidationResult, VisibilityQueryContext};
use crate::spi::VisibilityService;

/// Manager for loading and accessing VisibilityService implementations.
///
/// Mirrors `VisibilityPluginManager` in Nacos.
///
/// # Usage
/// ```rust,ignore
/// let manager = VisibilityPluginManager::instance();
/// if let Some(service) = manager.find_service("default") {
///     let result = service.validate_visibility("alice", "r", "admin", &resource).await;
/// }
/// ```
pub struct VisibilityPluginManager {
    services: RwLock<HashMap<String, Arc<dyn VisibilityService>>>,
    enabled: RwLock<bool>,
}

impl VisibilityPluginManager {
    /// Create a new manager (use `instance()` for the global singleton).
    pub fn new() -> Self {
        Self {
            services: RwLock::new(HashMap::new()),
            enabled: RwLock::new(true),
        }
    }

    /// Global singleton instance.
    ///
    /// In Nacos this is a static singleton; in Rust we use a lazy-static
    /// pattern via `instance()`.
    pub fn instance() -> Arc<Self> {
        use std::sync::OnceLock;
        static INSTANCE: OnceLock<Arc<VisibilityPluginManager>> = OnceLock::new();
        INSTANCE
            .get_or_init(|| Arc::new(VisibilityPluginManager::new()))
            .clone()
    }

    /// Register a visibility service.
    ///
    /// If a service with the same name already exists, the new one takes
    /// precedence (matches Nacos `PluginRegistryUtils.registerFirst` behavior).
    pub fn register(&self, service: Arc<dyn VisibilityService>) {
        let name = service.visibility_service_name().to_string();
        if name.is_empty() {
            warn!(
                target: "visibility_plugin",
                "VisibilityService({:?}) has empty serviceName, skip.",
                Arc::as_ptr(&service)
            );
            return;
        }

        let mut services = self.services.write().unwrap();
        if services.contains_key(&name) {
            warn!(
                target: "visibility_plugin",
                "VisibilityService '{}' already registered, replacing.",
                name
            );
        }
        services.insert(name.clone(), service);
        info!(
            target: "visibility_plugin",
            "Loaded VisibilityService '{}' successfully.",
            name
        );
    }

    /// Find a visibility service by name.
    ///
    /// Returns `None` if the visibility module is disabled or the service
    /// is not found.
    pub fn find_service(&self, service_name: &str) -> Option<Arc<dyn VisibilityService>> {
        if !self.is_enabled() {
            debug!(
                target: "visibility_plugin",
                "Plugin VISIBILITY is disabled"
            );
            return None;
        }
        let services = self.services.read().unwrap();
        services.get(service_name).cloned()
    }

    /// Get the default visibility service (first registered, or by name "default").
    pub fn default_service(&self) -> Option<Arc<dyn VisibilityService>> {
        if !self.is_enabled() {
            return None;
        }
        let services = self.services.read().unwrap();
        services
            .get("default")
            .cloned()
            .or_else(|| services.values().next().cloned())
    }

    /// Get all registered services.
    pub fn all_services(&self) -> HashMap<String, Arc<dyn VisibilityService>> {
        let services = self.services.read().unwrap();
        services.clone()
    }

    /// Set whether the visibility module is enabled.
    pub fn set_enabled(&self, enabled: bool) {
        let mut e = self.enabled.write().unwrap();
        *e = enabled;
    }

    /// Check whether the visibility module is enabled.
    pub fn is_enabled(&self) -> bool {
        *self.enabled.read().unwrap()
    }

    /// Convenience: validate visibility using the default service.
    pub async fn validate_with_default(
        &self,
        identity: &str,
        action: &str,
        api_type: &str,
        resource: &dyn crate::model::VisibilityResource,
    ) -> ValidationResult {
        match self.default_service() {
            Some(svc) => svc.validate_visibility(identity, action, api_type, resource).await,
            None => ValidationResult::allow(),
        }
    }

    /// Convenience: advise query using the default service.
    pub async fn advise_with_default(
        &self,
        identity: &str,
        action: &str,
        api_type: &str,
        context: &VisibilityQueryContext,
    ) -> QueryAdvisor {
        match self.default_service() {
            Some(svc) => svc.advise_query(identity, action, api_type, context).await,
            None => QueryAdvisor::new(),
        }
    }
}

impl Default for VisibilityPluginManager {
    fn default() -> Self {
        Self::new()
    }
}
