# Changelog

`batata-consul-client` maintains its own version in this crate's manifest, so
patch releases do not require bumping the version of the whole workspace. See
the [publishing checklist](../../docs/development.md#publishing-sdk-crates)
for release order.

## 1.0.0

First release built from the [`easynet-cn/batata`](https://github.com/easynet-cn/batata)
workspace. This is a **complete rewrite** of the standalone 0.0.x crate and is
not API compatible with it.

### Added

- Flat, typed `ConsulClient` API with method-level parity against the Consul Go
  SDK: `acl`, `agent`, `catalog`, `config_entry`, `config_entry_types`,
  `connect`, `coordinate`, `debug`, `discovery_chain`, `event`,
  `exported_services`, `health`, `imported_services`, `internal`, `kv`, `lock`,
  `namespace`, `operator`, `partition`, `peering`, `query`, `raw`, `session`,
  `snapshot`, `status`, `txn` and `watch` modules.
- Blocking-query support: `QueryOptions::wait_index` / `wait_time`,
  `QueryMeta::last_index`, and parsing of `X-Consul-Index`,
  `X-Consul-KnownLeader`, `X-Consul-LastContact` plus cache headers.
- ACL token handling with request-level override, static token and
  `token_file` re-read per request.
- TLS / mTLS with a private CA or client identity, and an explicit
  development-only `tls_insecure_skip_verify` switch. Bad PEM files fail at
  `ConsulClient::new` instead of silently falling back to plaintext.
- Multiple agent addresses with rotation on connection failure, retry with
  exponential backoff on 429/5xx, and client-side `ConsulClient::metrics`.
- `KVPair::value_bytes()` / `value_str()` helpers for Consul's base64-encoded
  KV payloads, and `get_optional` handling of `200 + JSON null` responses.
- Published crate files: `README.md`, `README_CN.md`, `LICENSE`, this changelog
  and the compilable `examples/kv_example.rs`.

### Changed (breaking vs 0.0.2)

- `Client` / `Config` are replaced by `ConsulClient` / `ConsulClientConfig`.
- Sub-API accessors (`client.kv()`, `client.agent()`, `client.health()`, …)
  are replaced by flat methods such as `kv_get`, `kv_put`,
  `agent_service_register` and `health_service`.
- `api::*` modules are replaced by top-level modules; wire models are
  re-exported at the crate root from `model`.
- `KVPair::value_string()` becomes `value_str()`; `kv.put_string(...)` becomes
  a `KVPair` with a base64-encoded `value` passed to `kv_put`.
- The crate has no workspace-internal dependencies and no `build.rs`, so it
  can be published and used independently of the other Batata SDK crates.
- Workspace moved to edition 2024 / Rust 1.89. The crate's own dependency
  closure needs Rust 1.88, but it declares 1.89 to keep a single MSRV across
  all four published SDK crates.

### Removed

- `AgentServiceRegistration` builders, the `urlencoding`-based helpers and the
  nested `api::*` namespace. Their replacements are the typed structs in
  `model`, constructed with `Default` plus field assignment.

See the [migration notes](README.md#migrating-from-00x) for a call-by-call
mapping.

## 0.0.2, 0.0.1 — legacy (yanked)

Published from the standalone
[`easynet-cn/batata-consul-client`](https://github.com/easynet-cn/batata-consul-client)
repository. These versions are superseded by 1.0.0 and were yanked on
crates.io on 2026-11-14 (release step 5, after 1.0.0 was published and
verified), and the standalone repository was archived on 2026-11-14. Yanking
does not delete the crate: existing `Cargo.lock` files still resolve the old
versions.
