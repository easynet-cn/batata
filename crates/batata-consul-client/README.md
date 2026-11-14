# batata-consul-client

[![Crates.io](https://img.shields.io/crates/v/batata-consul-client.svg)](https://crates.io/crates/batata-consul-client)
[![Documentation](https://docs.rs/batata-consul-client/badge.svg)](https://docs.rs/batata-consul-client)
[![License](https://img.shields.io/crates/l/batata-consul-client.svg)](LICENSE)

English | [中文](README_CN.md)

A Rust client for the Consul HTTP API, built for
[Batata](https://github.com/easynet-cn/batata) but usable against HashiCorp
Consul as well. It provides method-level parity with the official Go SDK
(`github.com/hashicorp/consul/api`): one Rust module per `api/*.go` file.

This crate has **no workspace-internal dependencies**. It depends only on
`reqwest`, `serde`, `tokio`, `tracing` and a few small utilities, so it can be
used on its own without pulling in the rest of the Batata workspace.

## Features

- **Full HTTP API surface** — ACL, agent, catalog, config entries, connect,
  coordinate, debug, discovery chain, events, exported/imported services,
  health, KV, lock/semaphore, namespaces, operator, partitions, peering,
  prepared queries, raw requests, sessions, snapshots, status, transactions
  and watches.
- **Flat, typed methods** — every call is a method on [`ConsulClient`], e.g.
  `kv_get`, `catalog_services`, `agent_service_register`, together with typed
  models and `QueryMeta` / `WriteMeta` results.
- **Blocking queries** — `QueryOptions::wait_index` / `wait_time` and
  `QueryMeta::last_index` mirror Consul's long-polling contract; `X-Consul-Index`,
  `X-Consul-KnownLeader`, `X-Consul-LastContact` and cache headers are parsed.
- **Auth, TLS and failover** — ACL tokens (`X-Consul-Token`, static or re-read
  from `token_file`), custom CA / mTLS / insecure-skip-verify, multiple agent
  addresses with rotation, retries with exponential backoff on 429/5xx.
- **Observability** — client-side metrics exposed through `ConsulClient::metrics`.

`KVPair::value` follows Consul's wire format: it holds **base64-encoded**
bytes. Use `value_bytes()` / `value_str()` to decode responses, and
base64-encode your payload before passing a `KVPair` to `kv_put`
(`kv_put` decodes the field and sends the raw bytes).

## Installation

```toml
[dependencies]
batata-consul-client = "1.0.0"
tokio = { version = "1", features = ["rt", "macros"] }
```

## Quick Start

```rust,no_run
use batata_consul_client::{
    ConsulClient, ConsulClientConfig, KVPair, QueryOptions, WriteOptions,
};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Batata exposes the Consul API on its SDK HTTP port (8848).
    let config = ConsulClientConfig::new("http://127.0.0.1:8848");
    let client = ConsulClient::new(config)?;

    let (leader, _) = client.status_leader(&QueryOptions::default()).await?;
    println!("leader: {leader}");

    // "key=value" base64-encoded, as required by Consul's JSON wire format.
    let pair = KVPair {
        key: "example/config".to_string(),
        value: Some("a2V5PXZhbHVl".to_string()),
        ..Default::default()
    };
    client.kv_put(&pair, &WriteOptions::default()).await?;

    let (stored, meta) = client
        .kv_get("example/config", &QueryOptions::default())
        .await?;
    println!(
        "value: {:?}, index: {}",
        stored.and_then(|p| p.value_str()),
        meta.last_index
    );

    let (keys, _) = client
        .kv_keys("example/", "", &QueryOptions::default())
        .await?;
    println!("keys under example/: {keys:?}");

    client
        .kv_delete("example/config", &WriteOptions::default())
        .await?;
    Ok(())
}
```

An empty `QueryOptions::default()` uses the client's configured datacenter,
token and namespace. Reads generally return `(T, QueryMeta)` and writes
generally return `(bool, WriteMeta)` (a few endpoints return only `WriteMeta`);
a missing KV key is `Ok((None, _))`, not an error.

## ACL Tokens and TLS

```rust,no_run
use batata_consul_client::{ConsulClient, ConsulClientConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = ConsulClientConfig::new("https://consul.example.com:8501")
        .with_token("acl-token")
        // For mTLS: CA, client certificate, client key (all PEM).
        .with_mtls(
            "/path/to/ca.pem",
            "/path/to/client.pem",
            "/path/to/client-key.pem",
        );
    let client = ConsulClient::new(config)?;
    drop(client);
    Ok(())
}
```

`with_tls(ca)` installs a private CA without a client identity. Optional
`token_file` is re-read on every request so short-lived tokens can be rotated.
`tls_insecure_skip_verify` is intended for development only. A malformed or
missing PEM file fails at `ConsulClient::new`, rather than silently falling
back to plaintext.

## Blocking Queries

```rust,no_run
use batata_consul_client::{ConsulClient, ConsulClientConfig, QueryOptions};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = ConsulClient::new(ConsulClientConfig::default())?;
    let mut opts = QueryOptions::default();
    let mut index = 0;

    loop {
        opts.wait_index = index;
        opts.wait_time = Some(std::time::Duration::from_secs(30));
        let (pairs, meta) = client.kv_list("app/", &opts).await?;
        if meta.last_index > 0 {
            index = meta.last_index;
        }
        println!("{} entries at index {index}", pairs.len());
    }
}
```

## Configuration Options

| Builder / field | Description | Default |
|-----------------|-------------|---------|
| `ConsulClientConfig::new(addr)` | Primary agent HTTP(S) address | `http://127.0.0.1:8500` |
| `with_addresses(vec)` | Multiple addresses for failover rotation | single address |
| `with_token(token)` | ACL token | empty |
| `with_token_file(path)` | Token file, re-read per request | disabled |
| `with_datacenter(dc)` | Datacenter override | agent default |
| `with_wait_time(d)` | Default blocking-query wait | disabled |
| `with_timeouts(connect, read)` | Connection / read timeouts | 5s / 30s |
| `with_namespace(ns)` / `with_partition(p)` | Consul Enterprise scoping | empty |
| `with_tls(ca)` / `with_mtls(ca, cert, key)` | TLS / mutual TLS | disabled |
| `tls_insecure_skip_verify` | Skip certificate verification | `false` |
| `with_retry(max, base_ms)` | Retry count and backoff base | 2 / 100ms |
| `max_retries`, `retry_backoff_max_ms` | Backoff ceiling | 5000ms |

Request-level overrides live on `QueryOptions` / `WriteOptions`; the ACL token
resolution order is request override > `token_file` > client default.

## Examples

```bash
cargo run -p batata-consul-client --example kv_example
```

The example expects a Consul-compatible endpoint on `127.0.0.1:8500` (set the
address in the source to `http://127.0.0.1:8848` for a Batata server).

The compatibility suites live in
[`sdk-tests/batata-consul-client-tests`](https://github.com/easynet-cn/batata/tree/main/sdk-tests/batata-consul-client-tests).

## Migrating from 0.0.x

`1.0.0` is a **complete rewrite** of the standalone `easynet-cn/batata-consul-client`
prototype and is not API compatible with `0.0.1` / `0.0.2`. Starting with
`1.0.0` the crate is built and published from the
[`easynet-cn/batata`](https://github.com/easynet-cn/batata) workspace.

Key differences:

- `Client` / `Config` are replaced by `ConsulClient` / `ConsulClientConfig`.
- Sub-API accessors (`client.kv()`, `client.agent()`, `client.health()`, …)
  are replaced by flat methods on `ConsulClient` (`kv_get`, `kv_put`,
  `agent_service_register`, `health_service`, …).
- `api::*` modules are replaced by top-level modules such as `kv`, `agent`,
  `health`, `catalog`, `session`; types are re-exported at the crate root from
  `model`.
- `KVPair::value_string()` is replaced by `value_str()`; `kv.put_string(...)`
  is replaced by building a `KVPair` with a base64-encoded `value` and calling
  `kv_put`.
- `AgentServiceRegistration` builders and the `urlencoding`-based helpers no
  longer exist; use the typed `model` structs with `Default` + field assignment.
- The workspace moved to edition 2024 / minimum supported Rust 1.89.

Existing `Cargo.lock` files keep resolving the old versions, so plan for a real
API migration rather than a version bump only. See [CHANGELOG.md](CHANGELOG.md).

## Related crates

- [`batata-client`](https://crates.io/crates/batata-client) — Nacos-compatible
  config and service discovery SDK over gRPC.
- [`batata-maintainer-client`](https://crates.io/crates/batata-maintainer-client)
  — typed admin / maintainer API client.
- [`batata-api`](https://crates.io/crates/batata-api) — shared wire models for
  the Nacos-compatible surface. The Consul SDK does not depend on it.

## Requirements

Rust 1.89 or newer (edition 2024), with a Tokio runtime. The crate's own
dependency closure reaches Rust 1.88, but it declares 1.89 to keep a single
MSRV across all four published Batata SDK crates. TLS uses the rustls backend;
no system OpenSSL development package is required.

## License

Apache-2.0. See [LICENSE](LICENSE).
