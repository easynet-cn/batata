// Benchmarks for NamingService performance
// Measures registration, lookup, and subscription operations

use batata_server::api::naming::model::Instance;
use batata_server::service::naming::NamingService;
use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::collections::HashMap;

fn create_test_instance(ip: &str, port: i32) -> Instance {
    Instance {
        instance_id: format!("{}#{}#DEFAULT#bench-service", ip, port),
        ip: ip.to_string(),
        port,
        weight: 1.0,
        healthy: true,
        enabled: true,
        ephemeral: true,
        cluster_name: "DEFAULT".to_string(),
        service_name: String::new(),
        metadata: HashMap::new(),
    }
}

/// Instance with realistic metadata (~10 entries), matching typical Nacos
/// deployments where services publish version, region, app, env, etc.
/// This is the shape that makes `Instance::clone()` expensive.
fn create_test_instance_with_metadata(ip: &str, port: i32) -> Instance {
    let mut metadata = HashMap::new();
    metadata.insert("version".to_string(), "1.2.3".to_string());
    metadata.insert("region".to_string(), "cn-shanghai".to_string());
    metadata.insert("zone".to_string(), "cn-shanghai-a".to_string());
    metadata.insert("env".to_string(), "production".to_string());
    metadata.insert("app".to_string(), "payment-service".to_string());
    metadata.insert("protocol".to_string(), "grpc".to_string());
    metadata.insert("weight".to_string(), "100".to_string());
    metadata.insert("tenant".to_string(), "default".to_string());
    metadata.insert("owner".to_string(), "platform-team".to_string());
    metadata.insert("revision".to_string(), "abc123def456".to_string());
    Instance {
        instance_id: format!("{}#{}#DEFAULT#bench-service", ip, port),
        ip: ip.to_string(),
        port,
        weight: 1.0,
        healthy: true,
        enabled: true,
        ephemeral: true,
        cluster_name: "DEFAULT".to_string(),
        service_name: "bench-service".to_string(),
        metadata,
    }
}

fn bench_register_instance(c: &mut Criterion) {
    let naming = NamingService::new();

    c.bench_function("register_instance", |b| {
        let mut port = 8000;
        b.iter(|| {
            port += 1;
            let instance = create_test_instance("192.168.1.1", port);
            naming.register_instance(
                black_box("public"),
                black_box("DEFAULT_GROUP"),
                black_box("bench-service"),
                black_box(instance),
            )
        })
    });
}

fn bench_get_instances(c: &mut Criterion) {
    let naming = NamingService::new();

    // Pre-populate with instances
    for i in 0..1000 {
        let instance = create_test_instance("192.168.1.1", 8000 + i);
        naming.register_instance("public", "DEFAULT_GROUP", "bench-service", instance);
    }

    c.bench_function("get_instances_1000", |b| {
        b.iter(|| {
            naming.get_instances(
                black_box("public"),
                black_box("DEFAULT_GROUP"),
                black_box("bench-service"),
                black_box(""),
                black_box(false),
            )
        })
    });
}

fn bench_get_instances_healthy_only(c: &mut Criterion) {
    let naming = NamingService::new();

    // Pre-populate with mixed healthy/unhealthy instances
    for i in 0..1000 {
        let mut instance = create_test_instance("192.168.1.1", 8000 + i);
        instance.healthy = i % 2 == 0; // Half healthy
        naming.register_instance("public", "DEFAULT_GROUP", "bench-service", instance);
    }

    c.bench_function("get_instances_healthy_only_1000", |b| {
        b.iter(|| {
            naming.get_instances(
                black_box("public"),
                black_box("DEFAULT_GROUP"),
                black_box("bench-service"),
                black_box(""),
                black_box(true),
            )
        })
    });
}

fn bench_get_instances_by_cluster(c: &mut Criterion) {
    let naming = NamingService::new();

    // Pre-populate with instances in different clusters
    for i in 0..1000 {
        let cluster = if i % 3 == 0 {
            "CLUSTER_A"
        } else if i % 3 == 1 {
            "CLUSTER_B"
        } else {
            "CLUSTER_C"
        };
        let mut instance = create_test_instance("192.168.1.1", 8000 + i);
        instance.cluster_name = cluster.to_string();
        naming.register_instance("public", "DEFAULT_GROUP", "bench-service", instance);
    }

    c.bench_function("get_instances_by_cluster_1000", |b| {
        b.iter(|| {
            naming.get_instances(
                black_box("public"),
                black_box("DEFAULT_GROUP"),
                black_box("bench-service"),
                black_box("CLUSTER_A"),
                black_box(false),
            )
        })
    });
}

