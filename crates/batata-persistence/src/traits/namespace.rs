//! Namespace persistence trait
//!
//! Defines the interface for namespace storage operations.

use async_trait::async_trait;

use crate::model::NamespaceInfo;

/// Namespace persistence operations
#[async_trait]
pub trait NamespacePersistence: Send + Sync {
    /// Find all namespaces
    async fn namespace_find_all(&self) -> anyhow::Result<Vec<NamespaceInfo>>;

    /// Get a namespace by its ID
    async fn namespace_get_by_id(
        &self,
        namespace_id: &str,
    ) -> anyhow::Result<Option<NamespaceInfo>>;

    /// Create a new namespace
    async fn namespace_create(
        &self,
        namespace_id: &str,
        name: &str,
        desc: &str,
    ) -> anyhow::Result<()>;

    /// Update an existing namespace
    async fn namespace_update(
        &self,
        namespace_id: &str,
        name: &str,
        desc: &str,
    ) -> anyhow::Result<bool>;

    /// Delete a namespace
    async fn namespace_delete(&self, namespace_id: &str) -> anyhow::Result<bool>;

    /// Check if namespace exists
    async fn namespace_check(&self, namespace_id: &str) -> anyhow::Result<bool>;
}
