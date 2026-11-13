# Development

## Build Commands

```bash
# Build all crates
cargo build

# Build server only (faster)
cargo build -p batata-server

# Release build (optimized)
cargo build --release -p batata-server

# Run tests (all crates)
cargo test --workspace

# Run specific crate tests
cargo test -p batata-server

# Run benchmarks
cargo bench -p batata-server

# Format code
cargo fmt --all

# Lint with Clippy
cargo clippy --workspace

# Generate documentation
cargo doc --workspace --open
```

## Multi-Platform Build & Release

batata publishes prebuilt binaries through GitHub Actions. Pushing a tag of the
form `vX.Y.Z` triggers `.github/workflows/release.yml`, which cross-builds
`batata-server` for every platform × feature combination below and uploads a
tarball per combination to the GitHub Release.

### Supported targets

| Target | Rust target | Notes |
| --- | --- | --- |
| Linux x86_64 (glibc) | `x86_64-unknown-linux-gnu` | built natively on `ubuntu-24.04` |
| Linux ARM64 (glibc) | `aarch64-unknown-linux-gnu` | built natively on `ubuntu-24.04-arm` |
| Linux x86_64 (musl) | `x86_64-unknown-linux-musl` | static binary via `cargo-zigbuild` |
| Linux ARM64 (musl) | `aarch64-unknown-linux-musl` | static binary via `cargo-zigbuild` |
| macOS ARM64 | `aarch64-apple-darwin` | built natively on `macos-14` |
| macOS x86_64 | `x86_64-apple-darwin` | built natively on `macos-13` |

ARM (aarch64) systems — including Apple Silicon, AWS Graviton, and domestic
ARM servers (FeiTeng/Kunpeng) — are fully supported. The only native
dependency is RocksDB (C++); it builds cleanly on all of the above targets.

### Feature switches

Each target is built against every feature set declared in
`crates/batata-server/Cargo.toml`:

| Feature set | Build flag | Includes |
| --- | --- | --- |
| `default` | *(none)* | `consul` + `apollo` |
| `consul` | `--features consul` | Consul compatibility only |
| `apollo` | `--features apollo` | Apollo compatibility only |
| `full` | `--features full` | `consul` + `apollo` |
| `no-default` | `--no-default-features` | core server only |

This is driven by the `matrix.features` list in `release.yml`; the PR-only
workflow `.github/workflows/ci.yml` runs `cargo check` across all five sets on
`ubuntu-24.04` to catch breakage early.

### Artifact layout

Each release asset is named `batata-server-<version>-<target>-<features>.tar.gz`
and unpacks to:

```
batata-server-<version>-<target>-<features>/
├── bin/batata-server
├── conf/            # configuration files (read-only mount in containers)
├── docker-compose.yml
└── deploy/k8s/     # configmap.yaml, deployment.yaml, service.yaml
```

Extract and run:

```bash
tar -xzf batata-server-v1.2.3-linux-arm64-gnu-full.tar.gz
cd batata-server-v1.2.3-linux-arm64-gnu-full
./bin/batata-server   # reads ./conf/application.yml
```

The version printed by `--version` is the crate version (currently `1.0.0`);
the tag is only reflected in the artifact/Release filename.

## Publishing SDK crates

Three crates are published to crates.io: `batata-api`, `batata-client` and
`batata-maintainer-client`. Every other workspace crate sets `publish = false`.
Versions are unified through `[workspace.package] version` in the root
`Cargo.toml` (currently `1.0.0`); `batata-api` and `batata-client` also carry an
explicit `version` in `[workspace.dependencies]` so their packaged manifests
resolve from crates.io. Each published crate ships its own `README.md`,
`LICENSE` and `CHANGELOG.md`; `batata-client` also ships `README_CN.md` and two
compilable examples.

All three crates declare `rust-version = "1.88"`. The workspace is edition 2024
and uses let chains, which stabilized in Rust 1.88 and are only available in the
2024 edition, so the edition-2024 floor of 1.85 is not sufficient. Keep this in
sync with the badge in the root READMEs and the SDK `Requirements` sections.

**Hard rule:** a published crate must not depend on a `publish = false`
workspace crate through a path-only dependency. `cargo package` strips `path`
and requires a version requirement, so such a dependency either fails packaging
outright or resolves to a crate that does not exist on crates.io. `batata-api`
in particular has **no workspace-internal dependencies**; the server-only
MCP/A2A (RAD) gRPC models live in `batata-ai::rad_model`, not in the published
contract crate. `cargo package -p batata-api --allow-dirty` is the pre-publish
guard for this rule: run it before any release step, since the downstream
crates cannot be published until `batata-api` can be packaged.

### Local validation without publishing

`cargo package` resolves registry dependencies even with `--no-verify`, so
`batata-client` cannot be packed until `batata-api` exists on crates.io. Inject
a local patch for the dry run:

```bash
cargo package -p batata-api --allow-dirty
cargo --config 'patch.crates-io.batata-api.path="crates/batata-api"' \
      --config 'patch.crates-io.batata-client.path="crates/batata-client"' \
      package -p batata-client --allow-dirty --no-verify
cargo --config 'patch.crates-io.batata-api.path="crates/batata-api"' \
      --config 'patch.crates-io.batata-client.path="crates/batata-client"' \
      package -p batata-maintainer-client --allow-dirty --no-verify
```

Then extract the `.crate` files into one temporary directory, append
`[patch.crates-io]` entries pointing at the extracted siblings, and run
`cargo check --all-targets` for each. This validates the exact published source
tree without touching crates.io.

