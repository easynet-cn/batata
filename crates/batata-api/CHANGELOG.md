# Changelog

`batata-api` follows the workspace version declared in the root `Cargo.toml`.

## 1.0.0 — 2026-11-13

Initial release.

- Shared gRPC service definitions and the HTTP request/response models used by
  the Nacos-compatible V2/V3 APIs.
- No workspace-internal dependencies: no `batata-common`, database, Raft,
  RocksDB or actix runtime. Server-only MCP/A2A (RAD) gRPC models live in
  `batata-ai::rad_model` so the published contract stays self-contained.
- Generated gRPC code is checked into the crate, so compiling consumers does not
  require `protoc` or a build script.
- `impl_request_trait!` / `impl_response_trait!` are exported so server-side
  crates can implement this crate's request/response traits for their own
  gRPC models.
- Published as a registry dependency of
  [`batata-client`](https://crates.io/crates/batata-client) and
  [`batata-maintainer-client`](https://crates.io/crates/batata-maintainer-client).

See the [README](README.md) and the
[publishing checklist](../../docs/development.md#publishing-sdk-crates).
