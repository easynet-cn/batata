// Benchmarks for authentication service performance
// Measures JWT token encoding, decoding, and caching performance

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use batata::auth::service::auth::{
    decode_jwt_token, decode_jwt_token_cached, encode_jwt_token, clear_token_cache,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};

fn test_secret_key() -> String {
    STANDARD.encode("benchmark-secret-key-that-is-long-enough-for-hs256")
}

fn bench_encode_jwt_token(c: &mut Criterion) {
    let secret = test_secret_key();

    c.bench_function("encode_jwt_token", |b| {
        b.iter(|| {
            encode_jwt_token(
                black_box("benchmark_user"),
                black_box(&secret),
                black_box(3600),
            )
        })
    });
}

fn bench_decode_jwt_token(c: &mut Criterion) {
    let secret = test_secret_key();
    let token = encode_jwt_token("benchmark_user", &secret, 3600).unwrap();

    c.bench_function("decode_jwt_token", |b| {
        b.iter(|| {
            decode_jwt_token(black_box(&token), black_box(&secret))
        })
    });
}

fn bench_decode_jwt_token_cached_miss(c: &mut Criterion) {
    let secret = test_secret_key();

    c.bench_function("decode_jwt_token_cached_miss", |b| {
        b.iter_batched(
            || {
                // Clear cache and create new token for each iteration
                clear_token_cache();
                encode_jwt_token("benchmark_user", &secret, 3600).unwrap()
            },
            |token| {
                decode_jwt_token_cached(black_box(&token), black_box(&secret))
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

fn bench_decode_jwt_token_cached_hit(c: &mut Criterion) {
    let secret = test_secret_key();
    let token = encode_jwt_token("cached_user", &secret, 3600).unwrap();

    // Warm up cache
    let _ = decode_jwt_token_cached(&token, &secret);

    c.bench_function("decode_jwt_token_cached_hit", |b| {
        b.iter(|| {
            decode_jwt_token_cached(black_box(&token), black_box(&secret))
        })
    });
}

fn bench_encode_decode_roundtrip(c: &mut Criterion) {
    let secret = test_secret_key();

    c.bench_function("encode_decode_roundtrip", |b| {
        b.iter(|| {
            let token = encode_jwt_token(
                black_box("roundtrip_user"),
                black_box(&secret),
                black_box(3600),
            ).unwrap();
            decode_jwt_token(black_box(&token), black_box(&secret))
        })
    });
}

fn bench_token_validation_throughput(c: &mut Criterion) {
    let secret = test_secret_key();

    // Create multiple tokens
    let tokens: Vec<String> = (0..100)
        .map(|i| encode_jwt_token(&format!("user_{}", i), &secret, 3600).unwrap())
        .collect();

    // Warm up cache
    for token in &tokens {
        let _ = decode_jwt_token_cached(token, &secret);
    }

    c.bench_function("token_validation_throughput_100", |b| {
        b.iter(|| {
            for token in &tokens {
                let _ = decode_jwt_token_cached(black_box(token), black_box(&secret));
            }
        })
    });
}

fn bench_username_length_impact(c: &mut Criterion) {
    let secret = test_secret_key();
    let mut group = c.benchmark_group("username_length");

    for len in [8, 32, 128, 512].iter() {
        let username: String = (0..*len).map(|_| 'a').collect();

        group.bench_with_input(BenchmarkId::from_parameter(len), &username, |b, username| {
            b.iter(|| {
                let token = encode_jwt_token(
                    black_box(username),
                    black_box(&secret),
                    black_box(3600),
                ).unwrap();
                decode_jwt_token(black_box(&token), black_box(&secret))
            })
        });
    }

    group.finish();
}

fn bench_expiration_time_impact(c: &mut Criterion) {
    let secret = test_secret_key();
    let mut group = c.benchmark_group("expiration_time");

    for expire in [60, 3600, 86400, 31536000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(expire), expire, |b, &expire| {
            b.iter(|| {
                encode_jwt_token(
                    black_box("benchmark_user"),
                    black_box(&secret),
                    black_box(expire),
                )
            })
        });
    }

    group.finish();
}

fn bench_concurrent_token_validation(c: &mut Criterion) {
    use std::sync::Arc;

    let secret = Arc::new(test_secret_key());
    let token = Arc::new(encode_jwt_token("concurrent_user", &secret, 3600).unwrap());

    // Warm up cache
    let _ = decode_jwt_token_cached(&token, &secret);

    c.bench_function("concurrent_token_validation", |b| {
        b.iter(|| {
            let handles: Vec<_> = (0..4)
                .map(|_| {
                    let secret = secret.clone();
                    let token = token.clone();
                    std::thread::spawn(move || {
                        for _ in 0..25 {
                            let _ = decode_jwt_token_cached(&token, &secret);
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

criterion_group!(
    benches,
    bench_encode_jwt_token,
    bench_decode_jwt_token,
    bench_decode_jwt_token_cached_miss,
    bench_decode_jwt_token_cached_hit,
    bench_encode_decode_roundtrip,
    bench_token_validation_throughput,
    bench_username_length_impact,
    bench_expiration_time_impact,
    bench_concurrent_token_validation,
);

criterion_main!(benches);
