//! Read-through RBAC cache shared by the auth plugins.
//!
//! Mirrors the caching layer Nacos places in its default auth plugin
//! (`AbstractCachedRoleService`): the cache lives in the *service* layer and is
//! backed by the auth persistence trait, so it covers every storage mode
//! (embedded / distributed / sql) rather than only the SeaORM direct
//! connection path.
//!
//! # Consistency model
//!
//! Entries expire on a TTL, and writes call [`invalidate_user`] /
//! [`invalidate_role`] so permission changes become visible immediately on the
//! node that applied them.
//!
//! In a cluster each node holds its own caches, so [`invalidate_user`] and
//! friends additionally notify the peers through [`set_peer_invalidator`],
//! which `batata-server` wires to the gRPC invalidation broadcast. Peers then
//! evict the same entries locally via [`invalidate_user_local`] — the `*_local`
//! variants deliberately do **not** notify, otherwise an eviction received from
//! a peer would trigger a broadcast back and loop across the cluster. A peer
//! that is unreachable falls back to TTL expiry, which is what Nacos does for
//! every node.

use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Duration;

use batata_persistence::model::{PermissionInfo, RoleInfo};
use batata_persistence::traits::AuthPersistence;
use moka::sync::Cache;

use crate::model::GLOBAL_ADMIN_ROLE;

/// Narrow read interface the RBAC cache needs.
///
/// Blanket-implemented for every auth persistence backend, which keeps the
/// cache usable from `Arc<dyn PersistenceService>` while staying trivial to
/// stub in tests.
#[async_trait::async_trait]
pub trait RbacSource {
    /// Load the role bindings of a single user.
    async fn load_roles(&self, username: &str) -> anyhow::Result<Vec<RoleInfo>>;

    /// Load the permission bindings of the given roles.
    async fn load_permissions(&self, roles: Vec<String>) -> anyhow::Result<Vec<PermissionInfo>>;
}

#[async_trait::async_trait]
impl<T: AuthPersistence + ?Sized> RbacSource for T {
    async fn load_roles(&self, username: &str) -> anyhow::Result<Vec<RoleInfo>> {
        AuthPersistence::role_find_by_username(self, username).await
    }

    async fn load_permissions(&self, roles: Vec<String>) -> anyhow::Result<Vec<PermissionInfo>> {
        AuthPersistence::permission_find_by_roles(self, roles).await
    }
}

/// Whether auth lookups may be served from cache.
///
/// Backed by `batata.core.auth.caching.enabled` — the Batata equivalent of
/// Nacos' `nacos.core.auth.caching.enabled`. When false every lookup reads
/// straight from the storage layer, which is how benchmarks measure the cache
/// contribution on a single binary.
fn enabled() -> bool {
    crate::service::auth::cache_config().enabled
}

/// Build a cache with the given capacity and TTL.
fn build_cache<K, V>(capacity: u64, ttl_secs: u64) -> Cache<K, V>
where
    K: std::hash::Hash + Eq + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    Cache::builder()
        .max_capacity(capacity)
        .time_to_live(Duration::from_secs(ttl_secs))
        .build()
}

/// Return the cache sizing rules the server installed, or the defaults.
fn config() -> crate::service::auth::AuthCacheConfig {
    crate::service::auth::cache_config()
}

/// Cache of `username -> role bindings`.
static USER_ROLES_CACHE: OnceLock<Cache<String, Vec<RoleInfo>>> = OnceLock::new();

/// Cache of `role -> permission bindings`.
static ROLE_PERMISSIONS_CACHE: OnceLock<Cache<String, Vec<PermissionInfo>>> = OnceLock::new();

/// Cache of `username -> holds the global admin role`.
static GLOBAL_ADMIN_CACHE: OnceLock<Cache<String, bool>> = OnceLock::new();

fn user_roles_cache() -> &'static Cache<String, Vec<RoleInfo>> {
    USER_ROLES_CACHE.get_or_init(|| {
        let config = config();

        build_cache(config.roles_capacity, config.roles_ttl_secs)
    })
}

fn role_permissions_cache() -> &'static Cache<String, Vec<PermissionInfo>> {
    ROLE_PERMISSIONS_CACHE.get_or_init(|| {
        let config = config();

        build_cache(config.permissions_capacity, config.permissions_ttl_secs)
    })
}