fn bench_subscribe(c: &mut Criterion) {
    let naming = NamingService::new();

    c.bench_function("subscribe", |b| {
        let mut conn_id = 0;
        b.iter(|| {
            conn_id += 1;
            naming.subscribe(
                black_box(&format!("conn-{}", conn_id)),
                black_box("public"),
                black_box("DEFAULT_GROUP"),
                black_box("bench-service"),
            )
        })
    });
}

fn bench_get_subscribers(c: &mut Criterion) {
    let naming = NamingService::new();

    // Pre-populate with subscribers
    for i in 0..1000 {
        naming.subscribe(
            &format!("conn-{}", i),
            "public",
            "DEFAULT_GROUP",
            "bench-service",
        );
    }

    c.bench_function("get_subscribers_1000", |b| {
        b.iter(|| {
            naming.get_subscribers(
                black_box("public"),
                black_box("DEFAULT_GROUP"),
                black_box("bench-service"),
            )
        })
    });
}

fn bench_list_services(c: &mut Criterion) {
    let naming = NamingService::new();

    // Pre-populate with many services
    for i in 0..100 {
        let instance = create_test_instance("192.168.1.1", 8000 + i);
        naming.register_instance(
            "public",
            "DEFAULT_GROUP",
            &format!("service-{}", i),
            instance,
        );
    }

    c.bench_function("list_services_100", |b| {
        b.iter(|| {
            naming.list_services(
                black_box("public"),
                black_box("DEFAULT_GROUP"),
                black_box(1),
                black_box(20),
            )
        })
    });
}

fn bench_deregister_instance(c: &mut Criterion) {
    c.bench_function("deregister_instance", |b| {
        b.iter_batched(
            || {
                let naming = NamingService::new();
                let instance = create_test_instance("192.168.1.1", 8080);
                naming.register_instance(
                    "public",
                    "DEFAULT_GROUP",
                    "bench-service",
                    instance.clone(),
                );
                (naming, instance)
            },
            |(naming, instance)| {
                naming.deregister_instance(
                    black_box("public"),
                    black_box("DEFAULT_GROUP"),
                    black_box("bench-service"),
                    black_box(&instance),
                )
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

fn bench_instance_count_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("instance_count_scaling");

    for size in [100, 500, 1000, 5000].iter() {
        let naming = NamingService::new();

        // Pre-populate
        for i in 0..*size {
            let instance = create_test_instance("192.168.1.1", 8000 + i);
            naming.register_instance("public", "DEFAULT_GROUP", "bench-service", instance);
        }

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                naming.get_instances(
                    black_box("public"),
                    black_box("DEFAULT_GROUP"),
                    black_box("bench-service"),
                    black_box(""),
                    black_box(false),
                )
            })
        });
    }

    group.finish();
}

fn bench_get_instances_with_metadata(c: &mut Criterion) {
    let naming = NamingService::new();

    // Pre-populate with 1000 instances carrying realistic metadata.
    for i in 0..1000 {
        let instance = create_test_instance_with_metadata("192.168.1.1", 8000 + i);
        naming.register_instance("public", "DEFAULT_GROUP", "bench-service-md", instance);
    }

    c.bench_function("get_instances_1000_with_metadata", |b| {
        b.iter(|| {
            naming.get_instances(
                black_box("public"),
                black_box("DEFAULT_GROUP"),
                black_box("bench-service-md"),
                black_box(""),
                black_box(false),
            )
        })
    });
}

fn bench_get_instances_snapshot_with_metadata(c: &mut Criterion) {
    let naming = NamingService::new();

    // Same pre-population as the above bench so the two are directly comparable.
    for i in 0..1000 {
        let instance = create_test_instance_with_metadata("192.168.1.1", 8000 + i);
        naming.register_instance("public", "DEFAULT_GROUP", "bench-service-md2", instance);
    }

    c.bench_function("get_instances_snapshot_1000_with_metadata", |b| {
        b.iter(|| {
            naming.get_instances_snapshot(
                black_box("public"),
                black_box("DEFAULT_GROUP"),
                black_box("bench-service-md2"),
                black_box(""),
                black_box(false),
            )
        })
    });
}

criterion_group!(
    benches,
    bench_register_instance,
    bench_get_instances,
    bench_get_instances_healthy_only,
    bench_get_instances_by_cluster,
    bench_subscribe,
    bench_get_subscribers,
    bench_list_services,
    bench_deregister_instance,
    bench_instance_count_scaling,
    bench_get_instances_with_metadata,
    bench_get_instances_snapshot_with_metadata,
);

criterion_main!(benches);
