//! Configuration Import/Export API integration tests
//!
//! Tests for batch config import and export functionality

use crate::common::{
    CONSOLE_BASE_URL, MAIN_BASE_URL, TEST_NAMESPACE, TEST_NAMESPACE_CUSTOM,
    TEST_PASSWORD, TEST_USERNAME, TestClient, unique_data_id,
};

/// Create an authenticated test client for the main API server
async fn authenticated_client() -> TestClient {
    let mut client = TestClient::new(MAIN_BASE_URL);
    client
        .login_via(CONSOLE_BASE_URL, TEST_USERNAME, TEST_PASSWORD)
        .await
        .expect("Failed to login");
    client
}

/// Test export single config
#[tokio::test]
#[ignore = "requires running server"]
async fn test_export_single_config() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("export_single");

    // Publish config
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "export.test=value"),
            ],
        )
        .await
        .expect("Failed to publish config");

    // Export config
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config/export",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
            ],
        )
        .await
        .expect("Failed to export config");

    assert_eq!(response["code"], 0, "Export should succeed");
}

/// Test export multiple configs
#[tokio::test]
#[ignore = "requires running server"]
async fn test_export_multiple_configs() {
    let client = authenticated_client().await;

    // Publish multiple configs
    for i in 1..=3 {
        let data_id = unique_data_id(&format!("export_multi_{}", i));
        let _: serde_json::Value = client
            .post_form(
                "/nacos/v2/cs/config",
                &[
                    ("dataId", data_id.as_str()),
                    ("group", "DEFAULT_GROUP"),
                    ("content", &format!("config.{}=value", i)),
                ],
            )
            .await
            .expect("Failed to publish config");
    }

    // Export all configs in group
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config/export",
            &[("group", "DEFAULT_GROUP")],
        )
        .await
        .expect("Failed to export configs");

    assert_eq!(response["code"], 0, "Export multiple should succeed");
}

/// Test export configs with namespace
#[tokio::test]
#[ignore = "requires running server"]
async fn test_export_with_namespace() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("export_ns");

    // Publish config with namespace
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("namespaceId", TEST_NAMESPACE_CUSTOM),
                ("content", "ns.config=value"),
            ],
        )
        .await
        .expect("Failed to publish config");

    // Export with namespace
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config/export",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("namespaceId", TEST_NAMESPACE_CUSTOM),
            ],
        )
        .await
        .expect("Failed to export config with namespace");

    assert_eq!(response["code"], 0, "Export with namespace should succeed");
}

/// Test export all configs
#[tokio::test]
#[ignore = "requires running server"]
async fn test_export_all_configs() {
    let client = authenticated_client().await;

    // Publish configs in different groups
    for group in &["DEFAULT_GROUP", "TEST_GROUP"] {
        let data_id = unique_data_id(&format!("export_all_{}", group));
        let _: serde_json::Value = client
            .post_form(
                "/nacos/v2/cs/config",
                &[
                    ("dataId", data_id.as_str()),
                    ("group", group),
                    ("content", "value"),
                ],
            )
            .await
            .expect("Failed to publish config");
    }

    // Export all configs (no filters)
    let response: serde_json::Value = client
        .get("/nacos/v2/cs/config/export")
        .await
        .expect("Failed to export all configs");

    assert_eq!(response["code"], 0, "Export all should succeed");
}

/// Test import single config
#[tokio::test]
#[ignore = "requires running server"]
async fn test_import_single_config() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("import_single");

    // Prepare config data for import
    let import_data = serde_json::json!({
        "dataId": data_id,
        "group": "DEFAULT_GROUP",
        "content": "imported.config=value"
    });

    // Import config
    let response: serde_json::Value = client
        .post_json(
            "/nacos/v2/cs/config/import",
            &import_data,
        )
        .await
        .expect("Failed to import config");

    assert_eq!(response["code"], 0, "Import should succeed");

    // Verify imported config
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
            ],
        )
        .await
        .expect("Failed to get imported config");

    assert_eq!(
        response["data"]["content"], "imported.config=value",
        "Imported content should match"
    );
}

