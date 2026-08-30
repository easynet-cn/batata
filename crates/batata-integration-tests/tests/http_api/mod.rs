//! HTTP API integration tests
//!
//! Tests for Nacos V2/V3 HTTP APIs

/// Tests for the authentication API.
pub mod auth_api_test;
/// Tests for the configuration API.
pub mod config_api_test;
/// Tests for the configuration beta API.
pub mod config_beta_api_test;
/// Tests for the configuration encryption API.
pub mod config_encryption_test;
/// Tests for the configuration import/export API.
pub mod config_import_export_test;
/// Tests for the configuration listener (long-polling) API.
pub mod config_listener_test;
/// Tests for the configuration long-polling API.
pub mod config_longpoll_test;
/// Tests for the console management API.
pub mod console_api_test;
/// Tests for error paths and edge cases.
pub mod error_path_test;
/// Tests for the namespace API.
pub mod namespace_api_test;
/// Tests for the naming (service discovery) API.
pub mod naming_api_test;
/// Tests for the naming cluster API.
pub mod naming_cluster_test;
/// Tests for the naming health-check API.
pub mod naming_healthcheck_test;
/// Tests for the naming heartbeat API.
pub mod naming_heartbeat_test;
/// Tests for route completeness coverage.
pub mod route_completeness_test;
/// Tests for server separation (multi-port) behavior.
pub mod server_separation_test;
/// Tests for additional V2 API endpoints.
pub mod v2_additional_api_test;
/// Tests for the V3 admin AI API.
pub mod v3_admin_ai_api_test;
/// Tests for the V3 admin config API.
pub mod v3_admin_config_api_test;
/// Tests for the V3 admin core API.
pub mod v3_admin_core_api_test;
/// Tests for the V3 admin naming API.
pub mod v3_admin_naming_api_test;
/// Tests for the V3 client API.
pub mod v3_client_api_test;
