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

The version printed by `--version` is the crate version (currently `0.1.0`);
the tag is only reflected in the artifact/Release filename.

## Project Statistics

- **~50,000+ lines** of Rust code
- **19 internal crates** in workspace
- **333 unit tests** with comprehensive coverage
- **3 benchmark suites** for performance testing
