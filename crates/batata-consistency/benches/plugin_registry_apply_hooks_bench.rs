//! Micro-benchmark: three generations of the Raft apply-handle snapshot.
//!
//! Generation 1 (commit `bf856219`) cloned the whole plugin registry on every
//! Raft apply:
//!
//! ```ignore
//! let registry = self.plugin_registry.read().await;
//! ApplyHooks {
//!     naming: self.naming_hook.read().await.clone(),
//!     health_check: self.health_check_hook.read().await.clone(),
//!     plugins: Arc::new(PluginRegistry::clone(&registry)),
//! }
//! ```
//!
//! `PluginRegistry` is a `BTreeMap<String, Arc<dyn RaftPluginHandler>>`, so that
//! is an O(n) tree copy plus one heap allocation per apply.
//!
//! Generation 2 (P2, uncommitted at the time) replaced the registry field with
//! `Arc<RwLock<Arc<PluginRegistry>>>`: three read locks, but only one O(1)
//! `Arc::clone` for the registry. Generation 3 (P1, current production)
//! collapses all three slots into one `Arc<RwLock<Arc<HookSet>>>`, so one read
//! lock and one `Arc::clone` yield an atomically consistent set.
//!
//! All three legs are reproduced verbatim, including guard lifetimes, so the
//! deltas are attributable to the snapshot shape alone. The bench also scales
//! the registry size to expose the O(n) clone cost as plugins are added.
//!
//! End-to-end perf cannot resolve these deltas: the Raft `Config Write` path is
//! ~6.9 ms per request, while the same-binary run-to-run noise band is 7.8%.
//! A function-level measurement is the only honest way to attribute them.
//!
//! ## Measured on 2026-11-21 (criterion defaults, lower bound of CI)
//!
//!   n     gen1 bf856219   gen2 P2 (3 locks)   gen3 P1 (1 Arc)   P2 -> P1
//!   2       124.73 ns         65.60 ns           46.12 ns        -29.7% / 1.42x
//!   8       190.29 ns         65.59 ns           46.25 ns
//!   32      570.96 ns         65.67 ns           46.12 ns
//!   128       2.130 us        65.70 ns           46.98 ns
//!
//! P1 is flat in the number of plugins (one `Arc` clone). The 19.5 ns saved
//! over P2 is 0.00028% of a 6.9 ms Config Write, ~28000x below the noise band,
//! so the value of P1 is the atomic hook generation and a single lock, not
//! throughput. Pure registry clone primitive: old O(n) 14.94 ns (n=0) to
//! 2.082 us (n=128), new `Arc` clone a constant ~3.17 ns.
//!
//! Run with `cargo bench -p batata-consistency --bench plugin_registry_apply_hooks_bench`.

use std::hint::black_box;
use std::sync::Arc;

use batata_consistency::raft::health_check_hook::{
    HealthCheckApplyHook, SharedHealthCheckHook, new_shared_health_check_hook,
};
use batata_consistency::raft::naming_hook::{
    NamingApplyHook, SharedNamingHook, new_shared_naming_hook,
};
use batata_consistency::raft::plugin::{PluginRegistry, RaftPluginHandler, SharedPluginRegistry};
use batata_consistency::raft::request::RaftResponse;
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use rocksdb::DB;
use tokio::sync::RwLock;

// ============================================================================
// Test double for a registered plugin (no RocksDB access is performed)
// ============================================================================

struct BenchPlugin {
    id: String,
}

impl RaftPluginHandler for BenchPlugin {
    fn plugin_id(&self) -> &str {
        &self.id
    }

    fn column_families(&self) -> Vec<String> {
        vec![format!("cf_{}", self.id)]
    }

    fn apply(&self, _db: &DB, _op_type: &str, _payload: &[u8], _log_index: u64) -> RaftResponse {
        RaftResponse::success()
    }

    fn build_snapshot(&self, _db: &DB) -> Result<Vec<u8>, String> {
        Ok(Vec::new())
    }

    fn install_snapshot(&self, _db: &DB, _data: &[u8]) -> Result<(), String> {
        Ok(())
    }
}

/// Build a registry with `n` handlers. The first two ids mirror the real
/// production registrations (Apollo and Consul); the rest are synthetic.
fn registry_with(n: usize) -> PluginRegistry {
    let mut registry = PluginRegistry::new();
    for i in 0..n {
        let id = match i {
            0 => "apollo".to_string(),
            1 => "consul".to_string(),
            _ => format!("plugin_{i:03}"),
        };
        registry
            .register(Arc::new(BenchPlugin { id }))
            .expect("plugin ids are unique");
    }
    registry
}

