# batata-api

[![Crates.io](https://img.shields.io/crates/v/batata-api.svg)](https://crates.io/crates/batata-api)
[![Documentation](https://docs.rs/batata-api/badge.svg)](https://docs.rs/batata-api)
[![License](https://img.shields.io/crates/l/batata-api.svg)](LICENSE)

Shared protocol contract for [Batata](https://github.com/easynet-cn/batata) SDK
clients and the server: gRPC service definitions plus the HTTP request/response
models used by the Nacos-compatible V2/V3 APIs.

This crate deliberately has **no workspace-internal dependencies** (no
`batata-common`, database, Raft or RocksDB). It is published so that
`batata-client` and `batata-maintainer-client` can be resolved from crates.io;
application code normally depends on those crates instead of importing this one
directly. The server-only MCP/A2A (RAD) gRPC models live in the unpublished
`batata-ai` crate; `remote::model` here only carries the request/response
contract shared with SDK clients.

## Install

```toml
[dependencies]
batata-api = "1.0.0"
```

## Usage

```rust
// Common models and constants are re-exported at the crate root.
let address = batata_api::local_ip();
println!("local address: {address}");
```

The crate exposes:

- `grpc` — generated gRPC service definitions for config and naming (already
  checked in; no `protoc` or build script is needed to compile this crate).
- `config`, `naming`, `remote`, `model` — HTTP API request/response models.

## Related crates

- [`batata-client`](https://crates.io/crates/batata-client) — config and service
  discovery SDK. It re-exports `Instance`, so most applications do not need to
  import this crate directly.
- [`batata-maintainer-client`](https://crates.io/crates/batata-maintainer-client)
  — typed admin / maintainer API client.
- [`batata-consul-client`](https://crates.io/crates/batata-consul-client)
  — independent Consul-compatible HTTP client; it does not depend on `batata-api`.

See [CHANGELOG.md](CHANGELOG.md) for release history.

## Requirements

Rust 1.89 or newer (edition 2024). The SDK line shares one floor: let chains need Rust 1.88, and the client's `uuid` 1.29 runtime dependency needs Rust 1.89.

## License

Apache-2.0. See [LICENSE](LICENSE).
