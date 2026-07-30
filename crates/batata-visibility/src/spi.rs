//! Visibility Service SPI — aligned with Nacos VisibilityService

use async_trait::async_trait;

use crate::model::{
    QueryAdvisor, ValidationResult, VisibilityQueryContext, VisibilityResource,
};

/// SPI for resource visibility service.
///
/// Aligned with Nacos `com.alibaba.nacos.plugin.visibility.spi.VisibilityService`.
///
/// # Lifecycle
/// 1. Implementations are discovered and registered with `VisibilityPluginManager`.
/// 2. `resolve_default_scope_for_create` is called when a new resource is created.
/// 3. `validate_visibility` is called before read/write operations on a single resource.
/// 4. `advise_query` is called before list/range queries to produce a `QueryAdvisor`.
#[async_trait]
pub trait VisibilityService: Send + Sync {
    /// Service name identifier (must be unique across all implementations).
    fn visibility_service_name(&self) -> &str;

    /// Whether this service supports dynamic configuration.
    ///
    /// If `false`, the manager will call `init` with legacy properties.
    /// If `true`, configuration is managed through the unified plugin config system.
    fn is_configurable(&self) -> bool {
        false
    }

    /// Initialize the service with legacy properties (deprecated path).
    ///
    /// Only called when `is_configurable() == false`.
    fn init(&mut self, _properties: &std::collections::HashMap<String, String>) {}

    /// Resolve default scope for a newly created resource.
    ///
    /// Default returns `SCOPE_PRIVATE` to keep backward compatibility.
    fn resolve_default_scope_for_create(
        &self,
        _identity: &str,
        _api_type: &str,
        _resource_type: &str,
    ) -> String {
        crate::constants::SCOPE_PRIVATE.to_string()
    }

    /// Validate whether `identity` is allowed to perform `action` on `resource`.
    ///
    /// * `identity` — current user identity (username or empty for anonymous)
    /// * `action` — `"r"` for read, `"w"` for write
    /// * `api_type` — e.g. `"admin"`, `"client"`
    /// * `resource` — the resource being accessed
    async fn validate_visibility(
        &self,
        identity: &str,
        action: &str,
        api_type: &str,
        resource: &dyn VisibilityResource,
    ) -> ValidationResult;

    /// Advise on how to filter a list/range query for visibility.
    ///
    /// Returns a `QueryAdvisor` that the persistence layer uses to apply
    /// additional filters (scope, owner, authorized resource list).
    async fn advise_query(
        &self,
        identity: &str,
        action: &str,
        api_type: &str,
        context: &VisibilityQueryContext,
    ) -> QueryAdvisor;
}
