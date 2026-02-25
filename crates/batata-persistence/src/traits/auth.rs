//! Auth persistence trait
//!
//! Defines the interface for authentication and authorization storage operations.

use async_trait::async_trait;

use crate::model::{Page, PermissionInfo, RoleInfo, UserInfo};

/// Authentication and authorization persistence operations
#[async_trait]
pub trait AuthPersistence: Send + Sync {
    // ==================== User Operations ====================

    /// Find a user by username
    async fn user_find_by_username(&self, username: &str) -> anyhow::Result<Option<UserInfo>>;

    /// Search users with pagination
    async fn user_find_page(
        &self,
        username: &str,
        page_no: u64,
        page_size: u64,
        accurate: bool,
    ) -> anyhow::Result<Page<UserInfo>>;

    /// Create a new user
    async fn user_create(
        &self,
        username: &str,
        password_hash: &str,
        enabled: bool,
    ) -> anyhow::Result<()>;

    /// Update user password
    async fn user_update_password(&self, username: &str, password_hash: &str)
    -> anyhow::Result<()>;

    /// Delete a user
    async fn user_delete(&self, username: &str) -> anyhow::Result<()>;

    /// Search usernames matching a pattern
    async fn user_search(&self, username: &str) -> anyhow::Result<Vec<String>>;

    // ==================== Role Operations ====================

    /// Find roles by username
    async fn role_find_by_username(&self, username: &str) -> anyhow::Result<Vec<RoleInfo>>;

    /// Search roles with pagination
    async fn role_find_page(
        &self,
        username: &str,
        role: &str,
        page_no: u64,
        page_size: u64,
        accurate: bool,
    ) -> anyhow::Result<Page<RoleInfo>>;

    /// Create a role assignment
    async fn role_create(&self, role: &str, username: &str) -> anyhow::Result<()>;

    /// Delete a role assignment
    async fn role_delete(&self, role: &str, username: &str) -> anyhow::Result<()>;

    /// Check if any global admin role exists
    async fn role_has_global_admin(&self) -> anyhow::Result<bool>;

    /// Check if a specific user has the global admin role
    async fn role_has_global_admin_by_username(&self, username: &str) -> anyhow::Result<bool>;

    /// Search role names matching a pattern
    async fn role_search(&self, role: &str) -> anyhow::Result<Vec<String>>;

    // ==================== Permission Operations ====================

    /// Find permissions by role
    async fn permission_find_by_role(&self, role: &str) -> anyhow::Result<Vec<PermissionInfo>>;

    /// Find permissions by multiple roles
    async fn permission_find_by_roles(
        &self,
        roles: Vec<String>,
    ) -> anyhow::Result<Vec<PermissionInfo>>;

    /// Search permissions with pagination
    async fn permission_find_page(
        &self,
        role: &str,
        page_no: u64,
        page_size: u64,
        accurate: bool,
    ) -> anyhow::Result<Page<PermissionInfo>>;

    /// Find a specific permission by role, resource, and action
    async fn permission_find_by_id(
        &self,
        role: &str,
        resource: &str,
        action: &str,
    ) -> anyhow::Result<Option<PermissionInfo>>;

    /// Grant a permission to a role
    async fn permission_grant(
        &self,
        role: &str,
        resource: &str,
        action: &str,
    ) -> anyhow::Result<()>;

    /// Revoke a permission from a role
    async fn permission_revoke(
        &self,
        role: &str,
        resource: &str,
        action: &str,
    ) -> anyhow::Result<()>;
}
