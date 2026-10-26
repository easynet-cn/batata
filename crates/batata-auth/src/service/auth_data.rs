//! Auth data access boundary shared by every deployment mode.
//!
//! Mirrors the seam Nacos places between its default auth plugin and the
//! user/role/permission services: **the plugin itself always runs locally**, and
//! only the *data* it reads switches backend with `nacos.deployment.type`
//! (`NacosUserServiceDirectImpl` for merged/server, `NacosUserServiceRemoteImpl`
//! for an independently deployed console).
//!
//! The same split applies here:
//!
//! - merged / server deployment -> [`PersistenceAuthDataService`] (direct storage)
//! - console deployment         -> [`RemoteAuthDataService`] (HTTP to the core)
//!
//! Keeping the boundary here rather than at the persistence layer matters: the
//! console must be able to run the real [`DefaultAuthPlugin`] locally so that
//! token validation and RBAC decisions happen in the console, which is the only
//! side holding the end user's identity.
//!
//! The trait covers two consumers:
//!
//! - the auth plugin (login + RBAC), and
//! - the `/v3/auth/**` management handlers (user, role and permission CRUD).
//!
//! Both are mounted in the console process, so both need a remote backend in a
//! separated deployment — otherwise they would reach for a database the console
//! is not allowed to have.

use std::sync::Arc;

use batata_persistence::{
    PersistenceService,
    model::{Page, PermissionInfo, RoleInfo, UserInfo},
    traits::AuthPersistence,
};

use crate::model::GLOBAL_ADMIN_ROLE;
use crate::service::rbac_cache::RbacSource;
use crate::service::visibility_grant::{grant_with, revoke_with};

/// bcrypt cost for locally stored passwords, kept identical to the handlers.
const BCRYPT_COST: u32 = 10;

/// Hash a plaintext password the way the `/v3/auth/**` handlers do.
fn hash_password(password: &str) -> anyhow::Result<String> {
    bcrypt::hash(password, BCRYPT_COST).map_err(|e| anyhow::anyhow!("hash password: {e}"))
}

/// Everything the auth plugin and the `/v3/auth/**` handlers read and write.
///
/// The method set mirrors [`AuthPersistence`] so the direct backend is a pure
/// delegation and the remote backend can serve the same handlers over HTTP.
#[async_trait::async_trait]
pub trait AuthDataService: RbacSource + Send + Sync {
    // ==================== User Operations ====================

    /// Load a single user record, including its password hash and source.
    async fn user_find_by_username(&self, username: &str) -> anyhow::Result<Option<UserInfo>>;

    /// Search users with pagination.
    async fn user_find_page(
        &self,
        username: &str,
        page_no: u64,
        page_size: u64,
        accurate: bool,
    ) -> anyhow::Result<Page<UserInfo>>;

    /// Create a user with an explicit identity source.
    ///
    /// `password_hash` is already hashed for the direct backend; the remote
    /// backend forwards it as-is and lets the core store it.
    async fn user_create_with_source(
        &self,
        username: &str,
        password_hash: &str,
        enabled: bool,
        source: &str,
    ) -> anyhow::Result<()>;

    /// Update a user's password hash.
    async fn user_update_password(&self, username: &str, password_hash: &str)
    -> anyhow::Result<()>;

    /// Delete a user.
    async fn user_delete(&self, username: &str) -> anyhow::Result<()>;

    /// Search usernames matching a pattern.
    async fn user_search(&self, username: &str) -> anyhow::Result<Vec<String>>;

    // ==================== Role Operations ====================

    /// Load the roles assigned to a user.
    async fn role_find_by_username(&self, username: &str) -> anyhow::Result<Vec<RoleInfo>>;

    /// Search role assignments with pagination.
    async fn role_find_page(
        &self,
        username: &str,
        role: &str,
        page_no: u64,
        page_size: u64,
        accurate: bool,
    ) -> anyhow::Result<Page<RoleInfo>>;

    /// Assign a role to a user.
    async fn role_create(&self, role: &str, username: &str) -> anyhow::Result<()>;

    /// Remove a role assignment. An empty `username` removes it from every holder.
    async fn role_delete(&self, role: &str, username: &str) -> anyhow::Result<()>;

    /// Search role names matching a pattern.
    async fn role_search(&self, role: &str) -> anyhow::Result<Vec<String>>;

