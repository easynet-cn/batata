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

Four crates are published to crates.io: `batata-api`, `batata-client`,
`batata-maintainer-client` and `batata-consul-client`. Every other workspace
crate sets `publish = false`. All four start at `1.0.0`. The first three take
their version from `[workspace.package] version` in the root `Cargo.toml` and
are linked by `^1.0.0` dependencies, so they move together.
`batata-consul-client` declares `version = "1.0.0"` in its own manifest: it has
no inbound workspace dependencies and must be able to publish patch releases
without bumping the version of all 25 workspace members. When the root version
changes, update that crate separately. Do not add it to
`[workspace.dependencies]`; an unused entry triggers Cargo's
`unused_dependencies` lint. `batata-api` and `batata-client` carry an explicit
`version` in `[workspace.dependencies]` so their packaged manifests resolve
from crates.io. Each published crate ships its own `README.md`, `LICENSE` and
`CHANGELOG.md`; `batata-client` also ships `README_CN.md` and two compilable
examples, and `batata-consul-client` ships `README_CN.md` plus one example.

All four crates declare `rust-version = "1.89"`. The workspace is edition 2024
and uses let chains, which stabilized in Rust 1.88 and are only available in the
2024 edition, so the edition-2024 floor of 1.85 is not sufficient. The floor is
1.89 because `batata-client` depends on `uuid` 1.29 at runtime, whose MSRV is
1.89 (`uuid` >= 1.27 all require 1.89). `batata-consul-client` has no
`batata-*` dependencies and its own closure only reaches 1.88 (through
`icu_provider` 2.4.0), but it declares 1.89 so the SDK line keeps one MSRV.
Verify before publishing with
`cargo +1.89 check -p batata-api -p batata-client -p batata-maintainer-client -p batata-consul-client`.
Keep this in sync with the badge in the root READMEs and the SDK `Requirements`
sections.

**Hard rule:** a published crate must not depend on a `publish = false`
workspace crate through a path-only dependency. `cargo package` strips `path`
and requires a version requirement, so such a dependency either fails packaging
outright or resolves to a crate that does not exist on crates.io. `batata-api`
in particular has **no workspace-internal dependencies**; the server-only
MCP/A2A (RAD) gRPC models live in `batata-ai::rad_model`, not in the published
contract crate. `batata-consul-client` likewise has no workspace-internal
dependencies, so it packages and publishes independently of the other three.
`cargo package -p batata-api --allow-dirty` is the pre-publish guard for this
rule: run it before any release step, since the downstream crates cannot be
published until `batata-api` can be packaged.

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
tree without touching crates.io. `batata-consul-client` has no path-only
internal dependencies, so `cargo package -p batata-consul-client` needs none of
those patches.

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
   step; credentials are never committed. No CI job publishes crates: `ci.yml`
   only runs checks and `release.yml` only builds tagged binaries, so publishing
   is always performed locally.
2. Publish in dependency order, waiting for the crates.io index after each
   step: `batata-api` -> `batata-client` -> `batata-maintainer-client`.
   `batata-consul-client` has no internal dependencies and is published
   independently; it does not require any other Batata crate on crates.io.
3. Push the `v1.0.0` tag; `.github/workflows/release.yml` builds the binary
   artifacts. The tag itself does not publish crates.
4. Verify from a clean project with `cargo add batata-client@1.0.0` and
   `cargo add batata-consul-client@1.0.0`, then re-run the SDK suites under
   `sdk-tests/` against version dependencies instead of `path` dependencies.
   Keep the in-repo suites on `path` dependencies so the development loop tests
   the working tree rather than the published package.
5. Retire the standalone 0.0.x lines (completed 2026-11-14; see below). This
   must happen **after** step 4: yanking the 0.0.x versions while 1.0.0 is
   still unpublished would leave those crate names with no selectable version.

`batata-client` and `batata-consul-client` 1.0.0 are rewrites of their
standalone `0.0.x` releases; see their READMEs and CHANGELOGs for migration
notes. Published versions cannot be reused, so the metadata, README and
migration notes must be correct before step 2.

### Retiring the standalone 0.0.x lines

