# Batata Client

A Rust client library for [Batata](https://github.com/easynet-cn/batata), a
Nacos-compatible service discovery and configuration management server.

[![Crates.io](https://img.shields.io/crates/v/batata-client.svg)](https://crates.io/crates/batata-client)
[![Documentation](https://docs.rs/batata-client/badge.svg)](https://docs.rs/batata-client)
[![License](https://img.shields.io/crates/l/batata-client.svg)](LICENSE)

English | [中文](README_CN.md)

## Features

- **Configuration management**
  - Get, publish and remove configurations over gRPC
  - Publish with a content type or a CAS (compare-and-swap) MD5 guard
  - Listen for changes with callbacks, or receive field-level change diffs
  - Local cache with MD5 validation, config filters and encryption hooks
- **Service discovery**
  - Register, deregister and batch-register service instances
  - Query all instances, healthy instances, or pick one weighted healthy instance
  - Subscribe / unsubscribe with callbacks, fuzzy watch and service listing
  - Automatic heartbeat for ephemeral instances and redo after reconnect
- **Authentication**
  - Username/password JWT authentication with automatic token refresh
  - AccessKey/SecretKey (HMAC) authentication
  - Server identity headers for the admin API (via `batata-maintainer-client`)
- **Resilience and transport**
  - gRPC bidirectional streaming plus typed HTTP admin APIs
  - Multiple server addresses with failover, retry and address-server refresh
  - Proxy modes (off / system / custom), configurable timeouts and labels
  - TLS / mTLS for the gRPC channel (custom CA or native roots, SNI override,
    client certificates) and private CAs for HTTPS admin endpoints
  - Local file cache for disaster recovery and empty-push protection
  - Shared wire models come from [`batata-api`](https://crates.io/crates/batata-api)

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
batata-client = "1.0.0"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

All examples below use `anyhow::Result` for error handling; add `anyhow = "1"` if
your application does not already depend on it. `Instance` is re-exported from
this crate, so a direct `batata-api` dependency is only needed when you use other
shared wire models.

## Quick Start

### Basic Usage

```rust,no_run
use batata_client::{BatataClient, ClientConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Server addresses are "host:port" (an "http://" prefix is also accepted).
    let config = ClientConfig::new("127.0.0.1:8848")
        .with_auth("nacos", "nacos")
        .namespace("public");

    // Connections are established lazily when a service is first requested.
    let mut client = BatataClient::new(config).await?;

    let config_svc = client.config_service().await?;
    config_svc
        .publish_config("my-config", "DEFAULT_GROUP", "", "key=value")
        .await?;
    let content = config_svc
        .get_config("my-config", "DEFAULT_GROUP", "")
        .await?;
    println!("Config: {content}");

    let naming_svc = client.naming_service().await?;
    naming_svc
        .register_instance_simple("my-service", "127.0.0.1", 8080)
        .await?;
    let instances = naming_svc
        .select_instances("", "DEFAULT_GROUP", "my-service", true)
        .await?;
    println!("Healthy instances: {}", instances.len());

    client.shutdown();
    Ok(())
}
```

An empty `tenant` / `namespace` argument selects the public namespace. The
`ClientConfig::namespace(...)` value is used for gRPC connection setup.

### With Authentication

```rust,no_run
use batata_client::{BatataClient, ClientConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Username/password (JWT, refreshed automatically).
    let config = ClientConfig::new("127.0.0.1:8848").with_auth("nacos", "nacos");
    let jwt_client = BatataClient::new(config).await?;

    // AccessKey/SecretKey (HMAC signing).
    let config =
        ClientConfig::new("127.0.0.1:8848").with_access_key("your-ak", "your-sk");
    let aksk_client = BatataClient::new(config).await?;

    drop((jwt_client, aksk_client));
    Ok(())
}
```

### Listen for Configuration Changes

```rust,no_run
use std::sync::Arc;
use batata_client::config::listener::FnConfigChangeListener;
use batata_client::{BatataClient, ClientConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut client =
        BatataClient::new(ClientConfig::new("127.0.0.1:8848")).await?;
    let config_svc = client.config_service().await?;

    let listener = Arc::new(FnConfigChangeListener::new(|info| {
        println!("{} changed: {}", info.data_id, info.content);
    }));
    config_svc
        .add_listener("my-config", "DEFAULT_GROUP", "", listener)
        .await?;

    // Callbacks run on a Tokio runtime thread: never block inside them.
    // `add_change_event_listener` additionally delivers parsed field-level diffs.
    Ok(())
}
```

### Subscribe to Service Changes

```rust,no_run
use std::sync::Arc;
use batata_client::naming::listener::FnEventListener;
use batata_client::{BatataClient, ClientConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut client =
        BatataClient::new(ClientConfig::new("127.0.0.1:8848")).await?;
    let naming_svc = client.naming_service().await?;

    let listener = Arc::new(FnEventListener::new(|event| {
        println!("{} now has {} instances", event.service_name, event.instances.len());
    }));
    let initial = naming_svc
        .subscribe("", "DEFAULT_GROUP", "my-service", "", listener)
        .await?;
    println!("Initial instances: {}", initial.len());

    // `unsubscribe` takes the same namespace/group/service/clusters tuple.
    Ok(())
}
```

### Search Configurations

Configuration search is an HTTP admin API, not a gRPC SDK operation. Use the
typed API client:

```rust,no_run
use batata_client::{BatataClient, ClientConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut client = BatataClient::new(
        ClientConfig::new("127.0.0.1:8848").with_auth("nacos", "nacos"),
    )
    .await?;
    let api = client.api_client().await?;
    let page = api
        .config_list(1, 10, "", "", "DEFAULT_GROUP", "", "", "", "")
        .await?;

    println!("Found {} configs", page.total_count);
    for item in &page.page_items {
        println!("- {}: {}", item.data_id, item.group_name);
    }
    Ok(())
}
```

### Customize a Service Instance

There is no separate "update instance" call: build an `Instance` and register it
again with the same `ip`/`port`/`cluster_name`.

```rust,no_run
use batata_client::{BatataClient, ClientConfig, Instance};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut client =
        BatataClient::new(ClientConfig::new("127.0.0.1:8848")).await?;
    let naming_svc = client.naming_service().await?;

    let mut instance = Instance::new("127.0.0.1".to_string(), 8080);
    instance.weight = 2.0;
    instance.enabled = false;
    instance
        .metadata
        .insert("version".to_string(), "2.0.0".to_string());

    naming_svc
        .register_instance("", "DEFAULT_GROUP", "my-service", instance)
        .await?;
    Ok(())
}
```

### Local Cache and Failover

```rust,no_run
use batata_client::{BatataClient, ClientConfig, ProxyConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = ClientConfig::new("127.0.0.1:8848")
        // Disk cache for naming failover data.
        .naming_cache_dir("/tmp/batata-cache")
        .naming_load_cache_at_start(true)
        .naming_push_empty_protection(true)
        // Multiple servers, custom proxy, and an address-server refresh.
        .proxy(ProxyConfig::SystemProxy)
        .address_server("http://127.0.0.1:8080/nacos/serverlist");
    let client = BatataClient::new(config).await?;
    drop(client);
    Ok(())
}
```

## Configuration Options

| Builder / field | Description | Default |
|-----------------|-------------|---------|
| `ClientConfig::new(addr)` | Single server `host:port` | `127.0.0.1:8848` |
| `with_server_addrs(vec)` | Multiple server addresses for failover | - |
| `context_path(path)` | HTTP API context path | `/nacos` |
| `auth_endpoint(path)` | Login endpoint | `/v3/auth/user/login` |
| `with_auth(u, p)` | Username/password JWT auth | disabled |
| `with_access_key(ak, sk)` | HMAC AK/SK auth | disabled |
| `proxy(mode)` | `NoProxy` / `SystemProxy` / `Custom(url)` | `NoProxy` |
| `connect_timeout(d)` | Connection timeout | 5s |
| `request_timeout(d)` | Request timeout | 30s |
| `grpc_port_offset(n)` | gRPC port = HTTP port + n | 1000 |
| `namespace(ns)` | Namespace / tenant ID (empty = public) | empty |
| `label(k, v)` | Client label sent on connection setup | - |
| `address_server(url)` | Dynamic server-list endpoint | disabled |
| `server_list_refresh_interval(d)` | Refresh period | 30s |
| `server_max_failures(n)` | Failures before a server is unhealthy | 3 |
| `max_retries(n)` / `retry_initial_delay(d)` | Retry policy | 3 / 500ms |
| `redo_delay(d)` | Replay delay after reconnect | 3s |
| `config_long_poll_timeout(d)` | Config listen timeout | 30s |
| `naming_cache_dir(path)` / `naming_load_cache_at_start(b)` | Disk failover cache | temp / false |
| `naming_push_empty_protection(b)` | Reject empty instance pushes | false |
| `with_tls(ca_path)` | Enable gRPC TLS; `None` uses native roots | disabled |
| `with_mtls(ca, cert, key)` | Enable gRPC TLS with a client identity | disabled |
| `tls_domain(domain)` | SNI / certificate-name override for gRPC | derived from the address |

## TLS and mTLS

Enable TLS on the server's SDK gRPC listener in `conf/application.yml`:

```yaml
batata.remote.server.grpc.sdk.tls.enabled: true
batata.remote.server.grpc.tls.cert.path: /path/to/server.crt
batata.remote.server.grpc.tls.key.path: /path/to/server.key
# For mutual TLS:
batata.remote.server.grpc.tls.mtls.enabled: true
batata.remote.server.grpc.tls.ca.cert.path: /path/to/ca.crt
```

Then point the SDK at the same CA. The gRPC port is still derived from the
HTTP port (`+1000` by default); only the channel scheme changes to `https://`.

```rust,no_run
use batata_client::{BatataClient, ClientConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Verify the server with a private CA. Pass `None` to use the platform's
    // native root certificates instead.
    let config = ClientConfig::new("127.0.0.1:8848")
        .with_tls(Some("/path/to/ca.crt"))
        // Needed when connecting by IP but the certificate names a DNS name.
        .tls_domain("batata.example.com")
        .with_auth("nacos", "nacos");

    let mut client = BatataClient::new(config).await?;
    let config_svc = client.config_service().await?;
    let _ = config_svc.get_config("app.yaml", "DEFAULT_GROUP", "").await?;
    Ok(())
}
```

For mutual TLS add the client certificate and key:

```rust,no_run
use batata_client::ClientConfig;

let config = ClientConfig::new("127.0.0.1:8848")
    .with_mtls(
        Some("/path/to/ca.crt"),
        "/path/to/client.crt",
        "/path/to/client.key",
    )
    .tls_domain("batata.example.com");
# let _ = config;
```

The same CA and client identity are also applied to the JWT login and typed
HTTP admin calls. HTTP URLs use the scheme in `server_addrs`, so use
`https://host:port` when the HTTP endpoint sits behind a TLS-terminating
proxy. `GrpcClientConfig::with_tls` / `with_mtls` / `with_tls_domain` provide
the same settings when constructing a `GrpcClient` directly. Malformed or
missing PEM files fail at configuration time instead of silently falling back
to plaintext.

## Examples

The examples need a running Batata server with `nacos`/`nacos` credentials:

```bash
cargo run -p batata-client --example config_example
cargo run -p batata-client --example naming_example
```

The full SDK compatibility suites live in
[`sdk-tests/batata-client-tests`](https://github.com/easynet-cn/batata/tree/main/sdk-tests/batata-client-tests).

## Migrating from 0.0.x

`1.0.0` is a **complete rewrite**, not a drop-in upgrade. The `0.0.1` and
`0.0.2` releases came from the standalone `easynet-cn/batata-client`
repository; they are **yanked** and that repository is **archived**. Starting
with `1.0.0` the crate is built and published from the
[`easynet-cn/batata`](https://github.com/easynet-cn/batata) workspace.

Key differences:

- `BatataClient::builder()` is replaced by
  `BatataClient::new(ClientConfig::new(...).with_auth(...).namespace(...))`.
- `config_service()` / `naming_service()` are now `async` and return `Result`;
  `shutdown()` is synchronous.
- Config calls take an explicit `tenant` argument and return `Result<String>`
  / `Result<bool>`; `search_config` moved to the typed HTTP API client.
- The `AuthManager`, `Credentials`, `FileCache`, `TlsConfig` and
  `common::constants::*` re-exports no longer exist; authentication, proxy and
  cache settings live on `ClientConfig`. TLS is configured with
  `ClientConfig::with_tls` / `with_mtls` / `tls_domain` and is now applied to
  the real gRPC channel (see "TLS and mTLS").
- Wire models now live in the separate `batata-api` crate (`Instance` is
  re-exported here for convenience).
- The generated gRPC code is checked into `batata-api`, so compiling the client
  no longer needs `protoc` or a `build.rs`.

Yanked versions can no longer be selected for new resolutions, but existing
`Cargo.lock` files still resolve them. Plan for a real API migration rather
than a version bump only. See [CHANGELOG.md](CHANGELOG.md).

## Requirements

Rust 1.89 or newer (edition 2024), with a Tokio runtime. The 1.89 floor comes
from the `uuid` 1.29 runtime dependency, which requires Rust 1.89; the let chains
used by the SDK stabilized earlier, in Rust 1.88 for the 2024 edition. Both gRPC
and HTTP TLS use the rustls backend; no system OpenSSL development package is
required.

## Known Limitations

- TLS protects the gRPC channel; the HTTP login/admin URLs keep the scheme in
  `server_addrs`. Supply `https://host:port` explicitly when HTTP is served
  over TLS.
- Config search, history and clone are admin/console APIs; use
  `BatataClient::api_client()` or `batata-maintainer-client`, not the gRPC
  config service.
- There is no `update_instance` method; re-register the instance with the same
  identity (see "Customize a Service Instance" above).

## License

Apache-2.0. See [LICENSE](LICENSE).