    /// Check whether a user holds the global admin role.
    async fn role_has_global_admin_by_username(&self, username: &str) -> anyhow::Result<bool>;

    /// Check whether *any* user already holds the global admin role.
    ///
    /// Backs the "is the cluster still uninitialised?" question the admin
    /// bootstrap endpoint asks before it creates the first administrator.
    async fn role_has_global_admin(&self) -> anyhow::Result<bool>;

    /// Bootstrap the first administrator.
    ///
    /// `password` is **plaintext**: the direct backend hashes it before storing
    /// and then assigns the global admin role, while the remote backend forwards
    /// the bootstrap to the core's `/v3/auth/user/admin`, which owns both steps.
    /// Handing the hash to the core instead would hash it twice.
    async fn create_admin_user(&self, username: &str, password: &str) -> anyhow::Result<()>;

    // ==================== Permission Operations ====================

    /// Search permissions with pagination.
    async fn permission_find_page(
        &self,
        role: &str,
        page_no: u64,
        page_size: u64,
        accurate: bool,
    ) -> anyhow::Result<Page<PermissionInfo>>;

    /// Look up one permission by role, resource and action.
    async fn permission_find_by_id(
        &self,
        role: &str,
        resource: &str,
        action: &str,
    ) -> anyhow::Result<Option<PermissionInfo>>;

    /// Grant a permission to a role.
    async fn permission_grant(
        &self,
        role: &str,
        resource: &str,
        action: &str,
    ) -> anyhow::Result<()>;

    /// Revoke a permission from a role.
    async fn permission_revoke(
        &self,
        role: &str,
        resource: &str,
        action: &str,
    ) -> anyhow::Result<()>;

    // ==================== Visibility Operations ====================

    /// Grant one visibility action on a resource to a user.
    ///
    /// `r` grants read visibility; `w` and `rw` grant read/write visibility.
    /// A blank `namespace_id` means the default namespace.
    async fn visibility_grant(
        &self,
        namespace_id: &str,
        resource_type: &str,
        resource_name: &str,
        username: &str,
        action: &str,
    ) -> anyhow::Result<()>;

    /// Revoke one visibility action on a resource from a user.
    async fn visibility_revoke(
        &self,
        namespace_id: &str,
        resource_type: &str,
        resource_name: &str,
        username: &str,
        action: &str,
    ) -> anyhow::Result<()>;
}

#[async_trait::async_trait]
impl<T: AuthPersistence + ?Sized> AuthDataService for T {
    async fn user_find_by_username(&self, username: &str) -> anyhow::Result<Option<UserInfo>> {
        AuthPersistence::user_find_by_username(self, username).await
    }

    async fn user_find_page(
        &self,
        username: &str,
        page_no: u64,
        page_size: u64,
        accurate: bool,
    ) -> anyhow::Result<Page<UserInfo>> {
        AuthPersistence::user_find_page(self, username, page_no, page_size, accurate).await
    }

    async fn user_create_with_source(
        &self,
        username: &str,
        password_hash: &str,
        enabled: bool,
        source: &str,
    ) -> anyhow::Result<()> {
        AuthPersistence::user_create_with_source(self, username, password_hash, enabled, source)
            .await
    }

    async fn user_update_password(
        &self,
        username: &str,
        password_hash: &str,
    ) -> anyhow::Result<()> {
        AuthPersistence::user_update_password(self, username, password_hash).await
    }

    async fn user_delete(&self, username: &str) -> anyhow::Result<()> {
        AuthPersistence::user_delete(self, username).await
    }

    async fn user_search(&self, username: &str) -> anyhow::Result<Vec<String>> {
        AuthPersistence::user_search(self, username).await
    }

    async fn role_find_by_username(&self, username: &str) -> anyhow::Result<Vec<RoleInfo>> {
        AuthPersistence::role_find_by_username(self, username).await
    }

    async fn role_find_page(
        &self,
        username: &str,
        role: &str,
        page_no: u64,
        page_size: u64,
        accurate: bool,
    ) -> anyhow::Result<Page<RoleInfo>> {
        AuthPersistence::role_find_page(self, username, role, page_no, page_size, accurate).await
    }

    async fn role_create(&self, role: &str, username: &str) -> anyhow::Result<()> {
        AuthPersistence::role_create(self, role, username).await
    }

