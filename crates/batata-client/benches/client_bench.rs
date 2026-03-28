//! Benchmarks for batata-client hot paths

use criterion::{Criterion, criterion_group, criterion_main};

use batata_client::server_list::ServerListManager;

/// Benchmark server list round-robin selection (hot path for every RPC)
fn bench_server_list_next(c: &mut Criterion) {
    let mgr = ServerListManager::new("10.0.0.1:8848,10.0.0.2:8848,10.0.0.3:8848");

    c.bench_function("server_list_next_server", |b| {
        b.iter(|| {
            std::hint::black_box(mgr.next_server());
        });
    });
}

/// Benchmark server list selection with some unhealthy nodes
fn bench_server_list_with_failures(c: &mut Criterion) {
    let mgr = ServerListManager::new(
        "10.0.0.1:8848,10.0.0.2:8848,10.0.0.3:8848,10.0.0.4:8848,10.0.0.5:8848",
    );
    // Mark 2/5 nodes as unhealthy
    for _ in 0..3 {
        mgr.mark_failure("10.0.0.1:8848");
        mgr.mark_failure("10.0.0.3:8848");
    }

    c.bench_function("server_list_next_with_failures", |b| {
        b.iter(|| {
            std::hint::black_box(mgr.next_server());
        });
    });
}

/// Benchmark server list health tracking (mark_success/mark_failure)
fn bench_server_list_health_tracking(c: &mut Criterion) {
    let mgr = ServerListManager::new("10.0.0.1:8848,10.0.0.2:8848,10.0.0.3:8848");

    c.bench_function("server_list_mark_success", |b| {
        b.iter(|| {
            mgr.mark_success(std::hint::black_box("10.0.0.1:8848"));
        });
    });

    c.bench_function("server_list_mark_failure_and_recover", |b| {
        b.iter(|| {
            mgr.mark_failure(std::hint::black_box("10.0.0.2:8848"));
            mgr.mark_success(std::hint::black_box("10.0.0.2:8848"));
        });
    });
}

/// Benchmark server list update (simulates address discovery refresh)
fn bench_server_list_update(c: &mut Criterion) {
    let mgr = ServerListManager::new("10.0.0.1:8848,10.0.0.2:8848,10.0.0.3:8848");

    c.bench_function("server_list_update_3_servers", |b| {
        b.iter(|| {
            mgr.update_servers(vec![
                "10.0.0.1:8848".into(),
                "10.0.0.2:8848".into(),
                "10.0.0.4:8848".into(),
            ]);
        });
    });
}

criterion_group!(
    benches,
    bench_server_list_next,
    bench_server_list_with_failures,
    bench_server_list_health_tracking,
    bench_server_list_update,
);
criterion_main!(benches);
