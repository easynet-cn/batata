//! Micro-benchmark: async-trait boxing overhead vs native async fn in trait.
//!
//! Goal: decide whether a 29-file `#[async_trait]` removal refactor is justified.
//!
//! Three scenarios:
//!
//! 1. **async-trait + dyn dispatch** — current state. Every call boxes the
//!    future via `Pin<Box<dyn Future>>`.
//! 2. **Native async fn in trait + dyn dispatch** — what a mechanical
//!    "strip `#[async_trait]`" edit would produce. In Rust 1.75+, the compiler
//!    still boxes the future at the vtable boundary (there is no way to
//!    return an `impl Future` through a dynamic vtable), so this is expected
//!    to be equivalent to (1).
//! 3. **Native async fn + generic static dispatch** — requires replacing
//!    `Arc<dyn Trait>` with `Arc<impl Trait>` across the whole callgraph.
//!    Future is inlined; no boxing.
//!
//! The payload is deliberately trivial: `a + b` returning `u64`. This
//! isolates the boxing/vtable cost from any real work — so whatever delta
//! we see here is a **generous upper bound** on what async-trait removal
//! could save in production, where real persistence ops do microseconds of
//! RocksDB I/O and millseconds of Raft consensus.

use std::hint::black_box;
use std::sync::Arc;

use async_trait::async_trait;
use criterion::{Criterion, criterion_group, criterion_main};

// ============================================================================
// Scenario 1: async-trait + dyn dispatch (current state)
// ============================================================================

#[async_trait]
trait AsyncTraitOp: Send + Sync {
    async fn compute(&self, a: u64, b: u64) -> u64;
}

struct AsyncTraitImpl;

#[async_trait]
impl AsyncTraitOp for AsyncTraitImpl {
    async fn compute(&self, a: u64, b: u64) -> u64 {
        a.wrapping_add(b)
    }
}

// ============================================================================
// Scenario 2: native async fn in trait + dyn dispatch
// ============================================================================
//
// Rust 1.75+ allows `async fn` in traits, and they are dyn-compatible.
// The compiler generates a boxing shim at the vtable boundary, so the
// runtime cost should match async-trait.

trait NativeDynOp: Send + Sync {
    fn compute(
        &self,
        a: u64,
        b: u64,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = u64> + Send + '_>>;
}

struct NativeDynImpl;

impl NativeDynOp for NativeDynImpl {
    fn compute(
        &self,
        a: u64,
        b: u64,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = u64> + Send + '_>> {
        Box::pin(async move { a.wrapping_add(b) })
    }
}

// ============================================================================
// Scenario 3: native async fn + generic static dispatch (no dyn)
// ============================================================================
//
// Future is fully inlined at the call site. No boxing, no vtable.

trait StaticOp {
    fn compute(&self, a: u64, b: u64) -> impl std::future::Future<Output = u64> + Send;
}

struct StaticImpl;

impl StaticOp for StaticImpl {
    async fn compute(&self, a: u64, b: u64) -> u64 {
        a.wrapping_add(b)
    }
}

// ============================================================================
// Benchmarks
// ============================================================================

fn bench_async_trait_dyn(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let op: Arc<dyn AsyncTraitOp> = Arc::new(AsyncTraitImpl);

    c.bench_function("async_trait/dyn_dispatch", |b| {
        b.iter(|| {
            rt.block_on(async {
                let r = op.compute(black_box(1), black_box(2)).await;
                black_box(r)
            })
        })
    });
}

fn bench_native_dyn(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let op: Arc<dyn NativeDynOp> = Arc::new(NativeDynImpl);

    c.bench_function("native_async/dyn_dispatch", |b| {
        b.iter(|| {
            rt.block_on(async {
                let r = op.compute(black_box(1), black_box(2)).await;
                black_box(r)
            })
        })
    });
}

fn bench_static(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let op = StaticImpl;

    c.bench_function("native_async/static_dispatch", |b| {
        b.iter(|| {
            rt.block_on(async {
                let r = op.compute(black_box(1), black_box(2)).await;
                black_box(r)
            })
        })
    });
}

criterion_group!(
    benches,
    bench_async_trait_dyn,
    bench_native_dyn,
    bench_static,
);
criterion_main!(benches);