Decision: **yank** `batata-client` 0.0.1 / 0.0.2 and `batata-consul-client`
0.0.1 / 0.0.2 from crates.io, and **archive** both standalone repositories
(`easynet-cn/batata-client` and `easynet-cn/batata-consul-client`). These are
manual owner-only actions and are deliberately not automated in CI. Yanking
only blocks new resolutions: existing `Cargo.lock` files keep working, and it
can be undone. Never yank before the corresponding 1.0.0 is published and
verified.

**Status (2026-11-14): completed.** All four 0.0.x versions are
`yanked=true` (verified via the crates.io API and the sparse index), both
1.0.0 releases remain `yanked=false` with `default_version=1.0.0`, the
Archived README banners were pushed to both standalone repositories, and both
repositories are `archived=true` on GitHub. The commands and banner below are
kept as a runbook for future release lines, not as pending work.

```bash
# Requires `cargo login <token>` (token: https://crates.io/settings/tokens)
cargo yank batata-client --version 0.0.1
cargo yank batata-client --version 0.0.2
cargo yank batata-consul-client --version 0.0.1
cargo yank batata-consul-client --version 0.0.2

# Undo a mistaken yank if needed:
# cargo yank --undo batata-client --version 0.0.2
# cargo yank --undo batata-consul-client --version 0.0.2
```

The Archived banner committed at the top of each standalone repository's
README points to the monorepo:

```text
> **Archived.** This repository contains the 0.0.x prototype of batata-client.
> 0.0.1 / 0.0.2 are yanked on crates.io. Development continues in the Batata
> workspace: https://github.com/easynet-cn/batata/tree/main/crates/batata-client
```

Use the analogous text for `batata-consul-client`, pointing at
`https://github.com/easynet-cn/batata/tree/main/crates/batata-consul-client`.

Archive each repository from GitHub Settings -> "Archive this repository", or
with the GitHub CLI (`gh repo archive easynet-cn/batata-client --yes` and
`gh repo archive easynet-cn/batata-consul-client --yes`). This step was
performed by the repository owner on 2026-11-14; `gh` is not required when the
Settings UI is used. Cargo credentials live in `~/.cargo/credentials.toml`
and are never committed.

## Project Statistics

- **~50,000+ lines** of Rust code
- **25 internal crates** in workspace
- **4047 unit tests** (`cargo test --workspace --exclude batata-integration-tests`, 2026-11-14; 0 failed / 33 ignored)
- **3 benchmark suites** for performance testing; absolute RPS is not a gate —
  use `scripts/bench_ab.py` or the same-binary A/B guards in the code (see
  ADR-0002 in [Architecture](architecture.md))

## Ignored test inventory

`#[ignore]` means the test does not exist as far as the gate is concerned. There
are 161 ignored Rust tests; they are **not** bulk-enabled. Classification
(2026-11-14; the 2026-11-08 census had 162):

| Reason string | Count | Interpretation |
|---|---|---|
| `requires running server` | 108 | Environment-dependent; includes 26 cases reclassified on 2026-11-14 from a stale "unimplemented" label |
| `#[ignore]` (no reason) | 16 | Environment-dependent; 4 of these sit in the unused `test_both_databases!` macro |
| `performance tests require specialized setup` | 11 | Environment-dependent; run via the perf gate |
| `requires test database` | 10 | Environment-dependent |
| `requires running server with Consul plugin` | 10 | Environment-dependent |
| `requires running gRPC server` | 2 | Environment-dependent |
| `requires PostgreSQL database` | 2 | Environment-dependent |
| `requires MySQL database` | 2 | Environment-dependent |

All 161 are environment-dependent. On 2026-11-14 the 26 stale
`feature not implemented yet` labels were corrected to
`requires running server`: the endpoints exist, but all 26 tests log in
through a live server first, so they cannot be enabled in the unit gate. The
single `cluster gate removed; needs Raft status replication first` entry was
removed from the inventory entirely: Raft status replication landed in
`27e72b00`, the responsibility gate is back in `HealthCheckReactor`, and that
test now runs in the normal unit suite. Per-node check identity (for example
`serfHealth` registered by several Consul agents) is handled by
`InstanceCheckConfig::registry_key()` / `qualify_check_key()` since
`d53093ca`, so no functional gap remains in this inventory. None of the
ignored tests are enabled here; the authoritative source is the
`#[ignore = "..."]` reason string in each file.
