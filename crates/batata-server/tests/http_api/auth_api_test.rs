//! Authentication API integration tests
//!
//! Tests for /v3/auth endpoints

use crate::common::{TEST_PASSWORD, TEST_USERNAME, TestClient, unique_test_id};

/// Test login success
#[tokio::test]
#[ignore = "requires running server"]
async fn test_login_success() {
    let client = TestClient::new("http://127.0.0.1:8848");

    let response: serde_json::Value = client
        .post_form(
            "/v3/auth/user/login",
            &[("username", TEST_USERNAME), ("password", TEST_PASSWORD)],
        )
        .await
        .expect("Login request should succeed");

    assert_eq!(response["code"], 0, "Login should succeed");
    assert!(
        response["data"]["accessToken"].is_string(),
        "Should return access token"
    );
}

/// Test login failure with wrong password
#[tokio::test]
#[ignore = "requires running server"]
async fn test_login_failure_wrong_password() {
    let client = TestClient::new("http://127.0.0.1:8848");

    let response: serde_json::Value = client
        .post_form(
            "/v3/auth/user/login",
            &[("username", TEST_USERNAME), ("password", "wrong_password")],
        )
        .await
        .expect("Request should complete");

    assert_ne!(response["code"], 0, "Login should fail");
}

/// Test login failure with wrong username
#[tokio::test]
#[ignore = "requires running server"]
async fn test_login_failure_wrong_username() {
    let client = TestClient::new("http://127.0.0.1:8848");

    let response: serde_json::Value = client
        .post_form(
            "/v3/auth/user/login",
            &[
                ("username", "nonexistent_user"),
                ("password", TEST_PASSWORD),
            ],
        )
        .await
        .expect("Request should complete");

    assert_ne!(response["code"], 0, "Login should fail");
}

/// Test token validation on protected endpoint
#[tokio::test]
#[ignore = "requires running server"]
async fn test_token_validation() {
    let mut client = TestClient::new("http://127.0.0.1:8848");

    // Login first
    client
        .login(TEST_USERNAME, TEST_PASSWORD)
        .await
        .expect("Login should succeed");

    // Access protected endpoint
    let response: serde_json::Value = client
        .get("/v3/auth/users")
        .await
        .expect("Should access with valid token");

    assert_eq!(response["code"], 0, "Should succeed with valid token");
}

/// Test access without token
#[tokio::test]
#[ignore = "requires running server"]
async fn test_access_without_token() {
    let client = TestClient::new("http://127.0.0.1:8848");

    // Try to access protected endpoint without token
    let result = client.get::<serde_json::Value>("/v3/auth/users").await;

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
    let mut client = TestClient::new("http://127.0.0.1:8848");
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
    let mut client = TestClient::new("http://127.0.0.1:8848");
    client
        .login(TEST_USERNAME, TEST_PASSWORD)
        .await
        .expect("Login failed");

    let response: serde_json::Value = client
        .get_with_query("/v3/auth/users", &[("pageNo", "1"), ("pageSize", "10")])
        .await
        .expect("Get users failed");

    assert_eq!(response["code"], 0, "Get users should succeed");
}

/// Test create role
#[tokio::test]
#[ignore = "requires running server"]
async fn test_create_role() {
    let mut client = TestClient::new("http://127.0.0.1:8848");
    client
        .login(TEST_USERNAME, TEST_PASSWORD)
        .await
        .expect("Login failed");

    let role_name = format!("testrole_{}", unique_test_id());

    let response: serde_json::Value = client
        .post_form(
            "/v3/auth/role",
            &[
                ("role", role_name.as_str()),
                ("description", "Test role for integration testing"),
            ],
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
    let mut client = TestClient::new("http://127.0.0.1:8848");
    client
        .login(TEST_USERNAME, TEST_PASSWORD)
        .await
        .expect("Login failed");

    let role_name = format!("permrole_{}", unique_test_id());

    // Create role first
    let _: serde_json::Value = client
        .post_form("/v3/auth/role", &[("role", role_name.as_str())])
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
