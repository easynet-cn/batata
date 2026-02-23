//! Integration tests for Batata server
//!
//! This file serves as the entry point for integration tests.
//! It imports the common test utilities and defines test modules.

mod common;

// Re-export common utilities for use in other test files
pub use common::*;

// HTTP API Tests
mod http_api;

// gRPC API Tests
mod grpc_api;

// Database persistence tests
mod persistence;

// Compatibility layer tests
mod compatibility;

// Special characters and boundary conditions tests
mod special_chars_and_boundary_test;

// Performance and load tests
mod performance_and_load_test;

#[cfg(test)]
mod tests {
    use super::common::*;

    #[test]
    fn test_unique_id_generation() {
        let id1 = unique_test_id();
        let id2 = unique_test_id();
        assert_ne!(id1, id2);
        assert!(id1.starts_with("test_"));
    }

    #[test]
    fn test_unique_data_id() {
        let id = unique_data_id("config");
        assert!(id.starts_with("config_test_"));
    }

    #[test]
    fn test_unique_service_name() {
        let name = unique_service_name("service");
        assert!(name.starts_with("service_test_"));
    }
}
