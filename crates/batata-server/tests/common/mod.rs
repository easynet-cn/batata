//! Common test utilities for integration testing
//!
//! This module provides shared test infrastructure including:
//! - TestServer: Start and manage a Batata server instance for testing
//! - TestClient: HTTP client for API testing
//! - TestDatabase: Database connection management for persistence tests

pub mod client;
pub mod db;
pub mod server;

pub use client::TestClient;
pub use db::TestDatabase;
pub use server::TestServer;

/// Default test credentials
pub const TEST_USERNAME: &str = "nacos";
pub const TEST_PASSWORD: &str = "nacos";

/// Server URLs
/// Main HTTP server for API endpoints (/nacos/v2/*, /nacos/v3/admin/*, /nacos/v3/client/*)
pub const MAIN_BASE_URL: &str = "http://127.0.0.1:8848";
/// Console HTTP server for auth and management endpoints (/v3/auth/*)
pub const CONSOLE_BASE_URL: &str = "http://127.0.0.1:8081";

/// Test namespaces
pub const TEST_NAMESPACE: &str = "public";
pub const TEST_NAMESPACE_CUSTOM: &str = "test-namespace";

/// Test groups
pub const DEFAULT_GROUP: &str = "DEFAULT_GROUP";
pub const TEST_GROUP: &str = "TEST_GROUP";

/// Generate a unique test ID to avoid conflicts between tests
pub fn unique_test_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("test_{}", timestamp)
}

/// Generate a unique data ID for config tests
pub fn unique_data_id(prefix: &str) -> String {
    format!("{}_{}", prefix, unique_test_id())
}

/// Generate a unique service name for naming tests
pub fn unique_service_name(prefix: &str) -> String {
    format!("{}_{}", prefix, unique_test_id())
}
