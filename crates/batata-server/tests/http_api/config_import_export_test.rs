//! Config Import/Export API integration tests
//!
//! Tests for batch configuration import and export

use crate::common::{
    CONSOLE_BASE_URL, DEFAULT_GROUP, MAIN_BASE_URL, TEST_PASSWORD, TEST_USERNAME, TestClient,
    unique_data_id,
};

/// Create an authenticated test client
async fn authenticated_client() -> TestClient {
    let mut client = TestClient::new(MAIN_BASE_URL);
    client
        .login_via(CONSOLE_BASE_URL, TEST_USERNAME, TEST_PASSWORD)
        .await
        .expect("Failed to login");
    client
}

/// Test export single configuration
#[tokio::test]
#[ignore = "feature not implemented yet"]

async fn test_export_single_config() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("export_single");

    // Publish config
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("content", "export.test=1"),
            ],
        )
        .await
        .expect("Failed to publish config");

    // Export config
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v3/admin/cs/config/export",
            &[("dataId", data_id.as_str()), ("group", DEFAULT_GROUP)],
        )
        .await
        .expect("Failed to export config");

    assert_eq!(response["code"], 0, "Export should succeed");
    assert!(response["data"].is_object() || response["data"].is_string());
}

/// Test export multiple configurations
#[tokio::test]
#[ignore = "feature not implemented yet"]

async fn test_export_multiple_configs() {
    let client = authenticated_client().await;
    let data_id1 = unique_data_id("export_multi1");
    let data_id2 = unique_data_id("export_multi2");

    // Publish multiple configs
    for data_id in [&data_id1, &data_id2] {
        let _: serde_json::Value = client
            .post_form(
                "/nacos/v2/cs/config",
                &[
                    ("dataId", data_id.as_str()),
                    ("group", DEFAULT_GROUP),
                    ("content", &format!("{}.test=1", data_id)),
                ],
            )
            .await
            .expect("Failed to publish config");
    }

    // Export configs
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v3/admin/cs/config/export",
            &[("dataId", data_id1.as_str()), ("group", DEFAULT_GROUP)],
        )
        .await
        .expect("Failed to export configs");

    assert_eq!(response["code"], 0, "Export should succeed");
}

/// Test import configuration
#[tokio::test]
#[ignore = "feature not implemented yet"]

async fn test_import_config() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("import_single");

    // Import config (simulate using POST with import data)
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("content", "imported.value=1"),
            ],
        )
        .await
        .expect("Failed to import config");

    // Verify imported config
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config",
            &[("dataId", data_id.as_str()), ("group", DEFAULT_GROUP)],
        )
        .await
        .expect("Failed to get config");

    assert_eq!(response["code"], 0, "Get should succeed");
    assert!(
        response["data"]
            .as_str()
            .unwrap()
            .contains("imported.value=1")
    );
}

/// Test import with namespace
#[tokio::test]
#[ignore = "feature not implemented yet"]

async fn test_import_config_with_namespace() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("import_namespace");
    let namespace = "test-import-ns";

    // Import config with namespace
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("tenant", namespace),
                ("content", "namespace.import=1"),
            ],
        )
        .await
        .expect("Failed to import config");

    // Verify imported config
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("tenant", namespace),
            ],
        )
        .await
        .expect("Failed to get config");

    assert_eq!(response["code"], 0, "Get should succeed");
    assert!(
        response["data"]
            .as_str()
            .unwrap()
            .contains("namespace.import=1")
    );
}

/// Test export with namespace
#[tokio::test]
#[ignore = "feature not implemented yet"]

async fn test_export_config_with_namespace() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("export_namespace");
    let namespace = "test-export-ns";

    // Publish config with namespace
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("tenant", namespace),
                ("content", "namespace.export=1"),
            ],
        )
        .await
        .expect("Failed to publish config");

    // Export config with namespace
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v3/admin/cs/config/export",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("tenant", namespace),
            ],
        )
        .await
        .expect("Failed to export config");

    assert_eq!(response["code"], 0, "Export should succeed");
}

/// Test batch import
#[tokio::test]
#[ignore = "feature not implemented yet"]

async fn test_batch_import() {
    let client = authenticated_client().await;

    // Import multiple configs
    let configs = [
        ("batch_import1", "value1"),
        ("batch_import2", "value2"),
        ("batch_import3", "value3"),
    ];

    for (data_id_prefix, value) in configs.iter() {
        let data_id = format!("{}_{}", data_id_prefix, unique_test_timestamp());
        let _: serde_json::Value = client
            .post_form(
                "/nacos/v2/cs/config",
                &[
                    ("dataId", data_id.as_str()),
                    ("group", DEFAULT_GROUP),
                    ("content", value),
                ],
            )
            .await
            .expect("Failed to import config");
    }
}

/// Test round-trip export/import
#[tokio::test]
#[ignore = "feature not implemented yet"]

async fn test_export_import_roundtrip() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("roundtrip");

    // Publish config
    let original_content = "roundtrip.test=original";
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("content", original_content),
            ],
        )
        .await
        .expect("Failed to publish config");

    // Export config
    let export_response: serde_json::Value = client
        .get_with_query(
            "/nacos/v3/admin/cs/config/export",
            &[("dataId", data_id.as_str()), ("group", DEFAULT_GROUP)],
        )
        .await
        .expect("Failed to export config");

    assert_eq!(export_response["code"], 0, "Export should succeed");

    // Delete config
    let _: serde_json::Value = client
        .delete_with_query(
            "/nacos/v2/cs/config",
            &[("dataId", data_id.as_str()), ("group", DEFAULT_GROUP)],
        )
        .await
        .expect("Failed to delete config");

    // Re-import config (simulate from export data)
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", DEFAULT_GROUP),
                ("content", original_content),
            ],
        )
        .await
        .expect("Failed to import config");

    // Verify round-trip
    let get_response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config",
            &[("dataId", data_id.as_str()), ("group", DEFAULT_GROUP)],
        )
        .await
        .expect("Failed to get config");

    assert_eq!(get_response["code"], 0, "Get should succeed");
    assert!(
        get_response["data"]
            .as_str()
            .unwrap()
            .contains(original_content)
    );
}

/// Helper function to generate unique timestamp
fn unique_test_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    format!("{}", timestamp)
}
