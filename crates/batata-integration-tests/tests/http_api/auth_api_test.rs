//! Authentication API integration tests
//!
//! Tests for /v3/auth endpoints

use batata_integration_tests::{
    CONSOLE_BASE_URL, TEST_PASSWORD, TEST_USERNAME, TestClient, unique_test_id,
};

/// Test login success
#[tokio::test]

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

async fn test_login_failure_wrong_password() {
    let client = TestClient::new(CONSOLE_BASE_URL);

    // Use raw request to inspect the full response
    let result = client
        .raw_post_form(
            "/v3/auth/user/login",
            &[("username", TEST_USERNAME), ("password", "wrong_password")],
        )
        .await
        .expect("HTTP request itself should not fail");

    let status = result.status();
    let body = result.text().await.expect("Should read response body");

    // Login failure should return a non-success status or an error code in body
    assert!(
        status.is_client_error() || status.is_server_error() || status.as_u16() == 200,
        "Should return an HTTP error or 200 with error body"
    );

    // Parse body as JSON and check for specific error code
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
        // Should NOT have an access token
        assert!(
            json.get("accessToken").is_none()
                || json["accessToken"].is_null()
                || json["code"].as_i64().unwrap_or(0) != 0,
            "Should not return a valid access token for wrong password"
        );
        // If there's an error code, assert it is a known auth error
        if let Some(code) = json["code"].as_i64() {
            assert_ne!(code, 0, "Error code should not be 0 for wrong password");
            // Nacos auth error codes: 20021 (password wrong), 20020 (login failed)
            assert!(
                code == 20021 || code == 20020 || code == 403,
                "Expected auth error code (20021 or 20020 or 403), got {}",
                code
            );
        }
    } else {
        // Non-JSON error response is also acceptable for failed login,
        // but status should indicate failure
        assert!(
            !status.is_success(),
            "Non-JSON response for failed login should have error HTTP status, got {} with body: {}",
            status,
            body
        );
    }
}

/// Test login failure with wrong username
#[tokio::test]

async fn test_login_failure_wrong_username() {
    let client = TestClient::new(CONSOLE_BASE_URL);

    let result = client
        .raw_post_form(
            "/v3/auth/user/login",
            &[
                ("username", "nonexistent_user"),
                ("password", TEST_PASSWORD),
            ],
        )
        .await
        .expect("HTTP request itself should not fail");

    let status = result.status();
    let body = result.text().await.expect("Should read response body");

    // Login with nonexistent user should fail
    assert!(
        status.is_client_error() || status.is_server_error() || status.as_u16() == 200,
        "Should return an HTTP error or 200 with error body"
    );

    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
        assert!(
            json.get("accessToken").is_none()
                || json["accessToken"].is_null()
                || json["code"].as_i64().unwrap_or(0) != 0,
            "Should not return a valid access token for nonexistent user"
        );
        if let Some(code) = json["code"].as_i64() {
            assert_ne!(code, 0, "Error code should not be 0 for nonexistent user");
            // Nacos error codes: 20020 (user not found), 20021 (login failed)
            assert!(
                code == 20020 || code == 20021 || code == 403,
                "Expected user-not-found error code (20020 or 20021 or 403), got {}",
                code
            );
        }
    } else {
        assert!(
            !status.is_success(),
            "Non-JSON response for failed login should have error HTTP status, got {} with body: {}",
            status,
            body
        );
    }
}

/// Test token validation on protected endpoint
#[tokio::test]

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

    // Parse the user list and verify it contains data
    let page_items = &response["data"]["pageItems"];
    assert!(
        page_items.is_array(),
        "User list data should contain pageItems array"
    );

    let users = page_items.as_array().unwrap();
    assert!(
        !users.is_empty(),
        "User list should contain at least one user"
    );

    // Verify the test user (nacos) exists in the list
    let has_test_user = users.iter().any(|u| u["username"] == TEST_USERNAME);
    assert!(
        has_test_user,
        "User list should contain the test user '{}'",
        TEST_USERNAME
    );
}

/// Test access without token
#[tokio::test]

async fn test_access_without_token() {
    let client = TestClient::new(CONSOLE_BASE_URL);

    // Use raw request to inspect HTTP status and body
    let result = client
        .raw_get("/v3/auth/user/list?pageNo=1&pageSize=10&search=accurate")
        .await
        .expect("HTTP request itself should not fail");

    let status = result.status();
    let body = result.text().await.expect("Should read response body");

    // Should return 403 Forbidden or 401 Unauthorized
    assert!(
        status.as_u16() == 403 || status.as_u16() == 401 || status.is_client_error(),
        "Should return 403/401 without token, got {}",
        status
    );

    // Check error message mentions token or auth
    let body_lower = body.to_lowercase();
    assert!(
        body_lower.contains("token")
            || body_lower.contains("auth")
            || body_lower.contains("forbidden")
            || body_lower.contains("unauthorized")
            || body_lower.contains("login"),
        "Error response should mention token/auth/forbidden, got body: {}",
        body
    );
}

/// Test create user
#[tokio::test]

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

    // Verify the user was actually created by querying the user list
    let list_response: serde_json::Value = client
        .get_with_query(
            "/v3/auth/user/list",
            &[
                ("pageNo", "1"),
                ("pageSize", "100"),
                ("search", "accurate"),
                ("username", new_username.as_str()),
            ],
        )
        .await
        .expect("Get user list failed");

    assert_eq!(list_response["code"], 0, "User list query should succeed");

    let page_items = &list_response["data"]["pageItems"];
    assert!(
        page_items.is_array(),
        "User list should contain pageItems array"
    );

    let users = page_items.as_array().unwrap();
    let found = users.iter().any(|u| u["username"] == new_username.as_str());
    assert!(
        found,
        "Created user '{}' should appear in the user list",
        new_username
    );

    // Cleanup - delete the user
    let _: serde_json::Value = client
        .delete_with_query("/v3/auth/user", &[("username", new_username.as_str())])
        .await
        .ok()
        .unwrap_or_default();
}

/// Test user list
#[tokio::test]

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

    // Assert the list has content
    let page_items = &response["data"]["pageItems"];
    assert!(
        page_items.is_array(),
        "Response should contain pageItems array"
    );

    let users = page_items.as_array().unwrap();
    assert!(
        !users.is_empty(),
        "User list should have at least one user (the admin user)"
    );

    // The default admin user should be present
    let has_admin = users.iter().any(|u| u["username"] == TEST_USERNAME);
    assert!(
        has_admin,
        "User list should contain the admin user '{}'",
        TEST_USERNAME
    );
}

/// Test create role
#[tokio::test]

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

    // Verify the role was actually created by querying the role list
    let list_response: serde_json::Value = client
        .get_with_query(
            "/v3/auth/role/list",
            &[
                ("pageNo", "1"),
                ("pageSize", "100"),
                ("search", "accurate"),
                ("role", role_name.as_str()),
                ("username", ""),
            ],
        )
        .await
        .expect("Get role list failed");

    assert_eq!(list_response["code"], 0, "Role list query should succeed");

    let page_items = &list_response["data"]["pageItems"];
    assert!(
        page_items.is_array(),
        "Role list should contain pageItems array"
    );

    let roles = page_items.as_array().unwrap();
    let found = roles.iter().any(|r| r["role"] == role_name.as_str());
    assert!(
        found,
        "Created role '{}' should appear in the role list",
        role_name
    );

    // Cleanup
    let _: serde_json::Value = client
        .delete_with_query("/v3/auth/role", &[("role", role_name.as_str())])
        .await
        .ok()
        .unwrap_or_default();
}

/// Test assign permission
#[tokio::test]

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
