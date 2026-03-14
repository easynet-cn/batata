# Batata Testing Guide

## Overview

Batata uses a multi-layered testing strategy covering unit tests, integration tests, property-based tests, and benchmarks across 19+ crates.

## Running Tests

### Quick Start

```bash
# Run all unit tests (fast, no external dependencies)
cargo test --workspace --lib

# Run all tests including integration tests
cargo test --workspace

# Run tests for a specific crate
cargo test -p batata-auth
cargo test -p batata-common
cargo test -p batata-api

# Run with output visible
cargo test --workspace -- --nocapture

# Run only ignored tests (require running server or database)
cargo test -p batata-server -- --ignored

# Run a specific test
cargo test -p batata-auth test_encode_decode_roundtrip -- --exact
```

### Running Benchmarks

```bash
# Run all benchmarks
cargo bench -p batata-server

# Run specific benchmark
cargo bench -p batata-server -- auth_bench
```

### Code Quality

```bash
# Format code
cargo fmt --all

# Lint with Clippy
cargo clippy --workspace

# Check for unused dependencies
cargo +nightly udeps --workspace
```

## Test Categories

### 1. Unit Tests (inline `#[cfg(test)]`)

Located within each crate's source files. These test individual functions and types without external dependencies.

| Crate | Location | Focus |
|-------|----------|-------|
| `batata-common` | `src/*.rs` | Error types, utilities, crypto, validation |
| `batata-auth` | `src/**/*.rs` | JWT tokens, RBAC, LDAP config, OAuth models |
| `batata-api` | `src/**/*.rs` | Validation, serialization, message types |
| `batata-persistence` | `src/model.rs` | Storage models, pagination |
| `batata-config` | `src/**/*.rs` | Config models, encryption patterns |
| `batata-naming` | `src/**/*.rs` | Instance, selectors, health checks |
| `batata-core` | `src/**/*.rs` | Cluster, connections |
| `batata-plugin-consul` | `src/**/*.rs` | Consul compatibility |

### 2. Integration Tests (`tests/` directory)

Located in `crates/batata-server/tests/`. These test HTTP API endpoints and gRPC handlers.

**Prerequisites:**
- Running Batata server (start with `./scripts/start-embedded.sh`)
- Admin user created (`./scripts/init-admin.sh`)

**Structure:**
```
crates/batata-server/tests/
├── common/           # Shared test infrastructure
│   ├── mod.rs        # Constants, ID generators
│   ├── client.rs     # TestClient HTTP client
│   ├── server.rs     # TestServer lifecycle
│   ├── db.rs         # Database test utilities
│   └── fixtures.rs   # Test data builders
├── http_api/         # HTTP API tests
│   ├── auth_api_test.rs
│   ├── config_api_test.rs
│   ├── naming_api_test.rs
│   └── ...
├── grpc_api/         # gRPC handler tests
│   ├── config_handler_test.rs
│   └── naming_handler_test.rs
├── persistence/      # Database tests
│   ├── config_test.rs
│   └── auth_test.rs
└── compatibility/    # Protocol compatibility
    └── consul_api_test.rs
```

### 3. Benchmarks (`benches/` directory)

Located in `crates/batata-server/benches/`. Uses Criterion for statistical benchmarking.

```
crates/batata-server/benches/
├── auth_bench.rs             # JWT encode/decode performance
├── naming_bench.rs           # Instance registration throughput
└── circuit_breaker_bench.rs  # Circuit breaker pattern performance
```

## Test Infrastructure

### TestClient

The `TestClient` provides HTTP API testing with authentication support:

```rust
// Create client and login
let mut client = TestClient::new(MAIN_BASE_URL);
client.login_via(CONSOLE_BASE_URL, "nacos", "nacos").await?;

// Make API calls
let response: NacosResponse<Value> = client
    .post_form("/nacos/v2/cs/config", &params)
    .await?;
assert!(response.is_success());
```

### Test Fixtures

Use fixtures to create test data with unique IDs:

```rust
use common::fixtures::*;

// Config fixture with builder pattern
let config = ConfigFixture::new()
    .yaml()
    .with_tenant("production");
let params = config.to_v2_publish_params();

// Instance fixture
let instance = InstanceFixture::new()
    .with_ip("10.0.0.1")
    .with_port(9090)
    .persistent();

// Namespace fixture
let ns = NamespaceFixture::new()
    .with_name("Production");
```

### Database Testing

For persistence tests requiring a database:

```rust
// Set TEST_DATABASE_URL environment variable
// MySQL: mysql://user:pass@localhost:3306/batata_test
// PostgreSQL: postgres://user:pass@localhost:5432/batata_test

#[tokio::test]
#[ignore = "requires test database"]
async fn test_config_crud() {
    skip_if_no_db!();
    let db = TestDatabase::connect().await.unwrap();
    // ... test code
}
```

## Naming Conventions

| Type | Pattern | Example |
|------|---------|---------|
| Unit test | `test_<function>_<scenario>` | `test_encode_jwt_token_success` |
| Integration test file | `<module>_<feature>_test.rs` | `config_api_test.rs` |
| Benchmark file | `<module>_bench.rs` | `auth_bench.rs` |
| Fixture | `<Entity>Fixture` | `ConfigFixture` |
| Test helper | `<action>_<entity>` | `unique_data_id()` |

## Adding New Tests

### Adding Unit Tests

1. Open the relevant source file
2. Find or create the `#[cfg(test)] mod tests` block at the end
3. Add test functions with `#[test]` or `#[tokio::test]`
4. Use descriptive test names: `test_<what>_<scenario>`

### Adding Integration Tests

1. Create a new file in `crates/batata-server/tests/http_api/` or `grpc_api/`
2. Import common utilities: `mod common;`
3. Use `TestClient` for HTTP or direct gRPC calls
4. Mark server-dependent tests with `#[ignore = "requires running server"]`

### Adding Benchmarks

1. Create a new file in `crates/batata-server/benches/`
2. Use Criterion framework
3. Add to `Cargo.toml` `[[bench]]` section
4. Run with `cargo bench`

## CI Pipeline

Tests are organized for CI:

```yaml
# Fast checks (< 2 min)
cargo fmt --all -- --check
cargo clippy --workspace
cargo test --workspace --lib

# Integration tests (require server)
cargo test --workspace --test '*'

# Benchmarks (on PR, compare with main)
cargo bench --workspace
```
