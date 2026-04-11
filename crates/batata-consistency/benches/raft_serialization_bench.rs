//! Baseline benchmarks for `RaftRequest` serialization.
//!
//! Every write path in batata goes through `serde_json::to_vec(&RaftRequest)`
//! before the Raft log entry is appended, and the inverse deserialization
//! on every `apply`. These benches quantify that cost today so we can
//! measure the impact of future optimizations (bincode migration, enum
//! layout changes, zero-copy paths, etc.).

use std::hint::black_box;

use batata_consistency::raft::request::RaftRequest;
use batata_consistency::raft::state_machine::{StoredConfig, StoredInstance};
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

fn config_publish_small() -> RaftRequest {
    RaftRequest::ConfigPublish {
        data_id: "app.properties".to_string(),
        group: "DEFAULT_GROUP".to_string(),
        tenant: "public".to_string(),
        content: "key1=value1\nkey2=value2".to_string(),
        md5: "abc123".to_string(),
        config_type: Some("properties".to_string()),
        app_name: Some("bench-app".to_string()),
        tag: None,
        desc: Some("bench".to_string()),
        src_user: Some("bench".to_string()),
        src_ip: Some("127.0.0.1".to_string()),
        r#use: None,
        effect: None,
        schema: None,
        encrypted_data_key: None,
        cas_md5: None,
        history: None,
    }
}

fn config_publish_large() -> RaftRequest {
    // ~8 KiB content — closer to real-world config files
    let content: String = (0..256)
        .map(|i| format!("key{:03}=value-with-some-padding-{}\n", i, i))
        .collect();
    RaftRequest::ConfigPublish {
        data_id: "large.yml".to_string(),
        group: "PROD".to_string(),
        tenant: "public".to_string(),
        content,
        md5: "0123456789abcdef0123456789abcdef".to_string(),
        config_type: Some("yaml".to_string()),
        app_name: Some("bench-app".to_string()),
        tag: Some("prod".to_string()),
        desc: Some("Large production config".to_string()),
        src_user: Some("ops".to_string()),
        src_ip: Some("10.0.0.1".to_string()),
        r#use: None,
        effect: None,
        schema: None,
        encrypted_data_key: None,
        cas_md5: None,
        history: None,
    }
}

fn persistent_instance_register() -> RaftRequest {
    RaftRequest::PersistentInstanceRegister {
        namespace_id: "public".to_string(),
        group_name: "DEFAULT_GROUP".to_string(),
        service_name: "bench-service".to_string(),
        instance_id: "10.0.0.1#8080#DEFAULT".to_string(),
        ip: "10.0.0.1".to_string(),
        port: 8080,
        weight: 1.0,
        healthy: true,
        enabled: true,
        metadata: r#"{"version":"1.0","env":"prod"}"#.to_string(),
        cluster_name: "DEFAULT".to_string(),
    }
}

fn plugin_write_consul_kv() -> RaftRequest {
    // Typical Consul KV put payload (~200 bytes)
    let payload = serde_json::to_vec(&serde_json::json!({
        "KVPut": {
            "key": "services/web/config",
            "stored_kv_json": r#"{"pair":{"key":"services/web/config","value":"aGVsbG8=","flags":0,"create_index":1,"modify_index":1,"lock_index":0,"session":null},"modified_at":1700000000000}"#,
            "session_index_key": null,
        }
    }))
    .unwrap();
    RaftRequest::PluginWrite {
        plugin_id: "consul".to_string(),
        op_type: "KVPut".to_string(),
        payload,
    }
}

fn bench_serialize(c: &mut Criterion) {
    let mut group = c.benchmark_group("raft_request_serialize");

    let cases = [
        ("config_publish_small", config_publish_small()),
        ("config_publish_large", config_publish_large()),
        ("persistent_instance_register", persistent_instance_register()),
        ("plugin_write_consul_kv", plugin_write_consul_kv()),
    ];

    for (name, req) in cases.iter() {
        group.bench_with_input(BenchmarkId::new("to_vec", name), req, |b, req| {
            b.iter(|| {
                let bytes = serde_json::to_vec(black_box(req)).unwrap();
                black_box(bytes)
            })
        });
    }

    group.finish();
}

fn bench_deserialize(c: &mut Criterion) {
    let mut group = c.benchmark_group("raft_request_deserialize");

    let cases = [
        ("config_publish_small", config_publish_small()),
        ("config_publish_large", config_publish_large()),
        ("persistent_instance_register", persistent_instance_register()),
        ("plugin_write_consul_kv", plugin_write_consul_kv()),
    ];

    for (name, req) in cases.iter() {
        let bytes = serde_json::to_vec(req).unwrap();
        group.bench_with_input(BenchmarkId::new("from_slice", name), &bytes, |b, bytes| {
            b.iter(|| {
                let req: RaftRequest = serde_json::from_slice(black_box(bytes)).unwrap();
                black_box(req)
            })
        });
    }

    group.finish();
}

