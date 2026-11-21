//! Atomic snapshot of every handle a Raft apply may dispatch to.
//!
//! Registration is startup-only, but an apply must never observe a *mixed*
//! generation (for example a new plugin registry together with the old naming
//! hook). Three independent `RwLock`s made that possible: `apply_hooks()` read
//! them one after another, and a concurrent registration could land between
//! two reads. All handles now live in one immutable [`HookSet`] behind a
//! single `Arc<RwLock<Arc<HookSet>>>`. Writers clone the set, mutate one
//! field, and atomically replace the inner `Arc`; readers take one read lock
//! and clone that `Arc` (O(1)), which also keeps a live apply snapshot valid
//! after a later registration.
//!
//! The read guard is always dropped before the blocking apply hop — a
//! `tokio::sync::RwLock` guard cannot be awaited on Tokio's blocking pool.

use std::collections::HashSet;
use std::sync::Arc;

use super::health_check_hook::HealthCheckApplyHook;
use super::naming_hook::NamingApplyHook;
use super::plugin::{PluginRegistry, RaftPluginHandler};
use super::state_machine::CORE_SNAPSHOT_CF_NAMES;

/// Owned, immutable set of handles used by one apply (or one snapshot build).
///
/// `Clone` is cheap: every field is an `Arc` or an `Option<Arc<_>>`. It is
/// used only by the startup registration path, never by the apply path.
#[derive(Clone)]
pub struct HookSet {
    /// Apply-back hook for persistent naming instances, installed after
    /// `NamingService` and `RaftNode` are both constructed.
    pub naming: Option<Arc<dyn NamingApplyHook>>,
    /// Apply-back hook for replicated health-check status, installed after
    /// `InstanceCheckRegistry` exists. Single-owner, see installer docs.
    pub health_check: Option<Arc<dyn HealthCheckApplyHook>>,
    /// Registry of plugin handlers for `PluginWrite` operations, ordered by
    /// plugin id so every replica enumerates snapshot CFs identically.
    pub plugins: Arc<PluginRegistry>,
    /// Ordered, de-duplicated snapshot column families derived from `plugins`.
    /// Cached at registration so `build_snapshot` only clones an `Arc`; the
    /// list and the registry it describes are always replaced together.
    pub snapshot_cf_names: Arc<Vec<String>>,
}

/// The single shared handle set, shared by `RaftNode`, `RocksStateMachine`
/// and the openraft snapshot builder.
pub type SharedHooks = Arc<tokio::sync::RwLock<Arc<HookSet>>>;

/// Derive the snapshot CF sequence for a registry.
///
/// Core CFs come first, then each plugin's declared CFs in plugin-id order
/// (the registry is a `BTreeMap`). Duplicates are dropped so every replica
/// writes the same sequence and produces byte-identical snapshot bodies.
/// Forgetting a plugin CF here silently drops that plugin's data from
/// snapshots, so this lives next to registration rather than in the core.
fn compute_snapshot_cf_names(plugins: &PluginRegistry) -> Arc<Vec<String>> {
    let mut names: Vec<String> = CORE_SNAPSHOT_CF_NAMES
        .iter()
        .map(|name| (*name).to_string())
        .collect();
    let mut seen: HashSet<String> = names.iter().cloned().collect();
    for name in plugins.all_snapshot_column_families() {
        if seen.insert(name.clone()) {
            names.push(name);
        }
    }
    Arc::new(names)
}

/// Create an empty hook set. Hooks and plugins are installed after both the
/// `RaftNode` and the services they mirror have been constructed.
pub fn new_shared_hooks() -> SharedHooks {
    let plugins = Arc::new(PluginRegistry::new());
    let snapshot_cf_names = compute_snapshot_cf_names(&plugins);
    Arc::new(tokio::sync::RwLock::new(Arc::new(HookSet {
        naming: None,
        health_check: None,
        plugins,
        snapshot_cf_names,
    })))
}

/// Register a plugin handler, replacing the whole set atomically.
///
/// A duplicate id is rejected *before* the inner `Arc` is replaced, so a
/// failed registration cannot leave the set half-updated. The `BTreeMap`
/// clone here is startup-only; `apply_hooks()` never clones the registry.
pub async fn register_plugin_handler(
    hooks: &SharedHooks,
    handler: Arc<dyn RaftPluginHandler>,
) -> Result<(), String> {
    let mut guard = hooks.write().await;
    let mut next = (**guard).clone();
    let mut registry = (*next.plugins).clone();
    registry.register(handler)?;
    next.plugins = Arc::new(registry);
    // The cache must describe exactly the registry being published, and both
    // must become visible in the same atomic swap.
    next.snapshot_cf_names = compute_snapshot_cf_names(&next.plugins);
    *guard = Arc::new(next);
    Ok(())
}

