use reqwest::Client;
use serde_json::{Value, json};
use std::time::Duration;
use uuid::Uuid;

const APOLLO_ADMIN_URL: &str = "http://localhost:8080";
const APOLLO_CONFIG_URL: &str = "http://localhost:8080";
const BASE_URL: &str = "http://localhost:8080";

fn unique_test_id(prefix: &str) -> String {
    format!(
        "{}_test_{}",
        prefix,
        Uuid::new_v4().to_string().split('-').next().unwrap()
    )
}

/// HTTP client for the live-server suite.
///
/// `no_proxy()` is deliberate: reqwest 0.12+ picks up the macOS system proxy
/// by default and (unlike curl) does not honor the `localhost` bypass entry,
/// so with a system proxy enabled every request to the local server comes
/// back as a 502 from the proxy instead of reaching batata.
fn test_http_client() -> Client {
    Client::builder()
        .no_proxy()
        .build()
        .expect("failed to build http client")
}

/// Whether a probe response is the `GET /apps` contract of an Apollo admin
/// surface this suite can actually drive: 2xx carrying a JSON array.
///
/// A reachable port alone is not a usable test target. An unrelated local
/// service, an SPA dev server, a proxy error page, or a batata build with
/// admin access control enabled can all answer on `:8080` and then reject
/// every request below with 401. Treating "any HTTP response" as "server
/// available" turns such a port collision into three false-red tests that
/// say nothing about the code under test.
fn is_live_apollo_admin(status: reqwest::StatusCode, body: &str) -> bool {
    status.is_success() && serde_json::from_str::<Value>(body).is_ok_and(|value| value.is_array())
}

#[test]
fn live_server_probe_requires_a_real_apollo_admin_surface() {
    let ok = reqwest::StatusCode::OK;
    let unauthorized = reqwest::StatusCode::UNAUTHORIZED;

    // Success path: the admin surface lists apps as a JSON array.
    assert!(is_live_apollo_admin(ok, "[]"));
    assert!(is_live_apollo_admin(
        ok,
        "[{\"appId\":\"demo\",\"name\":\"Demo\"}]"
    ));

    // A reachable but auth-gated server (the :8080 collision case).
    assert!(!is_live_apollo_admin(
        unauthorized,
        "{\"code\":40100,\"data\":null,\"msg\":\"unauthenticated\"}"
    ));
    // An array body alone is not enough: the status must be 2xx.
    assert!(!is_live_apollo_admin(unauthorized, "[]"));
    // 2xx replies from an unrelated service / SPA / proxy page.
    assert!(!is_live_apollo_admin(ok, "<!doctype html><html></html>"));
    assert!(!is_live_apollo_admin(ok, "{\"code\":40100}"));
    assert!(!is_live_apollo_admin(ok, ""));
}

/// Env-dependent suite guard: returns `true` when the caller should return
/// early.
///
/// Every test in this file drives the HTTP surface end to end, so it needs an
/// unauthenticated batata Apollo admin surface on `BASE_URL`. Mirroring the
/// SQL suites in `common` — which self-skip when `TEST_DATABASE_URL` is
/// absent — these return early with an explicit message instead of failing:
/// a red test caused by a missing (or wrong) server says nothing about the
/// code under test, and listing it in `regression_whitelist.txt` only hides
/// the fact that it never actually ran. Start a server on `:8080` (or point
/// `BASE_URL` at one) to exercise them.
async fn skip_without_server(test: &str) -> bool {
    let probe = tokio::time::timeout(Duration::from_millis(500), async {
        let response = test_http_client()
            .get(format!("{BASE_URL}/apps"))
            .send()
            .await?;
        let status = response.status();
        let body = response.text().await?;
        Ok::<_, reqwest::Error>((status, body))
    })
    .await;
    let ready = probe
        .ok()
        .and_then(|result| result.ok())
        .is_some_and(|(status, body)| is_live_apollo_admin(status, &body));
    if ready {
        return false;
    }
    eprintln!(
        "SKIP {test}: no unauthenticated batata Apollo admin surface at {BASE_URL} \
         (GET /apps must return 2xx with a JSON array)"
    );
    true
}