fn bench_roundtrip(c: &mut Criterion) {
    c.bench_function("raft_request_roundtrip_persistent_instance", |b| {
        let req = persistent_instance_register();
        b.iter(|| {
            let bytes = serde_json::to_vec(black_box(&req)).unwrap();
            let decoded: RaftRequest = serde_json::from_slice(&bytes).unwrap();
            black_box(decoded)
        })
    });
}

// ============================================================================
// StoredInstance state-machine value benches
//
// Measures the real hot path in `apply_instance_register` / `_update` /
// `_replay`, which reads/writes `StoredInstance` blobs to CF_INSTANCES.
// Compares bincode (new) vs the legacy serde_json::Value dance (old).
// ============================================================================

fn stored_instance_sample() -> StoredInstance {
    StoredInstance {
        namespace_id: "public".to_string(),
        group_name: "DEFAULT_GROUP".to_string(),
        service_name: "bench-service".to_string(),
        instance_id: "10.0.0.1#8080#DEFAULT".to_string(),
        ip: "10.0.0.1".to_string(),
        port: 8080,
        weight: 1.0,
        healthy: true,
        enabled: true,
        metadata: r#"{"version":"1.0","env":"prod"}"#.to_string(),
        cluster_name: "DEFAULT".to_string(),
        registered_time: 1_700_000_000_000,
        modified_time: 1_700_000_000_000,
    }
}

fn stored_instance_as_json_value(s: &StoredInstance) -> serde_json::Value {
    serde_json::json!({
        "namespace_id": s.namespace_id,
        "group_name": s.group_name,
        "service_name": s.service_name,
        "instance_id": s.instance_id,
        "ip": s.ip,
        "port": s.port,
        "weight": s.weight,
        "healthy": s.healthy,
        "enabled": s.enabled,
        "metadata": s.metadata,
        "cluster_name": s.cluster_name,
        "registered_time": s.registered_time,
        "modified_time": s.modified_time,
    })
}

fn bench_stored_instance_encode(c: &mut Criterion) {
    let sample = stored_instance_sample();
    let mut group = c.benchmark_group("stored_instance_encode");

    group.bench_function("json_value_to_vec", |b| {
        b.iter(|| {
            let v = stored_instance_as_json_value(black_box(&sample));
            let bytes = serde_json::to_vec(&v).unwrap();
            black_box(bytes)
        })
    });

    group.bench_function("bincode_serialize", |b| {
        b.iter(|| {
            let bytes = bincode::serialize(black_box(&sample)).unwrap();
            black_box(bytes)
        })
    });

    group.finish();
}

fn bench_stored_instance_decode(c: &mut Criterion) {
    let sample = stored_instance_sample();
    let json_bytes = serde_json::to_vec(&stored_instance_as_json_value(&sample)).unwrap();
    let bincode_bytes = bincode::serialize(&sample).unwrap();

    let mut group = c.benchmark_group("stored_instance_decode");

    group.bench_function("json_value_from_slice", |b| {
        b.iter(|| {
            let v: serde_json::Value = serde_json::from_slice(black_box(&json_bytes)).unwrap();
            // Field-by-field access matches replay_persistent_instances path
            let _ = v["namespace_id"].as_str();
            let _ = v["group_name"].as_str();
            let _ = v["service_name"].as_str();
            let _ = v["instance_id"].as_str();
            let _ = v["ip"].as_str();
            let _ = v["port"].as_u64();
            let _ = v["weight"].as_f64();
            let _ = v["healthy"].as_bool();
            let _ = v["enabled"].as_bool();
            let _ = v["metadata"].as_str();
            let _ = v["cluster_name"].as_str();
            black_box(v)
        })
    });

    group.bench_function("bincode_deserialize", |b| {
        b.iter(|| {
            let s: StoredInstance = bincode::deserialize(black_box(&bincode_bytes)).unwrap();
            black_box(s)
        })
    });

    group.finish();
}

fn bench_stored_instance_roundtrip(c: &mut Criterion) {
    let sample = stored_instance_sample();
    let mut group = c.benchmark_group("stored_instance_roundtrip");

    group.bench_function("json_value", |b| {
        b.iter(|| {
            let v = stored_instance_as_json_value(black_box(&sample));
            let bytes = serde_json::to_vec(&v).unwrap();
            let decoded: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            black_box(decoded)
        })
    });

    group.bench_function("bincode", |b| {
        b.iter(|| {
            let bytes = bincode::serialize(black_box(&sample)).unwrap();
            let decoded: StoredInstance = bincode::deserialize(&bytes).unwrap();
            black_box(decoded)
        })
    });

    group.finish();
}

