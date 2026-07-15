use reqwest::Client;
use serde_json::{json, Value};
use uuid::Uuid;

const APOLLO_ADMIN_URL: &str = "http://localhost:8080/admin";
const APOLLO_CONFIG_URL: &str = "http://localhost:8080/config";

fn unique_test_id(prefix: &str) -> String {
    format!("{}_test_{}", prefix, Uuid::new_v4().to_string().split('-').next().unwrap())
}

#[tokio::test]
async fn test_config_service_gray_release() {
    let client = Client::new();
    let app_id = unique_test_id("gray");

    let create_app_resp = client
        .post(format!("{}/apps", APOLLO_ADMIN_URL))
        .json(&json!({
            "appId": app_id,
            "name": "Gray Test App",
            "orgId": "default",
            "orgName": "Default",
            "ownerName": "admin",
            "ownerEmail": "admin@test.com",
            "dataChangeCreatedBy": "admin"
        }))
        .send()
        .await
        .expect("Create app failed");
    assert!(create_app_resp.status().is_success(), "Create app failed: {:?}", create_app_resp.text().await);

    let create_ns_resp = client
        .post(format!("{}/apps/{}/clusters/default/namespaces", APOLLO_ADMIN_URL, app_id))
        .json(&json!({
            "appId": app_id,
            "clusterName": "default",
            "namespaceName": "application",
            "format": "properties",
            "isPublic": false,
            "comment": "test",
            "dataChangeCreatedBy": "admin"
        }))
        .send()
        .await
        .expect("Create namespace failed");
    assert!(create_ns_resp.status().is_success(), "Create namespace failed: {:?}", create_ns_resp.text().await);

    let create_item_resp = client
        .post(format!("{}/apps/{}/clusters/default/namespaces/application/items", APOLLO_ADMIN_URL, app_id))
        .json(&json!({
            "key": "test.key",
            "value": "default_value",
            "comment": "test",
            "dataChangeCreatedBy": "admin"
        }))
        .send()
        .await
        .expect("Create item failed");
    assert!(create_item_resp.status().is_success(), "Create item failed: {:?}", create_item_resp.text().await);

    let publish_resp = client
        .post(format!("{}/apps/{}/clusters/default/namespaces/application/releases?name=release1&comment=initial&operator=admin", APOLLO_ADMIN_URL, app_id))
        .send()
        .await
        .expect("Publish failed");
    assert!(publish_resp.status().is_success(), "Publish failed: {:?}", publish_resp.text().await);

    let config_resp: Value = client
        .get(format!("{}/configs/{}/default/application", APOLLO_CONFIG_URL, app_id))
        .send()
        .await
        .expect("Get config failed")
        .json()
        .await
        .expect("Parse config failed");
    assert_eq!(config_resp["configurations"]["test.key"], "default_value", "Default config mismatch");

    let update_item_resp = client
        .put(format!("{}/apps/{}/clusters/default/namespaces/application/items/1", APOLLO_ADMIN_URL, app_id))
        .json(&json!({
            "key": "test.key",
            "value": "gray_value",
            "comment": "gray",
            "dataChangeLastModifiedBy": "admin"
        }))
        .send()
        .await
        .expect("Update item failed");
    assert!(update_item_resp.status().is_success(), "Update item failed: {:?}", update_item_resp.text().await);

    let publish_gray_resp = client
        .post(format!("{}/apps/{}/clusters/default/namespaces/application/releases?name=release2&comment=gray&operator=admin", APOLLO_ADMIN_URL, app_id))
        .send()
        .await
        .expect("Publish gray failed");
    assert!(publish_gray_resp.status().is_success(), "Publish gray failed: {:?}", publish_gray_resp.text().await);

    let release_info: Value = publish_gray_resp.json().await.expect("Parse release failed");
    let gray_release_id = release_info["releaseId"].as_i64().expect("No releaseId");

    let create_gray_rule_resp = client
        .post(format!("{}/apps/{}/clusters/default/namespaces/application/gray-release-rules", APOLLO_ADMIN_URL, app_id))
        .json(&json!({
            "appId": app_id,
            "clusterName": "default",
            "namespaceName": "application",
            "branchName": "gray_test",
            "rules": "[{\"ip\":\"192.168.1.0/24\"}]",
            "releaseId": gray_release_id,
            "branchStatus": 1,
            "dataChangeCreatedBy": "admin"
        }))
        .send()
        .await
        .expect("Create gray rule failed");
    assert!(create_gray_rule_resp.status().is_success(), "Create gray rule failed: {:?}", create_gray_rule_resp.text().await);

    let config_without_gray: Value = client
        .get(format!("{}/configs/{}/default/application", APOLLO_CONFIG_URL, app_id))
        .header("X-Forwarded-For", "10.0.0.1")
        .send()
        .await
        .expect("Get config without gray failed")
        .json()
        .await
        .expect("Parse config without gray failed");
    assert_eq!(config_without_gray["configurations"]["test.key"], "gray_value", "Config without gray should get latest");

    let config_with_gray: Value = client
        .get(format!("{}/configs/{}/default/application", APOLLO_CONFIG_URL, app_id))
        .header("X-Forwarded-For", "192.168.1.100")
        .send()
        .await
        .expect("Get config with gray failed")
        .json()
        .await
        .expect("Parse config with gray failed");
    assert_eq!(config_with_gray["configurations"]["test.key"], "gray_value", "Config with gray IP should get gray config");
}