    async fn role_delete(&self, role: &str, username: &str) -> anyhow::Result<()> {
        AuthPersistence::role_delete(self, role, username).await
    }

    async fn role_search(&self, role: &str) -> anyhow::Result<Vec<String>> {
        AuthPersistence::role_search(self, role).await
    }

    async fn role_has_global_admin_by_username(&self, username: &str) -> anyhow::Result<bool> {
        AuthPersistence::role_has_global_admin_by_username(self, username).await
    }

    async fn role_has_global_admin(&self) -> anyhow::Result<bool> {
        AuthPersistence::role_has_global_admin(self).await
    }

    async fn create_admin_user(&self, username: &str, password: &str) -> anyhow::Result<()> {
        let password_hash = hash_password(password)?;
        AuthPersistence::user_create(self, username, &password_hash, true).await?;
        AuthPersistence::role_create(self, GLOBAL_ADMIN_ROLE, username).await
    }

    async fn permission_find_page(
        &self,
        role: &str,
        page_no: u64,
        page_size: u64,
        accurate: bool,
    ) -> anyhow::Result<Page<PermissionInfo>> {
        AuthPersistence::permission_find_page(self, role, page_no, page_size, accurate).await
    }

    async fn permission_find_by_id(
        &self,
        role: &str,
        resource: &str,
        action: &str,
    ) -> anyhow::Result<Option<PermissionInfo>> {
        AuthPersistence::permission_find_by_id(self, role, resource, action).await
    }

    async fn permission_grant(
        &self,
        role: &str,
        resource: &str,
        action: &str,
    ) -> anyhow::Result<()> {
        AuthPersistence::permission_grant(self, role, resource, action).await
    }

    async fn permission_revoke(
        &self,
        role: &str,
        resource: &str,
        action: &str,
    ) -> anyhow::Result<()> {
        AuthPersistence::permission_revoke(self, role, resource, action).await
    }

    async fn visibility_grant(
        &self,
        namespace_id: &str,
        resource_type: &str,
        resource_name: &str,
        username: &str,
        action: &str,
    ) -> anyhow::Result<()> {
        grant_with(
            self,
            namespace_id,
            resource_type,
            resource_name,
            username,
            action,
        )
        .await
    }

    async fn visibility_revoke(
        &self,
        namespace_id: &str,
        resource_type: &str,
        resource_name: &str,
        username: &str,
        action: &str,
    ) -> anyhow::Result<()> {
        revoke_with(
            self,
            namespace_id,
            resource_type,
            resource_name,
            username,
            action,
        )
        .await
    }
}

/// Direct backend: adapts the process-local persistence to [`AuthDataService`].
///
/// Needed because `Arc<dyn PersistenceService>` cannot be coerced to
/// `Arc<dyn AuthDataService>`; the wrapper supplies the vtable.
pub struct PersistenceAuthDataService {
    inner: Arc<dyn PersistenceService>,
}

impl PersistenceAuthDataService {
    /// Wrap a persistence service as an auth data service.
    pub fn new(inner: Arc<dyn PersistenceService>) -> Self {
        Self { inner }
    }
}

#[async_trait::async_trait]
impl RbacSource for PersistenceAuthDataService {
    async fn load_roles(&self, username: &str) -> anyhow::Result<Vec<RoleInfo>> {
        RbacSource::load_roles(&*self.inner, username).await
    }

    async fn load_permissions(&self, roles: Vec<String>) -> anyhow::Result<Vec<PermissionInfo>> {
        RbacSource::load_permissions(&*self.inner, roles).await
    }
}

#[async_trait::async_trait]
impl AuthDataService for PersistenceAuthDataService {
    async fn user_find_by_username(&self, username: &str) -> anyhow::Result<Option<UserInfo>> {
        AuthDataService::user_find_by_username(&*self.inner, username).await
    }

    async fn user_find_page(
        &self,
        username: &str,
        page_no: u64,
        page_size: u64,
        accurate: bool,
    ) -> anyhow::Result<Page<UserInfo>> {
        AuthDataService::user_find_page(&*self.inner, username, page_no, page_size, accurate).await
    }

