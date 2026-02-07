//! Authentication API integration tests
//!
//! Tests for /v3/auth endpoints

use crate::common::{CONSOLE_BASE_URL, TEST_PASSWORD, TEST_USERNAME, TestClient, unique_test_id};

/// Test login success
#[tokio::test]
#[ignore = "requires running server"]
async fn test_login_success() {
    let client = TestClient::new(CONSOLE_BASE_URL);

    // Console login returns flat format: {"accessToken":"...","tokenTtl":...,"globalAdmin":...,"username":"..."}
    let response: serde_json::Value = client
        .post_form(
            "/v3/auth/user/login",
            &[("username", TEST_USERNAME), ("password", TEST_PASSWORD)],
        )
        .await
        .expect("Login request should succeed");

    assert!(
        response["accessToken"].is_string(),
        "Should return access token"
    );
    assert!(response["tokenTtl"].is_number(), "Should return token TTL");
}

/// Test login failure with wrong password
#[tokio::test]
#[ignore = "requires running server"]
async fn test_login_failure_wrong_password() {
    let client = TestClient::new(CONSOLE_BASE_URL);

    // Failed login returns non-JSON error text, so parsing as JSON should fail or return error
    let result = client
        .post_form::<serde_json::Value, _>(
            "/v3/auth/user/login",
            &[("username", TEST_USERNAME), ("password", "wrong_password")],
        )
        .await;

    match result {
        Ok(response) => {
            // If parsed as JSON, code should indicate failure
            assert!(
                response.get("accessToken").is_none() || response["code"] != 0,
                "Login should fail with wrong password"
            );
        }
        Err(_) => {
            // Parse error or HTTP error is expected for failed login
        }
    }
}

/// Test login failure with wrong username
#[tokio::test]
#[ignore = "requires running server"]
async fn test_login_failure_wrong_username() {
    let client = TestClient::new(CONSOLE_BASE_URL);

    // Failed login returns non-JSON error text, so parsing as JSON should fail or return error
    let result = client
        .post_form::<serde_json::Value, _>(
            "/v3/auth/user/login",
            &[
                ("username", "nonexistent_user"),
                ("password", TEST_PASSWORD),
            ],
        )
        .await;

    match result {
        Ok(response) => {
            // If parsed as JSON, code should indicate failure
            assert!(
                response.get("accessToken").is_none() || response["code"] != 0,
                "Login should fail with wrong username"
            );
        }
        Err(_) => {
            // Parse error or HTTP error is expected for failed login
        }
    }
}

/// Test token validation on protected endpoint
#[tokio::test]
#[ignore = "requires running server"]
async fn test_token_validation() {
    let mut client = TestClient::new(CONSOLE_BASE_URL);

    // Login first
    client
        .login(TEST_USERNAME, TEST_PASSWORD)
        .await
        .expect("Login should succeed");

    // Access protected endpoint
    let response: serde_json::Value = client
        .get("/v3/auth/user/list?pageNo=1&pageSize=10&search=accurate")
        .await
        .expect("Should access with valid token");

    assert_eq!(response["code"], 0, "Should succeed with valid token");
}

/// Test access without token
#[tokio::test]
#[ignore = "requires running server"]
async fn test_access_without_token() {
    let client = TestClient::new(CONSOLE_BASE_URL);

    // Try to access protected endpoint without token
    let result = client
        .get::<serde_json::Value>("/v3/auth/user/list?pageNo=1&pageSize=10&search=accurate")
        .await;

    // Should fail or return error
    match result {
        Ok(response) => {
            assert_ne!(response["code"], 0, "Should fail without token");
        }
        Err(_) => {
            // Expected - unauthorized
        }
    }
}

/// Test create user
#[tokio::test]
#[ignore = "requires running server"]
async fn test_create_user() {
    let mut client = TestClient::new(CONSOLE_BASE_URL);
    client
        .login(TEST_USERNAME, TEST_PASSWORD)
        .await
        .expect("Login failed");

    let new_username = format!("testuser_{}", unique_test_id());

    let response: serde_json::Value = client
        .post_form(
            "/v3/auth/user",
            &[("username", new_username.as_str()), ("password", "test123")],
        )
        .await
        .expect("Create user request failed");

    assert_eq!(response["code"], 0, "Create user should succeed");

    // Cleanup - delete the user
    let _: serde_json::Value = client
        .delete_with_query("/v3/auth/user", &[("username", new_username.as_str())])
        .await
        .ok()
        .unwrap_or_default();
}

/// Test user list
#[tokio::test]
#[ignore = "requires running server"]
async fn test_user_list() {
    let mut client = TestClient::new(CONSOLE_BASE_URL);
    client
        .login(TEST_USERNAME, TEST_PASSWORD)
        .await
        .expect("Login failed");

    let response: serde_json::Value = client
        .get_with_query(
            "/v3/auth/user/list",
            &[("pageNo", "1"), ("pageSize", "10"), ("search", "accurate")],
        )
        .await
        .expect("Get users failed");

    assert_eq!(response["code"], 0, "Get users should succeed");
}

/// Test create role
#[tokio::test]
#[ignore = "requires running server"]
async fn test_create_role() {
    let mut client = TestClient::new(CONSOLE_BASE_URL);
    client
        .login(TEST_USERNAME, TEST_PASSWORD)
        .await
        .expect("Login failed");

    let role_name = format!("testrole_{}", unique_test_id());

    let response: serde_json::Value = client
        .post_form(
            "/v3/auth/role",
            &[("role", role_name.as_str()), ("username", TEST_USERNAME)],
        )
        .await
        .expect("Create role request failed");

    assert_eq!(response["code"], 0, "Create role should succeed");

    // Cleanup
    let _: serde_json::Value = client
        .delete_with_query("/v3/auth/role", &[("role", role_name.as_str())])
        .await
        .ok()
        .unwrap_or_default();
}

/// Test assign permission
#[tokio::test]
#[ignore = "requires running server"]
async fn test_assign_permission() {
    let mut client = TestClient::new(CONSOLE_BASE_URL);
    client
        .login(TEST_USERNAME, TEST_PASSWORD)
        .await
        .expect("Login failed");

    let role_name = format!("permrole_{}", unique_test_id());

    // Create role first (requires username)
    let _: serde_json::Value = client
        .post_form(
            "/v3/auth/role",
            &[("role", role_name.as_str()), ("username", TEST_USERNAME)],
        )
        .await
        .expect("Create role failed");

    // Assign permission
    let response: serde_json::Value = client
        .post_form(
            "/v3/auth/permission",
            &[
                ("role", role_name.as_str()),
                ("resource", "public:*:config/*"),
                ("action", "r"),
            ],
        )
        .await
        .expect("Assign permission failed");

    assert_eq!(response["code"], 0, "Assign permission should succeed");

    // Cleanup
    let _: serde_json::Value = client
        .delete_with_query(
            "/v3/auth/permission",
            &[
                ("role", role_name.as_str()),
                ("resource", "public:*:config/*"),
                ("action", "r"),
            ],
        )
        .await
        .ok()
        .unwrap_or_default();

    let _: serde_json::Value = client
        .delete_with_query("/v3/auth/role", &[("role", role_name.as_str())])
        .await
        .ok()
        .unwrap_or_default();
}