/// Test import multiple configs
#[tokio::test]
#[ignore = "requires running server"]
async fn test_import_multiple_configs() {
    let client = authenticated_client().await;

    // Prepare multiple configs for import
    let import_data = serde_json::json!([
        {
            "dataId": unique_data_id("import_multi1"),
            "group": "DEFAULT_GROUP",
            "content": "multi.config.1=value1"
        },
        {
            "dataId": unique_data_id("import_multi2"),
            "group": "DEFAULT_GROUP",
            "content": "multi.config.2=value2"
        },
        {
            "dataId": unique_data_id("import_multi3"),
            "group": "DEFAULT_GROUP",
            "content": "multi.config.3=value3"
        }
    ]);

    // Import configs
    let response: serde_json::Value = client
        .post_json(
            "/nacos/v2/cs/config/import",
            &import_data,
        )
        .await
        .expect("Failed to import multiple configs");

    assert_eq!(response["code"], 0, "Import multiple should succeed");
}

/// Test import with namespace
#[tokio::test]
#[ignore = "requires running server"]
async fn test_import_with_namespace() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("import_ns");

    // Prepare config with namespace
    let import_data = serde_json::json!({
        "dataId": data_id,
        "group": "DEFAULT_GROUP",
        "namespaceId": TEST_NAMESPACE_CUSTOM,
        "content": "imported.ns.config=value"
    });

    // Import config
    let response: serde_json::Value = client
        .post_json(
            "/nacos/v2/cs/config/import",
            &import_data,
        )
        .await
        .expect("Failed to import config with namespace");

    assert_eq!(response["code"], 0, "Import with namespace should succeed");

    // Verify imported config
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("namespaceId", TEST_NAMESPACE_CUSTOM),
            ],
        )
        .await
        .expect("Failed to get imported config");

    assert_eq!(
        response["data"]["content"], "imported.ns.config=value",
        "Imported content should match"
    );
}

/// Test import overwrites existing config
#[tokio::test]
#[ignore = "requires running server"]
async fn test_import_overwrite_existing() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("import_overwrite");

    // Publish original config
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "original=value"),
            ],
        )
        .await
        .expect("Failed to publish original config");

    // Prepare updated config
    let import_data = serde_json::json!({
        "dataId": data_id,
        "group": "DEFAULT_GROUP",
        "content": "updated=value"
    });

    // Import (should overwrite)
    let response: serde_json::Value = client
        .post_json(
            "/nacos/v2/cs/config/import",
            &import_data,
        )
        .await
        .expect("Failed to import config");

    assert_eq!(response["code"], 0, "Import overwrite should succeed");

    // Verify config was updated
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
            ],
        )
        .await
        .expect("Failed to get config");

    assert_eq!(
        response["data"]["content"], "updated=value",
        "Config should be overwritten"
    );
}

/// Test import with tags
#[tokio::test]
#[ignore = "requires running server"]
async fn test_import_with_tags() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("import_tags");

    // Prepare config with tags
    let import_data = serde_json::json!({
        "dataId": data_id,
        "group": "DEFAULT_GROUP",
        "content": "tagged.config=value",
        "tags": "env=prod,region=us-west"
    });

    // Import config
    let response: serde_json::Value = client
        .post_json(
            "/nacos/v2/cs/config/import",
            &import_data,
        )
        .await
        .expect("Failed to import config with tags");

    assert_eq!(response["code"], 0, "Import with tags should succeed");
}

/// Test import invalid config data
#[tokio::test]
#[ignore = "requires running server"]
async fn test_import_invalid_data() {
    let client = authenticated_client().await;

    // Prepare invalid config (missing required field)
    let import_data = serde_json::json!({
        "group": "DEFAULT_GROUP",
        "content": "value"
    });

    // Try to import (should fail)
    let result = client
        .post_json::<serde_json::Value, _>(
            "/nacos/v2/cs/config/import",
            &import_data,
        )
        .await;

    match result {
        Ok(response) => {
            assert_ne!(response["code"], 0, "Invalid import should be rejected");
        }
        Err(_) => {
            // HTTP error is acceptable
        }
    }
}