/// Same shape as the private `ApplyHooks` in `state_machine/mod.rs`.
// `consume()` passes the whole value to black_box, so no individual field is
// read; the fields exist to reproduce the exact production struct layout.
#[allow(dead_code)]
struct BenchHooks {
    naming: Option<Arc<dyn NamingApplyHook>>,
    health_check: Option<Arc<dyn HealthCheckApplyHook>>,
    plugins: Arc<PluginRegistry>,
}

fn consume<T>(hooks: T) {
    black_box(hooks);
}

fn new_hooks() -> (SharedNamingHook, SharedHealthCheckHook) {
    (new_shared_naming_hook(), new_shared_health_check_hook())
}

// ============================================================================
// Leg 1: pre-P2 implementation, reproduced verbatim from bf856219
// ============================================================================

struct OldApplyPath {
    plugin_registry: Arc<RwLock<PluginRegistry>>,
    naming_hook: SharedNamingHook,
    health_check_hook: SharedHealthCheckHook,
}

impl OldApplyPath {
    // The read guard is deliberately held across the two hook `.await`s: that
    // is exactly what bf856219 did, and dropping it is part of P2's robustness
    // fix. Do not "clean this up" — it would invalidate the comparison.
    #[allow(clippy::await_holding_lock)]
    async fn apply_hooks(&self) -> BenchHooks {
        let registry = self.plugin_registry.read().await;
        BenchHooks {
            naming: self.naming_hook.read().await.clone(),
            health_check: self.health_check_hook.read().await.clone(),
            plugins: Arc::new(PluginRegistry::clone(&registry)),
        }
    }
}

fn old_path(n: usize) -> OldApplyPath {
    let (naming_hook, health_check_hook) = new_hooks();
    OldApplyPath {
        plugin_registry: Arc::new(RwLock::new(registry_with(n))),
        naming_hook,
        health_check_hook,
    }
}

// ============================================================================
// Leg 2: P2 intermediate implementation (three locks, O(1) registry clone)
// ============================================================================

struct NewApplyPath {
    plugin_registry: SharedPluginRegistry,
    naming_hook: SharedNamingHook,
    health_check_hook: SharedHealthCheckHook,
}

impl NewApplyPath {
    async fn apply_hooks(&self) -> BenchHooks {
        let plugins = {
            let guard = self.plugin_registry.read().await;
            Arc::clone(&guard)
        };
        let naming = {
            let guard = self.naming_hook.read().await;
            guard.clone()
        };
        let health_check = {
            let guard = self.health_check_hook.read().await;
            guard.clone()
        };
        BenchHooks {
            naming,
            health_check,
            plugins,
        }
    }
}

fn new_path(n: usize) -> NewApplyPath {
    let (naming_hook, health_check_hook) = new_hooks();
    NewApplyPath {
        plugin_registry: Arc::new(RwLock::new(Arc::new(registry_with(n)))),
        naming_hook,
        health_check_hook,
    }
}

// ============================================================================
// Leg 3: current production (P1) — all three handles in one Arc<HookSet>
// ============================================================================

/// Production's `apply_hooks()` returns the inner `Arc<HookSet>` itself: one
/// read lock, one atomic refcount increment, and no per-field clones. This leg
/// mirrors it exactly; `assert_reference_shapes()` fails if production drifts.
#[allow(dead_code)]
struct CombinedHooks {
    naming: Option<Arc<dyn NamingApplyHook>>,
    health_check: Option<Arc<dyn HealthCheckApplyHook>>,
    plugins: Arc<PluginRegistry>,
}

struct SingleArcPath {
    hooks: Arc<RwLock<Arc<CombinedHooks>>>,
}

impl SingleArcPath {
    async fn apply_hooks(&self) -> Arc<CombinedHooks> {
        let guard = self.hooks.read().await;
        Arc::clone(&guard)
    }
}

fn single_arc_path(n: usize) -> SingleArcPath {
    SingleArcPath {
        hooks: Arc::new(RwLock::new(Arc::new(CombinedHooks {
            naming: None,
            health_check: None,
            plugins: Arc::new(registry_with(n)),
        }))),
    }
}

// ============================================================================
// Guard: fail fast if either leg stops matching the production source
// ============================================================================

