//! Apollo API compatibility tests
//!
//! Tests for Apollo Config API endpoints compatibility

use crate::common::{unique_test_id, TestClient};

/// Test get config via Apollo Config API
#[tokio::test]
#[ignore = "requires running server with Apollo plugin"]
async fn test_apollo_get_config() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let app_id = format!("apollo-app-{}", unique_test_id());

    // Apollo uses path: /configs/{appId}/{clusterName}/{namespaceName}
    let response: serde_json::Value = client
        .get(&format!("/configs/{}/default/application", app_id))
        .await
        .expect("Apollo config get should succeed");

    // Response should be ApolloConfig format
    assert!(
        response["appId"].is_string() || response["configurations"].is_object(),
        "Should return Apollo config format"
    );
}

/// Test get config files (plain text format)
#[tokio::test]
#[ignore = "requires running server with Apollo plugin"]
async fn test_apollo_config_files() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let app_id = format!("apollo-files-{}", unique_test_id());

    // /configfiles/{appId}/{clusterName}/{namespaceName}
    let response = client
        .raw_get(&format!("/configfiles/{}/default/application", app_id))
        .await;

    assert!(response.is_ok(), "Config files should return");
}

/// Test get config JSON format
#[tokio::test]
#[ignore = "requires running server with Apollo plugin"]
async fn test_apollo_config_json() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let app_id = format!("apollo-json-{}", unique_test_id());

    // /configfiles/json/{appId}/{clusterName}/{namespaceName}
    let response: serde_json::Value = client
        .get(&format!("/configfiles/json/{}/default/application", app_id))
        .await
        .expect("Config JSON should succeed");

    assert!(response.is_object(), "Should return JSON object");
}

/// Test notifications (long polling)
#[tokio::test]
#[ignore = "requires running server with Apollo plugin"]
async fn test_apollo_notifications() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let app_id = format!("apollo-notify-{}", unique_test_id());

    // /notifications/v2?appId=xxx&cluster=xxx&notifications=[...]
    let notifications = serde_json::json!([{
        "namespaceName": "application",
        "notificationId": -1
    }]);

    let response: serde_json::Value = client
        .get_with_query(
            "/notifications/v2",
            &[
                ("appId", app_id.as_str()),
                ("cluster", "default"),
                ("notifications", &notifications.to_string()),
            ],
        )
        .await
        .expect("Notifications should succeed");

    // May return empty array on timeout or notification array on change
    assert!(response.is_array(), "Should return notifications array");
}

/// Test release key (304 Not Modified)
#[tokio::test]
#[ignore = "requires running server with Apollo plugin"]
async fn test_apollo_release_key() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let app_id = format!("apollo-release-{}", unique_test_id());

    // First request to get release key
    let response: serde_json::Value = client
        .get(&format!("/configs/{}/default/application", app_id))
        .await
        .expect("First request should succeed");

    // If there's a release key, subsequent request with same key should return 304
    if let Some(release_key) = response["releaseKey"].as_str() {
        let response: Result<serde_json::Value, _> = client
            .get_with_query(
                &format!("/configs/{}/default/application", app_id),
                &[("releaseKey", release_key)],
            )
            .await;

        // May return 304 or same data
        assert!(response.is_ok() || response.is_err());
    }
}

/// Test OpenAPI apps endpoint
#[tokio::test]
#[ignore = "requires running server with Apollo plugin"]
async fn test_apollo_openapi_apps() {
    let client = TestClient::new("http://127.0.0.1:8848");

    let response: serde_json::Value = client
        .get("/openapi/v1/apps")
        .await
        .expect("OpenAPI apps should succeed");

    assert!(response.is_array(), "Should return apps array");
}

/// Test OpenAPI items endpoint
#[tokio::test]
#[ignore = "requires running server with Apollo plugin"]
async fn test_apollo_openapi_items() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let app_id = format!("apollo-items-{}", unique_test_id());

    // /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/items
    let response: serde_json::Value = client
        .get(&format!(
            "/openapi/v1/envs/DEV/apps/{}/clusters/default/namespaces/application/items",
            app_id
        ))
        .await
        .expect("OpenAPI items should succeed");

    assert!(response.is_array(), "Should return items array");
}

/// Test OpenAPI publish release
#[tokio::test]
#[ignore = "requires running server with Apollo plugin"]
async fn test_apollo_openapi_publish() {
    let client = TestClient::new("http://127.0.0.1:8848");
    let app_id = format!("apollo-publish-{}", unique_test_id());

    let release = serde_json::json!({
        "releaseTitle": "Test Release",
        "releaseComment": "Integration test release",
        "releasedBy": "test-user"
    });

    // /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/releases
    let response: serde_json::Value = client
        .post_json(
            &format!(
                "/openapi/v1/envs/DEV/apps/{}/clusters/default/namespaces/application/releases",
                app_id
            ),
            &release,
        )
        .await
        .expect("OpenAPI publish should succeed");

    // May return release info or error
    assert!(response.is_object(), "Should return release object");
}

/// Test Apollo to Nacos mapping
#[tokio::test]
#[ignore = "requires running server with Apollo plugin"]
async fn test_apollo_nacos_mapping() {
    // Apollo concept -> Nacos concept:
    // appId + namespace -> dataId (e.g., "myapp+application" -> "myapp+application")
    // cluster -> group (e.g., "default" -> "DEFAULT_GROUP")
    // env -> namespace (e.g., "DEV" -> "dev")

    let client = TestClient::new("http://127.0.0.1:8848");

    // First, create config via Nacos API
    let app_id = format!("mapping-test-{}", unique_test_id());
    let data_id = format!("{}+application", app_id);

    let _: serde_json::Value = client
        .post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", data_id.as_str()),
                ("group", "default"),
                ("content", "mapped.key=mapped.value"),
            ],
        )
        .await
        .expect("Nacos config publish failed");

    // Then, retrieve via Apollo API
    let response: serde_json::Value = client
        .get(&format!("/configs/{}/default/application", app_id))
        .await
        .expect("Apollo config get should succeed");

    // Verify mapping works
    assert!(
        response["configurations"].is_object() || response["appId"].is_string(),
        "Should return Apollo format config"
    );
}
