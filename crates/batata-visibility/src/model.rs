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
    pub resource_type: String,
    pub resources: Vec<String>,
}

/// Visibility query advisor for range/list operations.
///
/// Mirrors `QueryAdvisor` in Nacos. Produced by `VisibilityService::advise_query`
/// to guide the persistence layer on how to filter results.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QueryAdvisor {
    pub base_predicate: BaseVisibilityPredicate,
    pub authorized_predicate: AuthorizedResources,
}

impl QueryAdvisor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_predicate(mut self, predicate: BaseVisibilityPredicate) -> Self {
        self.base_predicate = predicate;
        self
    }

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
    pub fn allow() -> Self {
        Self {
            allowed: true,
            reason: None,
        }
    }

    pub fn deny(reason: &str) -> Self {
        Self {
            allowed: false,
            reason: Some(reason.to_string()),
        }
    }

    pub fn is_allowed(&self) -> bool {
        self.allowed
    }

    pub fn reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// Minimal query context for visibility planning.
///
/// Mirrors `VisibilityQueryContext` in Nacos.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VisibilityQueryContext {
    pub namespace_id: String,
    pub resource_type: String,
}

/// Base trait for resources that support visibility validation.
///
/// Mirrors `VisibilityResource` abstract class in Nacos.
///
/// Implementations should provide namespace, name, and type;
/// scope and owner have default values.
pub trait VisibilityResource: Send + Sync {
    fn namespace_id(&self) -> &str;
    fn resource_name(&self) -> &str;
    fn resource_type(&self) -> &str;
    fn scope(&self) -> &str;
    fn owner(&self) -> &str;
}

/// A concrete visibility resource for general use.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericVisibilityResource {
    pub namespace_id: String,
    pub resource_name: String,
    pub resource_type: String,
    pub scope: String,
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