#[tokio::test]
async fn test_config_service_gray_release() {
    if skip_without_server("test_config_service_gray_release").await {
        return;
    }
    let client = test_http_client();
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
    assert!(
        create_app_resp.status().is_success(),
        "Create app failed: {:?}",
        create_app_resp.text().await
    );

    let create_ns_resp = client
        .post(format!(
            "{}/apps/{}/clusters/default/namespaces",
            APOLLO_ADMIN_URL, app_id
        ))
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
    // The `application` namespace now exists via app-bootstrap (upstream
    // semantics) — a duplicate create must not fail the scenario.
    if !create_ns_resp.status().is_success() {
        let body = create_ns_resp.text().await.unwrap_or_default();
        assert!(
            body.contains("already exists"),
            "Create namespace failed: {}",
            body
        );
    }

    let create_item_resp = client
        .post(format!(
            "{}/apps/{}/clusters/default/namespaces/application/items",
            APOLLO_ADMIN_URL, app_id
        ))
        .json(&json!({
            "key": "test.key",
            "value": "default_value",
            "comment": "test",
            "dataChangeCreatedBy": "admin"
        }))
        .send()
        .await
        .expect("Create item failed");
    assert!(
        create_item_resp.status().is_success(),
        "Create item failed: {:?}",
        create_item_resp.text().await
    );

    let publish_resp = client
        .post(format!("{}/apps/{}/clusters/default/namespaces/application/releases?name=release1&comment=initial&operator=admin", APOLLO_ADMIN_URL, app_id))
        .send()
        .await
        .expect("Publish failed");
    assert!(
        publish_resp.status().is_success(),
        "Publish failed: {:?}",
        publish_resp.text().await
    );

    let config_resp: Value = client
        .get(format!(
            "{}/configs/{}/default/application",
            APOLLO_CONFIG_URL, app_id
        ))
        .send()
        .await
        .expect("Get config failed")
        .json()
        .await
        .expect("Parse config failed");
    assert_eq!(
        config_resp["configurations"]["test.key"], "default_value",
        "Default config mismatch"
    );

    // Resolve the item's real row id (ids are not stable across runs).
    let items_list: serde_json::Value = client
        .get(format!(
            "{}/apps/{}/clusters/default/namespaces/application/items",
            APOLLO_ADMIN_URL, app_id
        ))
        .send()
        .await
        .expect("List items failed")
        .json()
        .await
        .expect("Parse items failed");
    let item_id = items_list
        .as_array()
        .and_then(|arr| {
            arr.iter()
                .find(|i| i["key"] == "test.key")
                .and_then(|i| i["id"].as_i64())
        })
        .expect("test.key item should exist");

    let update_item_resp = client
        .put(format!(
            "{}/apps/{}/clusters/default/namespaces/application/items/{}",
            APOLLO_ADMIN_URL, app_id, item_id
        ))
        .json(&json!({
            "key": "test.key",
            "value": "gray_value",
            "comment": "gray",
            "dataChangeLastModifiedBy": "admin"
        }))
        .send()
        .await
        .expect("Update item failed");
    assert!(
        update_item_resp.status().is_success(),
        "Update item failed: {:?}",
        update_item_resp.text().await
    );

    let publish_gray_resp = client
        .post(format!("{}/apps/{}/clusters/default/namespaces/application/releases?name=release2&comment=gray&operator=admin", APOLLO_ADMIN_URL, app_id))
        .send()
        .await
        .expect("Publish gray failed");
    assert!(
        publish_gray_resp.status().is_success(),
        "Publish gray failed: {:?}",
        publish_gray_resp.text().await
    );

    let release_info: Value = publish_gray_resp
        .json()
        .await
        .expect("Parse release failed");
    let gray_release_id = release_info["releaseId"].as_i64().expect("No releaseId");

    let create_gray_rule_resp = client
        .post(format!(
            "{}/apps/{}/clusters/default/namespaces/application/gray-release-rules",
            APOLLO_ADMIN_URL, app_id
        ))
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
    assert!(
        create_gray_rule_resp.status().is_success(),
        "Create gray rule failed: {:?}",
        create_gray_rule_resp.text().await
    );

    let config_without_gray: Value = client
        .get(format!(
            "{}/configs/{}/default/application",
            APOLLO_CONFIG_URL, app_id
        ))
        .header("X-Forwarded-For", "10.0.0.1")
        .send()
        .await
        .expect("Get config without gray failed")
        .json()
        .await
        .expect("Parse config without gray failed");
    assert_eq!(
        config_without_gray["configurations"]["test.key"], "gray_value",
        "Config without gray should get latest"
    );

    let config_with_gray: Value = client
        .get(format!(
            "{}/configs/{}/default/application",
            APOLLO_CONFIG_URL, app_id
        ))
        .header("X-Forwarded-For", "192.168.1.100")
        .send()
        .await
        .expect("Get config with gray failed")
        .json()
        .await
        .expect("Parse config with gray failed");
    assert_eq!(
        config_with_gray["configurations"]["test.key"], "gray_value",
        "Config with gray IP should get gray config"
    );
}