fn global_admin_cache() -> &'static Cache<String, bool> {
    GLOBAL_ADMIN_CACHE.get_or_init(|| {
        let config = config();

        build_cache(config.roles_capacity, config.roles_ttl_secs)
    })
}

/// Rebuild the RBAC caches with the given capacities and TTLs.
///
/// Called from `init_auth_caches` so all auth caches share one configuration
/// source. Setting is a no-op once a cache is built, so the first caller wins
/// and a later reconfiguration cannot silently drop live entries.
pub fn init_caches(
    roles_capacity: u64,
    roles_ttl_secs: u64,
    permissions_capacity: u64,
    permissions_ttl_secs: u64,
) {
    let _ = USER_ROLES_CACHE.set(build_cache(roles_capacity, roles_ttl_secs));
    let _ = GLOBAL_ADMIN_CACHE.set(build_cache(roles_capacity, roles_ttl_secs));
    let _ = ROLE_PERMISSIONS_CACHE.set(build_cache(permissions_capacity, permissions_ttl_secs));
}

/// Return the role bindings of `username`, reading through the cache.
///
/// A failed or empty lookup is cached as an empty binding list so that
/// repeated requests for a user without roles do not hammer the storage layer.
pub async fn roles<P>(persistence: &P, username: &str) -> Vec<RoleInfo>
where
    P: RbacSource + ?Sized,
{
    if !enabled() {
        return persistence.load_roles(username).await.unwrap_or_default();
    }

    if let Some(cached) = user_roles_cache().get(username) {
        return cached;
    }

    let roles = persistence.load_roles(username).await.unwrap_or_default();

    user_roles_cache().insert(username.to_owned(), roles.clone());

    roles
}

/// Return whether `username` holds the global admin role, reading through the
/// cache.
///
/// On a miss the answer is derived from the cached role bindings rather than
/// from a dedicated storage lookup, mirroring
/// `AbstractCheckedRoleService.hasGlobalAdminRole` in Nacos.
pub async fn is_global_admin<P>(persistence: &P, username: &str) -> bool
where
    P: RbacSource + ?Sized,
{
    if !enabled() {
        return roles(persistence, username)
            .await
            .iter()
            .any(|role| role.role == GLOBAL_ADMIN_ROLE);
    }

    if let Some(cached) = global_admin_cache().get(username) {
        return cached;
    }

    let admin = roles(persistence, username)
        .await
        .iter()
        .any(|role| role.role == GLOBAL_ADMIN_ROLE);

    global_admin_cache().insert(username.to_owned(), admin);

    admin
}

/// Return the permission bindings granted to any of `roles`, reading through
/// the cache.
///
/// Permissions are cached per role (as in Nacos) so a user whose role set
/// changes still reuses the entries of the roles they kept.
pub async fn permissions<P>(persistence: &P, roles: &[RoleInfo]) -> Vec<PermissionInfo>
where
    P: RbacSource + ?Sized,
{
    if !enabled() {
        let role_names: Vec<String> = roles.iter().map(|role| role.role.clone()).collect();

        return persistence
            .load_permissions(role_names)
            .await
            .unwrap_or_default();
    }

    let mut merged: Vec<PermissionInfo> = Vec::new();
    let mut missing: Vec<String> = Vec::new();

    for role in roles {
        match role_permissions_cache().get(&role.role) {
            Some(cached) => merged.extend(cached),
            None => missing.push(role.role.clone()),
        }
    }

    if !missing.is_empty() {
        let fetched = persistence
            .load_permissions(missing.clone())
            .await
            .unwrap_or_default();

        for role in &missing {
            let own: Vec<PermissionInfo> = fetched
                .iter()
                .filter(|permission| &permission.role == role)
                .cloned()
                .collect();

            // Cache empty results too, so roles without permissions are not
            // re-fetched on every request.
            role_permissions_cache().insert(role.clone(), own.clone());
            merged.extend(own);
        }
    }

    merged
}

/// Mirrors a local invalidation to the peer nodes of the cluster.
///
/// Installed once at startup by `batata-server` when the cluster client exists;
/// in standalone mode it stays unset and invalidation remains purely local.
/// Arguments are the invalidation kind (`"user"` / `"role"` / `"all"`) and the
/// target (username, role name, or empty for `"all"`).
static PEER_INVALIDATOR: OnceLock<Arc<dyn Fn(&str, &str) + Send + Sync>> = OnceLock::new();