    async fn user_create_with_source(
        &self,
        username: &str,
        password_hash: &str,
        enabled: bool,
        source: &str,
    ) -> anyhow::Result<()> {
        AuthDataService::user_create_with_source(
            &*self.inner,
            username,
            password_hash,
            enabled,
            source,
        )
        .await
    }

    async fn user_update_password(
        &self,
        username: &str,
        password_hash: &str,
    ) -> anyhow::Result<()> {
        AuthDataService::user_update_password(&*self.inner, username, password_hash).await
    }

    async fn user_delete(&self, username: &str) -> anyhow::Result<()> {
        AuthDataService::user_delete(&*self.inner, username).await
    }

    async fn user_search(&self, username: &str) -> anyhow::Result<Vec<String>> {
        AuthDataService::user_search(&*self.inner, username).await
    }

    async fn role_find_by_username(&self, username: &str) -> anyhow::Result<Vec<RoleInfo>> {
        AuthDataService::role_find_by_username(&*self.inner, username).await
    }

    async fn role_find_page(
        &self,
        username: &str,
        role: &str,
        page_no: u64,
        page_size: u64,
        accurate: bool,
    ) -> anyhow::Result<Page<RoleInfo>> {
        AuthDataService::role_find_page(&*self.inner, username, role, page_no, page_size, accurate)
            .await
    }

    async fn role_create(&self, role: &str, username: &str) -> anyhow::Result<()> {
        AuthDataService::role_create(&*self.inner, role, username).await
    }

    async fn role_delete(&self, role: &str, username: &str) -> anyhow::Result<()> {
        AuthDataService::role_delete(&*self.inner, role, username).await
    }

    async fn role_search(&self, role: &str) -> anyhow::Result<Vec<String>> {
        AuthDataService::role_search(&*self.inner, role).await
    }

    async fn role_has_global_admin_by_username(&self, username: &str) -> anyhow::Result<bool> {
        AuthDataService::role_has_global_admin_by_username(&*self.inner, username).await
    }

    async fn role_has_global_admin(&self) -> anyhow::Result<bool> {
        AuthDataService::role_has_global_admin(&*self.inner).await
    }

    async fn create_admin_user(&self, username: &str, password: &str) -> anyhow::Result<()> {
        AuthDataService::create_admin_user(&*self.inner, username, password).await
    }

    async fn permission_find_page(
        &self,
        role: &str,
        page_no: u64,
        page_size: u64,
        accurate: bool,
    ) -> anyhow::Result<Page<PermissionInfo>> {
        AuthDataService::permission_find_page(&*self.inner, role, page_no, page_size, accurate)
            .await
    }

    async fn permission_find_by_id(
        &self,
        role: &str,
        resource: &str,
        action: &str,
    ) -> anyhow::Result<Option<PermissionInfo>> {
        AuthDataService::permission_find_by_id(&*self.inner, role, resource, action).await
    }

    async fn permission_grant(
        &self,
        role: &str,
        resource: &str,
        action: &str,
    ) -> anyhow::Result<()> {
        AuthDataService::permission_grant(&*self.inner, role, resource, action).await
    }

    async fn permission_revoke(
        &self,
        role: &str,
        resource: &str,
        action: &str,
    ) -> anyhow::Result<()> {
        AuthDataService::permission_revoke(&*self.inner, role, resource, action).await
    }

    async fn visibility_grant(
        &self,
        namespace_id: &str,
        resource_type: &str,
        resource_name: &str,
        username: &str,
        action: &str,
    ) -> anyhow::Result<()> {
        AuthDataService::visibility_grant(
            &*self.inner,
            namespace_id,
            resource_type,
            resource_name,
            username,
            action,
        )
        .await
    }

