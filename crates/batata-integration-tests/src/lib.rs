//! Batata integration test utilities
//!
//! Provides shared test infrastructure for integration testing:
//! - TestClient: HTTP client for API testing
//! - TestDatabase: Database connection management
//! - Fixtures: Test data builders
//! - Server: Test server management

#![warn(missing_docs)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]
#![allow(clippy::unnecessary_sort_by)]
#![allow(clippy::assertions_on_constants)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::result_large_err)]
#![allow(clippy::empty_line_after_doc_comments)]

/// HTTP test client utilities.
#[allow(dead_code, unused_imports)]
pub mod client;
/// Database test utilities.
#[allow(dead_code, unused_imports)]
pub mod db;
/// Test data fixture builders.
#[allow(dead_code, unused_imports)]
pub mod fixtures;
/// Test server management utilities.
#[allow(dead_code, unused_imports)]
pub mod server;

#[allow(unused_imports)]
pub use fixtures::{AuthFixture, ConfigFixture, InstanceFixture, NamespaceFixture};

pub use client::TestClient;
#[allow(unused_imports)]
pub use db::TestDatabase;

/// Default test credentials.
pub const TEST_USERNAME: &str = "nacos";
/// Default test password for authentication.
pub const TEST_PASSWORD: &str = "nacos";

/// Server URLs
/// Main HTTP server for API endpoints (/nacos/v2/*, /nacos/v3/admin/*, /nacos/v3/client/*)
pub const MAIN_BASE_URL: &str = "http://127.0.0.1:8848";
/// Console HTTP server for auth and management endpoints (/v3/auth/*)
pub const CONSOLE_BASE_URL: &str = "http://127.0.0.1:8081";

/// Test namespaces.
#[allow(dead_code)]
pub const TEST_NAMESPACE: &str = "public";
/// Custom test namespace identifier.
#[allow(dead_code)]
pub const TEST_NAMESPACE_CUSTOM: &str = "test-namespace";

/// Test groups.
#[allow(dead_code)]
pub const DEFAULT_GROUP: &str = "DEFAULT_GROUP";
/// Custom test group identifier.
#[allow(dead_code)]
pub const TEST_GROUP: &str = "TEST_GROUP";

/// Generate a unique test ID to avoid conflicts between tests
/// Monotonic sequence combined with the timestamp.
///
/// A plain timestamp is not sufficient: clock resolution on some platforms is
/// coarse enough that two calls in quick succession can return the same value.
static UNIQUE_ID_SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Generate a unique test ID combining a nanosecond timestamp with a monotonic
/// sequence counter, so fast successive calls never collide.
pub fn unique_test_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let seq = UNIQUE_ID_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("test_{}_{}", timestamp, seq)
}

/// Generate a unique data ID for config tests
pub fn unique_data_id(prefix: &str) -> String {
    format!("{}_{}", prefix, unique_test_id())
}

/// Generate a unique service name for naming tests
#[allow(dead_code)]
pub fn unique_service_name(prefix: &str) -> String {
    format!("{}_{}", prefix, unique_test_id())
}