/// Install the peer notifier. A no-op once one is installed.
pub fn set_peer_invalidator(hook: Arc<dyn Fn(&str, &str) + Send + Sync>) {
    let _ = PEER_INVALIDATOR.set(hook);
}

/// Forward an invalidation to the peers, if a notifier is installed.
fn notify_peers(kind: &str, target: &str) {
    if let Some(hook) = PEER_INVALIDATOR.get() {
        hook(kind, target);
    }
}

/// Drop every cached entry derived from `username`'s role bindings.
///
/// Call this after granting or revoking a role for a user. Peers are notified
/// so the change is visible on every node, not just the one that applied it.
pub fn invalidate_user(username: &str) {
    invalidate_user_local(username);
    notify_peers("user", username);
}

/// Drop the local entries of `username` without notifying the peers.
///
/// Used by the cluster handler that *receives* an eviction from a peer.
pub fn invalidate_user_local(username: &str) {
    invalidate_and_drain(user_roles_cache(), username);
    invalidate_and_drain(global_admin_cache(), username);
}

/// Drop the cached permission bindings of `role`.
///
/// Call this after granting or revoking a permission on a role.
pub fn invalidate_role(role: &str) {
    invalidate_role_local(role);
    notify_peers("role", role);
}

/// Drop the local permission bindings of `role` without notifying the peers.
///
/// Used by the cluster handler that *receives* an eviction from a peer.
pub fn invalidate_role_local(role: &str) {
    invalidate_and_drain(role_permissions_cache(), role);
}

/// Drop every cached user role binding.
///
/// Reserved for operations that touch the bindings of many users at once, such
/// as deleting a role for all of its holders.
pub fn invalidate_all_users() {
    invalidate_all_users_local();
    notify_peers("all", "");
}

/// Drop every local user role binding without notifying the peers.
///
/// Used by the cluster handler that *receives* an eviction from a peer.
pub fn invalidate_all_users_local() {
    invalidate_all_and_drain(user_roles_cache());
    invalidate_all_and_drain(global_admin_cache());
}

/// Remove `key` and wait until the removal is visible to the next lookup.
///
/// `moka` hands removals to a background housekeeper, so an entry queued for
/// eviction can still be served to a reader — and, worse, can be applied
/// *after* that reader refilled the cache from storage, resurrecting the stale
/// value the invalidation was meant to drop. Draining the queue here keeps
/// invalidation synchronous: after `invalidate_user`, the next lookup reads
/// storage exactly once, which is what the callers (and the tests) rely on.
pub(crate) fn invalidate_and_drain<K, V, Q>(cache: &Cache<K, V>, key: &Q)
where
    K: std::hash::Hash + Eq + Send + Sync + std::borrow::Borrow<Q> + 'static,
    Q: std::hash::Hash + Eq + ?Sized,
    V: Clone + Send + Sync + 'static,
{
    cache.invalidate(key);
    cache.run_pending_tasks();
}

/// Wait until every queued removal on `cache` is visible to the next lookup.
///
/// Use after invalidating several keys one by one, where a full
/// [`invalidate_all_and_drain`] would drop entries nobody asked to drop.
pub(crate) fn drain_pending_invalidations<K, V>(cache: &Cache<K, V>)
where
    K: std::hash::Hash + Eq + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    cache.run_pending_tasks();
}