#[tokio::test]
async fn test_namespace_lock() {
    if skip_without_server("test_namespace_lock").await {
        return;
    }
    let client = test_http_client();
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
        .post(format!(
            "{}/apps/{}/clusters/default/namespaces",
            APOLLO_ADMIN_URL, app_id
        ))
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
    if !create_ns_resp.status().is_success() {
        let body = create_ns_resp.text().await.unwrap_or_default();
        assert!(
            body.contains("already exists"),
            "Create namespace failed: {}",
            body
        );
    }

    let lock_resp = client
        .post(format!(
            "{}/apps/{}/clusters/default/namespaces/application/lock?lockedBy=user1",
            APOLLO_ADMIN_URL, app_id
        ))
        .send()
        .await
        .expect("Lock namespace failed");
    assert!(
        lock_resp.status().is_success(),
        "Lock namespace failed: {:?}",
        lock_resp.text().await
    );

    let get_lock_resp = client
        .get(format!(
            "{}/apps/{}/clusters/default/namespaces/application/lock",
            APOLLO_ADMIN_URL, app_id
        ))
        .send()
        .await
        .expect("Get lock failed");
    assert!(
        get_lock_resp.status().is_success(),
        "Get lock failed: {:?}",
        get_lock_resp.text().await
    );

    let lock_info: Value = get_lock_resp.json().await.expect("Parse lock info failed");
    assert_eq!(lock_info["lockedBy"], "user1", "Lock owner mismatch");

    let lock_by_another_resp = client
        .post(format!(
            "{}/apps/{}/clusters/default/namespaces/application/lock?lockedBy=user2",
            APOLLO_ADMIN_URL, app_id
        ))
        .send()
        .await
        .expect("Lock by another user failed");
    assert_eq!(
        lock_by_another_resp.status().as_u16(),
        400,
        "Lock by another user should fail"
    );

    let unlock_resp = client
        .delete(format!(
            "{}/apps/{}/clusters/default/namespaces/application/lock?lockedBy=user1",
            APOLLO_ADMIN_URL, app_id
        ))
        .send()
        .await
        .expect("Unlock namespace failed");
    assert!(
        unlock_resp.status().is_success(),
        "Unlock namespace failed: {:?}",
        unlock_resp.text().await
    );

    let get_lock_after_unlock_resp = client
        .get(format!(
            "{}/apps/{}/clusters/default/namespaces/application/lock",
            APOLLO_ADMIN_URL, app_id
        ))
        .send()
        .await
        .expect("Get lock after unlock failed");
    assert_eq!(
        get_lock_after_unlock_resp.status().as_u16(),
        404,
        "Lock should not exist after unlock"
    );
}

#[tokio::test]
async fn test_commit_openapi() {
    if skip_without_server("test_commit_openapi").await {
        return;
    }
    let client = test_http_client();
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
        .post(format!(
            "{}/apps/{}/clusters/default/namespaces",
            APOLLO_ADMIN_URL, app_id
        ))
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
    if !create_ns_resp.status().is_success() {
        let body = create_ns_resp.text().await.unwrap_or_default();
        assert!(
            body.contains("already exists"),
            "Create namespace failed: {}",
            body
        );
    }

    let create_commit_resp = client
        .post(format!(
            "{}/openapi/v1/envs/DEV/apps/{}/clusters/default/namespaces/application/commits",
            BASE_URL, app_id
        ))
        .json(&json!({
            "appId": app_id,
            "clusterName": "default",
            "namespaceName": "application",
            "changeSets": "[{\"key\":\"test.key\",\"value\":\"test.value\"}]",
            "comment": "test commit",
            "dataChangeCreatedBy": "admin"
        }))
        .send()
        .await
        .expect("Create commit failed");
    assert!(
        create_commit_resp.status().is_success(),
        "Create commit failed: {:?}",
        create_commit_resp.text().await
    );

    let commit_info: Value = create_commit_resp
        .json()
        .await
        .expect("Parse commit failed");
    let commit_id = commit_info["id"].as_i64().expect("No commit id") as i32;

    let list_commits_resp: Value = client
        .get(format!(
            "{}/openapi/v1/envs/DEV/apps/{}/clusters/default/namespaces/application/commits",
            BASE_URL, app_id
        ))
        .send()
        .await
        .expect("List commits failed")
        .json()
        .await
        .expect("Parse commits failed");
    assert!(
        !list_commits_resp.as_array().unwrap().is_empty(),
        "Should have at least one commit"
    );

    let get_commit_resp: Value = client
        .get(format!("{}/openapi/v1/commits/{}", BASE_URL, commit_id))
        .send()
        .await
        .expect("Get commit failed")
        .json()
        .await
        .expect("Parse commit failed");
    assert_eq!(get_commit_resp["id"], commit_id, "Commit id mismatch");
}
