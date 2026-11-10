# Changelog

`batata-api` follows the workspace version declared in the root `Cargo.toml`.

## 1.0.0 — pending publication

Initial release.

- Shared gRPC service definitions and the HTTP request/response models used by
  the Nacos-compatible V2/V3 APIs.
- No server-side dependencies: no database, Raft, RocksDB or actix runtime.
- Generated gRPC code is checked into the crate, so compiling consumers does not
  require `protoc` or a build script.
- Published as a registry dependency of
  [`batata-client`](https://crates.io/crates/batata-client) and
  [`batata-maintainer-client`](https://crates.io/crates/batata-maintainer-client).

See the [README](README.md) and the
[publishing checklist](../../docs/development.md#publishing-sdk-crates).
