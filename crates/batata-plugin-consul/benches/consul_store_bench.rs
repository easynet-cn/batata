//! Baseline benchmarks for the Consul plugin's hot read/write paths.
//!
//! Target metrics captured here are used as the "before" baseline for
//! architecture-level optimizations (ahash, inline, zero-alloc, etc).
//! Run with `cargo bench -p batata-plugin-consul --bench consul_store_bench`.

use std::hint::black_box;
use std::sync::Arc;

use batata_plugin::PluginNamingStore;
use batata_plugin_consul::naming_store::ConsulNamingStore;
use bytes::Bytes;
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

/// Mimic an `AgentServiceRegistration` payload so the JSON blob stored
/// in the naming store is realistic. Size ~200 bytes — matches typical
/// Consul service register call.
fn registration_json(name: &str, id: &str, port: u16) -> Bytes {
    let body = format!(
        r#"{{"Name":"{name}","ID":"{id}","Address":"10.0.0.1","Port":{port},"Tags":["bench","consul"],"Meta":{{"version":"1.0"}}}}"#
    );
    Bytes::from(body)
}

fn populate_store(store: &ConsulNamingStore, n: usize) {
    for i in 0..n {
        let id = format!("svc-{}", i);
        let key = ConsulNamingStore::build_key("default", "bench", &id);
        let data = registration_json("bench", &id, 8000 + (i as u16));
        store.register(&key, data).unwrap();
    }
}

fn bench_register(c: &mut Criterion) {
    let store = ConsulNamingStore::new();

    c.bench_function("consul_naming_store/register", |b| {
        let mut i = 0u32;
        b.iter(|| {
            i += 1;
            let id = format!("reg-{}", i);
            let key = ConsulNamingStore::build_key("default", "bench", &id);
            let data = registration_json("bench", &id, 8000);
            store.register(black_box(&key), black_box(data)).unwrap()
        })
    });
}

fn bench_deregister(c: &mut Criterion) {
    c.bench_function("consul_naming_store/deregister", |b| {
        b.iter_batched(
            || {
                let store = ConsulNamingStore::new();
                let key = ConsulNamingStore::build_key("default", "bench", "svc");
                store
                    .register(&key, registration_json("bench", "svc", 8000))
                    .unwrap();
                (store, key)
            },
            |(store, key)| {
                store.deregister(black_box(&key)).unwrap();
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

fn bench_get_service_entries(c: &mut Criterion) {
    let mut group = c.benchmark_group("consul_naming_store/get_service_entries");

    for size in [10usize, 100, 1000].iter() {
        let store = Arc::new(ConsulNamingStore::new());
        // Populate a single service with `size` instances (all share service_name)
        for i in 0..*size {
            let id = format!("inst-{}", i);
            let key = ConsulNamingStore::build_key("default", "bench-svc", &id);
            let data = registration_json("bench-svc", &id, 8000 + i as u16);
            store.register(&key, data).unwrap();
        }

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                let entries = store.get_service_entries(black_box("default"), black_box("bench-svc"));
                black_box(entries.len())
            })
        });
    }

    group.finish();
}

fn bench_service_names(c: &mut Criterion) {
    let store = ConsulNamingStore::new();
    populate_store(&store, 500);

    c.bench_function("consul_naming_store/service_names_500", |b| {
        b.iter(|| {
            let names = store.service_names(black_box("default"));
            black_box(names.len())
        })
    });
}

fn bench_build_key(c: &mut Criterion) {
    c.bench_function("consul_naming_store/build_key", |b| {
        b.iter(|| {
            let key = ConsulNamingStore::build_key(
                black_box("default"),
                black_box("service-name"),
                black_box("service-id"),
            );
            black_box(key)
        })
    });
}

fn bench_scan_prefix(c: &mut Criterion) {
    let store = ConsulNamingStore::new();
    populate_store(&store, 500);

    c.bench_function("consul_naming_store/scan_500_prefix", |b| {
        b.iter(|| {
            let out = PluginNamingStore::scan(&store, black_box(""));
            black_box(out.len())
        })
    });
}

criterion_group!(
    benches,
    bench_register,
    bench_deregister,
    bench_get_service_entries,
    bench_service_names,
    bench_build_key,
    bench_scan_prefix,
);
criterion_main!(benches);