/// Install the naming apply hook. A second installation replaces the first
/// (the original slot used direct assignment), while every other field of
/// the set is preserved.
pub async fn install_naming_hook(hooks: &SharedHooks, hook: Arc<dyn NamingApplyHook>) {
    let mut guard = hooks.write().await;
    let mut next = (**guard).clone();
    next.naming = Some(hook);
    *guard = Arc::new(next);
}

/// Install the health-check apply hook, reporting whether it displaced one.
///
/// The slot is single-owner, so losing a hook means committed health-check
/// updates stop reaching that facade's registry. That surfaces much later as
/// stale health state rather than as an error, so replacement is never silent.
pub async fn install_health_check_hook(
    hooks: &SharedHooks,
    hook: Arc<dyn HealthCheckApplyHook>,
) -> bool {
    let mut guard = hooks.write().await;
    let mut next = (**guard).clone();
    let displaced = next.health_check.replace(hook).is_some();
    if displaced {
        tracing::warn!(
            "health-check apply hook replaced: this slot supports a single owner, \
             so the previously registered facade will no longer receive committed \
             health-check updates"
        );
    }
    *guard = Arc::new(next);
    displaced
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raft::request::RaftResponse;
    use rocksdb::DB;

    struct NoopPluginHandler;

    impl RaftPluginHandler for NoopPluginHandler {
        fn plugin_id(&self) -> &str {
            "noop"
        }

        fn column_families(&self) -> Vec<String> {
            Vec::new()
        }

        fn apply(
            &self,
            _db: &DB,
            _op_type: &str,
            _payload: &[u8],
            _log_index: u64,
        ) -> RaftResponse {
            RaftResponse::success()
        }

        fn build_snapshot(&self, _db: &DB) -> Result<Vec<u8>, String> {
            Ok(Vec::new())
        }

        fn install_snapshot(&self, _db: &DB, _data: &[u8]) -> Result<(), String> {
            Ok(())
        }
    }

    struct NoopNamingHook;

    impl NamingApplyHook for NoopNamingHook {
        fn on_register(
            &self,
            _namespace_id: &str,
            _group_name: &str,
            _service_name: &str,
            _instance_id: &str,
            _ip: &str,
            _port: u16,
            _weight: f64,
            _healthy: bool,
            _enabled: bool,
            _metadata: &str,
            _cluster_name: &str,
        ) {
        }

        fn on_deregister(
            &self,
            _namespace_id: &str,
            _group_name: &str,
            _service_name: &str,
            _instance_id: &str,
        ) {
        }

        fn on_update(
            &self,
            _namespace_id: &str,
            _group_name: &str,
            _service_name: &str,
            _instance_id: &str,
            _ip: Option<&str>,
            _port: Option<u16>,
            _weight: Option<f64>,
            _healthy: Option<bool>,
            _enabled: Option<bool>,
            _metadata: Option<&str>,
        ) {
        }
    }

    struct NoopHealthHook;

    impl HealthCheckApplyHook for NoopHealthHook {
        fn on_status_update(
            &self,
            _check_key: &str,
            _success: bool,
            _output: &str,
            _response_time_ms: u64,
            _timestamp_ms: i64,
        ) {
        }

        fn on_ttl_update(
            &self,
            _check_key: &str,
            _status: &str,
            _output: Option<&str>,
            _timestamp_ms: i64,
        ) {
        }
    }

    struct CfPluginHandler {
        id: &'static str,
        cf: String,
    }

    impl RaftPluginHandler for CfPluginHandler {
        fn plugin_id(&self) -> &str {
            self.id
        }

        fn column_families(&self) -> Vec<String> {
            vec![self.cf.clone()]
        }

        fn apply(
            &self,
            _db: &DB,
            _op_type: &str,
            _payload: &[u8],
            _log_index: u64,
        ) -> RaftResponse {
            RaftResponse::success()
        }

        fn build_snapshot(&self, _db: &DB) -> Result<Vec<u8>, String> {
            Ok(Vec::new())
        }

        fn install_snapshot(&self, _db: &DB, _data: &[u8]) -> Result<(), String> {
            Ok(())
        }
    }

    /// The cached CF list must be derived from the very registry being
    /// published, stay stable/deduplicated, and keep plugin-id ordering.
    #[tokio::test]
    async fn registration_recomputes_the_cached_snapshot_cf_names() {
        let hooks = new_shared_hooks();
        let initial = Arc::clone(&hooks.read().await.snapshot_cf_names);
        assert_eq!(initial.len(), CORE_SNAPSHOT_CF_NAMES.len());
        for (index, core) in CORE_SNAPSHOT_CF_NAMES.iter().enumerate() {
            assert_eq!(initial[index], *core, "core CFs must come first");
        }

        register_plugin_handler(
            &hooks,
            Arc::new(CfPluginHandler {
                id: "zeta",
                cf: "cf_zeta".to_string(),
            }),
        )
        .await
        .expect("register zeta");
        let after_one = Arc::clone(&hooks.read().await.snapshot_cf_names);
        assert!(!Arc::ptr_eq(&initial, &after_one));
        assert_eq!(after_one.len(), CORE_SNAPSHOT_CF_NAMES.len() + 1);
        assert_eq!(after_one.last().map(String::as_str), Some("cf_zeta"));

        // Registering a lower plugin id appends its CF in BTreeMap order,
        // regardless of registration order.
        register_plugin_handler(
            &hooks,
            Arc::new(CfPluginHandler {
                id: "alpha",
                cf: "cf_alpha".to_string(),
            }),
        )
        .await
        .expect("register alpha");
        let after_two = Arc::clone(&hooks.read().await.snapshot_cf_names);
        assert_eq!(
            &after_two[CORE_SNAPSHOT_CF_NAMES.len()..],
            &["cf_alpha".to_string(), "cf_zeta".to_string()],
            "plugin CFs must be ordered by plugin id on every replica"
        );

        // A plugin CF that duplicates a core CF is dropped exactly once.
        register_plugin_handler(
            &hooks,
            Arc::new(CfPluginHandler {
                id: "dup",
                cf: CORE_SNAPSHOT_CF_NAMES[0].to_string(),
            }),
        )
        .await
        .expect("register duplicate CF");
        let after_three = Arc::clone(&hooks.read().await.snapshot_cf_names);
        assert_eq!(after_three.len(), CORE_SNAPSHOT_CF_NAMES.len() + 2);
        assert_eq!(
            after_three
                .iter()
                .filter(|name| *name == CORE_SNAPSHOT_CF_NAMES[0])
                .count(),
            1,
            "duplicate CFs must stay deduplicated"
        );
    }

    /// Replicas may register the same plugins in different orders (startup is
    /// not coordinated). Their cached CF sequences must still be identical and
    /// in plugin-id order, otherwise `build_snapshot` streams different column
    /// families on different nodes and the snapshot bodies stop being
    /// byte-identical for the same state.
    #[tokio::test]
    async fn replicas_with_different_registration_orders_share_the_cf_sequence() {
        let replica_a = new_shared_hooks();
        let replica_b = new_shared_hooks();

        for (id, cf) in [("alpha", "cf_alpha"), ("zeta", "cf_zeta")] {
            register_plugin_handler(
                &replica_a,
                Arc::new(CfPluginHandler {
                    id,
                    cf: cf.to_string(),
                }),
            )
            .await
            .expect("register on replica A");
        }
        for (id, cf) in [("zeta", "cf_zeta"), ("alpha", "cf_alpha")] {
            register_plugin_handler(
                &replica_b,
                Arc::new(CfPluginHandler {
                    id,
                    cf: cf.to_string(),
                }),
            )
            .await
            .expect("register on replica B");
        }

        let a = Arc::clone(&replica_a.read().await.snapshot_cf_names);
        let b = Arc::clone(&replica_b.read().await.snapshot_cf_names);
        assert_eq!(
            a, b,
            "replicas must stream the same CF sequence into their snapshots"
        );

        let mut expected = CORE_SNAPSHOT_CF_NAMES
            .iter()
            .map(|name| (*name).to_string())
            .collect::<Vec<_>>();
        expected.push("cf_alpha".to_string());
        expected.push("cf_zeta".to_string());
        assert_eq!(
            *a, expected,
            "CFs must stay core-first and ordered by plugin id, not registration order"
        );
    }

    #[tokio::test]
    async fn registration_replaces_the_set_without_mutating_a_held_snapshot() {
        let hooks = new_shared_hooks();
        let before = Arc::clone(&*hooks.read().await);
        assert!(before.plugins.get("noop").is_none());

        register_plugin_handler(&hooks, Arc::new(NoopPluginHandler))
            .await
            .expect("register plugin");

        let after = Arc::clone(&*hooks.read().await);
        assert!(!Arc::ptr_eq(&before, &after));
        assert!(after.plugins.get("noop").is_some());
        assert!(
            before.plugins.get("noop").is_none(),
            "an apply snapshot already handed to the blocking pool must stay immutable"
        );
    }

    #[tokio::test]
    async fn failed_registration_leaves_the_set_untouched() {
        let hooks = new_shared_hooks();
        register_plugin_handler(&hooks, Arc::new(NoopPluginHandler))
            .await
            .expect("first registration");
        let before = Arc::clone(&*hooks.read().await);

        let err = register_plugin_handler(&hooks, Arc::new(NoopPluginHandler))
            .await
            .expect_err("duplicate plugin id must fail");
        assert!(err.contains("already registered"));

        let after = Arc::clone(&*hooks.read().await);
        assert!(
            Arc::ptr_eq(&before, &after),
            "a rejected registration must not replace the inner Arc"
        );
    }

    #[tokio::test]
    async fn installing_a_naming_hook_preserves_every_other_slot() {
        let hooks = new_shared_hooks();
        install_health_check_hook(&hooks, Arc::new(NoopHealthHook)).await;
        register_plugin_handler(&hooks, Arc::new(NoopPluginHandler))
            .await
            .expect("register plugin");

        install_naming_hook(&hooks, Arc::new(NoopNamingHook)).await;

        let current = Arc::clone(&*hooks.read().await);
        assert!(current.naming.is_some());
        assert!(current.health_check.is_some());
        assert!(current.plugins.get("noop").is_some());
    }

    #[tokio::test]
    async fn health_check_install_reports_whether_it_displaced_a_hook() {
        let hooks = new_shared_hooks();
        assert!(!install_health_check_hook(&hooks, Arc::new(NoopHealthHook)).await);
        assert!(install_health_check_hook(&hooks, Arc::new(NoopHealthHook)).await);
        assert!(hooks.read().await.health_check.is_some());
    }

    /// Architecture guard: reverting to three independent locks, mutating
    /// the live set in place, or bypassing the shared installers must turn
    /// this test red.
    #[test]
    fn production_uses_one_atomic_hook_set() {
        let sm = include_str!("state_machine/mod.rs");
        let node = include_str!("node.rs");
        let source = include_str!("hooks.rs");
        let tests_start = source
            .find("#[cfg(test)]\nmod tests")
            .expect("tests module must exist");
        let production = &source[..tests_start];

        let start = sm
            .find("async fn apply_hooks(&self) -> ApplyHooks")
            .expect("production apply_hooks must exist");
        let end = sm[start..]
            .find("    fn apply_request(")
            .expect("apply_request must exist");
        let body = &sm[start..start + end];
        assert_eq!(
            body.matches("read().await").count(),
            1,
            "apply_hooks must take exactly one read lock"
        );
        assert!(
            body.contains("self.hooks.read().await") && body.contains("Arc::clone(&guard)"),
            "apply_hooks must clone the single inner Arc<HookSet>"
        );
        assert!(
            !body.contains("self.naming_hook") && !body.contains("self.health_check_hook"),
            "no per-slot hook lock may remain on the apply path"
        );
        assert!(
            !body.contains("PluginRegistry::clone("),
            "apply_hooks must never clone the registry BTreeMap"
        );

        assert!(
            sm.contains("hooks: SharedHooks") && node.contains("hooks: SharedHooks"),
            "both the state machine and RaftNode must hold the shared set"
        );
        assert_eq!(
            production.matches("*guard = Arc::new(next);").count(),
            3,
            "every installer must replace the whole inner Arc, not mutate it in place"
        );
        assert!(
            production
                .contains("next.snapshot_cf_names = compute_snapshot_cf_names(&next.plugins);"),
            "registration must recompute the cached CF list in the same atomic swap"
        );
        assert!(
            sm.contains("Arc::clone(&guard.snapshot_cf_names)"),
            "build_snapshot must read the cached CF list instead of recomputing it"
        );
        assert!(
            production.contains("pub type SharedHooks = Arc<tokio::sync::RwLock<Arc<HookSet>>>"),
            "the shared set must stay behind a single lock"
        );
        assert!(
            node.contains("hooks::install_health_check_hook")
                && node.contains("hooks::install_naming_hook"),
            "RaftNode must route hook installation through the shared installers"
        );
    }
}
