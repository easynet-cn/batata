//! Verifies that `batata.core.auth.caching.enabled=false` bypasses the caches.
//!
//! A dedicated integration-test binary: the switch is process-global, so
//! toggling it from a unit test would race with the cache tests that run in the
//! same binary and expect caching to be on.

use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use batata_auth::service::rbac_cache::{RbacSource, is_global_admin, roles};
use batata_persistence::model::{PermissionInfo, RoleInfo};

/// Minimal auth backend that counts the reads reaching the storage layer.
struct CountingBackend {
    role_reads: AtomicUsize,
}

impl CountingBackend {
    fn new() -> Self {
        Self {
            role_reads: AtomicUsize::new(0),
        }
    }

    fn role_reads(&self) -> usize {
        self.role_reads.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl RbacSource for CountingBackend {
    async fn load_roles(&self, _username: &str) -> anyhow::Result<Vec<RoleInfo>> {
        self.role_reads.fetch_add(1, Ordering::SeqCst);

        Ok(vec![])
    }

    async fn load_permissions(&self, _roles: Vec<String>) -> anyhow::Result<Vec<PermissionInfo>> {
        Ok(vec![])
    }
}

#[tokio::test]
async fn disabled_caching_reads_storage_on_every_lookup() {
    batata_auth::service::auth::init_auth_caches(batata_auth::service::auth::AuthCacheConfig {
        enabled: false,
        ..Default::default()
    });

    let backend = CountingBackend::new();

    roles(&backend, "bench-user").await;
    roles(&backend, "bench-user").await;
    assert!(!is_global_admin(&backend, "bench-user").await);

    // Two role lookups plus the one behind `is_global_admin`: with caching
    // disabled none of them may be served from cache.
    assert_eq!(backend.role_reads(), 3);
}