/// The comparison is only meaningful while leg 3 still matches current
/// production and leg 1 still matches the committed pre-P2 source. Both are
/// checked on every bench run; leg 2 is a frozen reproduction of the
/// uncommitted P2 generation, kept for the intermediate data point.
fn assert_reference_shapes() {
    let root = env!("CARGO_MANIFEST_DIR");
    let sm = std::fs::read_to_string(format!("{root}/src/raft/state_machine/mod.rs"))
        .expect("read state_machine/mod.rs");
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
        "leg 3 no longer matches production apply_hooks; update the bench"
    );
    assert!(
        body.contains("self.hooks.read().await") && body.contains("Arc::clone(&guard)"),
        "production apply_hooks must clone the single inner Arc<HookSet>"
    );
    assert!(
        !body.contains("PluginRegistry::clone("),
        "production apply_hooks regressed to the O(n) clone; update the bench"
    );

    let hooks_src =
        std::fs::read_to_string(format!("{root}/src/raft/hooks.rs")).expect("read hooks.rs");
    assert!(
        hooks_src.contains("pub type SharedHooks = Arc<tokio::sync::RwLock<Arc<HookSet>>>"),
        "SharedHooks shape changed; update the bench"
    );

    let plugin_src =
        std::fs::read_to_string(format!("{root}/src/raft/plugin.rs")).expect("read plugin.rs");
    assert!(
        plugin_src.contains(
            "pub type SharedPluginRegistry = Arc<tokio::sync::RwLock<Arc<PluginRegistry>>>"
        ),
        "legacy SharedPluginRegistry alias disappeared; update the bench"
    );
    assert!(
        plugin_src.contains("handlers: BTreeMap<String, Arc<dyn RaftPluginHandler>>"),
        "PluginRegistry is no longer a BTreeMap; the O(n) claim must be re-derived"
    );

    // Leg 1 is a reproduction, not a straw man: verify its two defining lines
    // still exist in the committed pre-P2 source.
    let output = std::process::Command::new("git")
        .args([
            "-C",
            root,
            "show",
            "bf856219:crates/batata-consistency/src/raft/state_machine/mod.rs",
        ])
        .output()
        .expect("run git show bf856219");
    assert!(
        output.status.success(),
        "bf856219 source unavailable: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let old = String::from_utf8(output.stdout).expect("utf8 source");
    assert!(
        old.contains("let registry = self.plugin_registry.read().await;")
            && old.contains("plugins: Arc::new(PluginRegistry::clone(&registry))"),
        "bf856219 apply_hooks no longer matches the reproduced old leg"
    );
}

// ============================================================================
// Benchmarks
// ============================================================================

fn bench_apply_hooks(c: &mut Criterion) {
    assert_reference_shapes();
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("current-thread runtime");

    let mut group = c.benchmark_group("apply_hooks_snapshot");
    // n=2 is the production shape today; larger sizes show the O(n) exposure.
    for n in [2usize, 8, 32, 128] {
        group.bench_with_input(
            BenchmarkId::new("old_bf856219_btree_clone", n),
            &n,
            |b, _| {
                let path = old_path(n);
                b.iter(|| {
                    let hooks = rt.block_on(path.apply_hooks());
                    consume(hooks)
                })
            },
        );
        group.bench_with_input(
            BenchmarkId::new("p2_three_locks_arc_clone", n),
            &n,
            |b, _| {
                let path = new_path(n);
                b.iter(|| {
                    let hooks = rt.block_on(path.apply_hooks());
                    consume(hooks)
                })
            },
        );
        group.bench_with_input(
            BenchmarkId::new("current_p1_single_arc_hookset", n),
            &n,
            |b, _| {
                let path = single_arc_path(n);
                b.iter(|| {
                    let hooks = rt.block_on(path.apply_hooks());
                    consume(hooks)
                })
            },
        );
    }
    group.finish();
}

fn bench_registry_primitive(c: &mut Criterion) {
    assert_reference_shapes();

    let mut group = c.benchmark_group("registry_snapshot_primitive");
    for n in [0usize, 2, 8, 32, 128] {
        let registry = Arc::new(registry_with(n));
        group.bench_with_input(BenchmarkId::new("old_btree_clone", n), &n, |b, _| {
            b.iter(|| {
                black_box(Arc::new(PluginRegistry::clone(black_box(
                    registry.as_ref(),
                ))))
            })
        });
        group.bench_with_input(BenchmarkId::new("new_arc_clone", n), &n, |b, _| {
            b.iter(|| black_box(Arc::clone(black_box(&registry))))
        });
    }
    group.finish();
}

criterion_group!(benches, bench_apply_hooks, bench_registry_primitive);
criterion_main!(benches);
