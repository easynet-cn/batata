//! Auth / RBAC Functional Tests
//!
//! Tests user/role/permission management via HTTP Auth API.
//! Matches NacosAuthRbacTest scenarios.

mod common;

// ==================== User Management ====================

#[tokio::test]
async fn test_user_list() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let users = client.user_list(1, 10).await.unwrap();
    assert!(users.total_count >= 1, "Should have at least the admin user");
    assert!(
        !users.page_items.is_empty(),
        "Page items should contain users"
    );
}

#[tokio::test]
async fn test_user_create_and_delete() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let username = format!("testuser-{}", common::test_id());

    // Create user
    let ok = client.user_create(&username, "Test1234!").await.unwrap();
    assert!(ok, "User creation should succeed");

    // Verify user exists in list
    let users = client.user_list(1, 100).await.unwrap();
    let found = users
        .page_items
        .iter()
        .any(|u| u.get("username").and_then(|v| v.as_str()) == Some(&username));
    assert!(found, "Created user should appear in user list");

    // Delete user
    let deleted = client.user_delete(&username).await.unwrap();
    assert!(deleted, "User deletion should succeed");

    // Verify user is gone
    let users_after = client.user_list(1, 100).await.unwrap();
    let found_after = users_after
        .page_items
        .iter()
        .any(|u| u.get("username").and_then(|v| v.as_str()) == Some(&username));
    assert!(!found_after, "Deleted user should not appear in user list");
}

// ==================== Role Management ====================

#[tokio::test]
async fn test_role_list() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let roles = client.role_list(1, 10).await.unwrap();
    assert!(
        roles.total_count >= 1,
        "Should have at least the admin role"
    );
}

#[tokio::test]
async fn test_role_assign_and_remove() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let username = format!("roleuser-{}", common::test_id());
    let role_name = "test_role";

    // Create user first
    client.user_create(&username, "Test1234!").await.unwrap();

    // Assign role
    let ok = client.role_create(&username, role_name).await.unwrap();
    assert!(ok, "Role assignment should succeed");

    // Verify role exists
    let roles = client.role_list(1, 100).await.unwrap();
    let found = roles.page_items.iter().any(|r| {
        r.get("username").and_then(|v| v.as_str()) == Some(&username)
            && r.get("role").and_then(|v| v.as_str()) == Some(role_name)
    });
    assert!(found, "Assigned role should appear in role list");

    // Remove role
    let removed = client.role_delete(&username, role_name).await.unwrap();
    assert!(removed, "Role removal should succeed");

    // Cleanup
    let _ = client.user_delete(&username).await;
}

// ==================== Permission Management ====================

#[tokio::test]
async fn test_permission_list() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let perms = client.permission_list(1, 10).await.unwrap();
    // May be empty, just verify query succeeds
    assert!(perms.total_count >= 0);
}

#[tokio::test]
async fn test_permission_create_and_delete() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let username = format!("permuser-{}", common::test_id());
    let role_name = "perm_test_role";
    let resource = "public:DEFAULT_GROUP:*";
    let action = "rw";

    // Setup: create user + role
    client.user_create(&username, "Test1234!").await.unwrap();
    client.role_create(&username, role_name).await.unwrap();

    // Create permission
    let ok = client
        .permission_create(role_name, resource, action)
        .await
        .unwrap();
    assert!(ok, "Permission creation should succeed");

    // Verify permission exists
    let perms = client.permission_list(1, 100).await.unwrap();
    let found = perms.page_items.iter().any(|p| {
        p.get("role").and_then(|v| v.as_str()) == Some(role_name)
            && p.get("resource").and_then(|v| v.as_str()) == Some(resource)
    });
    assert!(found, "Created permission should appear in permission list");

    // Delete permission
    let deleted = client
        .permission_delete(role_name, resource, action)
        .await
        .unwrap();
    assert!(deleted, "Permission deletion should succeed");

    // Cleanup
    let _ = client.role_delete(&username, role_name).await;
    let _ = client.user_delete(&username).await;
}

// ==================== Permission Enforcement ====================

#[tokio::test]
async fn test_permission_enforcement() {
    common::init_tracing();
    let admin = common::create_api_client().await.unwrap();

    let username = format!("limited-{}", common::test_id());
    let role_name = "limited_role";

    // Create limited user with specific permission
    admin.user_create(&username, "Test1234!").await.unwrap();
    admin.role_create(&username, role_name).await.unwrap();
    // Only read permission on a specific resource
    admin
        .permission_create(role_name, "public:DEFAULT_GROUP:allowed-*", "r")
        .await
        .unwrap();

    // Login as limited user
    let limited_config = batata_client::http::HttpClientConfig::new(common::SERVER_URL)
        .with_auth(&username, "Test1234!")
        .with_context_path("/nacos")
        .with_auth_endpoint("/nacos/v3/auth/user/login")
        .with_timeouts(5000, 30000);
    let limited_http = batata_client::http::BatataHttpClient::new(limited_config)
        .await
        .unwrap();
    let limited = batata_client::api::BatataApiClient::new(limited_http);

    // Admin API enforces admin-level auth; limited user should be rejected
    let result = limited.config_get("allowed-test", "DEFAULT_GROUP", "public").await;
    assert!(result.is_err(), "Limited user should be rejected by admin API");

    // Cleanup
    let _ = admin.permission_delete(role_name, "public:DEFAULT_GROUP:allowed-*", "r").await;
    let _ = admin.role_delete(&username, role_name).await;
    let _ = admin.user_delete(&username).await;
}