/// [`invalidate_and_drain`] for a whole cache.
pub(crate) fn invalidate_all_and_drain<K, V>(cache: &Cache<K, V>)
where
    K: std::hash::Hash + Eq + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    cache.invalidate_all();
    cache.run_pending_tasks();
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Minimal auth backend that counts role reads and returns a fixed shape.
    struct StubPersistence {
        role_reads: AtomicUsize,
        permission_reads: AtomicUsize,
    }

    impl StubPersistence {
        fn new() -> Self {
            Self {
                role_reads: AtomicUsize::new(0),
                permission_reads: AtomicUsize::new(0),
            }
        }

        fn role_reads(&self) -> usize {
            self.role_reads.load(Ordering::SeqCst)
        }

        fn permission_reads(&self) -> usize {
            self.permission_reads.load(Ordering::SeqCst)
        }
    }

    #[async_trait::async_trait]
    impl RbacSource for StubPersistence {
        async fn load_roles(&self, username: &str) -> anyhow::Result<Vec<RoleInfo>> {
            self.role_reads.fetch_add(1, Ordering::SeqCst);

            if username == "admin" {
                return Ok(vec![RoleInfo {
                    role: GLOBAL_ADMIN_ROLE.to_owned(),
                    username: username.to_owned(),
                }]);
            }

            Ok(vec![])
        }

        async fn load_permissions(
            &self,
            roles: Vec<String>,
        ) -> anyhow::Result<Vec<PermissionInfo>> {
            self.permission_reads.fetch_add(1, Ordering::SeqCst);

            Ok(roles
                .into_iter()
                .map(|role| PermissionInfo {
                    role,
                    resource: "public:*".to_owned(),
                    action: "rw".to_owned(),
                })
                .collect())
        }
    }

    /// The RBAC caches are process-global, and `invalidate_all_users` drops
    /// *every* entry, so a test that counts storage reads cannot run next to
    /// it: the wipe would land between two lookups and look like a cache miss.
    static CACHE_TEST_LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> =
        std::sync::OnceLock::new();

    async fn lock_caches() -> tokio::sync::MutexGuard<'static, ()> {
        CACHE_TEST_LOCK
            .get_or_init(|| tokio::sync::Mutex::new(()))
            .lock()
            .await
    }

    #[tokio::test]
    async fn global_admin_lookup_reads_storage_once() {
        let _guard = lock_caches().await;

        invalidate_user("admin");
        let backend = StubPersistence::new();

        assert!(is_global_admin(&backend, "admin").await);
        assert!(is_global_admin(&backend, "admin").await);
        assert!(is_global_admin(&backend, "admin").await);

        assert_eq!(backend.role_reads(), 1);
    }

    #[tokio::test]
    async fn global_admin_lookup_is_false_for_plain_user() {
        let _guard = lock_caches().await;

        invalidate_user("plain-user");
        let backend = StubPersistence::new();

        assert!(!is_global_admin(&backend, "plain-user").await);

        invalidate_user("plain-user");
        assert!(!is_global_admin(&backend, "plain-user").await);
    }

    #[tokio::test]
    async fn permissions_are_cached_per_role() {
        let _guard = lock_caches().await;

        invalidate_role("ROLE_A");
        invalidate_role("ROLE_B");
        let backend = StubPersistence::new();

        let first = vec![
            RoleInfo {
                role: "ROLE_A".to_owned(),
                username: "u1".to_owned(),
            },
            RoleInfo {
                role: "ROLE_B".to_owned(),
                username: "u1".to_owned(),
            },
        ];
        let second = vec![RoleInfo {
            role: "ROLE_A".to_owned(),
            username: "u1".to_owned(),
        }];

        assert_eq!(permissions(&backend, &first).await.len(), 2);
        assert_eq!(backend.permission_reads(), 1);

        // ROLE_A and ROLE_B are both cached now, so no further read happens.
        assert_eq!(permissions(&backend, &second).await.len(), 1);
        assert_eq!(backend.permission_reads(), 1);

        // Invalidating one role refetches only that role.
        invalidate_role("ROLE_B");
        assert_eq!(permissions(&backend, &first).await.len(), 2);
        assert_eq!(backend.permission_reads(), 2);
    }

    #[tokio::test]
    async fn invalidate_user_forces_refetch() {
        let _guard = lock_caches().await;

        invalidate_user("cycle-user");
        let backend = StubPersistence::new();

        assert_eq!(roles(&backend, "cycle-user").await.len(), 0);
        assert_eq!(roles(&backend, "cycle-user").await.len(), 0);
        assert_eq!(backend.role_reads(), 1);

        invalidate_user("cycle-user");
        assert_eq!(roles(&backend, "cycle-user").await.len(), 0);
        assert_eq!(backend.role_reads(), 2);
    }

    #[tokio::test]
    async fn invalidate_all_users_clears_role_caches() {
        let _guard = lock_caches().await;

        user_roles_cache().insert("bulk-user".to_owned(), vec![]);
        global_admin_cache().insert("bulk-user".to_owned(), false);

        invalidate_all_users();

        assert!(user_roles_cache().get("bulk-user").is_none());
        assert!(global_admin_cache().get("bulk-user").is_none());
    }
}
