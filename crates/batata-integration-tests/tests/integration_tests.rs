//! Integration tests for Batata server
//!
//! This file serves as entry point for integration tests.
//! It imports common test utilities from the batata-integration-tests crate.

// HTTP API Tests
mod http_api;

// gRPC API Tests
mod grpc_api;

// Compatibility layer tests
mod compatibility;

#[cfg(test)]
mod tests {
    use batata_integration_tests::*;
    use std::collections::HashSet;

    #[test]
    fn test_unique_id_generation() {
        let count = 100;
        let ids: HashSet<String> = (0..count).map(|_| unique_test_id()).collect();
        assert_eq!(
            ids.len(),
            count,
            "All {} generated IDs should be unique, but only {} were",
            count,
            ids.len()
        );
        for id in &ids {
            assert!(
                id.starts_with("test_"),
                "ID '{}' should start with 'test_'",
                id
            );
        }
    }

    #[test]
    fn test_unique_data_id() {
        let count = 100;
        let ids: HashSet<String> = (0..count).map(|_| unique_data_id("config")).collect();
        assert_eq!(
            ids.len(),
            count,
            "All {} generated data IDs should be unique, but only {} were",
            count,
            ids.len()
        );
        for id in &ids {
            assert!(
                id.starts_with("config_test_"),
                "Data ID '{}' should start with 'config_test_'",
                id
            );
        }
    }

    #[test]
    fn test_unique_service_name() {
        let count = 100;
        let names: HashSet<String> = (0..count).map(|_| unique_service_name("service")).collect();
        assert_eq!(
            names.len(),
            count,
            "All {} generated service names should be unique, but only {} were",
            count,
            names.len()
        );
        for name in &names {
            assert!(
                name.starts_with("service_test_"),
                "Service name '{}' should start with 'service_test_'",
                name
            );
        }
    }
}
