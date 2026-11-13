# Changelog

`batata-client` follows the workspace version declared in the root
`Cargo.toml`. See the [publishing checklist](../../docs/development.md#publishing-sdk-crates)
for release order.

## 1.0.0 — pending publication

First release built from the [`easynet-cn/batata`](https://github.com/easynet-cn/batata)
workspace. This is a **complete rewrite** of the standalone 0.0.x crate and is
not API compatible with it.

### Added

- Unified `BatataClient` / `ClientConfig` entry point with lazy gRPC and HTTP
  initialization, multiple server addresses, proxy modes, retry, address-server
  refresh and configurable labels.
- gRPC config service: get / publish / remove, typed and CAS publishing,
  callback and field-level change listeners, filters, encryption hooks and a
  local MD5 cache with redo after reconnect.
- gRPC naming service: register / deregister / batch register, health and
  weighted instance selection, subscriptions, fuzzy watch, service listing,
  empty-push protection and disk failover cache.
- JWT (username/password) and HMAC (AccessKey/SecretKey) authentication.
- TLS / mTLS for the SDK gRPC channel via `ClientConfig::with_tls`,
  `with_mtls`, `tls_domain` (or the matching `GrpcClientConfig` builders):
  custom CA or native roots, SNI override, client identity, `https://`
  endpoint on the derived gRPC port. The same CA/identity is applied to the
  HTTP login and admin clients, and bad PEM files fail at configuration time.
- Typed HTTP `BatataApiClient` for admin/console operations such as
  `config_list`.
- `Instance` re-export, so applications do not need a direct `batata-api`
  dependency for naming calls.

### Changed (breaking vs 0.0.2)

- `BatataClient::builder()` is replaced by
  `BatataClient::new(ClientConfig::new(...))`; `config_service()` and
  `naming_service()` are async and return `Result`; `shutdown()` is synchronous.
- Config methods take an explicit `tenant` argument and return `Result<String>`
  or `Result<bool>`. Configuration search moved to the HTTP API client.
- Wire models live in the separate [`batata-api`](https://crates.io/crates/batata-api)
  crate. Generated gRPC code is checked in, so neither `protoc` nor a `build.rs`
  is needed to compile the client.
- The whole workspace (including this crate) moved to version `1.0.0` and
  edition 2024 / minimum supported Rust 1.88 (let chains).

### Removed

- `AuthManager`, `Credentials`, `FileCache`, `TlsConfig`,
  `common::constants::*`, `ConfigChangeEvent` / `ServiceChangeEvent` callback
  types, `update_instance`, and the builder-style TLS/cache helpers. Their
  replacements are documented in the [README](README.md#tls-and-mtls).

### Known limitations

- TLS protects the gRPC channel; HTTP URLs keep the scheme supplied in
  `server_addrs`, so pass `https://host:port` when the HTTP endpoint is served
  over TLS.

## 0.0.2, 0.0.1 — legacy (yank pending)

Published on 2025-12-21 from the standalone
[`easynet-cn/batata-client`](https://github.com/easynet-cn/batata-client)
repository. Both versions are to be yanked **after** 1.0.0 is published and
verified (release step 5), so that new resolutions cannot select an
incompatible API; that repository is archived afterwards. They are not yanked
yet.
Yanking does not delete the crate: existing `Cargo.lock` files still resolve the
old versions. See the [migration notes](README.md#migrating-from-00x).
