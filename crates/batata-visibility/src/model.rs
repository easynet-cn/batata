//! Visibility models — aligned with Nacos visibility plugin model types

use serde::{Deserialize, Serialize};

// Re-export constants for convenience
pub use crate::constants::{ACTION_READ, ACTION_WRITE, SCOPE_PRIVATE, SCOPE_PUBLIC};

/// Base predicate shape for visibility query planning.
///
/// Mirrors `BaseVisibilityPredicate` in Nacos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BaseVisibilityPredicate {
    /// No filtering — return all resources
    All,
    /// Only public resources
    Public,
    /// Only resources owned by the current identity
    Owner,
    /// Public resources OR resources owned by the current identity
    PublicAndOwner,
}

impl Default for BaseVisibilityPredicate {
    fn default() -> Self {
        Self::PublicAndOwner
    }
}

/// Storage-neutral authorized resources set.
///
/// Mirrors `AuthorizedResources` in Nacos.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuthorizedResources {
    /// Type of resource this authorized set applies to (e.g. `"skill"`).
    pub resource_type: String,
    /// Identifiers of resources the identity is explicitly authorized to access.
    pub resources: Vec<String>,
}

/// Visibility query advisor for range/list operations.
///
/// Mirrors `QueryAdvisor` in Nacos. Produced by `VisibilityService::advise_query`
/// to guide the persistence layer on how to filter results.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QueryAdvisor {
    /// Base filtering strategy applied to the query.
    pub base_predicate: BaseVisibilityPredicate,
    /// Storage-neutral authorized resources used for additional filtering.
    pub authorized_predicate: AuthorizedResources,
}

impl QueryAdvisor {
    /// Create a new, empty query advisor.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the base filtering strategy for the query.
    pub fn with_base_predicate(mut self, predicate: BaseVisibilityPredicate) -> Self {
        self.base_predicate = predicate;
        self
    }

    /// Set the authorized resources used for additional filtering.
    pub fn with_authorized_resources(mut self, resource_type: &str, resources: Vec<String>) -> Self {
        self.authorized_predicate = AuthorizedResources {
            resource_type: resource_type.to_string(),
            resources,
        };
        self
    }
}

/// Result of single-resource visibility validation.
///
/// Mirrors `ValidationResult` in Nacos.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    allowed: bool,
    reason: Option<String>,
}

impl ValidationResult {
    /// Create an allowed result with no reason.
    pub fn allow() -> Self {
        Self {
            allowed: true,
            reason: None,
        }
    }

    /// Create a denied result with the given reason.
    pub fn deny(reason: &str) -> Self {
        Self {
            allowed: false,
            reason: Some(reason.to_string()),
        }
    }

    /// Whether the visibility check is allowed.
    pub fn is_allowed(&self) -> bool {
        self.allowed
    }

    /// Optional reason explaining a denial, if any.
    pub fn reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// Minimal query context for visibility planning.
///
/// Mirrors `VisibilityQueryContext` in Nacos.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VisibilityQueryContext {
    /// Namespace the query is scoped to.
    pub namespace_id: String,
    /// Type of resource the query targets (e.g. `"skill"`).
    pub resource_type: String,
}

/// Base trait for resources that support visibility validation.
///
/// Mirrors `VisibilityResource` abstract class in Nacos.
///
/// Implementations should provide namespace, name, and type;
/// scope and owner have default values.
pub trait VisibilityResource: Send + Sync {
    /// Namespace this resource belongs to.
    fn namespace_id(&self) -> &str;
    /// Name identifying the resource.
    fn resource_name(&self) -> &str;
    /// Type of the resource (e.g. `"skill"`).
    fn resource_type(&self) -> &str;
    /// Visibility scope (`SCOPE_PUBLIC` or `SCOPE_PRIVATE`).
    fn scope(&self) -> &str;
    /// Owner identity of the resource.
    fn owner(&self) -> &str;
}

/// A concrete visibility resource for general use.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericVisibilityResource {
    /// Namespace this resource belongs to.
    pub namespace_id: String,
    /// Name identifying the resource.
    pub resource_name: String,
    /// Type of the resource (e.g. `"skill"`).
    pub resource_type: String,
    /// Visibility scope (`SCOPE_PUBLIC` or `SCOPE_PRIVATE`).
    pub scope: String,
    /// Owner identity of the resource.
    pub owner: String,
}

impl VisibilityResource for GenericVisibilityResource {
    fn namespace_id(&self) -> &str {
        &self.namespace_id
    }

    fn resource_name(&self) -> &str {
        &self.resource_name
    }

    fn resource_type(&self) -> &str {
        &self.resource_type
    }

    fn scope(&self) -> &str {
        &self.scope
    }

    fn owner(&self) -> &str {
        &self.owner
    }
}
