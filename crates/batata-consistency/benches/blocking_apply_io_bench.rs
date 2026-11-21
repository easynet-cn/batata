//! Micro-benchmark: cost of one `spawn_blocking` hop per request.
//!
//! P1/T2 moved synchronous RocksDB work off the actix `current_thread`
//! runtime. `blocking_apply_io()` (one hop per committed entry),
//! `log_store::blocking_read()` (Raft log reads) and
//! `blocking_snapshot_io()` pay for that isolation on every call.
//!
//! This bench is **measurement-only**: it does not change production code. It
//! compares:
//!
//!   1. a direct synchronous call (no runtime),
//!   2. `Runtime::block_on` with the work inline (isolates `block_on` cost),
//!   3. `Runtime::block_on` + `spawn_blocking` on a `current_thread` runtime
//!      (the production shape),
//!   4. the same on a 4-worker multi-thread runtime (context only).
//!
//! The honest scheduling cost is (3) - (2), not (3) - (1).
//!
//! ## Measured on 2026-11-21 (this box, criterion defaults)
//!
//!   direct_sync_call                    626 ps
//!   current_thread_block_on_direct     38.8 ns
//!   current_thread_spawn_blocking       4.3375 us
//!   multi_thread_spawn_blocking         3.7355 us
//!   => pure dispatch overhead          ~4.30 us per hop
//!
//! ## Verdict: keep `spawn_blocking`, do not optimise this away
//!
//! The `rocks|standalone|Config Read` anchor is 162550 RPS (~6.15 us/request
//! aggregate), but that path does **not** pay this cost: `batata-config` has no
//! `spawn_blocking`, and `batata-persistence::local_auth_read` deliberately
//! reads inline to skip this thread-hop. The helpers measured here are on the
//! Raft apply / log-store / snapshot paths, whose cluster `Config Write` is
//! 2735 RPS (~366 us aggregate, ~6.9 ms measured per request under load) —
//! 4.3 us is well under 1% of that budget. Removing the hop would put inline
//! RocksDB I/O back on the single-threaded `main` runtime, which was the root
//! cause of the 3x7s stall and steady-state elections fixed in T2/P1.
//!
//! Run with:
//!   cargo bench -p batata-consistency --bench blocking_apply_io_bench
//! Quick guard/shape verification:
//!   cargo bench -p batata-consistency --bench blocking_apply_io_bench -- --test

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};

/// A small deterministic workload so the closure is not a pure no-op that an
/// optimiser could fold into the join handle. It is deliberately far cheaper
/// than a real RocksDB read: this bench measures dispatch, not storage.
fn tiny_work() -> u64 {
    let mut acc = 1u64;
    for i in 0..32u64 {
        acc = acc.wrapping_mul(31).wrapping_add(i);
    }
    black_box(acc)
}

/// The measurement is only meaningful while production still dispatches its
/// synchronous work through these helpers. Fail fast if that shape changes.
fn assert_reference_shapes() {
    let root = env!("CARGO_MANIFEST_DIR");
    let sm = std::fs::read_to_string(format!("{root}/src/raft/state_machine/mod.rs"))
        .expect("read state_machine/mod.rs");
    let log_store = std::fs::read_to_string(format!("{root}/src/raft/log_store.rs"))
        .expect("read log_store.rs");

    let apply = sm
        .find("async fn blocking_apply_io")
        .expect("blocking_apply_io must exist");
    assert!(
        sm[apply..apply + 700].contains("spawn_blocking(f)"),
        "blocking_apply_io no longer dispatches through spawn_blocking"
    );
    let read = log_store
        .find("async fn blocking_read")
        .expect("blocking_read must exist");
    assert!(
        log_store[read..read + 700].contains("spawn_blocking(f)"),
        "blocking_read no longer dispatches through spawn_blocking"
    );

    // Keep the headline RPS row honest: the conclusion cites this anchor.
    let baseline =
        std::fs::read_to_string(format!("{root}/../../scripts/regression_perf_baseline.txt"))
            .expect("read regression_perf_baseline.txt");
    assert!(
        baseline
            .lines()
            .any(|line| line.contains("|rocks|standalone|Config Read|")),
        "the rocks/standalone Config Read anchor disappeared; update the bench docs"
    );
}

fn bench_dispatch(c: &mut Criterion) {
    assert_reference_shapes();

    let current_thread = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("current-thread runtime");
    let multi_thread = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .build()
        .expect("multi-thread runtime");

    let mut group = c.benchmark_group("spawn_blocking_dispatch");
    group.bench_function("direct_sync_call", |b| {
        b.iter(|| black_box(tiny_work()));
    });
    group.bench_function("current_thread_block_on_direct", |b| {
        b.iter(|| {
            current_thread.block_on(async { black_box(tiny_work()) });
        });
    });
    group.bench_function("current_thread_spawn_blocking", |b| {
        b.iter(|| {
            current_thread.block_on(async {
                tokio::task::spawn_blocking(|| black_box(tiny_work()))
                    .await
                    .expect("blocking task")
            });
        });
    });
    group.bench_function("multi_thread_spawn_blocking", |b| {
        b.iter(|| {
            multi_thread.block_on(async {
                tokio::task::spawn_blocking(|| black_box(tiny_work()))
                    .await
                    .expect("blocking task")
            });
        });
    });
    group.finish();
}

criterion_group!(benches, bench_dispatch);
criterion_main!(benches);
