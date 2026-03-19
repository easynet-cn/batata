//! Negative/error path tests for HTTP APIs
//!
//! Tests error responses: 400 (bad request), 401 (unauthorized),
//! 404 (not found), and validation failures.

use batata_integration_tests::{
    CONSOLE_BASE_URL, MAIN_BASE_URL, TEST_PASSWORD, TEST_USERNAME, TestClient,
};

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
    let status = response.status().as_u16();
    assert_eq!(
        status, 400,
        "Missing required dataId parameter should return HTTP 400, got {}",
        status
    );
    let body = response.text().await.unwrap();
    let json: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
    // Verify the error code field indicates a parameter validation error
    assert!(
        json.get("code").is_some(),
        "Error response should contain 'code' field, got: {}",
        body
    );
    let code = json["code"].as_i64().unwrap_or(-1);
    assert_ne!(
        code, 0,
        "Error response code should not be 0 (success), got: {}",
        body
    );
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
    assert_eq!(
        status, 404,
        "Nonexistent config should return HTTP 404, got {}",
        status
    );
    let body = response.text().await.unwrap();
    let json: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
    // Nacos error code 20004 = "resource not found"
    let code = json.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
    assert_eq!(
        code, 20004,
        "Nonexistent config should return error code 20004 (resource not found), got code={} body={}",
        code, body
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
    let status = response.status().as_u16();
    assert_eq!(
        status, 400,
        "Missing required content parameter should return HTTP 400, got {}",
        status
    );
    let body = response.text().await.unwrap();
    let json: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
    let code = json.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
    assert_ne!(
        code, 0,
        "Error response code should not be 0 (success) for missing content, got: {}",
        body
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
    let status = response.status().as_u16();
    assert_eq!(
        status, 400,
        "Empty dataId should return HTTP 400, got {}",
        status
    );
    let body = response.text().await.unwrap();
    let json: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
    let message = json.get("message").and_then(|m| m.as_str()).unwrap_or("");
    assert!(
        !message.is_empty() || json.get("code").is_some(),
        "Error response should contain error message or code for empty dataId, got: {}",
        body
    );
}

// ============================================================================
// Auth API Error Tests
// ============================================================================

#[tokio::test]
#[ignore = "requires running server"]
async fn test_auth_login_wrong_password() {
    let client = TestClient::new(CONSOLE_BASE_URL);
    let response = client
        .raw_post_form(
            "/v3/auth/user/login",
            &[
                ("username", TEST_USERNAME),
                ("password", "wrong_password_xyz"),
            ],
        )
        .await
        .unwrap();
    let status = response.status().as_u16();
    assert!(
        status == 401 || status == 403,
        "Login with wrong password should return 401 or 403, got {}",
        status
    );
    let body = response.text().await.unwrap();
    let json: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
    let code = json.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
    assert_ne!(
        code, 0,
        "Wrong password login should return non-zero error code, got: {}",
        body
    );
}

#[tokio::test]
#[ignore = "requires running server"]
async fn test_auth_login_nonexistent_user() {
    let client = TestClient::new(CONSOLE_BASE_URL);
    let response = client
        .raw_post_form(
            "/v3/auth/user/login",
            &[
                ("username", "nonexistent_user_xyz"),
                ("password", "password"),
            ],
        )
        .await
        .unwrap();
    let status = response.status().as_u16();
    assert!(
        status == 401 || status == 403,
        "Login with nonexistent user should return 401 or 403, got {}",
        status
    );
    let body = response.text().await.unwrap();
    let json: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
    let code = json.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
    assert_ne!(
        code, 0,
        "Nonexistent user login should return non-zero error code, got: {}",
        body
    );
}

#[tokio::test]
#[ignore = "requires running server"]
async fn test_auth_login_empty_credentials() {
    let client = TestClient::new(CONSOLE_BASE_URL);
    let response = client
        .raw_post_form("/v3/auth/user/login", &[("username", ""), ("password", "")])
        .await
        .unwrap();
    let status = response.status().as_u16();
    assert!(
        status == 400 || status == 401 || status == 403,
        "Login with empty credentials should return 400/401/403, got {}",
        status
    );
    let body = response.text().await.unwrap();
    let json: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
    let code = json.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
    assert_ne!(
        code, 0,
        "Empty credentials login should return non-zero error code, got: {}",
        body
    );
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
    assert_eq!(
        status, 403,
        "API access without token should return HTTP 403, got {}",
        status
    );
    let body = response.text().await.unwrap();
    let json: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
    let message = json.get("message").and_then(|m| m.as_str()).unwrap_or("");
    assert!(
        message.to_lowercase().contains("token")
            || message.to_lowercase().contains("auth")
            || message.to_lowercase().contains("permission")
            || json.get("code").is_some(),
        "Error response should mention authentication issue, got: {}",
        body
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
    assert_eq!(
        status, 403,
        "API access with invalid token should return HTTP 403, got {}",
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
    assert_eq!(
        status, 403,
        "API access with expired token should return HTTP 403, got {}",
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
    assert_eq!(
        status, 400,
        "Missing required IP parameter should return HTTP 400, got {}",
        status
    );
    let body = response.text().await.unwrap();
    let json: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
    let code = json.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
    assert_ne!(
        code, 0,
        "Error response code should not be 0 (success) for missing IP, got: {}",
        body
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
    let status = response.status().as_u16();
    assert_eq!(
        status, 400,
        "Invalid port (0) should return HTTP 400, got {}",
        status
    );
    let body = response.text().await.unwrap();
    let json: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
    let code = json.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
    assert_ne!(
        code, 0,
        "Error response code should not be 0 (success) for invalid port, got: {}",
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
    let json: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
    let code = json.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
    // Nonexistent service should return error code 20004 (resource not found)
    // or a response indicating the service does not exist
    assert!(
        status == 404 || code == 20004 || code != 0 || body.contains("not exist"),
        "Expected service not found error (code 20004 or HTTP 404), got status={} code={} body={}",
        status,
        code,
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
    assert_eq!(
        status, 404,
        "Nonexistent namespace should return HTTP 404, got {}",
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
    assert_eq!(
        status, 400,
        "Creating namespace with empty name should return HTTP 400, got {}",
        status
    );
    let body = response.text().await.unwrap();
    let json: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
    let message = json.get("message").and_then(|m| m.as_str()).unwrap_or("");
    assert!(
        !message.is_empty() || json.get("code").is_some(),
        "Error response should contain error message for empty namespace name, got: {}",
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
