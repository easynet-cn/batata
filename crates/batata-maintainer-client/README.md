# batata-maintainer-client

[![Crates.io](https://img.shields.io/crates/v/batata-maintainer-client.svg)](https://crates.io/crates/batata-maintainer-client)
[![Documentation](https://docs.rs/batata-maintainer-client/badge.svg)](https://docs.rs/batata-maintainer-client)
[![License](https://img.shields.io/crates/l/batata-maintainer-client.svg)](LICENSE)

Admin / maintainer HTTP client for [Batata](https://github.com/easynet-cn/batata).
It wraps the Nacos-compatible admin APIs for server state, cluster, namespace,
configuration, naming, AI (MCP / agent), plugin and import/export management.

## Install

```toml
[dependencies]
batata-maintainer-client = "1.0.0"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

## Quick start

```rust,no_run
use batata_maintainer_client::MaintainerClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = MaintainerClient::from_server_addr(
        "http://127.0.0.1:8848",
        "nacos",
        "nacos",
    )
    .await?;

    let members = client.cluster_members().await?;
    println!("cluster members: {}", members.len());
    Ok(())
}
```

For finer control (multiple server addresses, context path, timeouts, server
identity headers), build a `MaintainerClientConfig` and call
`MaintainerClient::new(config)`.

## HTTPS and mutual TLS

Use an `https://` server address and, when the endpoint is terminated with a
private CA, register that CA (and optionally a client identity) on the config:

```rust,no_run
use batata_maintainer_client::{MaintainerClient, MaintainerClientConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = MaintainerClientConfig {
        server_addrs: vec!["https://batata.example.com:8848".to_string()],
        ..Default::default()
    }
    .with_mtls("/path/to/ca.crt", "/path/to/client.crt", "/path/to/client.key");

    let client = MaintainerClient::new(config).await?;
    let state = client.server_state().await?;
    println!("server state: {:?}", state);
    Ok(())
}
```

`with_tls(ca)` trusts a private CA without a client certificate. Both helpers
also apply to JWT login. Missing or malformed PEM files fail when the client
is constructed rather than silently disabling verification.

This crate builds on [`batata-client`](https://crates.io/crates/batata-client) and
shares its authentication and HTTP transport. It is intended for the admin API;
regular config and discovery workloads should use `batata-client` directly.

## Related crates

- [`batata-client`](https://crates.io/crates/batata-client) — config and service
  discovery SDK; this crate builds on its authentication and HTTP transport.
- [`batata-api`](https://crates.io/crates/batata-api) — shared wire models.

See [CHANGELOG.md](CHANGELOG.md) for release history. The 1.0.0 release of the
SDK line replaces the yanked standalone `batata-client` 0.0.x versions; see the
[batata-client migration notes](https://github.com/easynet-cn/batata/blob/main/crates/batata-client/README.md#migrating-from-00x).

## Requirements

Rust 1.88 or newer (edition 2024; let chains are used across the workspace).

## License

Apache-2.0. See [LICENSE](LICENSE).