// ============================================================================
// StoredConfig state-machine value benches
//
// Measures the hot path in `apply_config_publish_batched` / `_remove` /
// reader.rs get_config/list_configs. Compares JSON Value dance vs bincode.
// ============================================================================

fn stored_config_sample() -> StoredConfig {
    StoredConfig {
        data_id: "app.properties".to_string(),
        group: "DEFAULT_GROUP".to_string(),
        tenant: "public".to_string(),
        content: "key1=value1\nkey2=value2\nkey3=value3".to_string(),
        md5: "0123456789abcdef".to_string(),
        config_type: Some("properties".to_string()),
        app_name: Some("bench-app".to_string()),
        config_tags: Some(String::new()),
        desc: Some("bench".to_string()),
        r#use: None,
        effect: None,
        schema: None,
        encrypted_data_key: Some(String::new()),
        src_user: Some("bench".to_string()),
        src_ip: Some("127.0.0.1".to_string()),
        created_time: 1_700_000_000_000,
        modified_time: 1_700_000_000_000,
    }
}

fn stored_config_as_json_value(s: &StoredConfig) -> serde_json::Value {
    serde_json::json!({
        "data_id": s.data_id,
        "group": s.group,
        "tenant": s.tenant,
        "content": s.content,
        "md5": s.md5,
        "config_type": s.config_type,
        "app_name": s.app_name,
        "config_tags": s.config_tags,
        "desc": s.desc,
        "encrypted_data_key": s.encrypted_data_key,
        "src_user": s.src_user,
        "src_ip": s.src_ip,
        "created_time": s.created_time,
        "modified_time": s.modified_time,
    })
}

fn bench_stored_config_encode(c: &mut Criterion) {
    let sample = stored_config_sample();
    let mut group = c.benchmark_group("stored_config_encode");

    group.bench_function("json_value_to_vec", |b| {
        b.iter(|| {
            let v = stored_config_as_json_value(black_box(&sample));
            let bytes = serde_json::to_vec(&v).unwrap();
            black_box(bytes)
        })
    });

    group.bench_function("bincode_serialize", |b| {
        b.iter(|| {
            let bytes = bincode::serialize(black_box(&sample)).unwrap();
            black_box(bytes)
        })
    });

    group.finish();
}

fn bench_stored_config_decode(c: &mut Criterion) {
    let sample = stored_config_sample();
    let json_bytes = serde_json::to_vec(&stored_config_as_json_value(&sample)).unwrap();
    let bincode_bytes = bincode::serialize(&sample).unwrap();

    let mut group = c.benchmark_group("stored_config_decode");

    group.bench_function("json_value_from_slice", |b| {
        b.iter(|| {
            let v: serde_json::Value = serde_json::from_slice(black_box(&json_bytes)).unwrap();
            let _ = v["data_id"].as_str();
            let _ = v["group"].as_str();
            let _ = v["tenant"].as_str();
            let _ = v["content"].as_str();
            let _ = v["md5"].as_str();
            let _ = v["app_name"].as_str();
            let _ = v["config_type"].as_str();
            black_box(v)
        })
    });

    group.bench_function("bincode_deserialize", |b| {
        b.iter(|| {
            let s: StoredConfig = bincode::deserialize(black_box(&bincode_bytes)).unwrap();
            black_box(s)
        })
    });

    group.finish();
}

fn bench_stored_config_roundtrip(c: &mut Criterion) {
    let sample = stored_config_sample();
    let mut group = c.benchmark_group("stored_config_roundtrip");

    group.bench_function("json_value", |b| {
        b.iter(|| {
            let v = stored_config_as_json_value(black_box(&sample));
            let bytes = serde_json::to_vec(&v).unwrap();
            let decoded: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            black_box(decoded)
        })
    });

    group.bench_function("bincode", |b| {
        b.iter(|| {
            let bytes = bincode::serialize(black_box(&sample)).unwrap();
            let decoded: StoredConfig = bincode::deserialize(&bytes).unwrap();
            black_box(decoded)
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_serialize,
    bench_deserialize,
    bench_roundtrip,
    bench_stored_instance_encode,
    bench_stored_instance_decode,
    bench_stored_instance_roundtrip,
    bench_stored_config_encode,
    bench_stored_config_decode,
    bench_stored_config_roundtrip,
);
criterion_main!(benches);
