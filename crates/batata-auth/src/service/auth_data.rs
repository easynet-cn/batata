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

use std::sync::Arc;

use batata_persistence::{
    PersistenceService,
    model::{PermissionInfo, RoleInfo, UserInfo},
    traits::AuthPersistence,
};

use crate::service::rbac_cache::RbacSource;

/// Everything an auth plugin reads in order to authenticate and authorize.
///
/// Deliberately narrow: role and permission lookups (consumed through the
/// read-through RBAC cache) plus the single-user reads/writes the login and
/// LDAP provisioning paths need.
#[async_trait::async_trait]
pub trait AuthDataService: RbacSource + Send + Sync {
    /// Load a single user record, including its password hash and source.
    async fn user_find_by_username(&self, username: &str) -> anyhow::Result<Option<UserInfo>>;

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
}

#[async_trait::async_trait]
impl<T: AuthPersistence + ?Sized> AuthDataService for T {
    async fn user_find_by_username(&self, username: &str) -> anyhow::Result<Option<UserInfo>> {
        AuthPersistence::user_find_by_username(self, username).await
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

        async fn user_create_with_source(
            &self,
            _username: &str,
            _password_hash: &str,
            _enabled: bool,
            _source: &str,
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
}
