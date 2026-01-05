// Benchmarks for NamingService performance
// Measures registration, lookup, and subscription operations

use batata::api::naming::model::Instance;
use batata::service::naming::NamingService;
use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use std::collections::HashMap;

fn create_test_instance(ip: &str, port: i32) -> Instance {
    Instance {
        instance_id: format!("{}#{}#DEFAULT", ip, port),
        ip: ip.to_string(),
        port,
        weight: 1.0,
        healthy: true,
        enabled: true,
        ephemeral: true,
        cluster_name: "DEFAULT".to_string(),
        service_name: String::new(),
        metadata: HashMap::new(),
        instance_heart_beat_interval: 5000,
        instance_heart_beat_time_out: 15000,
        ip_delete_timeout: 30000,
        instance_id_generator: String::new(),
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
);

criterion_main!(benches);