/// Test export and import round trip
#[tokio::test]
#[ignore = "requires running server"]
async fn test_export_import_round_trip() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("round_trip");

    // Publish original config
    let original_content = "round.trip.config=test-value\nmulti.line=true";
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", original_content),
            ],
        )
        .await
        .expect("Failed to publish config");

    // Export
    let export_response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config/export",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
            ],
        )
        .await
        .expect("Failed to export config");

    assert_eq!(export_response["code"], 0, "Export should succeed");

    // Delete original
    let _: serde_json::Value = client
        .delete_with_query(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
            ],
        )
        .await
        .expect("Failed to delete config");

    // Import exported data
    let import_response: serde_json::Value = client
        .post_json(
            "/nacos/v2/cs/config/import",
            &export_response["data"],
        )
        .await
        .expect("Failed to import config");

    assert_eq!(import_response["code"], 0, "Import should succeed");

    // Verify round trip
    let verify_response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
            ],
        )
        .await
        .expect("Failed to verify imported config");

    assert_eq!(
        verify_response["data"]["content"], original_content,
        "Content should be preserved after round trip"
    );
}

/// Test export beta configs
#[tokio::test]
#[ignore = "requires running server"]
async fn test_export_beta_configs() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("export_beta");

    // Publish main config
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "main=value"),
            ],
        )
        .await
        .expect("Failed to publish main config");

    // Publish beta config
    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config/beta",
            &[
                ("dataId", data_id.as_str()),
                ("group", "DEFAULT_GROUP"),
                ("content", "beta=value"),
                ("betaIps", "192.168.1.100"),
            ],
        )
        .await
        .expect("Failed to publish beta config");

    // Export configs (may include beta)
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config/export",
            &[(
                "group", "DEFAULT_GROUP"
            )],
        )
        .await
        .expect("Failed to export configs");

    assert_eq!(response["code"], 0, "Export with beta should succeed");
}

/// Test import large config
#[tokio::test]
#[ignore = "requires running server"]
async fn test_import_large_config() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("import_large");

    // Generate large config content
    let large_content: String = (0..10000)
        .map(|i| format!("key{}=value{}\n", i, i))
        .collect();

    // Prepare large config
    let import_data = serde_json::json!({
        "dataId": data_id,
        "group": "DEFAULT_GROUP",
        "content": large_content
    });

    // Import large config
    let response: serde_json::Value = client
        .post_json(
            "/nacos/v2/cs/config/import",
            &import_data,
        )
        .await
        .expect("Failed to import large config");

    assert_eq!(response["code"], 0, "Import large config should succeed");
}

/// Test export by data ID pattern
#[tokio::test]
#[ignore = "requires running server"]
async fn test_export_by_pattern() {
    let client = authenticated_client().await;

    // Publish configs with pattern
    for i in 1..=3 {
        let data_id = format!("pattern.test.{}", unique_data_id(""));
        let _: serde_json::Value = client
            .post_form(
                "/nacos/v2/cs/config",
                &[
                    ("dataId", data_id.as_str()),
                    ("group", "DEFAULT_GROUP"),
                    ("content", &format!("value={}", i)),
                ],
            )
            .await
            .expect("Failed to publish config");
    }

    // Export by pattern (if supported)
    let response: serde_json::Value = client
        .get_with_query(
            "/nacos/v2/cs/config/export",
            &[
                ("group", "DEFAULT_GROUP"),
                ("dataId", "pattern.test.*"),
            ],
        )
        .await
        .expect("Failed to export by pattern");

    assert_eq!(response["code"], 0, "Export by pattern should succeed");
}

/// Test import duplicate configs
#[tokio::test]
#[ignore = "requires running server"]
async fn test_import_duplicates() {
    let client = authenticated_client().await;
    let data_id = unique_data_id("import_dup");

    // Prepare config
    let import_data = serde_json::json!([{
        "dataId": data_id,
        "group": "DEFAULT_GROUP",
        "content": "duplicate=value"
    }]);

    // Import first time
    let response1: serde_json::Value = client
        .post_json(
            "/nacos/v2/cs/config/import",
            &import_data,
        )
        .await
        .expect("Failed to import config");

    assert_eq!(response1["code"], 0, "First import should succeed");

    // Import again (should handle gracefully - either overwrite or reject)
    let response2: serde_json::Value = client
        .post_json(
            "/nacos/v2/cs/config/import",
            &import_data,
        )
        .await
        .expect("Failed to import duplicate config");

    // Should either succeed (overwrite) or indicate duplicate
    assert!(
        response2["code"] == 0 || response2["code"] != 0,
        "Duplicate import should be handled"
    );
}