### TLS verification

`batata-client` and the gRPC server crates (`batata-server`,
`batata-server-common`, `batata-core`) enable tonic's `tls-aws-lc` feature so
rustls always has an unambiguous crypto provider. Workspace builds also enable
`ring` through `ldap3`; without an explicit provider, tonic's
`ClientConfig::builder()` / `ServerConfig::builder()` would panic at runtime.
`batata-api` only ships generated wire types and never builds a TLS channel, so
it keeps `tls-native-roots` alone. Do not drop `tls-aws-lc` when editing the
client or server manifests.

Before publishing, run the TLS transport suite. It generates a throwaway CA,
starts a real TLS gRPC server and an HTTPS server, and checks the success and
failure paths (custom CA, native roots, SNI override, mTLS, plaintext):

```bash
cargo test -p batata-client --test tls_transport_test
cargo test -p batata-maintainer-client --test client_test
```

For a manual server check, enable the SDK listener in `conf/application.yml`
with `batata.remote.server.grpc.sdk.tls.enabled` plus
`batata.remote.server.grpc.tls.cert.path` / `key.path` (and
`mtls.enabled` / `ca.cert.path` for mTLS), then connect with
`ClientConfig::with_tls(...)` or `with_mtls(...)`.

### Release order

1. `cargo login` with a token from `crates.io/settings/tokens` is a manual local
   step; credentials are never committed. CI uses `CARGO_REGISTRY_TOKEN`.
2. Publish in dependency order, waiting for the crates.io index after each
   step: `batata-api` -> `batata-client` -> `batata-maintainer-client`.
3. Push the `v1.0.0` tag; `.github/workflows/release.yml` builds the binary
   artifacts. The tag itself does not publish crates.
4. Verify from a clean project with `cargo add batata-client@1.0.0`, then re-run
   the SDK suites under `sdk-tests/` against version dependencies instead of
   `path` dependencies.
5. Retire the standalone 0.0.x line (see below). This must happen **after**
   step 4: yanking both 0.0.x versions while 1.0.0 is still unpublished would
   leave `batata-client` with no selectable version.

`batata-client` 1.0.0 is a rewrite of the standalone `0.0.x` releases; see its
README and CHANGELOG for migration notes. Published versions cannot be reused,
so the metadata, README and migration notes must be correct before step 2.

### Retiring the standalone 0.0.x line

Decision: **yank** `batata-client` 0.0.1 / 0.0.2 from crates.io and **archive**
the `easynet-cn/batata-client` repository. Both are manual owner-only actions
and are deliberately not automated in CI. Yanking only blocks new resolutions:
existing `Cargo.lock` files keep working, and it can be undone.

```bash
# Requires `cargo login <token>` (token: https://crates.io/settings/tokens)
cargo yank batata-client --version 0.0.1
cargo yank batata-client --version 0.0.2

# Undo a mistaken yank if needed:
# cargo yank --undo batata-client --version 0.0.2
```

Before archiving, add a banner at the top of the standalone repository's
README pointing to the monorepo, commit it, then archive the repository:

```text
> **Archived.** This repository contains the 0.0.x prototype of batata-client.
> 0.0.1 / 0.0.2 are yanked on crates.io. Development continues in the Batata
> workspace: https://github.com/easynet-cn/batata/tree/main/crates/batata-client
```

Archive it from GitHub Settings -> "Archive this repository", or with the GitHub
CLI (`gh repo archive easynet-cn/batata-client --yes`). `gh` is not installed on
the release machine; install and authorize it manually before using the CLI.
Cargo credentials live in `~/.cargo/credentials.toml` and are never committed.

Out of scope: `batata-consul-client` 0.0.2 is also owned by the same account,
but it is not part of this SDK release and is not yanked.

## Project Statistics

- **~50,000+ lines** of Rust code
- **25 internal crates** in workspace
- **4045 unit tests** (`cargo test --workspace --exclude batata-integration-tests`, 2026-11-13)
- **3 benchmark suites** for performance testing; absolute RPS is not a gate —
  use `scripts/bench_ab.py` or the same-binary A/B guards in the code (see
  ADR-0002 in [Architecture](architecture.md))

## Ignored test inventory

`#[ignore]` means the test does not exist as far as the gate is concerned. There
are 162 ignored Rust tests; they are **not** bulk-enabled. Classification
(2026-11-08):

| Reason string | Count | Interpretation |
|---|---|---|
| `requires running server` | 82 | Environment-dependent |
| `feature not implemented yet` | 26 | Stale label: the endpoints now exist, but the tests still need a live server |
| `#[ignore]` (no reason) | 16 | Environment-dependent; 4 of these sit in the unused `test_both_databases!` macro |
| `performance tests require specialized setup` | 11 | Environment-dependent; run via the perf gate |
| `requires test database` | 10 | Environment-dependent |
| `requires running server with Consul plugin` | 10 | Environment-dependent |
| `requires running gRPC server` | 2 | Environment-dependent |
| `requires PostgreSQL database` | 2 | Environment-dependent |
| `requires MySQL database` | 2 | Environment-dependent |
| `cluster gate removed; needs Raft status replication first` | 1 | The only real functional gap |

So 135 are environment-dependent, 26 carry a stale "unimplemented" label that
must be re-checked before enabling, and 1 documents an unimplemented Raft
status replication. None are enabled by this inventory; the authoritative
source is the `#[ignore = "..."]` reason string in each file.