#[tokio::test]
async fn test_namespace_lock() {
    let client = Client::new();
    let app_id = unique_test_id("lock");

    let create_app_resp = client
        .post(format!("{}/apps", APOLLO_ADMIN_URL))
        .json(&json!({
            "appId": app_id,
            "name": "Lock Test App",
            "orgId": "default",
            "orgName": "Default",
            "ownerName": "admin",
            "ownerEmail": "admin@test.com",
            "dataChangeCreatedBy": "admin"
        }))
        .send()
        .await
        .expect("Create app failed");
    assert!(create_app_resp.status().is_success());

    let create_ns_resp = client
        .post(format!("{}/apps/{}/clusters/default/namespaces", APOLLO_ADMIN_URL, app_id))
        .json(&json!({
            "appId": app_id,
            "clusterName": "default",
            "namespaceName": "application",
            "format": "properties",
            "isPublic": false,
            "comment": "test",
            "dataChangeCreatedBy": "admin"
        }))
        .send()
        .await
        .expect("Create namespace failed");
    assert!(create_ns_resp.status().is_success());

    let lock_resp = client
        .post(format!("{}/apps/{}/clusters/default/namespaces/application/lock?lockedBy=user1", APOLLO_ADMIN_URL, app_id))
        .send()
        .await
        .expect("Lock namespace failed");
    assert!(lock_resp.status().is_success(), "Lock namespace failed: {:?}", lock_resp.text().await);

    let get_lock_resp = client
        .get(format!("{}/apps/{}/clusters/default/namespaces/application/lock", APOLLO_ADMIN_URL, app_id))
        .send()
        .await
        .expect("Get lock failed");
    assert!(get_lock_resp.status().is_success(), "Get lock failed: {:?}", get_lock_resp.text().await);

    let lock_info: Value = get_lock_resp.json().await.expect("Parse lock info failed");
    assert_eq!(lock_info["lockedBy"], "user1", "Lock owner mismatch");

    let lock_by_another_resp = client
        .post(format!("{}/apps/{}/clusters/default/namespaces/application/lock?lockedBy=user2", APOLLO_ADMIN_URL, app_id))
        .send()
        .await
        .expect("Lock by another user failed");
    assert_eq!(lock_by_another_resp.status().as_u16(), 400, "Lock by another user should fail");

    let unlock_resp = client
        .delete(format!("{}/apps/{}/clusters/default/namespaces/application/lock?lockedBy=user1", APOLLO_ADMIN_URL, app_id))
        .send()
        .await
        .expect("Unlock namespace failed");
    assert!(unlock_resp.status().is_success(), "Unlock namespace failed: {:?}", unlock_resp.text().await);

    let get_lock_after_unlock_resp = client
        .get(format!("{}/apps/{}/clusters/default/namespaces/application/lock", APOLLO_ADMIN_URL, app_id))
        .send()
        .await
        .expect("Get lock after unlock failed");
    assert_eq!(get_lock_after_unlock_resp.status().as_u16(), 404, "Lock should not exist after unlock");
}

#[tokio::test]
async fn test_commit_openapi() {
    let client = Client::new();
    let app_id = unique_test_id("commit");

    let create_app_resp = client
        .post(format!("{}/apps", APOLLO_ADMIN_URL))
        .json(&json!({
            "appId": app_id,
            "name": "Commit Test App",
            "orgId": "default",
            "orgName": "Default",
            "ownerName": "admin",
            "ownerEmail": "admin@test.com",
            "dataChangeCreatedBy": "admin"
        }))
        .send()
        .await
        .expect("Create app failed");
    assert!(create_app_resp.status().is_success());

    let create_ns_resp = client
        .post(format!("{}/apps/{}/clusters/default/namespaces", APOLLO_ADMIN_URL, app_id))
        .json(&json!({
            "appId": app_id,
            "clusterName": "default",
            "namespaceName": "application",
            "format": "properties",
            "isPublic": false,
            "comment": "test",
            "dataChangeCreatedBy": "admin"
        }))
        .send()
        .await
        .expect("Create namespace failed");
    assert!(create_ns_resp.status().is_success());

    let create_commit_resp = client
        .post(format!("/openapi/v1/envs/DEV/apps/{}/clusters/default/namespaces/application/commits", app_id))
        .json(&json!({
            "changeSets": "[{\"key\":\"test.key\",\"value\":\"test.value\"}]",
            "comment": "test commit",
            "dataChangeCreatedBy": "admin"
        }))
        .send()
        .await
        .expect("Create commit failed");
    assert!(create_commit_resp.status().is_success(), "Create commit failed: {:?}", create_commit_resp.text().await);

    let commit_info: Value = create_commit_resp.json().await.expect("Parse commit failed");
    let commit_id = commit_info["id"].as_i64().expect("No commit id") as i32;

    let list_commits_resp: Value = client
        .get(format!("/openapi/v1/envs/DEV/apps/{}/clusters/default/namespaces/application/commits", app_id))
        .send()
        .await
        .expect("List commits failed")
        .json()
        .await
        .expect("Parse commits failed");
    assert!(list_commits_resp.as_array().unwrap().len() >= 1, "Should have at least one commit");

    let get_commit_resp: Value = client
        .get(format!("/openapi/v1/commits/{}", commit_id))
        .send()
        .await
        .expect("Get commit failed")
        .json()
        .await
        .expect("Parse commit failed");
    assert_eq!(get_commit_resp["id"], commit_id, "Commit id mismatch");
}