    async fn visibility_revoke(
        &self,
        namespace_id: &str,
        resource_type: &str,
        resource_name: &str,
        username: &str,
        action: &str,
    ) -> anyhow::Result<()> {
        AuthDataService::visibility_revoke(
            &*self.inner,
            namespace_id,
            resource_type,
            resource_name,
            username,
            action,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Stub {
        role_reads: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl RbacSource for Stub {
        async fn load_roles(&self, _username: &str) -> anyhow::Result<Vec<RoleInfo>> {
            self.role_reads.fetch_add(1, Ordering::SeqCst);
            Ok(vec![])
        }

        async fn load_permissions(
            &self,
            _roles: Vec<String>,
        ) -> anyhow::Result<Vec<PermissionInfo>> {
            Ok(vec![])
        }
    }

    #[async_trait::async_trait]
    impl AuthDataService for Stub {
        async fn user_find_by_username(&self, _username: &str) -> anyhow::Result<Option<UserInfo>> {
            Ok(None)
        }

        async fn user_find_page(
            &self,
            _username: &str,
            _page_no: u64,
            _page_size: u64,
            _accurate: bool,
        ) -> anyhow::Result<Page<UserInfo>> {
            Ok(Page::default())
        }

        async fn user_create_with_source(
            &self,
            _username: &str,
            _password_hash: &str,
            _enabled: bool,
            _source: &str,
        ) -> anyhow::Result<()> {
            Ok(())
        }

        async fn user_update_password(
            &self,
            _username: &str,
            _password_hash: &str,
        ) -> anyhow::Result<()> {
            Ok(())
        }

        async fn user_delete(&self, _username: &str) -> anyhow::Result<()> {
            Ok(())
        }

        async fn user_search(&self, _username: &str) -> anyhow::Result<Vec<String>> {
            Ok(vec![])
        }

        async fn role_find_by_username(&self, _username: &str) -> anyhow::Result<Vec<RoleInfo>> {
            Ok(vec![])
        }

        async fn role_find_page(
            &self,
            _username: &str,
            _role: &str,
            _page_no: u64,
            _page_size: u64,
            _accurate: bool,
        ) -> anyhow::Result<Page<RoleInfo>> {
            Ok(Page::default())
        }

        async fn role_create(&self, _role: &str, _username: &str) -> anyhow::Result<()> {
            Ok(())
        }

        async fn role_delete(&self, _role: &str, _username: &str) -> anyhow::Result<()> {
            Ok(())
        }

        async fn role_search(&self, _role: &str) -> anyhow::Result<Vec<String>> {
            Ok(vec![])
        }

        async fn role_has_global_admin_by_username(&self, _username: &str) -> anyhow::Result<bool> {
            Ok(false)
        }

        async fn role_has_global_admin(&self) -> anyhow::Result<bool> {
            Ok(false)
        }

        async fn create_admin_user(&self, _username: &str, _password: &str) -> anyhow::Result<()> {
            Ok(())
        }

        async fn permission_find_page(
            &self,
            _role: &str,
            _page_no: u64,
            _page_size: u64,
            _accurate: bool,
        ) -> anyhow::Result<Page<PermissionInfo>> {
            Ok(Page::default())
        }

        async fn permission_find_by_id(
            &self,
            _role: &str,
            _resource: &str,
            _action: &str,
        ) -> anyhow::Result<Option<PermissionInfo>> {
            Ok(None)
        }

        async fn permission_grant(
            &self,
            _role: &str,
            _resource: &str,
            _action: &str,
        ) -> anyhow::Result<()> {
            Ok(())
        }

        async fn permission_revoke(
            &self,
            _role: &str,
            _resource: &str,
            _action: &str,
        ) -> anyhow::Result<()> {
            Ok(())
        }

        async fn visibility_grant(
            &self,
            _namespace_id: &str,
            _resource_type: &str,
            _resource_name: &str,
            _username: &str,
            _action: &str,
        ) -> anyhow::Result<()> {
            Ok(())
        }

        async fn visibility_revoke(
            &self,
            _namespace_id: &str,
            _resource_type: &str,
            _resource_name: &str,
            _username: &str,
            _action: &str,
        ) -> anyhow::Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn auth_data_service_is_usable_as_a_trait_object() {
        let backend: Arc<dyn AuthDataService> = Arc::new(Stub {
            role_reads: AtomicUsize::new(0),
        });

        assert!(backend.load_roles("alice").await.unwrap().is_empty());
        assert!(
            backend
                .user_find_by_username("alice")
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn management_calls_reach_the_backend() {
        let backend: Arc<dyn AuthDataService> = Arc::new(Stub {
            role_reads: AtomicUsize::new(0),
        });

        backend.role_create("ROLE_X", "alice").await.unwrap();
        backend
            .permission_grant("ROLE_X", "res", "rw")
            .await
            .unwrap();
        assert!(backend.role_search("ROLE_X").await.unwrap().is_empty());
    }
}
