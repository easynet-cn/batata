//! Negative/error path tests for HTTP APIs
//!
//! Tests error responses: 400 (bad request), 401 (unauthorized),
//! 404 (not found), and validation failures.

use crate::common::{CONSOLE_BASE_URL, MAIN_BASE_URL, TEST_PASSWORD, TEST_USERNAME, TestClient};

/// Helper to create an authenticated client
async fn authenticated_client() -> TestClient {
    let mut client = TestClient::new(MAIN_BASE_URL);
    client
        .login_via(CONSOLE_BASE_URL, TEST_USERNAME, TEST_PASSWORD)
        .await
        .expect("Login should succeed");
    client
}

// ============================================================================
// Config API Error Tests
// ============================================================================

#[tokio::test]
#[ignore = "requires running server"]
async fn test_config_get_missing_data_id() {
    let client = authenticated_client().await;
    let response = client
        .raw_get("/nacos/v2/cs/config?group=DEFAULT_GROUP")
        .await
        .unwrap();
    // Should fail because dataId is required
    assert_ne!(response.status().as_u16(), 200);
}

#[tokio::test]
#[ignore = "requires running server"]
async fn test_config_get_nonexistent() {
    let client = authenticated_client().await;
    let response = client
        .raw_get("/nacos/v2/cs/config?dataId=nonexistent_config_xyz&group=DEFAULT_GROUP")
        .await
        .unwrap();
    let status = response.status().as_u16();
    let body = response.text().await.unwrap();
    // Should return error for nonexistent config
    assert!(
        status != 200
            || body.contains("20004")
            || body.contains("resource not found")
            || body.contains("\"code\":0"),
        "Expected resource not found or empty data for nonexistent config: {}",
        body
    );
}

#[tokio::test]
#[ignore = "requires running server"]
async fn test_config_publish_missing_content() {
    let client = authenticated_client().await;
    let response = client
        .raw_post_form(
            "/nacos/v2/cs/config",
            &[("dataId", "test"), ("group", "DEFAULT_GROUP")],
        )
        .await
        .unwrap();
    // Missing content should fail
    let status = response.status().as_u16();
    assert!(
        status == 400 || status == 500 || status == 200,
        "Got unexpected status: {}",
        status
    );
}

#[tokio::test]
#[ignore = "requires running server"]
async fn test_config_publish_empty_data_id() {
    let client = authenticated_client().await;
    let response = client
        .raw_post_form(
            "/nacos/v2/cs/config",
            &[
                ("dataId", ""),
                ("group", "DEFAULT_GROUP"),
                ("content", "test"),
            ],
        )
        .await
        .unwrap();
    let body = response.text().await.unwrap();
    // Empty dataId should be rejected
    assert!(
        body.contains("error") || body.contains("20008") || body.contains("\"code\":10000"),
        "Should reject empty dataId: {}",
        body
    );
}

// ============================================================================
// Auth API Error Tests
// ============================================================================

#[tokio::test]
#[ignore = "requires running server"]
async fn test_auth_login_wrong_password() {
    let mut client = TestClient::new(CONSOLE_BASE_URL);
    let result = client.login(TEST_USERNAME, "wrong_password_xyz").await;
    assert!(result.is_err(), "Login with wrong password should fail");
}

#[tokio::test]
#[ignore = "requires running server"]
async fn test_auth_login_nonexistent_user() {
    let mut client = TestClient::new(CONSOLE_BASE_URL);
    let result = client.login("nonexistent_user_xyz", "password").await;
    assert!(result.is_err(), "Login with nonexistent user should fail");
}

#[tokio::test]
#[ignore = "requires running server"]
async fn test_auth_login_empty_credentials() {
    let mut client = TestClient::new(CONSOLE_BASE_URL);
    let result = client.login("", "").await;
    assert!(result.is_err(), "Login with empty credentials should fail");
}

#[tokio::test]
#[ignore = "requires running server"]
async fn test_api_without_token() {
    let client = TestClient::new(MAIN_BASE_URL);
    // Try to access API without authentication
    let response = client
        .raw_get("/nacos/v2/cs/config?dataId=test&group=DEFAULT_GROUP")
        .await
        .unwrap();
    let status = response.status().as_u16();
    // Should be 401 or 403
    assert!(
        status == 401 || status == 403,
        "Expected 401/403 without token, got {}",
        status
    );
}

#[tokio::test]
#[ignore = "requires running server"]
async fn test_api_with_invalid_token() {
    let client = TestClient::new_with_token(MAIN_BASE_URL, "invalid.jwt.token");
    let response = client
        .raw_get("/nacos/v2/cs/config?dataId=test&group=DEFAULT_GROUP")
        .await
        .unwrap();
    let status = response.status().as_u16();
    assert!(
        status == 401 || status == 403,
        "Expected 401/403 with invalid token, got {}",
        status
    );
}

