// Benchmarks for CircuitBreaker performance
// Measures state transitions and request handling throughput

use batata::core::service::circuit_breaker::{
    CircuitBreaker, CircuitBreakerConfig, with_circuit_breaker,
};
use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use std::time::Duration;

fn bench_allow_request_closed(c: &mut Criterion) {
    let cb = CircuitBreaker::new();

    c.bench_function("allow_request_closed", |b| {
        b.iter(|| black_box(cb.allow_request()))
    });
}

fn bench_allow_request_open(c: &mut Criterion) {
    let config = CircuitBreakerConfig {
        failure_threshold: 1,
        reset_timeout: Duration::from_secs(3600), // Long timeout to stay open
        ..Default::default()
    };
    let cb = CircuitBreaker::with_config(config);
    cb.record_failure(); // Open the circuit

    c.bench_function("allow_request_open", |b| {
        b.iter(|| black_box(cb.allow_request()))
    });
}

fn bench_record_success(c: &mut Criterion) {
    let cb = CircuitBreaker::new();

    c.bench_function("record_success", |b| b.iter(|| cb.record_success()));
}

fn bench_record_failure(c: &mut Criterion) {
    let config = CircuitBreakerConfig {
        failure_threshold: u32::MAX, // Never open
        ..Default::default()
    };
    let cb = CircuitBreaker::with_config(config);

    c.bench_function("record_failure", |b| b.iter(|| cb.record_failure()));
}

fn bench_state_transition_cycle(c: &mut Criterion) {
    c.bench_function("state_transition_cycle", |b| {
        b.iter_batched(
            || {
                CircuitBreaker::with_config(CircuitBreakerConfig {
                    failure_threshold: 3,
                    reset_timeout: Duration::from_millis(1),
                    success_threshold: 2,
                    failure_window: Duration::from_secs(60),
                })
            },
            |cb| {
                // Closed -> Open
                cb.record_failure();
                cb.record_failure();
                cb.record_failure();

                // Wait for half-open
                std::thread::sleep(Duration::from_millis(2));

                // Half-Open -> Closed
                cb.allow_request();
                cb.record_success();
                cb.record_success();
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

fn bench_reset(c: &mut Criterion) {
    c.bench_function("reset", |b| {
        b.iter_batched(
            || {
                let cb = CircuitBreaker::with_config(CircuitBreakerConfig {
                    failure_threshold: 1,
                    ..Default::default()
                });
                cb.record_failure(); // Open it
                cb
            },
            |cb| cb.reset(),
            criterion::BatchSize::SmallInput,
        )
    });
}

fn bench_get_state(c: &mut Criterion) {
    let cb = CircuitBreaker::new();

    c.bench_function("get_state", |b| b.iter(|| black_box(cb.state())));
}

fn bench_get_failure_count(c: &mut Criterion) {
    let cb = CircuitBreaker::new();
    for _ in 0..100 {
        cb.record_failure();
    }

    c.bench_function("get_failure_count", |b| {
        b.iter(|| black_box(cb.failure_count()))
    });
}

fn bench_concurrent_access(c: &mut Criterion) {
    use std::sync::Arc;

    let cb = Arc::new(CircuitBreaker::with_config(CircuitBreakerConfig {
        failure_threshold: u32::MAX,
        ..Default::default()
    }));

    c.bench_function("concurrent_access_4_threads", |b| {
        b.iter(|| {
            let handles: Vec<_> = (0..4)
                .map(|i| {
                    let cb = cb.clone();
                    std::thread::spawn(move || {
                        for _ in 0..25 {
                            cb.allow_request();
                            if i % 2 == 0 {
                                cb.record_success();
                            } else {
                                cb.record_failure();
                            }
                        }
                    })
                })
                .collect();

            for handle in handles {
                handle.join().unwrap();
            }
        })
    });
}

fn bench_with_circuit_breaker_success(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let cb = CircuitBreaker::new();

    c.bench_function("with_circuit_breaker_success", |b| {
        b.to_async(&rt).iter(|| async {
            let result: Result<
                i32,
                batata::core::service::circuit_breaker::CircuitBreakerError<std::io::Error>,
            > = with_circuit_breaker(&cb, async { Ok(42) }).await;
            black_box(result)
        })
    });
}

fn bench_with_circuit_breaker_failure(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let config = CircuitBreakerConfig {
        failure_threshold: u32::MAX,
        ..Default::default()
    };
    let cb = CircuitBreaker::with_config(config);

    c.bench_function("with_circuit_breaker_failure", |b| {
        b.to_async(&rt).iter(|| async {
            let result: Result<
                i32,
                batata::core::service::circuit_breaker::CircuitBreakerError<std::io::Error>,
            > = with_circuit_breaker(&cb, async {
                Err(std::io::Error::new(std::io::ErrorKind::Other, "error"))
            })
            .await;
            black_box(result)
        })
    });
}

fn bench_with_circuit_breaker_rejected(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let config = CircuitBreakerConfig {
        failure_threshold: 1,
        reset_timeout: Duration::from_secs(3600),
        ..Default::default()
    };
    let cb = CircuitBreaker::with_config(config);
    cb.record_failure(); // Open the circuit

    c.bench_function("with_circuit_breaker_rejected", |b| {
        b.to_async(&rt).iter(|| async {
            let result: Result<
                i32,
                batata::core::service::circuit_breaker::CircuitBreakerError<std::io::Error>,
            > = with_circuit_breaker(&cb, async { Ok(42) }).await;
            black_box(result)
        })
    });
}

fn bench_failure_threshold_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("failure_threshold_scaling");

    for threshold in [5, 10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(threshold),
            threshold,
            |b, &threshold| {
                b.iter_batched(
                    || {
                        CircuitBreaker::with_config(CircuitBreakerConfig {
                            failure_threshold: threshold,
                            ..Default::default()
                        })
                    },
                    |cb| {
                        for _ in 0..threshold {
                            cb.record_failure();
                        }
                    },
                    criterion::BatchSize::SmallInput,
                )
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_allow_request_closed,
    bench_allow_request_open,
    bench_record_success,
    bench_record_failure,
    bench_state_transition_cycle,
    bench_reset,
    bench_get_state,
    bench_get_failure_count,
    bench_concurrent_access,
    bench_with_circuit_breaker_success,
    bench_with_circuit_breaker_failure,
    bench_with_circuit_breaker_rejected,
    bench_failure_threshold_scaling,
);

criterion_main!(benches);
