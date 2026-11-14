# Changelog

`batata-maintainer-client` follows the workspace version declared in the root
`Cargo.toml`.

## 1.0.0 — 2026-11-13

Initial release.

- Typed HTTP client for the Batata/Nacos admin APIs: server state, cluster,
  namespace, configuration, history, naming, AI (MCP / agent), plugin and
  import/export management.
- Shares authentication and HTTP transport with
  [`batata-client`](https://crates.io/crates/batata-client).
- `MaintainerClient::from_server_addr(...)` for the common single-server case,
  and `MaintainerClient::new(MaintainerClientConfig)` for multiple addresses,
  context path, timeouts and server identity headers.
- `MaintainerClientConfig::with_tls(ca)` / `with_mtls(ca, cert, key)` for
  `https://` endpoints terminated with a private CA or requiring mutual TLS.

This crate has no 0.0.x history. The SDK line as a whole supersedes the
standalone `batata-client` 0.0.x releases; see the
[batata-client migration notes](https://github.com/easynet-cn/batata/blob/main/crates/batata-client/README.md#migrating-from-00x).