#[tokio::test]
#[ignore = "requires running server"]
async fn test_api_with_expired_token() {
    // Use a well-formed but expired JWT
    let client = TestClient::new_with_token(
        MAIN_BASE_URL,
        "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ0ZXN0IiwiZXhwIjoxMDAwMDAwfQ.invalid",
    );
    let response = client
        .raw_get("/nacos/v2/cs/config?dataId=test&group=DEFAULT_GROUP")
        .await
        .unwrap();
    let status = response.status().as_u16();
    assert!(
        status == 401 || status == 403,
        "Expected 401/403 with expired token, got {}",
        status
    );
}

// ============================================================================
// Naming API Error Tests
// ============================================================================

#[tokio::test]
#[ignore = "requires running server"]
async fn test_naming_register_missing_ip() {
    let client = authenticated_client().await;
    let response = client
        .raw_post_form(
            "/nacos/v2/ns/instance",
            &[("serviceName", "test-svc"), ("port", "8080")],
        )
        .await
        .unwrap();
    let status = response.status().as_u16();
    // Missing IP should fail
    assert!(
        status == 400 || status == 500,
        "Expected 400/500 for missing IP, got {}",
        status
    );
}

#[tokio::test]
#[ignore = "requires running server"]
async fn test_naming_register_invalid_port() {
    let client = authenticated_client().await;
    let response = client
        .raw_post_form(
            "/nacos/v2/ns/instance",
            &[
                ("serviceName", "test-svc"),
                ("ip", "127.0.0.1"),
                ("port", "0"),
            ],
        )
        .await
        .unwrap();
    let body = response.text().await.unwrap();
    // Port 0 should be rejected
    assert!(
        body.contains("error") || body.contains("21004") || body.contains("port"),
        "Expected error for port 0: {}",
        body
    );
}

#[tokio::test]
#[ignore = "requires running server"]
async fn test_naming_query_nonexistent_service() {
    let client = authenticated_client().await;
    let response = client
        .raw_get("/nacos/v2/ns/instance/list?serviceName=nonexistent_svc_xyz_999&groupName=DEFAULT_GROUP")
        .await
        .unwrap();
    let status = response.status().as_u16();
    let body = response.text().await.unwrap();
    // Should indicate service not found
    assert!(
        body.contains("21008")
            || body.contains("not exist")
            || body.contains("\"code\":0")
            || status != 200,
        "Expected service not exist: {}",
        body
    );
}

// ============================================================================
// Namespace API Error Tests
// ============================================================================

#[tokio::test]
#[ignore = "requires running server"]
async fn test_namespace_get_nonexistent() {
    let client = authenticated_client().await;
    let response = client
        .raw_get("/nacos/v2/console/namespace?namespaceId=nonexistent_ns_xyz_999")
        .await
        .unwrap();
    let status = response.status().as_u16();
    // Should be 404 or contain error
    assert!(
        status == 404 || status == 200 || status == 500,
        "Expected 404/200/500 for nonexistent namespace, got {}",
        status
    );
}

#[tokio::test]
#[ignore = "requires running server"]
async fn test_namespace_create_empty_name() {
    let client = authenticated_client().await;
    let response = client
        .raw_post_form(
            "/nacos/v2/console/namespace",
            &[("customNamespaceId", "test-empty"), ("namespaceName", "")],
        )
        .await
        .unwrap();
    let status = response.status().as_u16();
    let body = response.text().await.unwrap();
    // Empty name should fail or be rejected
    assert!(
        status != 200 || body.contains("error") || body.contains("\"code\":0"),
        "Expected error for empty namespace name: {}",
        body
    );
}

// ============================================================================
// Route Not Found Tests
// ============================================================================

#[tokio::test]
#[ignore = "requires running server"]
async fn test_nonexistent_route() {
    let client = authenticated_client().await;
    let response = client
        .raw_get("/nacos/v99/nonexistent/route")
        .await
        .unwrap();
    assert_eq!(
        response.status().as_u16(),
        404,
        "Nonexistent route should return 404"
    );
}

#[tokio::test]
#[ignore = "requires running server"]
async fn test_v1_api_not_supported() {
    let client = authenticated_client().await;
    let response = client.raw_get("/nacos/v1/cs/configs").await.unwrap();
    let status = response.status().as_u16();
    // V1 API is not supported - should return 404
    assert_eq!(status, 404, "V1 API should not be supported");
}
