// Integration tests for MaintainerClient using wiremock HTTP mock server

use std::collections::HashMap;

use batata_client::HttpClientConfig;
use batata_maintainer_client::MaintainerClient;
use batata_maintainer_client::MaintainerClientConfig;
use batata_maintainer_client::model::{
    AgentCard, AgentCardBasicInfo, AgentDraftCreateForm, AgentDraftUpdateForm, AgentUpdateForm,
    ClusterInfo, ConfigCloneInfo, Instance, McpServerBasicInfo, SameConfigPolicy,
};
use wiremock::matchers::{body_string_contains, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Create a MaintainerClient pointing at the wiremock server using identity auth
async fn make_client(server: &MockServer) -> MaintainerClient {
    let config = HttpClientConfig {
        server_addrs: vec![server.uri()],
        server_identity_key: "serverIdentity".to_string(),
        server_identity_value: "test-identity".to_string(),
        ..Default::default()
    };
    let http_client = batata_client::BatataHttpClient::new_without_auth(config).unwrap();
    MaintainerClient::from_http_client(http_client)
}

fn api_ok<T: serde::Serialize>(data: &T) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "code": 0,
        "message": "success",
        "data": data
    }))
}

fn api_ok_raw(data: serde_json::Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "code": 0,
        "message": "success",
        "data": data
    }))
}

fn page_json<T: serde::Serialize>(items: &[T], total: u64) -> serde_json::Value {
    serde_json::json!({
        "totalCount": total,
        "pageNumber": 1,
        "pagesAvailable": 1,
        "pageItems": items
    })
}

fn member_json(ip: &str, port: u16, state: &str) -> serde_json::Value {
    serde_json::json!({
        "ip": ip, "port": port, "state": state,
        "extendInfo": {}, "address": format!("{}:{}", ip, port),
        "abilities": {
            "remoteAbility": {"supportRemoteConnection": true, "grpcReportEnabled": true},
            "configAbility": {"supportRemoteMetrics": false},
            "namingAbility": {"supportJraft": true}
        },
        "grpcReportEnabled": true, "failAccessCnt": 0
    })
}

// ============================================================================
// Server State / Liveness / Readiness
// ============================================================================

#[tokio::test]
async fn test_server_state() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/core/state"))
        .respond_with(api_ok(
            &serde_json::json!({"stand_alone_mode": "stand_alone"}),
        ))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let state = client.server_state().await.unwrap();
    assert_eq!(
        state.get("stand_alone_mode"),
        Some(&Some("stand_alone".to_string()))
    );
}

#[tokio::test]
async fn test_liveness() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/core/state/liveness"))
        .respond_with(api_ok_raw(serde_json::json!("ok")))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    assert!(client.liveness().await.unwrap());
}

#[tokio::test]
async fn test_readiness() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/core/state/readiness"))
        .respond_with(api_ok_raw(serde_json::json!("ok")))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    assert!(client.readiness().await.unwrap());
}

// ============================================================================
// Core Ops APIs
// ============================================================================

#[tokio::test]
async fn test_raft_ops() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v3/admin/core/ops/raft"))
        .respond_with(api_ok(&"done".to_string()))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let result = client.raft_ops("doSnapshot", "", "naming").await.unwrap();
    assert_eq!(result, "done");
}

#[tokio::test]
async fn test_get_id_generators() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/core/ops/ids"))
        .respond_with(api_ok_raw(serde_json::json!([
            {"resource": "naming", "info": {"currentId": 100, "workerId": 1}}
        ])))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let ids = client.get_id_generators().await.unwrap();
    assert_eq!(ids.len(), 1);
    assert_eq!(ids[0].resource, "naming");
}

#[tokio::test]
async fn test_update_core_log_level() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/v3/admin/core/ops/log"))
        .respond_with(api_ok_raw(serde_json::json!(null)))
        .expect(1)
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    client
        .update_core_log_level("com.alibaba.nacos", "DEBUG")
        .await
        .unwrap();
}

// ============================================================================
// Cluster / Loader APIs
// ============================================================================

#[tokio::test]
async fn test_cluster_members() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/core/cluster/node/list"))
        .respond_with(api_ok_raw(serde_json::json!([member_json(
            "127.0.0.1",
            8848,
            "UP"
        )])))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let members = client.cluster_members().await.unwrap();
    assert_eq!(members.len(), 1);
    assert_eq!(members[0].ip, "127.0.0.1");
    assert_eq!(members[0].state, "UP");
}

#[tokio::test]
async fn test_cluster_members_filtered() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/core/cluster/node/list"))
        .respond_with(api_ok_raw(serde_json::json!([])))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let members = client
        .cluster_members_filtered(Some("192.168.1.1"), Some("UP"))
        .await
        .unwrap();
    assert!(members.is_empty());
}

#[tokio::test]
async fn test_cluster_health() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/core/cluster/node/self/health"))
        .respond_with(api_ok_raw(serde_json::json!({"healthy": true})))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/core/cluster/node/list"))
        .respond_with(api_ok_raw(serde_json::json!([member_json(
            "127.0.0.1",
            8848,
            "UP"
        )])))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let health = client.cluster_health().await.unwrap();
    assert!(health.is_healthy);
    assert!(health.standalone);
    assert_eq!(health.summary.total, 1);
    assert_eq!(health.summary.up, 1);
}

#[tokio::test]
async fn test_cluster_self() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/core/cluster/node/self"))
        .respond_with(api_ok_raw(serde_json::json!({
            "ip": "127.0.0.1",
            "port": 8848,
            "address": "127.0.0.1:8848",
            "state": "UP",
            "extendInfo": {"version": "3.0.0"}
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/core/cluster/node/list"))
        .respond_with(api_ok_raw(serde_json::json!([member_json(
            "127.0.0.1",
            8848,
            "UP"
        )])))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let self_member = client.cluster_self().await.unwrap();
    assert_eq!(self_member.ip, "127.0.0.1");
    assert_eq!(self_member.version, "3.0.0");
    assert!(self_member.is_standalone);
}

#[tokio::test]
async fn test_cluster_member() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/core/cluster/node/list"))
        .respond_with(api_ok_raw(serde_json::json!([member_json(
            "10.0.0.1", 8848, "UP"
        )])))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let member = client.cluster_member("10.0.0.1:8848").await.unwrap();
    assert!(member.is_some());
    assert_eq!(member.unwrap().address, "10.0.0.1:8848");
}

#[tokio::test]
async fn test_cluster_member_count() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/core/cluster/node/list"))
        .respond_with(api_ok_raw(serde_json::json!([])))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let count = client.cluster_member_count().await.unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_update_lookup_mode() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/v3/admin/core/cluster/lookup"))
        .respond_with(api_ok(&true))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let result = client.update_lookup_mode("file").await.unwrap();
    assert!(result);
}

#[tokio::test]
async fn test_get_current_clients() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/core/loader/current"))
        .respond_with(api_ok_raw(serde_json::json!({
            "conn-1": {
                "connectionId": "conn-1",
                "clientIp": "192.168.1.10",
                "createTime": "2025-01-01"
            }
        })))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let clients = client.get_current_clients().await.unwrap();
    assert_eq!(clients.len(), 1);
    assert!(clients.contains_key("conn-1"));
}

#[tokio::test]
async fn test_reload_connection_count() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/core/loader/reloadCurrent"))
        .respond_with(api_ok(&"ok".to_string()))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let result = client
        .reload_connection_count(Some(10), None)
        .await
        .unwrap();
    assert_eq!(result, "ok");
}

#[tokio::test]
async fn test_smart_reload_cluster() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/core/loader/smartReloadCluster"))
        .respond_with(api_ok(&"ok".to_string()))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let result = client.smart_reload_cluster(None).await.unwrap();
    assert_eq!(result, "ok");
}

#[tokio::test]
async fn test_reload_single_client() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/core/loader/reloadClient"))
        .respond_with(api_ok(&"ok".to_string()))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let result = client
        .reload_single_client("conn-1", "10.0.0.2:8848")
        .await
        .unwrap();
    assert_eq!(result, "ok");
}

#[tokio::test]
async fn test_get_cluster_loader_metrics() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/core/loader/cluster"))
        .respond_with(api_ok_raw(serde_json::json!({
            "total": 0,
            "completed": true,
            "detail": []
        })))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let metrics = client.get_cluster_loader_metrics().await.unwrap();
    assert!(metrics.completed);
}

// ============================================================================
// Namespace APIs
// ============================================================================

#[tokio::test]
async fn test_namespace_list() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/core/namespace/list"))
        .respond_with(api_ok_raw(serde_json::json!([
            {
                "namespace": "public",
                "namespaceShowName": "Public",
                "namespaceDesc": "",
                "quota": 200,
                "configCount": 10,
                "type": 0
            }
        ])))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let namespaces = client.namespace_list().await.unwrap();
    assert_eq!(namespaces.len(), 1);
    assert_eq!(namespaces[0].namespace, "public");
}

#[tokio::test]
async fn test_namespace_get() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/core/namespace"))
        .respond_with(api_ok_raw(serde_json::json!({
            "namespace": "test-ns",
            "namespaceShowName": "Test NS",
            "namespaceDesc": "test",
            "quota": 200,
            "configCount": 0,
            "type": 2
        })))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let ns = client.namespace_get("test-ns").await.unwrap();
    assert_eq!(ns.namespace, "test-ns");
    assert_eq!(ns.namespace_show_name, "Test NS");
}

#[tokio::test]
async fn test_namespace_create() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v3/admin/core/namespace"))
        .respond_with(api_ok(&true))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let result = client
        .namespace_create("ns-1", "Namespace 1", "desc")
        .await
        .unwrap();
    assert!(result);
}

#[tokio::test]
async fn test_namespace_update() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/v3/admin/core/namespace"))
        .respond_with(api_ok(&true))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let result = client
        .namespace_update("ns-1", "Updated NS", "new desc")
        .await
        .unwrap();
    assert!(result);
}

#[tokio::test]
async fn test_namespace_delete() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/v3/admin/core/namespace"))
        .respond_with(api_ok(&true))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let result = client.namespace_delete("ns-1").await.unwrap();
    assert!(result);
}

#[tokio::test]
async fn test_namespace_exists() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/core/namespace/check"))
        // The admin endpoint answers with the tenant row count, not a boolean.
        .respond_with(api_ok(&1))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let exists = client.namespace_exists("ns-1").await.unwrap();
    assert!(exists);
}

// ============================================================================
// Config CRUD APIs
// ============================================================================

#[tokio::test]
async fn test_config_get() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/cs/config"))
        .respond_with(api_ok_raw(serde_json::json!({
            "id": 1,
            "dataId": "test.yml",
            "groupName": "DEFAULT_GROUP",
            "namespaceId": "public",
            "md5": "abc123",
            "type": "yaml",
            "appName": "",
            "createTime": 0,
            "modifyTime": 0,
            "content": "key: value",
            "desc": "",
            "encryptedDataKey": "",
            "createUser": "",
            "createIp": "",
            "configTags": ""
        })))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let config = client
        .config_get("test.yml", "DEFAULT_GROUP", "public")
        .await
        .unwrap();
    let config = config.unwrap();
    assert_eq!(config.config_basic_info.data_id, "test.yml");
    assert_eq!(config.content, "key: value");
}

#[tokio::test]
async fn test_config_get_not_found() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/cs/config"))
        .respond_with(api_ok_raw(serde_json::json!(null)))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let config = client
        .config_get("nonexistent", "DEFAULT_GROUP", "")
        .await
        .unwrap();
    assert!(config.is_none());
}

#[tokio::test]
async fn test_config_search() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/cs/config/list"))
        .respond_with(api_ok_raw(page_json(
            &[serde_json::json!({
                "id": 1,
                "dataId": "app.yml",
                "groupName": "DEFAULT_GROUP",
                "namespaceId": "",
                "md5": "",
                "type": "",
                "appName": "",
                "createTime": 0,
                "modifyTime": 0
            })],
            1,
        )))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let page = client
        .config_search(1, 10, "", "", "DEFAULT_GROUP", "", "", "", "")
        .await
        .unwrap();
    assert_eq!(page.total_count, 1);
    assert_eq!(page.page_items.len(), 1);
    assert_eq!(page.page_items[0].data_id, "app.yml");
}

#[tokio::test]
async fn test_config_search_by_detail() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/cs/config/searchDetail"))
        .respond_with(api_ok_raw(page_json(&[] as &[serde_json::Value], 0)))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let page = client
        .config_search_by_detail("", "", "", "blur", "", "", "", "", 1, 10)
        .await
        .unwrap();
    assert_eq!(page.total_count, 0);
}

#[tokio::test]
async fn test_config_publish() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v3/admin/cs/config"))
        .respond_with(api_ok(&true))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let result = client
        .config_publish(
            "app.yml",
            "DEFAULT_GROUP",
            "",
            "key: value",
            "",
            "",
            "",
            "",
            "",
            "yaml",
            "",
            "",
        )
        .await
        .unwrap();
    assert!(result);
}

#[tokio::test]
async fn test_config_update_metadata() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/v3/admin/cs/config/metadata"))
        .respond_with(api_ok(&true))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let result = client
        .config_update_metadata("app.yml", "DEFAULT_GROUP", "", "desc", "tag1,tag2")
        .await
        .unwrap();
    assert!(result);
}

#[tokio::test]
async fn test_config_delete() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/v3/admin/cs/config"))
        .respond_with(api_ok(&true))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let result = client
        .config_delete("app.yml", "DEFAULT_GROUP", "")
        .await
        .unwrap();
    assert!(result);
}

#[tokio::test]
async fn test_config_batch_delete() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/v3/admin/cs/config"))
        .respond_with(api_ok(&true))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let result = client.config_batch_delete(&[1, 2, 3]).await.unwrap();
    assert!(result);
}

#[tokio::test]
async fn test_config_clone() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v3/admin/cs/config/clone"))
        .respond_with(api_ok_raw(serde_json::json!({"succCount": 2})))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let clone_infos = vec![ConfigCloneInfo {
        config_id: 1,
        target_group_name: None,
        target_data_id: None,
    }];
    let result = client
        .config_clone("target-ns", &clone_infos, "admin", SameConfigPolicy::Abort)
        .await
        .unwrap();
    assert!(result.contains_key("succCount"));
}

#[tokio::test]
async fn test_config_export() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/cs/config/export"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![0x50, 0x4b, 0x03, 0x04]))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let bytes = client.config_export("", None, None, None).await.unwrap();
    assert_eq!(bytes, vec![0x50, 0x4b, 0x03, 0x04]);
}

#[tokio::test]
async fn test_config_import() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v3/admin/cs/config/import"))
        .respond_with(api_ok_raw(serde_json::json!({
            "successCount": 3,
            "skipCount": 0,
            "failCount": 0,
            "failData": []
        })))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let result = client
        .config_import(vec![0x50, 0x4b], "", SameConfigPolicy::Skip)
        .await
        .unwrap();
    assert_eq!(result.success_count, 3);
}

// ============================================================================
// Config Beta APIs
// ============================================================================

#[tokio::test]
async fn test_config_get_beta() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/cs/config/beta"))
        .respond_with(api_ok_raw(serde_json::json!({
            "configDetailInfo": {
                "id": 1,
                "dataId": "app.yml",
                "groupName": "DEFAULT_GROUP",
                "namespaceId": "",
                "md5": "",
                "type": "",
                "appName": "",
                "createTime": 0,
                "modifyTime": 0,
                "content": "beta: true",
                "desc": "",
                "encryptedDataKey": "",
                "createUser": "",
                "createIp": "",
                "configTags": ""
            },
            "grayName": "beta",
            "grayRule": ""
        })))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let beta = client
        .config_get_beta("app.yml", "DEFAULT_GROUP", "")
        .await
        .unwrap();
    let beta = beta.unwrap();
    assert_eq!(beta.gray_name, "beta");
    assert_eq!(beta.config_detail_info.content, "beta: true");
}

#[tokio::test]
async fn test_config_publish_beta() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v3/admin/cs/config/beta"))
        .respond_with(api_ok(&true))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let result = client
        .config_publish_beta(
            "app.yml",
            "DEFAULT_GROUP",
            "",
            "beta: true",
            "",
            "admin",
            "",
            "",
            "yaml",
            "10.0.0.1",
        )
        .await
        .unwrap();
    assert!(result);
}

#[tokio::test]
async fn test_config_stop_beta() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/v3/admin/cs/config/beta"))
        .respond_with(api_ok(&true))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let result = client
        .config_stop_beta("app.yml", "DEFAULT_GROUP", "")
        .await
        .unwrap();
    assert!(result);
}

// ============================================================================
// Config History APIs
// ============================================================================

#[tokio::test]
async fn test_history_get() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/cs/history"))
        .respond_with(api_ok_raw(serde_json::json!({
            "id": 100,
            "dataId": "app.yml",
            "groupName": "DEFAULT_GROUP",
            "namespaceId": "",
            "md5": "",
            "type": "",
            "appName": "",
            "createTime": 0,
            "modifyTime": 0,
            "srcIp": "127.0.0.1",
            "srcUser": "admin",
            "opType": "U",
            "publishType": "",
            "content": "old: value",
            "encryptedDataKey": "",
            "grayName": "",
            "extInfo": ""
        })))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let history = client
        .history_get(100, "app.yml", "DEFAULT_GROUP", "")
        .await
        .unwrap();
    let history = history.unwrap();
    assert_eq!(
        history.config_history_basic_info.config_basic_info.data_id,
        "app.yml"
    );
    assert_eq!(history.content, "old: value");
}

#[tokio::test]
async fn test_history_list() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/cs/history/list"))
        .respond_with(api_ok_raw(page_json(
            &[serde_json::json!({
                "id": 1,
                "dataId": "app.yml",
                "groupName": "DEFAULT_GROUP",
                "namespaceId": "",
                "md5": "",
                "type": "",
                "appName": "",
                "createTime": 0,
                "modifyTime": 0,
                "srcIp": "",
                "srcUser": "",
                "opType": "I",
                "publishType": ""
            })],
            1,
        )))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let page = client
        .history_list("app.yml", "DEFAULT_GROUP", "", 1, 10)
        .await
        .unwrap();
    assert_eq!(page.total_count, 1);
    assert_eq!(page.page_items[0].op_type, "I");
}

#[tokio::test]
async fn test_history_configs_by_namespace() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/cs/history/configs"))
        .respond_with(api_ok_raw(serde_json::json!([{
            "id": 1,
            "dataId": "a.yml",
            "groupName": "DEFAULT_GROUP",
            "namespaceId": "ns1",
            "md5": "",
            "type": "",
            "appName": "",
            "createTime": 0,
            "modifyTime": 0
        }])))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let configs = client.history_configs_by_namespace("ns1").await.unwrap();
    assert_eq!(configs.len(), 1);
    assert_eq!(configs[0].data_id, "a.yml");
}

#[tokio::test]
async fn test_history_get_previous() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/cs/history/previous"))
        .respond_with(api_ok_raw(serde_json::json!(null)))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let prev = client
        .history_get_previous(1, "app.yml", "DEFAULT_GROUP", "")
        .await
        .unwrap();
    assert!(prev.is_none());
}

// ============================================================================
// Config Listener APIs
// ============================================================================

#[tokio::test]
async fn test_config_listeners() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/cs/listener"))
        .respond_with(api_ok_raw(serde_json::json!({
            "queryType": "config",
            "listenersStatus": {"10.0.0.1": "md5-hash"}
        })))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let info = client
        .config_listeners("app.yml", "DEFAULT_GROUP", "")
        .await
        .unwrap();
    assert_eq!(info.query_type, "config");
    assert_eq!(info.listeners_status.len(), 1);
}

#[tokio::test]
async fn test_config_listeners_with_aggregation() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/cs/listener"))
        .respond_with(api_ok_raw(serde_json::json!({
            "queryType": "config",
            "listenersStatus": {}
        })))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let info = client
        .config_listeners_with_aggregation("app.yml", "DEFAULT_GROUP", "", true)
        .await
        .unwrap();
    assert_eq!(info.query_type, "config");
}

#[tokio::test]
async fn test_config_listeners_by_ip() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/cs/listener"))
        .respond_with(api_ok_raw(serde_json::json!({
            "queryType": "ip",
            "listenersStatus": {}
        })))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let info = client
        .config_listeners_by_ip("10.0.0.1", true, "", false)
        .await
        .unwrap();
    assert_eq!(info.query_type, "ip");
}

// ============================================================================
// Config Ops APIs
// ============================================================================

#[tokio::test]
async fn test_config_update_local_cache() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v3/admin/cs/ops/localCache"))
        .respond_with(api_ok(&"ok".to_string()))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let result = client.config_update_local_cache().await.unwrap();
    assert_eq!(result, "ok");
}

#[tokio::test]
async fn test_config_set_log_level() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/v3/admin/cs/ops/log"))
        .respond_with(api_ok(&"ok".to_string()))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let result = client
        .config_set_log_level("com.alibaba.nacos.config", "DEBUG")
        .await
        .unwrap();
    assert_eq!(result, "ok");
}

// ============================================================================
// Service APIs
// ============================================================================

#[tokio::test]
async fn test_service_list() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ns/service/list"))
        .respond_with(api_ok_raw(page_json(
            &[serde_json::json!({
                "serviceName": "my-service",
                "groupName": "DEFAULT_GROUP",
                "namespaceId": "",
                "clusterCount": 1,
                "ipCount": 2,
                "healthyInstanceCount": 2,
                "triggerFlag": ""
            })],
            1,
        )))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let page = client
        .service_list("", "DEFAULT_GROUP", "", 1, 10)
        .await
        .unwrap();
    assert_eq!(page.total_count, 1);
    assert_eq!(page.page_items[0].service_name, "my-service");
}

/// Upstream `ServiceView` — and Batata's admin service list, which mirrors it —
/// names the service `name`, not `serviceName`.
#[tokio::test]
async fn test_service_list_reads_name_field() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ns/service/list"))
        .respond_with(api_ok_raw(page_json(
            &[serde_json::json!({
                "name": "my-service",
                "groupName": "DEFAULT_GROUP",
                "clusterCount": 1,
                "ipCount": 2,
                "healthyInstanceCount": 2,
                "triggerFlag": ""
            })],
            1,
        )))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let page = client
        .service_list("", "DEFAULT_GROUP", "", 1, 10)
        .await
        .unwrap();
    assert_eq!(page.page_items[0].service_name, "my-service");
}

#[tokio::test]
async fn test_service_list_with_options() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ns/service/list"))
        .respond_with(api_ok_raw(page_json(&[] as &[serde_json::Value], 0)))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let page = client
        .service_list_with_options("", "", "", true, 1, 10)
        .await
        .unwrap();
    assert_eq!(page.total_count, 0);
}

#[tokio::test]
async fn test_service_list_with_detail() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ns/service/list/withDetail"))
        .respond_with(api_ok_raw(page_json(
            &[serde_json::json!({
                "serviceName": "svc",
                "groupName": "DEFAULT_GROUP",
                "namespaceId": "",
                "protectThreshold": 0.0,
                "metadata": {},
                "selector": null,
                "clusterMap": {}
            })],
            1,
        )))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let page = client
        .service_list_with_detail("", "", "", 1, 10)
        .await
        .unwrap();
    assert_eq!(page.total_count, 1);
}

#[tokio::test]
async fn test_service_get() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ns/service"))
        .respond_with(api_ok_raw(serde_json::json!({
            "serviceName": "my-service",
            "groupName": "DEFAULT_GROUP",
            "namespaceId": "",
            "protectThreshold": 0.8,
            "metadata": {},
            "selector": null,
            "clusterMap": {}
        })))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let svc = client
        .service_get("", "DEFAULT_GROUP", "my-service")
        .await
        .unwrap();
    assert_eq!(svc.service_name, "my-service");
}

#[tokio::test]
async fn test_service_create() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v3/admin/ns/service"))
        .respond_with(api_ok(&"ok".to_string()))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let result = client
        .service_create("", "DEFAULT_GROUP", "new-svc", 0.8, "", "", false)
        .await
        .unwrap();
    assert_eq!(result, "ok");
}

/// The admin naming endpoints are form-encoded: a JSON body is answered with
/// 415, which is what a separated console hit when it created a service.
#[tokio::test]
async fn test_service_create_is_form_encoded() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v3/admin/ns/service"))
        .and(body_string_contains("serviceName=new-svc"))
        .respond_with(api_ok(&"ok".to_string()))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let result = client
        .service_create("", "DEFAULT_GROUP", "new-svc", 0.8, "", "", false)
        .await
        .unwrap();
    assert_eq!(result, "ok");
}

/// Upstream's console forwards `ServiceForm.ephemeral` to the core on create
/// (`ConsoleServiceController.createService` -> `ServiceMetadata.setEphemeral`,
/// and `ServiceRemoteHandler` re-sends it when the console is deployed
/// separately), so the separated console has to put it in the form body.
#[tokio::test]
async fn test_service_create_forwards_ephemeral() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v3/admin/ns/service"))
        .and(body_string_contains("ephemeral=true"))
        .respond_with(api_ok(&"ok".to_string()))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let result = client
        .service_create("", "DEFAULT_GROUP", "new-svc", 0.8, "", "", true)
        .await
        .unwrap();
    assert_eq!(result, "ok");
}

/// Upstream answers service CRUD with the string `"ok"`; Batata's own admin API
/// answers with `true`. Both have to read as success.
#[tokio::test]
async fn test_service_create_accepts_boolean_payload() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v3/admin/ns/service"))
        .respond_with(api_ok_raw(serde_json::json!(true)))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let result = client
        .service_create("", "DEFAULT_GROUP", "new-svc", 0.8, "", "", false)
        .await
        .unwrap();
    assert_eq!(result, "ok");
}

#[tokio::test]
async fn test_service_update() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/v3/admin/ns/service"))
        .respond_with(api_ok(&"ok".to_string()))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let result = client
        .service_update("", "DEFAULT_GROUP", "my-svc", 0.9, "", "")
        .await
        .unwrap();
    assert_eq!(result, "ok");
}

#[tokio::test]
async fn test_service_delete() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/v3/admin/ns/service"))
        .respond_with(api_ok(&"ok".to_string()))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let result = client
        .service_delete("", "DEFAULT_GROUP", "my-svc")
        .await
        .unwrap();
    assert_eq!(result, "ok");
}

#[tokio::test]
async fn test_service_subscribers() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ns/client/subscribe/list"))
        .respond_with(api_ok_raw(page_json(
            &[serde_json::json!({
                "namespaceId": "",
                "groupName": "DEFAULT_GROUP",
                "serviceName": "my-svc",
                "ip": "10.0.0.1",
                "port": 0,
                "agent": "Nacos-Java-Client:v2.4.0",
                "appName": "my-app"
            })],
            1,
        )))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let page = client
        .service_subscribers("", "DEFAULT_GROUP", "my-svc", 1, 10)
        .await
        .unwrap();
    assert_eq!(page.total_count, 1);
}

#[tokio::test]
async fn test_service_selector_types() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ns/service/selector/types"))
        .respond_with(api_ok_raw(serde_json::json!(["none", "label"])))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let types = client.service_selector_types().await.unwrap();
    assert_eq!(types, vec!["none", "label"]);
}

// ============================================================================
// Instance APIs
// ============================================================================

fn instance_json(ip: &str, port: i32) -> serde_json::Value {
    serde_json::json!({
        "ip": ip,
        "port": port,
        "weight": 1.0,
        "healthy": true,
        "enabled": true,
        "ephemeral": true,
        "clusterName": "DEFAULT",
        "serviceName": "my-svc",
        "metadata": {},
        "instanceHeartBeatInterval": 5000,
        "instanceHeartBeatTimeOut": 15000,
        "ipDeleteTimeout": 30000
    })
}

#[tokio::test]
async fn test_instance_list() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ns/instance/list"))
        .respond_with(api_ok_raw(serde_json::json!([instance_json(
            "10.0.0.1", 8080
        )])))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let instances = client
        .instance_list("", "DEFAULT_GROUP", "my-svc", "")
        .await
        .unwrap();
    assert_eq!(instances.len(), 1);
    assert_eq!(instances[0].ip, "10.0.0.1");
    assert_eq!(instances[0].port, 8080);
    assert!(instances[0].healthy);
}

#[tokio::test]
async fn test_instance_list_with_healthy_filter() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ns/instance/list"))
        .respond_with(api_ok_raw(serde_json::json!([])))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let instances = client
        .instance_list_with_healthy_filter("", "DEFAULT_GROUP", "my-svc", "", true)
        .await
        .unwrap();
    assert!(instances.is_empty());
}

#[tokio::test]
async fn test_instance_detail() {
    let server = MockServer::start().await;
    let mut inst = instance_json("10.0.0.1", 8080);
    inst["metadata"] = serde_json::json!({"env": "prod"});
    Mock::given(method("GET"))
        .and(path("/v3/admin/ns/instance"))
        .respond_with(api_ok_raw(inst))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let instance = client
        .instance_detail("", "DEFAULT_GROUP", "my-svc", "10.0.0.1", 8080, "DEFAULT")
        .await
        .unwrap();
    assert_eq!(instance.ip, "10.0.0.1");
    assert_eq!(instance.metadata.get("env").unwrap(), "prod");
}

#[tokio::test]
async fn test_instance_register() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v3/admin/ns/instance"))
        .respond_with(api_ok(&"ok".to_string()))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let instance = Instance {
        ip: "10.0.0.2".to_string(),
        port: 9090,
        weight: 1.0,
        enabled: true,
        healthy: true,
        ephemeral: true,
        cluster_name: "DEFAULT".to_string(),
        ..Default::default()
    };
    let result = client
        .instance_register("", "DEFAULT_GROUP", "my-svc", &instance)
        .await
        .unwrap();
    assert_eq!(result, "ok");
}

#[tokio::test]
async fn test_instance_deregister() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/v3/admin/ns/instance"))
        .respond_with(api_ok(&"ok".to_string()))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let instance = Instance {
        ip: "10.0.0.2".to_string(),
        port: 9090,
        ephemeral: true,
        ..Default::default()
    };
    let result = client
        .instance_deregister("", "DEFAULT_GROUP", "my-svc", &instance)
        .await
        .unwrap();
    assert_eq!(result, "ok");
}

#[tokio::test]
async fn test_instance_update() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/v3/admin/ns/instance"))
        .respond_with(api_ok(&"ok".to_string()))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let result = client
        .instance_update(
            "",
            "DEFAULT_GROUP",
            "my-svc",
            "10.0.0.1",
            8080,
            "DEFAULT",
            2.0,
            true,
            "",
        )
        .await
        .unwrap();
    assert_eq!(result, "ok");
}

#[tokio::test]
async fn test_instance_partial_update() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/v3/admin/ns/instance/partial"))
        .respond_with(api_ok(&"ok".to_string()))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let instance = Instance {
        ip: "10.0.0.1".to_string(),
        port: 8080,
        weight: 2.0,
        enabled: false,
        ..Default::default()
    };
    let result = client
        .instance_partial_update("", "DEFAULT_GROUP", "my-svc", &instance)
        .await
        .unwrap();
    assert_eq!(result, "ok");
}

#[tokio::test]
async fn test_instance_batch_update_metadata() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/v3/admin/ns/instance/metadata/batch"))
        .respond_with(api_ok_raw(serde_json::json!({"updated": []})))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let mut metadata = HashMap::new();
    metadata.insert("env".to_string(), "staging".to_string());
    let result = client
        .instance_batch_update_metadata(
            "",
            "DEFAULT_GROUP",
            "my-svc",
            "[{\"ip\":\"10.0.0.1\",\"port\":8080}]",
            &metadata,
        )
        .await
        .unwrap();
    assert!(result.updated.is_empty());
}

#[tokio::test]
async fn test_instance_batch_delete_metadata() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/v3/admin/ns/instance/metadata/batch"))
        .respond_with(api_ok_raw(serde_json::json!({"updated": []})))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let mut metadata = HashMap::new();
    metadata.insert("env".to_string(), "".to_string());
    let result = client
        .instance_batch_delete_metadata(
            "",
            "DEFAULT_GROUP",
            "my-svc",
            "[{\"ip\":\"10.0.0.1\",\"port\":8080}]",
            &metadata,
        )
        .await
        .unwrap();
    assert!(result.updated.is_empty());
}

// ============================================================================
// Naming Health APIs
// ============================================================================

#[tokio::test]
async fn test_update_instance_health_status() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/v3/admin/ns/health/instance"))
        .respond_with(api_ok(&"ok".to_string()))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let result = client
        .update_instance_health_status("", "DEFAULT_GROUP", "my-svc", "10.0.0.1", 8080, false)
        .await
        .unwrap();
    assert_eq!(result, "ok");
}

#[tokio::test]
async fn test_get_health_checkers() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ns/health/checkers"))
        .respond_with(api_ok_raw(serde_json::json!({
            "TCP": {"type": "TCP"},
            "HTTP": {"type": "HTTP", "path": "/health"}
        })))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let checkers = client.get_health_checkers().await.unwrap();
    assert!(checkers.contains_key("TCP"));
    assert!(checkers.contains_key("HTTP"));
}

// ============================================================================
// Naming Cluster APIs
// ============================================================================

#[tokio::test]
async fn test_update_naming_cluster() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/v3/admin/ns/cluster"))
        .respond_with(api_ok(&"ok".to_string()))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let cluster = ClusterInfo {
        cluster_name: "DEFAULT".to_string(),
        health_checker: Some(serde_json::json!({"type": "TCP"})),
        metadata: HashMap::new(),
        ..Default::default()
    };
    let result = client
        .update_naming_cluster("", "DEFAULT_GROUP", "my-svc", &cluster)
        .await
        .unwrap();
    assert_eq!(result, "ok");
}

// ============================================================================
// Naming Ops APIs
// ============================================================================

#[tokio::test]
async fn test_naming_metrics() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ns/ops/metrics"))
        .respond_with(api_ok_raw(serde_json::json!({
            "status": "UP",
            "serviceCount": 5,
            "instanceCount": 10,
            "subscribeCount": 3,
            "clientCount": 2,
            "connectionBasedClientCount": 2,
            "ephemeralIpPortClientCount": 0,
            "persistentIpPortClientCount": 0,
            "responsibleClientCount": 2
        })))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let metrics = client.naming_metrics(false).await.unwrap();
    assert_eq!(metrics.status, Some("UP".to_string()));
    assert_eq!(metrics.service_count, Some(5));
    assert_eq!(metrics.instance_count, Some(10));
}

#[tokio::test]
async fn test_naming_set_log_level() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/v3/admin/ns/ops/log"))
        .respond_with(api_ok(&"ok".to_string()))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let result = client
        .naming_set_log_level("com.alibaba.nacos.naming", "WARN")
        .await
        .unwrap();
    assert_eq!(result, "ok");
}

// ============================================================================
// Naming Client APIs
// ============================================================================

#[tokio::test]
async fn test_client_list() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ns/client/list"))
        .respond_with(api_ok_raw(serde_json::json!(["conn-1", "conn-2"])))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let clients = client.client_list().await.unwrap();
    assert_eq!(clients, vec!["conn-1", "conn-2"]);
}

#[tokio::test]
async fn test_client_detail() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ns/client"))
        .respond_with(api_ok_raw(serde_json::json!({
            "clientId": "conn-1",
            "clientIp": "192.168.1.10",
            "connectType": "grpc",
            "clientType": "naming",
            "lastUpdatedTime": 1704067200000_i64,
            "isEphemeral": true
        })))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let detail = client.client_detail("conn-1").await.unwrap();
    assert_eq!(detail.client_id, "conn-1");
    assert_eq!(detail.client_ip, "192.168.1.10");
}

#[tokio::test]
async fn test_client_published_services() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ns/client/publish/list"))
        .respond_with(api_ok_raw(serde_json::json!([
            {
                "namespace": "",
                "group": "DEFAULT_GROUP",
                "serviceName": "svc-1",
                "registeredInstance": {"ip": "10.0.0.1", "port": 8080}
            }
        ])))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let services = client.client_published_services("conn-1").await.unwrap();
    assert_eq!(services.len(), 1);
    assert_eq!(services[0].service_name, "svc-1");
}

#[tokio::test]
async fn test_client_subscribed_services() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ns/client/subscribe/list"))
        .respond_with(api_ok_raw(serde_json::json!([
            {
                "namespace": "",
                "group": "DEFAULT_GROUP",
                "serviceName": "svc-2",
                "registeredInstance": null
            }
        ])))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let services = client.client_subscribed_services("conn-1").await.unwrap();
    assert_eq!(services.len(), 1);
    assert_eq!(services[0].service_name, "svc-2");
}

#[tokio::test]
async fn test_service_published_clients() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ns/client/service/publisher/list"))
        .respond_with(api_ok_raw(serde_json::json!([
            {"clientId": "conn-1", "ip": "10.0.0.1", "port": 8080}
        ])))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let publishers = client
        .service_published_clients("", "DEFAULT_GROUP", "svc-1", None, None)
        .await
        .unwrap();
    assert_eq!(publishers.len(), 1);
    assert_eq!(publishers[0].client_id, "conn-1");
}

#[tokio::test]
async fn test_service_subscribed_clients() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ns/client/service/subscriber/list"))
        .respond_with(api_ok_raw(serde_json::json!([
            {"clientId": "conn-2", "ip": "10.0.0.2", "port": 0}
        ])))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let subscribers = client
        .service_subscribed_clients("", "DEFAULT_GROUP", "svc-1", None, None)
        .await
        .unwrap();
    assert_eq!(subscribers.len(), 1);
    assert_eq!(subscribers[0].client_id, "conn-2");
}

// ============================================================================
// AI MCP APIs
// ============================================================================

#[tokio::test]
async fn test_mcp_server_list() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ai/mcp/list"))
        .respond_with(api_ok_raw(page_json(
            &[serde_json::json!({
                "name": "my-mcp",
                "version": "1.0",
                "description": "test mcp"
            })],
            1,
        )))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let page = client.mcp_server_list("", "my-mcp", 1, 10).await.unwrap();
    assert_eq!(page.total_count, 1);
}

#[tokio::test]
async fn test_mcp_server_search() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ai/mcp/list"))
        .respond_with(api_ok_raw(page_json(&[] as &[serde_json::Value], 0)))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let page = client
        .mcp_server_search("", "pattern", 1, 10)
        .await
        .unwrap();
    assert_eq!(page.total_count, 0);
}

#[tokio::test]
async fn test_mcp_server_detail() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ai/mcp"))
        .respond_with(api_ok_raw(serde_json::json!({
            "name": "my-mcp",
            "version": "1.0",
            "description": "test",
            "toolSpec": null,
            "allVersions": [],
            "namespaceId": ""
        })))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let detail = client
        .mcp_server_detail("", "my-mcp", "mcp-1", "1.0")
        .await
        .unwrap();
    assert_eq!(detail.basic_info.name, "my-mcp");
}

#[tokio::test]
async fn test_mcp_server_create() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v3/admin/ai/mcp"))
        .respond_with(api_ok(&"created".to_string()))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let spec = McpServerBasicInfo {
        name: "new-mcp".to_string(),
        version: "1.0".to_string(),
        ..Default::default()
    };
    let result = client
        .mcp_server_create("", "new-mcp", &spec, None, None)
        .await
        .unwrap();
    assert_eq!(result, "created");
}

#[tokio::test]
async fn test_mcp_server_update() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/v3/admin/ai/mcp"))
        .respond_with(api_ok_raw(serde_json::json!(null)))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let spec = McpServerBasicInfo {
        name: "my-mcp".to_string(),
        version: "2.0".to_string(),
        ..Default::default()
    };
    let result = client
        .mcp_server_update("", "my-mcp", true, &spec, None, None, false)
        .await
        .unwrap();
    assert!(result);
}

#[tokio::test]
async fn test_mcp_server_delete() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/v3/admin/ai/mcp"))
        .respond_with(api_ok_raw(serde_json::json!(null)))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let result = client
        .mcp_server_delete("", "my-mcp", "mcp-1", "1.0")
        .await
        .unwrap();
    assert!(result);
}

// ----------------------------------------------------------------------------
// MCP server version lifecycle (GA maintainer contract)
// ----------------------------------------------------------------------------

fn version_detail_json() -> serde_json::Value {
    serde_json::json!({
        "version": "1.0",
        "status": "draft",
        "latest": false,
        "namespaceId": "",
        "mcpName": "my-mcp",
        "serverSpecification": { "name": "my-mcp" }
    })
}

#[tokio::test]
async fn test_mcp_server_list_versions() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ai/mcp/versions"))
        .and(query_param("namespaceId", ""))
        .and(query_param("mcpName", "my-mcp"))
        .and(query_param("status", "draft"))
        .and(query_param("pageNo", "2"))
        .and(query_param("pageSize", "5"))
        .respond_with(api_ok_raw(page_json(&[version_detail_json()], 1)))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let page = client
        .mcp_server_list_versions("", "my-mcp", Some("draft"), 2, 5)
        .await
        .unwrap();
    assert_eq!(page.total_count, 1);
    assert_eq!(page.page_items.len(), 1);
    assert_eq!(page.page_items[0].version, "1.0");
    assert_eq!(page.page_items[0].status, "draft");
    assert!(
        page.page_items[0].extra.contains_key("serverSpecification"),
        "nested specification fields must be preserved"
    );
}

#[tokio::test]
async fn test_mcp_server_list_versions_omits_empty_status() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ai/mcp/versions"))
        .and(query_param("mcpName", "my-mcp"))
        .respond_with(api_ok_raw(page_json(&[] as &[serde_json::Value], 0)))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let page = client
        .mcp_server_list_versions("", "my-mcp", None, 1, 20)
        .await
        .unwrap();
    assert_eq!(page.total_count, 0);
}

#[tokio::test]
async fn test_mcp_server_get_version() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ai/mcp/version"))
        .and(query_param("namespaceId", ""))
        .and(query_param("mcpName", "my-mcp"))
        .and(query_param("version", "1.0"))
        .respond_with(api_ok_raw(version_detail_json()))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let detail = client
        .mcp_server_get_version("", "my-mcp", "1.0")
        .await
        .unwrap();
    assert_eq!(detail.version, "1.0");
    assert_eq!(detail.status, "draft");
    assert_eq!(detail.mcp_name, "my-mcp");
}

/// Every transition must hit its own ADMIN path with the exact GA query shape.
/// A wrong path or a dropped query parameter turns the wiremock request into a
/// 404, so this is also the mutant guard for `mcp_server_transition`.
#[tokio::test]
async fn test_mcp_server_version_transitions() {
    let server = MockServer::start().await;
    let paths = [
        "/v3/admin/ai/mcp/submit",
        "/v3/admin/ai/mcp/publish",
        "/v3/admin/ai/mcp/force-publish",
        "/v3/admin/ai/mcp/redraft",
        "/v3/admin/ai/mcp/online",
        "/v3/admin/ai/mcp/offline",
    ];
    for route_path in paths {
        Mock::given(method("POST"))
            .and(path(route_path))
            .and(query_param("namespaceId", ""))
            .and(query_param("mcpName", "my-mcp"))
            .and(query_param("version", "1.0"))
            .respond_with(api_ok_raw(version_detail_json()))
            .mount(&server)
            .await;
    }

    let client = make_client(&server).await;
    let results = [
        (
            "submit",
            client.mcp_server_submit_version("", "my-mcp", "1.0").await,
        ),
        (
            "publish",
            client.mcp_server_publish_version("", "my-mcp", "1.0").await,
        ),
        (
            "force-publish",
            client
                .mcp_server_force_publish_version("", "my-mcp", "1.0")
                .await,
        ),
        (
            "redraft",
            client.mcp_server_redraft_version("", "my-mcp", "1.0").await,
        ),
        (
            "online",
            client.mcp_server_online_version("", "my-mcp", "1.0").await,
        ),
        (
            "offline",
            client.mcp_server_offline_version("", "my-mcp", "1.0").await,
        ),
    ];
    for (name, result) in results {
        let detail = result.unwrap_or_else(|e| panic!("transition {name} failed: {e}"));
        assert_eq!(detail.version, "1.0", "transition {name}");
        assert_eq!(detail.status, "draft", "transition {name}");
    }
}

// ============================================================================
// AI Agent (A2A) APIs
// ============================================================================

#[tokio::test]
async fn test_agent_register() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v3/admin/ai/a2a"))
        .respond_with(api_ok_raw(serde_json::json!(null)))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let agent = AgentCard {
        basic_info: AgentCardBasicInfo {
            name: "my-agent".to_string(),
            ..Default::default()
        },
        ..Default::default()
    };
    let result = client.agent_register(&agent, "", "direct").await.unwrap();
    assert!(result);
}

#[tokio::test]
async fn test_agent_get() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ai/a2a"))
        .respond_with(api_ok_raw(serde_json::json!({
            "name": "my-agent",
            "registrationType": "direct"
        })))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let detail = client.agent_get("my-agent", "", "direct").await.unwrap();
    assert_eq!(detail.agent_card.basic_info.name, "my-agent");
    assert_eq!(detail.registration_type, "direct");
}

#[tokio::test]
async fn test_agent_update() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/v3/admin/ai/a2a"))
        .respond_with(api_ok_raw(serde_json::json!(null)))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let agent = AgentCard {
        basic_info: AgentCardBasicInfo {
            name: "my-agent".to_string(),
            ..Default::default()
        },
        ..Default::default()
    };
    let result = client
        .agent_update(&agent, "", true, "direct")
        .await
        .unwrap();
    assert!(result);
}

#[tokio::test]
async fn test_agent_delete() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/v3/admin/ai/a2a"))
        .respond_with(api_ok_raw(serde_json::json!(null)))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let result = client.agent_delete("my-agent", "", "1.0").await.unwrap();
    assert!(result);
}

#[tokio::test]
async fn test_agent_list_versions() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ai/a2a/version/list"))
        .respond_with(api_ok_raw(serde_json::json!([
            {"version": "1.0", "latest": false},
            {"version": "2.0", "latest": true}
        ])))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let versions = client.agent_list_versions("my-agent", "").await.unwrap();
    assert_eq!(versions.len(), 2);
}

#[tokio::test]
async fn test_agent_list() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ai/a2a/list"))
        .respond_with(api_ok_raw(page_json(
            &[serde_json::json!({
                "name": "my-agent",
                "latestVersion": "2.0"
            })],
            1,
        )))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let page = client.agent_list("", "my-agent", 1, 10).await.unwrap();
    assert_eq!(page.total_count, 1);
}

#[tokio::test]
async fn test_agent_search() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ai/a2a/list"))
        .respond_with(api_ok_raw(page_json(&[] as &[serde_json::Value], 0)))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let page = client.agent_search("", "pattern", 1, 10).await.unwrap();
    assert_eq!(page.total_count, 0);
}

// ============================================================================
// Error Handling / Edge Cases
// ============================================================================

#[tokio::test]
async fn test_server_error_returns_err() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/core/namespace/list"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let result = client.namespace_list().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_invalid_json_returns_err() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/core/namespace/list"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let result = client.namespace_list().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_server_state_graceful_degradation() {
    // No mock registered — server returns 404
    // server_state uses unwrap_or, so it should return empty map
    let server = MockServer::start().await;
    let client = make_client(&server).await;
    let state = client.server_state().await.unwrap();
    assert!(state.is_empty());
}

#[tokio::test]
async fn test_server_readiness_delegates_to_cluster_health() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/core/cluster/node/self/health"))
        .respond_with(api_ok_raw(serde_json::json!({"healthy": true})))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/core/cluster/node/list"))
        .respond_with(api_ok_raw(serde_json::json!([])))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    assert!(client.server_readiness().await);
}

#[tokio::test]
async fn test_service_published_clients_with_filters() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ns/client/service/publisher/list"))
        .respond_with(api_ok_raw(serde_json::json!([])))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let publishers = client
        .service_published_clients("", "DEFAULT_GROUP", "svc-1", Some("10.0.0.1"), Some(8080))
        .await
        .unwrap();
    assert!(publishers.is_empty());
}

#[tokio::test]
async fn test_service_subscribed_clients_with_filters() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ns/client/service/subscriber/list"))
        .respond_with(api_ok_raw(serde_json::json!([])))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let subscribers = client
        .service_subscribed_clients("", "DEFAULT_GROUP", "svc-1", Some("10.0.0.2"), Some(9090))
        .await
        .unwrap();
    assert!(subscribers.is_empty());
}

#[tokio::test]
async fn test_service_subscribers_with_aggregation() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ns/client/subscribe/list"))
        .respond_with(api_ok_raw(page_json(&[] as &[serde_json::Value], 0)))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let page = client
        .service_subscribers_with_aggregation("", "DEFAULT_GROUP", "svc", 1, 10, true)
        .await
        .unwrap();
    assert_eq!(page.total_count, 0);
}

#[tokio::test]
async fn test_instance_register_with_metadata() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v3/admin/ns/instance"))
        .respond_with(api_ok(&"ok".to_string()))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let mut metadata = HashMap::new();
    metadata.insert("env".to_string(), "prod".to_string());
    let instance = Instance {
        ip: "10.0.0.3".to_string(),
        port: 8080,
        weight: 1.0,
        enabled: true,
        healthy: true,
        ephemeral: true,
        metadata,
        ..Default::default()
    };
    let result = client
        .instance_register("ns1", "DEFAULT_GROUP", "svc", &instance)
        .await
        .unwrap();
    assert_eq!(result, "ok");
}

// Sync tests for model/config types

#[test]
fn test_same_config_policy_display() {
    assert_eq!(format!("{}", SameConfigPolicy::Abort), "ABORT");
    assert_eq!(format!("{}", SameConfigPolicy::Skip), "SKIP");
    assert_eq!(format!("{}", SameConfigPolicy::Overwrite), "OVERWRITE");
}

#[test]
fn test_config_clone_info() {
    let info = ConfigCloneInfo {
        config_id: 1,
        target_group_name: Some("TARGET_GROUP".to_string()),
        target_data_id: None,
    };
    let json = serde_json::to_string(&info).unwrap();
    assert!(json.contains("\"configId\":1"));
    assert!(json.contains("\"targetGroupName\":\"TARGET_GROUP\""));
}

// ============================================================================
// Plugin APIs
// ============================================================================

#[tokio::test]
async fn test_plugin_list() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/core/plugin/list"))
        .respond_with(api_ok_raw(serde_json::json!([
            {
                "pluginId": "auth:nacos-auth",
                "pluginType": "auth",
                "pluginName": "nacos-auth",
                "enabled": true,
                "critical": true,
                "configurable": false,
                "exclusive": true
            }
        ])))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let plugins = client.plugin_list(None).await.unwrap();
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].plugin_id, "auth:nacos-auth");
    assert_eq!(plugins[0].plugin_type, "auth");
    assert!(plugins[0].enabled);
    assert!(plugins[0].critical);
}

#[tokio::test]
async fn test_plugin_list_filtered() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/core/plugin/list"))
        .respond_with(api_ok_raw(serde_json::json!([])))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let plugins = client.plugin_list(Some("config-encryption")).await.unwrap();
    assert!(plugins.is_empty());
}

#[tokio::test]
async fn test_plugin_detail() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/core/plugin/detail"))
        .respond_with(api_ok_raw(serde_json::json!({
            "pluginId": "auth:nacos-auth",
            "pluginType": "auth",
            "pluginName": "nacos-auth",
            "enabled": true,
            "critical": true,
            "configurable": false
        })))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let detail = client.plugin_detail("auth", "nacos-auth").await.unwrap();
    assert_eq!(detail.plugin_name, "nacos-auth");
    assert_eq!(detail.plugin_type, "auth");
    assert!(detail.enabled);
    assert!(!detail.configurable);
}

#[tokio::test]
async fn test_plugin_update_status() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/v3/admin/core/plugin/status"))
        .respond_with(api_ok(&true))
        .expect(1)
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let ok = client
        .plugin_update_status("auth", "nacos-auth", true, false)
        .await
        .unwrap();
    assert!(ok);
}

#[tokio::test]
async fn test_plugin_update_config() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/v3/admin/core/plugin/config"))
        .respond_with(api_ok(&true))
        .expect(1)
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let mut config = HashMap::new();
    config.insert("keySize".to_string(), "256".to_string());
    let ok = client
        .plugin_update_config("config-encryption", "aes", &config, false)
        .await
        .unwrap();
    assert!(ok);
}

#[tokio::test]
async fn test_plugin_availability() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/core/plugin/availability"))
        .respond_with(api_ok_raw(serde_json::json!({
            "pluginId": "auth:nacos-auth",
            "available": true,
            "availableCount": 1,
            "totalCount": 1
        })))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let avail = client
        .plugin_availability("auth", "nacos-auth")
        .await
        .unwrap();
    assert_eq!(avail.plugin_id, "auth:nacos-auth");
    assert!(avail.available);
    assert_eq!(avail.available_count, Some(1));
    assert_eq!(avail.total_count, Some(1));
}

/// TLS paths on `MaintainerClientConfig` must reach the reqwest builder.
/// A missing CA file is surfaced as an error rather than silently ignored.
#[tokio::test]
async fn test_maintainer_tls_paths_are_forwarded() {
    let missing_ca =
        MaintainerClientConfig::default().with_tls("/this/path/does/not/exist/batata-ca.pem");
    assert!(MaintainerClient::new(missing_ca).await.is_err());

    let missing_identity = MaintainerClientConfig::default().with_mtls(
        "/this/path/does/not/exist/batata-ca.pem",
        "/this/path/does/not/exist/client.pem",
        "/this/path/does/not/exist/client.key",
    );
    assert!(MaintainerClient::new(missing_identity).await.is_err());
}

#[test]
fn test_maintainer_tls_builders() {
    let config = MaintainerClientConfig::default().with_mtls("ca.pem", "client.pem", "client.key");
    assert_eq!(config.tls_ca_cert_path.as_deref(), Some("ca.pem"));
    assert_eq!(config.tls_client_cert_path.as_deref(), Some("client.pem"));
    assert_eq!(config.tls_client_key_path.as_deref(), Some("client.key"));

    let defaults = MaintainerClientConfig::default();
    assert!(defaults.tls_ca_cert_path.is_none());
    assert!(defaults.tls_client_cert_path.is_none());
    assert!(defaults.tls_client_key_path.is_none());
}

// ============================================================================
// Protocol-neutral agent (ADMIN /v3/admin/ai/agents) wiremock coverage
// ============================================================================

fn agent_version_json() -> serde_json::Value {
    serde_json::json!({
        "version": "1.0",
        "status": "online",
        "latest": true,
        "namespaceId": "",
        "agentName": "my-agent",
        "labels": { "latest": "1.0" }
    })
}

#[tokio::test]
async fn test_agents_get_and_get_version() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ai/agents"))
        .and(query_param("namespaceId", "ns-1"))
        .and(query_param("agentName", "my-agent"))
        .and(query_param("version", "1.0"))
        .respond_with(api_ok_raw(agent_version_json()))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ai/agents/version"))
        .and(query_param("namespaceId", "ns-1"))
        .and(query_param("agentName", "my-agent"))
        .and(query_param("version", "1.0"))
        .respond_with(api_ok_raw(agent_version_json()))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let detail = client
        .agents_get("ns-1", "my-agent", Some("1.0"))
        .await
        .unwrap();
    assert_eq!(detail.version, "1.0");
    assert_eq!(detail.agent_name, "my-agent");
    assert_eq!(detail.latest, Some(true));
    assert!(detail.extra.contains_key("labels"));

    let exact = client
        .agents_get_version("ns-1", "my-agent", "1.0")
        .await
        .unwrap();
    assert_eq!(exact.status, "online");
}

#[tokio::test]
async fn test_agents_list_query_shape() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ai/agents/list"))
        .and(query_param("namespaceId", "ns-1"))
        .and(query_param("agentName", "my-agent"))
        .and(query_param("bizTag", "chat"))
        .and(query_param("scope", "public"))
        .and(query_param("owner", "alice"))
        .and(query_param("orderBy", "name"))
        .and(query_param("pageNo", "2"))
        .and(query_param("pageSize", "5"))
        .respond_with(api_ok_raw(page_json(&[agent_version_json()], 1)))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let page = client
        .agents_list("ns-1", "my-agent", "chat", "public", "alice", "name", 2, 5)
        .await
        .unwrap();
    assert_eq!(page.total_count, 1);
    assert_eq!(page.page_items[0].version, "1.0");
}

#[tokio::test]
async fn test_agents_list_versions_forwards_status() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ai/agents/versions"))
        .and(query_param("namespaceId", ""))
        .and(query_param("agentName", "my-agent"))
        .and(query_param("status", "online"))
        .and(query_param("pageNo", "1"))
        .and(query_param("pageSize", "20"))
        .respond_with(api_ok_raw(page_json(&[agent_version_json()], 1)))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let page = client
        .agents_list_versions("", "my-agent", "online", 1, 20)
        .await
        .unwrap();
    // Inspect through serde_json::Value so this guard stays compile-safe
    // against a `Page -> Vec` regression: a Vec-returning implementation
    // fails to decode the paged payload at runtime (its `unwrap` panics)
    // instead of tripping a compile error that would mask the real guard.
    let value = serde_json::to_value(&page).unwrap();
    assert_eq!(value["totalCount"], 1);
    assert_eq!(value["pageItems"].as_array().unwrap().len(), 1);
    assert_eq!(value["pageItems"][0]["version"], "1.0");
}

/// The `*_json` aliases must keep the legacy batata dual-channel JSON wire
/// (AgentCard body + identity/overwrite query) so those handlers do not
/// regress. The primary `agents_create_draft`/`agents_update_draft` methods are
/// covered separately by the GA form tests.
#[tokio::test]
async fn test_agents_create_and_update_draft_json_alias_use_json_body() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v3/admin/ai/agents/draft"))
        .and(query_param("namespaceId", "ns-1"))
        .and(query_param("overwrite", "true"))
        .and(body_string_contains("\"name\":\"my-agent\""))
        .respond_with(api_ok_raw(agent_version_json()))
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/v3/admin/ai/agents/draft"))
        .and(query_param("namespaceId", "ns-1"))
        .and(body_string_contains("\"name\":\"my-agent\""))
        .respond_with(api_ok_raw(agent_version_json()))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let card = AgentCard {
        basic_info: AgentCardBasicInfo {
            name: "my-agent".to_string(),
            ..Default::default()
        },
        ..Default::default()
    };
    let created = client
        .agents_create_draft_json("ns-1", &card, true)
        .await
        .unwrap();
    assert_eq!(created.version, "1.0");
    let updated = client
        .agents_update_draft_json("ns-1", &card)
        .await
        .unwrap();
    assert_eq!(updated.status, "online");
}

#[tokio::test]
async fn test_agents_delete_draft() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/v3/admin/ai/agents/draft"))
        .and(query_param("namespaceId", ""))
        .and(query_param("agentName", "my-agent"))
        .and(query_param("version", "draft-1"))
        .respond_with(api_ok_raw(serde_json::json!("ok")))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    assert!(
        client
            .agents_delete_draft("", "my-agent", "draft-1")
            .await
            .unwrap()
    );
}

/// Each transition must hit its own ADMIN path with the exact GA query shape.
/// A wrong path or a dropped query parameter turns the wiremock request into
/// a 404, so this is also the mutant guard for `agent_version_transition`.
#[tokio::test]
async fn test_agents_version_transitions() {
    let server = MockServer::start().await;
    let paths = [
        "/v3/admin/ai/agents/submit",
        "/v3/admin/ai/agents/publish",
        "/v3/admin/ai/agents/force-publish",
        "/v3/admin/ai/agents/redraft",
        "/v3/admin/ai/agents/online",
        "/v3/admin/ai/agents/offline",
    ];
    for route_path in paths {
        Mock::given(method("POST"))
            .and(path(route_path))
            .and(query_param("namespaceId", "ns-1"))
            .and(query_param("agentName", "my-agent"))
            .and(query_param("version", "1.0"))
            .respond_with(api_ok_raw(agent_version_json()))
            .mount(&server)
            .await;
    }

    let client = make_client(&server).await;
    let calls = [
        client
            .agents_submit_version("ns-1", "my-agent", "1.0")
            .await,
        client
            .agents_publish_version("ns-1", "my-agent", "1.0")
            .await,
        client
            .agents_force_publish_version("ns-1", "my-agent", "1.0")
            .await,
        client
            .agents_redraft_version("ns-1", "my-agent", "1.0")
            .await,
        client
            .agents_online_version("ns-1", "my-agent", "1.0")
            .await,
        client
            .agents_offline_version("ns-1", "my-agent", "1.0")
            .await,
    ];
    for (index, result) in calls.into_iter().enumerate() {
        assert_eq!(result.unwrap().version, "1.0", "call {index} failed");
    }
}

#[tokio::test]
async fn test_agents_update_labels_and_scope() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/v3/admin/ai/agents/labels"))
        .and(query_param("namespaceId", ""))
        .and(query_param("agentName", "my-agent"))
        .and(body_string_contains("\"labels\""))
        .and(body_string_contains("\"stable\":\"1.0\""))
        .respond_with(api_ok_raw(serde_json::json!({"stable": "1.0"})))
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/v3/admin/ai/agents/scope"))
        .and(query_param("namespaceId", ""))
        .and(query_param("agentName", "my-agent"))
        .and(query_param("scope", "public"))
        .respond_with(api_ok_raw(serde_json::json!("ok")))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let mut labels = HashMap::new();
    labels.insert("stable".to_string(), "1.0".to_string());
    // The `_json` alias keeps the legacy batata `{"labels": {...}}` body and
    // its map return value; the primary method now speaks GA form (tested
    // separately).
    let updated = client
        .agents_update_labels_json("", "my-agent", &labels)
        .await
        .unwrap();
    assert_eq!(updated.get("stable").map(String::as_str), Some("1.0"));
    assert!(
        client
            .agents_update_scope("", "my-agent", "public")
            .await
            .unwrap()
    );
}

// ============================================================================
// AI resource facades (`F-NAC-MAINT-007`) — GA delegates' read surface + ZIP I/O
// ============================================================================

use wiremock::{Match, Request};

/// Return true when `needle` occurs anywhere in `haystack`.
fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && haystack.windows(needle.len()).any(|w| w == needle)
}

/// Guard the multipart contract: both GA and batata look for a part named
/// `file`, and the ZIP bytes must be transmitted unchanged.
struct ZipFilePart(Vec<u8>);

impl Match for ZipFilePart {
    fn matches(&self, request: &Request) -> bool {
        contains_bytes(&request.body, b"name=\"file\"") && contains_bytes(&request.body, &self.0)
    }
}

fn zip_bytes() -> &'static [u8] {
    b"PK\x03\x04-batata-zip-fixture"
}

/// Decode one `application/x-www-form-urlencoded` component.
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                if let Ok(value) = u8::from_str_radix(&input[i + 1..i + 3], 16) {
                    out.push(value);
                    i += 3;
                } else {
                    out.push(b'%');
                    i += 1;
                }
            }
            other => {
                out.push(other);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Parse a urlencoded body into an exact, comparable field map.
fn form_fields(request: &Request) -> HashMap<String, String> {
    String::from_utf8_lossy(&request.body)
        .split('&')
        .filter(|pair| !pair.is_empty())
        .filter_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            Some((percent_decode(key), percent_decode(value)))
        })
        .collect()
}

/// Guard the *whole* form contract: a missing, extra or renamed field must not
/// match, so a mutant that drops an optional parameter turns the test red.
struct FormFields(HashMap<String, String>);

impl Match for FormFields {
    fn matches(&self, request: &Request) -> bool {
        form_fields(request) == self.0
    }
}

fn form(expected: &[(&str, &str)]) -> FormFields {
    FormFields(
        expected
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect(),
    )
}

/// Mount one AI ADMIN write endpoint. POST/PUT carry identity in the form body;
/// DELETE carries it in the query string, matching GA's `ClientHttpProxy`.
async fn mount_ai_write(
    server: &MockServer,
    verb: &'static str,
    route: &'static str,
    expected: &[(&str, &str)],
    data: serde_json::Value,
) {
    let mut builder = Mock::given(method(verb)).and(path(route));
    if verb == "DELETE" {
        for (key, value) in expected {
            builder = builder.and(query_param((*key).to_string(), (*value).to_string()));
        }
    } else {
        builder = builder.and(form(expected));
    }
    builder.respond_with(api_ok_raw(data)).mount(server).await;
}

#[tokio::test]
async fn test_skill_reads_and_download() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ai/skills"))
        .and(query_param("namespaceId", "ns-1"))
        .and(query_param("skillName", "my-skill"))
        .respond_with(api_ok_raw(serde_json::json!({
            "namespaceId": "ns-1",
            "name": "my-skill",
            "owner": "alice",
            "enable": true,
            "labels": {"latest": "1.0.0"},
            "onlineCnt": 2,
            "versions": [{"version": "1.0.0", "status": "online", "commitMsg": "first"}],
            "untypedGaField": "kept"
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ai/skills/version"))
        .and(query_param("namespaceId", "ns-1"))
        .and(query_param("skillName", "my-skill"))
        .and(query_param("version", "1.0"))
        .respond_with(api_ok_raw(serde_json::json!({
            "namespaceId": "ns-1",
            "name": "my-skill",
            "skillMd": "# demo",
            "resource": {
                "template::demo.json": {
                    "name": "demo.json",
                    "type": "template",
                    "content": "{}"
                }
            },
            "version": "1.0"
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ai/skills/version/download"))
        .and(query_param("namespaceId", "ns-1"))
        .and(query_param("skillName", "my-skill"))
        .and(query_param("version", "1.0"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"SKILLZIP".to_vec()))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ai/skills/list"))
        .and(query_param("namespaceId", "ns-1"))
        .and(query_param("skillName", "demo"))
        .and(query_param("search", "blur"))
        .and(query_param("orderBy", "download_count"))
        .and(query_param("owner", "alice"))
        .and(query_param("scope", "PUBLIC"))
        .and(query_param("bizTag", "chat"))
        .and(query_param("pageNo", "2"))
        .and(query_param("pageSize", "5"))
        .respond_with(api_ok_raw(page_json(
            &[serde_json::json!({"name": "demo", "downloadCount": 9, "versions": []})],
            1,
        )))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let meta = client.skill_get_meta("ns-1", "my-skill").await.unwrap();
    assert_eq!(meta.governance.name.as_deref(), Some("my-skill"));
    assert_eq!(meta.governance.owner.as_deref(), Some("alice"));
    assert_eq!(meta.governance.enable, Some(true));
    assert_eq!(
        meta.governance.labels.get("latest").map(String::as_str),
        Some("1.0.0")
    );
    assert_eq!(meta.versions.len(), 1);
    assert_eq!(meta.versions[0].commit_msg.as_deref(), Some("first"));
    assert_eq!(meta.extra["untypedGaField"], "kept");
    assert!(!meta.extra.contains_key("versions"));
    let version = client
        .skill_get_version_detail("ns-1", "my-skill", "1.0")
        .await
        .unwrap();
    assert_eq!(version.name.as_deref(), Some("my-skill"));
    assert_eq!(version.skill_md.as_deref(), Some("# demo"));
    assert_eq!(
        version.resource["template::demo.json"]
            .resource_type
            .as_deref(),
        Some("template")
    );
    assert_eq!(version.extra["version"], "1.0");
    let bytes = client
        .skill_download_version("ns-1", "my-skill", "1.0")
        .await
        .unwrap();
    assert_eq!(bytes, b"SKILLZIP");
    let page = client
        .skill_list(
            "ns-1",
            Some("demo"),
            Some("blur"),
            Some("download_count"),
            Some("alice"),
            Some("PUBLIC"),
            Some("chat"),
            2,
            5,
        )
        .await
        .unwrap();
    assert_eq!(page.total_count, 1);
    assert_eq!(page.page_items[0].governance.name.as_deref(), Some("demo"));
    assert_eq!(page.page_items[0].governance.download_count, Some(9));
    assert!(page.page_items[0].extra.contains_key("versions"));
}

#[tokio::test]
async fn test_skill_zip_uploads() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v3/admin/ai/skills/upload"))
        .and(query_param("namespaceId", "ns-1"))
        .and(query_param("overwrite", "true"))
        .and(ZipFilePart(zip_bytes().to_vec()))
        .respond_with(api_ok_raw(serde_json::json!("my-skill")))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v3/admin/ai/skills/upload/precheck"))
        .and(query_param("namespaceId", "ns-1"))
        .and(query_param("overwrite", "false"))
        .and(ZipFilePart(zip_bytes().to_vec()))
        .respond_with(api_ok_raw(serde_json::json!([{
            "namespaceId": "ns-1",
            "skillName": "my-skill",
            "precheckCode": "READY",
            "exists": false
        }])))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v3/admin/ai/skills/upload/batch"))
        .and(query_param("namespaceId", "ns-1"))
        .and(query_param("overwrite", "true"))
        .and(ZipFilePart(zip_bytes().to_vec()))
        .respond_with(api_ok_raw(serde_json::json!({
            "succeeded": ["my-skill"],
            "results": [{"name": "my-skill", "success": true, "errorCode": "SUCCESS"}],
            "successCount": 1
        })))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    assert_eq!(
        client
            .skill_upload_from_zip("ns-1", zip_bytes(), true, None, None, false, None)
            .await
            .unwrap(),
        "my-skill"
    );
    let precheck = client
        .skill_precheck_upload("ns-1", zip_bytes())
        .await
        .unwrap();
    assert_eq!(precheck.len(), 1);
    assert_eq!(precheck[0].skill_name.as_deref(), Some("my-skill"));
    assert_eq!(precheck[0].precheck_code.as_deref(), Some("READY"));
    let batch = client
        .skill_batch_upload("ns-1", zip_bytes(), true)
        .await
        .unwrap();
    assert_eq!(batch.succeeded, vec!["my-skill".to_string()]);
    assert_eq!(batch.results[0].error_code.as_deref(), Some("SUCCESS"));
    assert_eq!(batch.extra["successCount"], 1);
}

#[tokio::test]
async fn test_prompt_reads_and_delete() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ai/prompt/list"))
        .and(query_param("namespaceId", "ns-1"))
        .and(query_param("promptKey", "hello"))
        .and(query_param("search", "blur"))
        .and(query_param("bizTags", "chat"))
        .and(query_param("pageNo", "1"))
        .and(query_param("pageSize", "20"))
        .respond_with(api_ok_raw(page_json(
            &[serde_json::json!({
                "promptKey": "hello",
                "schemaVersion": 1,
                "versionDetails": []
            })],
            1,
        )))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ai/prompt/versions"))
        .and(query_param("namespaceId", "ns-1"))
        .and(query_param("promptKey", "hello"))
        .and(query_param("pageNo", "1"))
        .and(query_param("pageSize", "10"))
        .respond_with(api_ok_raw(page_json(
            &[serde_json::json!({"promptKey": "hello", "version": "1.0", "status": "online"})],
            1,
        )))
        .mount(&server)
        .await;
    for (route_path, value) in [
        ("/v3/admin/ai/prompt/metadata", "metadata"),
        ("/v3/admin/ai/prompt/governance", "governance"),
        ("/v3/admin/ai/prompt/version", "version"),
        ("/v3/admin/ai/prompt/detail", "detail"),
    ] {
        Mock::given(method("GET"))
            .and(path(route_path))
            .and(query_param("namespaceId", "ns-1"))
            .and(query_param("promptKey", "hello"))
            .respond_with(api_ok_raw(serde_json::json!({
                "kind": value,
                "promptKey": "hello",
                "version": "1.0",
                "template": "hello {{name}}",
                "schemaVersion": 1
            })))
            .mount(&server)
            .await;
    }
    Mock::given(method("GET"))
        .and(path("/v3/admin/ai/prompt/version/download"))
        .and(query_param("namespaceId", "ns-1"))
        .and(query_param("promptKey", "hello"))
        .and(query_param("version", "1.0"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"PROMPTZIP".to_vec()))
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/v3/admin/ai/prompt"))
        .and(query_param("namespaceId", "ns-1"))
        .and(query_param("promptKey", "hello"))
        .respond_with(api_ok_raw(serde_json::json!("ok")))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let listed = client
        .prompt_list("ns-1", Some("hello"), Some("blur"), Some("chat"), 1, 20)
        .await
        .unwrap();
    assert_eq!(listed.page_items[0].prompt_key.as_deref(), Some("hello"));
    assert_eq!(listed.page_items[0].schema_version, Some(1));
    assert!(listed.page_items[0].extra.contains_key("versionDetails"));
    let versions = client
        .prompt_list_versions("ns-1", "hello", 1, 10)
        .await
        .unwrap();
    assert_eq!(versions.page_items[0].version.as_deref(), Some("1.0"));
    assert_eq!(versions.page_items[0].status.as_deref(), Some("online"));
    let meta = client.prompt_get_meta("ns-1", "hello").await.unwrap();
    assert_eq!(meta.prompt_key.as_deref(), Some("hello"));
    assert_eq!(meta.extra["kind"], "metadata");
    assert!(!meta.extra.contains_key("schemaVersion"));
    let governance = client
        .prompt_get_governance_detail("ns-1", "hello")
        .await
        .unwrap();
    assert!(governance.versions.is_empty());
    assert_eq!(governance.extra["kind"], "governance");
    let version = client
        .prompt_get_version_detail("ns-1", "hello", "1.0")
        .await
        .unwrap();
    assert_eq!(version.version.as_deref(), Some("1.0"));
    assert_eq!(version.template.as_deref(), Some("hello {{name}}"));
    assert_eq!(version.extra["kind"], "version");
    assert!(!version.extra.contains_key("template"));
    let detail = client
        .prompt_query_detail("ns-1", "hello", Some("1.0"), Some("stable"), Some("md5"))
        .await
        .unwrap();
    assert_eq!(detail.template.as_deref(), Some("hello {{name}}"));
    assert_eq!(detail.extra["kind"], "detail");
    assert!(!detail.extra.contains_key("template"));
    assert_eq!(
        client
            .prompt_download_version("ns-1", "hello", "1.0")
            .await
            .unwrap(),
        b"PROMPTZIP"
    );
    assert!(client.prompt_delete("ns-1", "hello").await.unwrap());
}

#[tokio::test]
async fn test_agentspec_reads_upload_and_delete() {
    let server = MockServer::start().await;
    for (route_path, value) in [
        ("/v3/admin/ai/agentspecs", "detail"),
        ("/v3/admin/ai/agentspecs/version", "version"),
        ("/v3/admin/ai/agentspecs/version/meta", "version-meta"),
    ] {
        Mock::given(method("GET"))
            .and(path(route_path))
            .and(query_param("namespaceId", "ns-1"))
            .and(query_param("agentSpecName", "my-spec"))
            .respond_with(api_ok_raw(serde_json::json!({
                "kind": value,
                "namespaceId": "ns-1",
                "name": "my-spec",
                "owner": "alice",
                "content": "spec-body"
            })))
            .mount(&server)
            .await;
    }
    Mock::given(method("GET"))
        .and(path("/v3/admin/ai/agentspecs/list"))
        .and(query_param("namespaceId", "ns-1"))
        .and(query_param("agentSpecName", "my-spec"))
        .and(query_param("search", "blur"))
        .and(query_param("pageNo", "1"))
        .and(query_param("pageSize", "20"))
        .respond_with(api_ok_raw(page_json(
            &[serde_json::json!({"namespaceId": "ns-1", "name": "my-spec"})],
            1,
        )))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ai/agentspecs/version/download"))
        .and(query_param("namespaceId", "ns-1"))
        .and(query_param("agentSpecName", "my-spec"))
        .and(query_param("version", "1.0"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"SPECZIP".to_vec()))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v3/admin/ai/agentspecs/upload"))
        .and(query_param("namespaceId", "ns-1"))
        .and(query_param("overwrite", "true"))
        .and(ZipFilePart(zip_bytes().to_vec()))
        .respond_with(api_ok_raw(serde_json::json!("my-spec")))
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/v3/admin/ai/agentspecs"))
        .and(query_param("namespaceId", "ns-1"))
        .and(query_param("agentSpecName", "my-spec"))
        .respond_with(api_ok_raw(serde_json::json!(true)))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let detail = client
        .agentspec_get_detail("ns-1", "my-spec")
        .await
        .unwrap();
    assert_eq!(detail.governance.name.as_deref(), Some("my-spec"));
    assert_eq!(detail.governance.owner.as_deref(), Some("alice"));
    assert_eq!(detail.extra["kind"], "detail");
    let version = client
        .agentspec_get_version_detail("ns-1", "my-spec", "1.0")
        .await
        .unwrap();
    assert_eq!(version.name.as_deref(), Some("my-spec"));
    assert_eq!(version.extra["kind"], "version");
    assert!(!version.extra.contains_key("content"));
    let version_meta = client
        .agentspec_get_version_meta("ns-1", "my-spec", "1.0")
        .await
        .unwrap();
    assert_eq!(version_meta.name.as_deref(), Some("my-spec"));
    assert_eq!(version_meta.extra["kind"], "version-meta");
    let page = client
        .agentspec_list("ns-1", Some("my-spec"), Some("blur"), 1, 20)
        .await
        .unwrap();
    assert_eq!(page.page_items[0].name.as_deref(), Some("my-spec"));
    assert_eq!(
        client
            .agentspec_download_version("ns-1", "my-spec", "1.0")
            .await
            .unwrap(),
        b"SPECZIP"
    );
    assert_eq!(
        client
            .agentspec_upload_from_zip("ns-1", zip_bytes(), true)
            .await
            .unwrap(),
        "my-spec"
    );
    assert!(client.agentspec_delete("ns-1", "my-spec").await.unwrap());
}

#[tokio::test]
async fn test_skill_delete_and_pipeline_reads() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/v3/admin/ai/skills"))
        .and(query_param("namespaceId", ""))
        .and(query_param("skillName", "my-skill"))
        .respond_with(api_ok_raw(serde_json::json!(true)))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ai/pipelines/detail"))
        .and(query_param("pipelineId", "pipe-1"))
        .respond_with(api_ok_raw(serde_json::json!({
            "id": "pipe-1",
            "executionId": "pipe-1",
            "resourceType": "skill",
            "status": "SUCCESS"
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ai/pipelines/list"))
        .and(query_param("resourceType", "skill"))
        .and(query_param("resourceName", "my-skill"))
        .and(query_param("namespaceId", "ns-1"))
        .and(query_param("version", "1.0"))
        .and(query_param("pageNo", "1"))
        .and(query_param("pageSize", "20"))
        .respond_with(api_ok_raw(page_json(
            &[serde_json::json!({
                "id": "pipe-1",
                "executionId": "pipe-1",
                "status": "SUCCESS"
            })],
            1,
        )))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    assert!(client.skill_delete("", "my-skill").await.unwrap());
    let execution = client.pipeline_get_detail("pipe-1").await.unwrap();
    assert_eq!(execution.execution_id.as_deref(), Some("pipe-1"));
    assert_eq!(execution.status.as_deref(), Some("SUCCESS"));
    assert_eq!(execution.extra["id"], "pipe-1");
    assert!(!execution.extra.contains_key("executionId"));
    let page = client
        .pipeline_list_executions("skill", Some("my-skill"), Some("ns-1"), Some("1.0"), 1, 20)
        .await
        .unwrap();
    assert_eq!(page.page_items[0].execution_id.as_deref(), Some("pipe-1"));
    assert_eq!(page.page_items[0].status.as_deref(), Some("SUCCESS"));
    assert_eq!(page.page_items[0].extra["id"], "pipe-1");
}

/// A non-2xx response must surface as an error for reads, downloads and
/// multipart uploads alike, rather than being decoded as an empty payload.
#[tokio::test]
async fn test_ai_resource_error_paths() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ai/skills"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ai/skills/version/download"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v3/admin/ai/skills/upload"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    assert!(client.skill_get_meta("", "my-skill").await.is_err());
    assert!(
        client
            .skill_download_version("", "my-skill", "1.0")
            .await
            .is_err()
    );
    assert!(
        client
            .skill_upload_from_zip("", zip_bytes(), false, None, None, false, None)
            .await
            .is_err()
    );
}

// ============================================================================
// AI resource write facade (`F-NAC-MAINT-007`, batch 7)
// ============================================================================

/// Every skill lifecycle endpoint: verb, path and the exact form/query fields.
#[tokio::test]
async fn test_skill_write_lifecycle() {
    let server = MockServer::start().await;
    let card = r#"{"name":"demo"}"#;
    let labels = r#"{"latest":"1.0.0"}"#;
    let tags = r#"["chat"]"#;
    mount_ai_write(
        &server,
        "POST",
        "/v3/admin/ai/skills/draft",
        &[
            ("namespaceId", "ns-1"),
            ("skillName", "demo"),
            ("basedOnVersion", "1.0.0"),
            ("targetVersion", "1.1.0"),
            ("skillCard", card),
            ("commitMsg", "msg"),
        ],
        serde_json::json!("1.1.0"),
    )
    .await;
    mount_ai_write(
        &server,
        "PUT",
        "/v3/admin/ai/skills/draft",
        &[
            ("namespaceId", "ns-1"),
            ("skillName", "demo"),
            ("version", "1.1.0"),
            ("skillCard", card),
            ("setAsLatest", "true"),
        ],
        serde_json::json!(true),
    )
    .await;
    mount_ai_write(
        &server,
        "DELETE",
        "/v3/admin/ai/skills/draft",
        &[("namespaceId", "ns-1"), ("skillName", "demo")],
        serde_json::json!(true),
    )
    .await;
    mount_ai_write(
        &server,
        "POST",
        "/v3/admin/ai/skills/submit",
        &[
            ("namespaceId", "ns-1"),
            ("skillName", "demo"),
            ("version", "1.1.0"),
        ],
        serde_json::json!("1.1.0"),
    )
    .await;
    mount_ai_write(
        &server,
        "POST",
        "/v3/admin/ai/skills/publish",
        &[
            ("namespaceId", "ns-1"),
            ("skillName", "demo"),
            ("version", "1.1.0"),
            ("updateLatestLabel", "false"),
        ],
        serde_json::json!(true),
    )
    .await;
    mount_ai_write(
        &server,
        "POST",
        "/v3/admin/ai/skills/force-publish",
        &[
            ("namespaceId", "ns-1"),
            ("skillName", "demo"),
            ("version", "1.1.0"),
        ],
        serde_json::json!(true),
    )
    .await;
    mount_ai_write(
        &server,
        "POST",
        "/v3/admin/ai/skills/redraft",
        &[
            ("namespaceId", "ns-1"),
            ("skillName", "demo"),
            ("version", "1.1.0"),
        ],
        serde_json::json!(true),
    )
    .await;
    mount_ai_write(
        &server,
        "PUT",
        "/v3/admin/ai/skills/labels",
        &[
            ("namespaceId", "ns-1"),
            ("skillName", "demo"),
            ("labels", labels),
        ],
        serde_json::json!(true),
    )
    .await;
    mount_ai_write(
        &server,
        "PUT",
        "/v3/admin/ai/skills/biz-tags",
        &[
            ("namespaceId", "ns-1"),
            ("skillName", "demo"),
            ("bizTags", tags),
        ],
        serde_json::json!(true),
    )
    .await;
    mount_ai_write(
        &server,
        "POST",
        "/v3/admin/ai/skills/online",
        &[
            ("namespaceId", "ns-1"),
            ("skillName", "demo"),
            ("scope", "skill"),
            ("version", "1.1.0"),
        ],
        serde_json::json!(true),
    )
    .await;
    mount_ai_write(
        &server,
        "POST",
        "/v3/admin/ai/skills/offline",
        &[("namespaceId", "ns-1"), ("skillName", "demo")],
        serde_json::json!(true),
    )
    .await;
    mount_ai_write(
        &server,
        "PUT",
        "/v3/admin/ai/skills/scope",
        &[
            ("namespaceId", "ns-1"),
            ("skillName", "demo"),
            ("scope", "PUBLIC"),
        ],
        serde_json::json!(true),
    )
    .await;

    let client = make_client(&server).await;
    assert_eq!(
        client
            .skill_create_draft(
                "ns-1",
                Some("demo"),
                Some("1.0.0"),
                Some("1.1.0"),
                Some(card),
                Some("msg"),
            )
            .await
            .unwrap(),
        "1.1.0"
    );
    assert!(
        client
            .skill_update_draft("ns-1", Some("demo"), Some("1.1.0"), card, Some(true), None,)
            .await
            .unwrap()
    );
    assert!(client.skill_delete_draft("ns-1", "demo").await.unwrap());
    assert_eq!(
        client.skill_submit("ns-1", "demo", "1.1.0").await.unwrap(),
        "1.1.0"
    );
    assert!(
        client
            .skill_publish("ns-1", "demo", "1.1.0", Some(false))
            .await
            .unwrap()
    );
    assert!(
        client
            .skill_force_publish("ns-1", "demo", "1.1.0", None)
            .await
            .unwrap()
    );
    assert!(client.skill_redraft("ns-1", "demo", "1.1.0").await.unwrap());
    assert!(
        client
            .skill_update_labels("ns-1", "demo", labels)
            .await
            .unwrap()
    );
    assert!(
        client
            .skill_update_biz_tags("ns-1", "demo", tags)
            .await
            .unwrap()
    );
    assert!(
        client
            .skill_change_online_status("ns-1", "demo", Some("skill"), Some("1.1.0"), true)
            .await
            .unwrap()
    );
    assert!(
        client
            .skill_change_online_status("ns-1", "demo", None, None, false)
            .await
            .unwrap()
    );
    assert!(
        client
            .skill_update_scope("ns-1", "demo", "PUBLIC")
            .await
            .unwrap()
    );
}

/// Every prompt lifecycle endpoint plus the four legacy (deprecated) APIs.
#[tokio::test]
async fn test_prompt_write_lifecycle() {
    let server = MockServer::start().await;
    let variables = r#"[{"name":"who"}]"#;
    let labels = r#"{"stable":"1.0.0"}"#;
    mount_ai_write(
        &server,
        "POST",
        "/v3/admin/ai/prompt/draft",
        &[
            ("namespaceId", "ns-1"),
            ("promptKey", "hello"),
            ("version", "1.1.0"),
            ("targetVersion", "1.1.0"),
            ("basedOnVersion", "1.0.0"),
            ("template", "Hello {{who}}"),
            ("variables", variables),
            ("commitMsg", "msg"),
            ("description", "greeting"),
            ("bizTags", "chat"),
        ],
        serde_json::json!("1.1.0"),
    )
    .await;
    mount_ai_write(
        &server,
        "PUT",
        "/v3/admin/ai/prompt/draft",
        &[
            ("namespaceId", "ns-1"),
            ("promptKey", "hello"),
            ("template", "Hello {{who}}"),
            ("variables", variables),
        ],
        serde_json::json!(true),
    )
    .await;
    mount_ai_write(
        &server,
        "DELETE",
        "/v3/admin/ai/prompt/draft",
        &[("namespaceId", "ns-1"), ("promptKey", "hello")],
        serde_json::json!(true),
    )
    .await;
    mount_ai_write(
        &server,
        "POST",
        "/v3/admin/ai/prompt/submit",
        &[
            ("namespaceId", "ns-1"),
            ("promptKey", "hello"),
            ("version", "1.1.0"),
        ],
        serde_json::json!("1.1.0"),
    )
    .await;
    mount_ai_write(
        &server,
        "POST",
        "/v3/admin/ai/prompt/publish",
        &[
            ("namespaceId", "ns-1"),
            ("promptKey", "hello"),
            ("version", "1.1.0"),
            ("updateLatestLabel", "true"),
        ],
        serde_json::json!(true),
    )
    .await;
    mount_ai_write(
        &server,
        "POST",
        "/v3/admin/ai/prompt/force-publish",
        &[
            ("namespaceId", "ns-1"),
            ("promptKey", "hello"),
            ("version", "1.1.0"),
        ],
        serde_json::json!(true),
    )
    .await;
    mount_ai_write(
        &server,
        "POST",
        "/v3/admin/ai/prompt/redraft",
        &[
            ("namespaceId", "ns-1"),
            ("promptKey", "hello"),
            ("version", "1.1.0"),
        ],
        serde_json::json!(true),
    )
    .await;
    mount_ai_write(
        &server,
        "POST",
        "/v3/admin/ai/prompt/online",
        &[
            ("namespaceId", "ns-1"),
            ("promptKey", "hello"),
            ("version", "1.1.0"),
        ],
        serde_json::json!(true),
    )
    .await;
    mount_ai_write(
        &server,
        "POST",
        "/v3/admin/ai/prompt/offline",
        &[
            ("namespaceId", "ns-1"),
            ("promptKey", "hello"),
            ("version", "1.0.0"),
        ],
        serde_json::json!(true),
    )
    .await;
    mount_ai_write(
        &server,
        "PUT",
        "/v3/admin/ai/prompt/labels",
        &[
            ("namespaceId", "ns-1"),
            ("promptKey", "hello"),
            ("labels", labels),
        ],
        serde_json::json!(true),
    )
    .await;
    mount_ai_write(
        &server,
        "PUT",
        "/v3/admin/ai/prompt/description",
        &[
            ("namespaceId", "ns-1"),
            ("promptKey", "hello"),
            ("description", "greeting"),
        ],
        serde_json::json!(true),
    )
    .await;
    mount_ai_write(
        &server,
        "PUT",
        "/v3/admin/ai/prompt/biz-tags",
        &[
            ("namespaceId", "ns-1"),
            ("promptKey", "hello"),
            ("bizTags", "chat"),
        ],
        serde_json::json!(true),
    )
    .await;
    mount_ai_write(
        &server,
        "PUT",
        "/v3/admin/ai/prompt/label",
        &[
            ("namespaceId", "ns-1"),
            ("promptKey", "hello"),
            ("label", "stable"),
            ("version", "1.1.0"),
        ],
        serde_json::json!(true),
    )
    .await;
    mount_ai_write(
        &server,
        "DELETE",
        "/v3/admin/ai/prompt/label",
        &[
            ("namespaceId", "ns-1"),
            ("promptKey", "hello"),
            ("label", "stable"),
        ],
        serde_json::json!(true),
    )
    .await;
    mount_ai_write(
        &server,
        "POST",
        "/v3/admin/ai/prompt",
        &[
            ("namespaceId", "ns-1"),
            ("promptKey", "hello"),
            ("version", "1.1.0"),
            ("template", "Hello {{who}}"),
            ("commitMsg", "msg"),
            ("description", "greeting"),
            ("bizTags", "chat"),
            ("variables", variables),
        ],
        serde_json::json!(true),
    )
    .await;
    mount_ai_write(
        &server,
        "PUT",
        "/v3/admin/ai/prompt/metadata",
        &[
            ("namespaceId", "ns-1"),
            ("promptKey", "hello"),
            ("description", "greeting"),
        ],
        serde_json::json!(true),
    )
    .await;

    let client = make_client(&server).await;
    assert_eq!(
        client
            .prompt_create_draft(
                "ns-1",
                "hello",
                Some("1.1.0"),
                Some("1.0.0"),
                "Hello {{who}}",
                Some(variables),
                Some("msg"),
                Some("greeting"),
                Some("chat"),
            )
            .await
            .unwrap(),
        "1.1.0"
    );
    assert!(
        client
            .prompt_update_draft("ns-1", "hello", "Hello {{who}}", Some(variables), None)
            .await
            .unwrap()
    );
    assert!(client.prompt_delete_draft("ns-1", "hello").await.unwrap());
    assert_eq!(
        client
            .prompt_submit("ns-1", "hello", "1.1.0")
            .await
            .unwrap(),
        "1.1.0"
    );
    assert!(
        client
            .prompt_publish("ns-1", "hello", "1.1.0", Some(true))
            .await
            .unwrap()
    );
    assert!(
        client
            .prompt_force_publish("ns-1", "hello", "1.1.0", None)
            .await
            .unwrap()
    );
    assert!(
        client
            .prompt_redraft("ns-1", "hello", "1.1.0")
            .await
            .unwrap()
    );
    assert!(
        client
            .prompt_change_online_status("ns-1", "hello", "1.1.0", true)
            .await
            .unwrap()
    );
    assert!(
        client
            .prompt_change_online_status("ns-1", "hello", "1.0.0", false)
            .await
            .unwrap()
    );
    assert!(
        client
            .prompt_update_labels("ns-1", "hello", labels)
            .await
            .unwrap()
    );
    assert!(
        client
            .prompt_update_description("ns-1", "hello", "greeting")
            .await
            .unwrap()
    );
    assert!(
        client
            .prompt_update_biz_tags("ns-1", "hello", "chat")
            .await
            .unwrap()
    );
    assert!(
        client
            .prompt_bind_label("ns-1", "hello", "stable", "1.1.0")
            .await
            .unwrap()
    );
    assert!(
        client
            .prompt_unbind_label("ns-1", "hello", "stable")
            .await
            .unwrap()
    );
    assert!(
        client
            .prompt_publish_prompt(
                "ns-1",
                "hello",
                "1.1.0",
                "Hello {{who}}",
                Some("msg"),
                Some("greeting"),
                Some("chat"),
                Some(variables),
            )
            .await
            .unwrap()
    );
    assert!(
        client
            .prompt_update_metadata("ns-1", "hello", Some("greeting"), None)
            .await
            .unwrap()
    );
}

/// Every agentspec lifecycle endpoint and GA's admin item list.
#[tokio::test]
async fn test_agentspec_write_lifecycle() {
    let server = MockServer::start().await;
    let card = r#"{"name":"my-spec"}"#;
    let labels = r#"{"latest":"1.0.0"}"#;
    let tags = r#"["chat"]"#;
    mount_ai_write(
        &server,
        "POST",
        "/v3/admin/ai/agentspecs/draft",
        &[
            ("namespaceId", "ns-1"),
            ("agentSpecName", "my-spec"),
            ("basedOnVersion", "1.0.0"),
            ("targetVersion", "1.1.0"),
            ("agentSpecCard", card),
            ("commitMsg", "msg"),
        ],
        serde_json::json!("1.1.0"),
    )
    .await;
    mount_ai_write(
        &server,
        "PUT",
        "/v3/admin/ai/agentspecs/draft",
        &[
            ("namespaceId", "ns-1"),
            ("agentSpecName", "my-spec"),
            ("version", "1.1.0"),
            ("agentSpecCard", card),
            ("setAsLatest", "false"),
        ],
        serde_json::json!(true),
    )
    .await;
    mount_ai_write(
        &server,
        "DELETE",
        "/v3/admin/ai/agentspecs/draft",
        &[("namespaceId", "ns-1"), ("agentSpecName", "my-spec")],
        serde_json::json!(true),
    )
    .await;
    mount_ai_write(
        &server,
        "POST",
        "/v3/admin/ai/agentspecs/submit",
        &[
            ("namespaceId", "ns-1"),
            ("agentSpecName", "my-spec"),
            ("version", "1.1.0"),
        ],
        serde_json::json!("1.1.0"),
    )
    .await;
    mount_ai_write(
        &server,
        "POST",
        "/v3/admin/ai/agentspecs/publish",
        &[
            ("namespaceId", "ns-1"),
            ("agentSpecName", "my-spec"),
            ("version", "1.1.0"),
            ("updateLatestLabel", "true"),
        ],
        serde_json::json!(true),
    )
    .await;
    mount_ai_write(
        &server,
        "POST",
        "/v3/admin/ai/agentspecs/force-publish",
        &[
            ("namespaceId", "ns-1"),
            ("agentSpecName", "my-spec"),
            ("version", "1.1.0"),
        ],
        serde_json::json!(true),
    )
    .await;
    mount_ai_write(
        &server,
        "POST",
        "/v3/admin/ai/agentspecs/redraft",
        &[
            ("namespaceId", "ns-1"),
            ("agentSpecName", "my-spec"),
            ("version", "1.1.0"),
        ],
        serde_json::json!(true),
    )
    .await;
    mount_ai_write(
        &server,
        "PUT",
        "/v3/admin/ai/agentspecs/labels",
        &[
            ("namespaceId", "ns-1"),
            ("agentSpecName", "my-spec"),
            ("labels", labels),
        ],
        serde_json::json!(true),
    )
    .await;
    mount_ai_write(
        &server,
        "PUT",
        "/v3/admin/ai/agentspecs/biz-tags",
        &[
            ("namespaceId", "ns-1"),
            ("agentSpecName", "my-spec"),
            ("bizTags", tags),
        ],
        serde_json::json!(true),
    )
    .await;
    mount_ai_write(
        &server,
        "POST",
        "/v3/admin/ai/agentspecs/online",
        &[
            ("namespaceId", "ns-1"),
            ("agentSpecName", "my-spec"),
            ("scope", "spec"),
            ("version", "1.1.0"),
        ],
        serde_json::json!(true),
    )
    .await;
    mount_ai_write(
        &server,
        "POST",
        "/v3/admin/ai/agentspecs/offline",
        &[("namespaceId", "ns-1"), ("agentSpecName", "my-spec")],
        serde_json::json!(true),
    )
    .await;
    mount_ai_write(
        &server,
        "PUT",
        "/v3/admin/ai/agentspecs/scope",
        &[
            ("namespaceId", "ns-1"),
            ("agentSpecName", "my-spec"),
            ("scope", "PRIVATE"),
        ],
        serde_json::json!(true),
    )
    .await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ai/agentspecs/list"))
        .and(query_param("namespaceId", "ns-1"))
        .and(query_param("agentSpecName", "my-spec"))
        .and(query_param("search", "blur"))
        .and(query_param("orderBy", "download_count"))
        .and(query_param("owner", "alice"))
        .and(query_param("scope", "PUBLIC"))
        .and(query_param("pageNo", "2"))
        .and(query_param("pageSize", "5"))
        .respond_with(api_ok_raw(page_json(
            &[serde_json::json!({"name": "my-spec"})],
            1,
        )))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let page = client
        .agentspec_list_admin_items(
            "ns-1",
            Some("my-spec"),
            Some("blur"),
            Some("download_count"),
            Some("alice"),
            Some("PUBLIC"),
            2,
            5,
        )
        .await
        .unwrap();
    assert_eq!(
        page.page_items[0].governance.name.as_deref(),
        Some("my-spec")
    );
    assert_eq!(
        client
            .agentspec_create_draft(
                "ns-1",
                "my-spec",
                Some("1.0.0"),
                Some("1.1.0"),
                Some(card),
                Some("msg"),
            )
            .await
            .unwrap(),
        "1.1.0"
    );
    assert!(
        client
            .agentspec_update_draft(
                "ns-1",
                Some("my-spec"),
                Some("1.1.0"),
                card,
                Some(false),
                None,
            )
            .await
            .unwrap()
    );
    assert!(
        client
            .agentspec_delete_draft("ns-1", "my-spec")
            .await
            .unwrap()
    );
    assert_eq!(
        client
            .agentspec_submit("ns-1", "my-spec", "1.1.0")
            .await
            .unwrap(),
        "1.1.0"
    );
    assert!(
        client
            .agentspec_publish("ns-1", "my-spec", "1.1.0", Some(true))
            .await
            .unwrap()
    );
    assert!(
        client
            .agentspec_force_publish("ns-1", "my-spec", "1.1.0", None)
            .await
            .unwrap()
    );
    assert!(
        client
            .agentspec_redraft("ns-1", "my-spec", "1.1.0")
            .await
            .unwrap()
    );
    assert!(
        client
            .agentspec_update_labels("ns-1", "my-spec", labels)
            .await
            .unwrap()
    );
    assert!(
        client
            .agentspec_update_biz_tags("ns-1", "my-spec", tags)
            .await
            .unwrap()
    );
    assert!(
        client
            .agentspec_change_online_status("ns-1", "my-spec", Some("spec"), Some("1.1.0"), true,)
            .await
            .unwrap()
    );
    assert!(
        client
            .agentspec_change_online_status("ns-1", "my-spec", None, None, false)
            .await
            .unwrap()
    );
    assert!(
        client
            .agentspec_update_scope("ns-1", "my-spec", "PRIVATE")
            .await
            .unwrap()
    );
}

/// The four MCP resource updates that batch 1 left out.
#[tokio::test]
async fn test_mcp_write_lifecycle() {
    let server = MockServer::start().await;
    let labels = r#"{"latest":"1.0.0"}"#;
    mount_ai_write(
        &server,
        "DELETE",
        "/v3/admin/ai/mcp/draft",
        &[
            ("namespaceId", "ns-1"),
            ("mcpName", "my-mcp"),
            ("version", "1.1.0"),
        ],
        serde_json::json!(true),
    )
    .await;
    mount_ai_write(
        &server,
        "PUT",
        "/v3/admin/ai/mcp/labels",
        &[
            ("namespaceId", "ns-1"),
            ("mcpName", "my-mcp"),
            ("labels", labels),
        ],
        serde_json::json!({"latest": "1.0.0"}),
    )
    .await;
    mount_ai_write(
        &server,
        "PUT",
        "/v3/admin/ai/mcp/status",
        &[
            ("namespaceId", "ns-1"),
            ("mcpName", "my-mcp"),
            ("enabled", "false"),
        ],
        serde_json::json!(true),
    )
    .await;
    mount_ai_write(
        &server,
        "PUT",
        "/v3/admin/ai/mcp/scope",
        &[
            ("namespaceId", "ns-1"),
            ("mcpName", "my-mcp"),
            ("scope", "PRIVATE"),
        ],
        serde_json::json!(true),
    )
    .await;

    let client = make_client(&server).await;
    assert!(
        client
            .mcp_server_delete_draft("ns-1", "my-mcp", "1.1.0")
            .await
            .unwrap()
    );
    let updated = client
        .mcp_server_update_labels("ns-1", "my-mcp", labels)
        .await
        .unwrap();
    assert_eq!(updated.get("latest").map(String::as_str), Some("1.0.0"));
    assert!(
        client
            .mcp_server_update_status("ns-1", "my-mcp", false)
            .await
            .unwrap()
    );
    assert!(
        client
            .mcp_server_update_scope("ns-1", "my-mcp", "PRIVATE")
            .await
            .unwrap()
    );
}

/// A non-2xx write must surface as an error for POST, PUT and DELETE alike.
#[tokio::test]
async fn test_ai_write_error_paths() {
    let server = MockServer::start().await;
    for (verb, route) in [
        ("POST", "/v3/admin/ai/skills/draft"),
        ("PUT", "/v3/admin/ai/skills/labels"),
        ("DELETE", "/v3/admin/ai/skills/draft"),
    ] {
        Mock::given(method(verb))
            .and(path(route))
            .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
            .mount(&server)
            .await;
    }

    let client = make_client(&server).await;
    assert!(
        client
            .skill_create_draft("ns-1", Some("demo"), None, None, Some("{}"), None)
            .await
            .is_err()
    );
    assert!(
        client
            .skill_update_labels("ns-1", "demo", "{}")
            .await
            .is_err()
    );
    assert!(client.skill_delete_draft("ns-1", "demo").await.is_err());
}

/// A successful HTTP status with a failing envelope code is still a failed
/// write: GA checks `ErrorCode.SUCCESS` rather than the transport status.
#[tokio::test]
async fn test_ai_write_failure_code_is_false() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/v3/admin/ai/skills/labels"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "code": 500,
            "message": "denied",
            "data": false
        })))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    assert!(
        !client
            .skill_update_labels("ns-1", "demo", "{}")
            .await
            .unwrap()
    );
}

// ============================================================================
// F-NAC-MAINT-004/005/008 — GA form wire, `version`, Page and new wrappers
// ============================================================================

/// Parse a raw urlencoded body/query into an exact key->value map. Every value
/// is percent-decoded, so JSON-string form values round-trip back to their
/// original `{"...":...}` text.
fn parse_urlencoded(raw: &str) -> HashMap<String, String> {
    serde_urlencoded::from_str(raw).unwrap_or_default()
}

/// Assert the EXACT urlencoded BODY field table. Missing, extra or renamed
/// fields all make the parsed map differ from `expected` and fail the match.
struct ExactForm(Vec<(String, String)>);

impl Match for ExactForm {
    fn matches(&self, request: &Request) -> bool {
        let body = String::from_utf8_lossy(&request.body);
        let parsed = parse_urlencoded(&body);
        let expected: HashMap<String, String> = self.0.iter().cloned().collect();
        parsed == expected
    }
}

/// Assert the EXACT urlencoded QUERY field table.
struct ExactQuery(Vec<(String, String)>);

impl Match for ExactQuery {
    fn matches(&self, request: &Request) -> bool {
        let query = request.url.query().unwrap_or("");
        let parsed = parse_urlencoded(query);
        let expected: HashMap<String, String> = self.0.iter().cloned().collect();
        parsed == expected
    }
}

fn draft_create_form() -> AgentDraftCreateForm {
    AgentDraftCreateForm {
        namespace_id: "ns-1".to_string(),
        agent_name: "my-agent".to_string(),
        version: "1.0".to_string(),
        display_name: Some("My Agent".to_string()),
        description: Some("desc".to_string()),
        icon_url: Some("http://icon".to_string()),
        provider: Some("{\"org\":\"acme\"}".to_string()),
        tags: Some("[\"chat\"]".to_string()),
        extensions: Some("{\"x\":1}".to_string()),
        call_interfaces: Some("[]".to_string()),
        author: Some("alice".to_string()),
        change_description: Some("initial".to_string()),
        based_on_version: Some("0.9".to_string()),
    }
}

fn agent_summary_json() -> serde_json::Value {
    serde_json::json!({
        "namespaceId": "ns-1",
        "agentName": "my-agent",
        "displayName": "My Agent",
        "status": "online",
        "versionInfo": {"editingVersion": "1.0"},
        "serverOnlyField": 7
    })
}

fn runtime_endpoint_snapshot_json(version: &str) -> serde_json::Value {
    serde_json::json!({
        "namespaceId": "ns-1",
        "agentName": "my-agent",
        "version": version,
        "callInterface": {"transport": "JSONRPC"},
        "batataSpecific": {"a": 1}
    })
}

fn ga_runtime_endpoint_snapshot_json() -> serde_json::Value {
    serde_json::json!({
        "namespaceId": "public",
        "agentName": "demo",
        "version": "1.0.0",
        "callInterface": {
            "protocol": "a2a",
            "endpointSets": [{
                "source": "RUNTIME",
                "lastUpdatedTime": 1763000000000i64,
                "endpoints": [{
                    "uri": "http://127.0.0.1:8080/a2a",
                    "transport": "HTTP+JSON",
                    "healthy": true,
                    "enabled": true,
                    "bindings": [{
                        "runtimeVersion": "1.0.6",
                        "versionRange": "[1.0.0,2.0.0)"
                    }]
                }]
            }]
        }
    })
}

/// GA `getAgentCard` binds AND consumes `version`; the new method must emit it
/// as a query parameter. A mutant dropping `version` loses this match (404).
#[tokio::test]
async fn test_agent_get_with_version_sends_version_query() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ai/a2a"))
        .and(query_param("version", "2.0"))
        .respond_with(api_ok_raw(serde_json::json!({
            "name": "my-agent", "registrationType": "direct"
        })))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let detail = client
        .agent_get_with_version("my-agent", "ns-1", "direct", Some("2.0"))
        .await
        .unwrap();
    assert_eq!(detail.agent_card.basic_info.name, "my-agent");
}

/// With `None`, `skip_serializing_if` must drop `version` entirely, leaving
/// exactly the legacy three-field query (no stray `version=` key).
#[tokio::test]
async fn test_agent_get_with_version_omits_none() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ai/a2a"))
        .and(ExactQuery(vec![
            ("agentName".into(), "my-agent".into()),
            ("namespaceId".into(), "ns-1".into()),
            ("registrationType".into(), "direct".into()),
        ]))
        .respond_with(api_ok_raw(serde_json::json!({
            "name": "my-agent", "registrationType": "direct"
        })))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let detail = client
        .agent_get_with_version("my-agent", "ns-1", "direct", None)
        .await
        .unwrap();
    assert_eq!(detail.registration_type, "direct");
}

/// `agents_create_draft` must POST the exact 13-field GA urlencoded form.
#[tokio::test]
async fn test_agents_create_draft_uses_ga_form() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v3/admin/ai/agents/draft"))
        .and(ExactForm(vec![
            ("namespaceId".into(), "ns-1".into()),
            ("agentName".into(), "my-agent".into()),
            ("version".into(), "1.0".into()),
            ("displayName".into(), "My Agent".into()),
            ("description".into(), "desc".into()),
            ("iconUrl".into(), "http://icon".into()),
            ("provider".into(), "{\"org\":\"acme\"}".into()),
            ("tags".into(), "[\"chat\"]".into()),
            ("extensions".into(), "{\"x\":1}".into()),
            ("callInterfaces".into(), "[]".into()),
            ("author".into(), "alice".into()),
            ("changeDescription".into(), "initial".into()),
            ("basedOnVersion".into(), "0.9".into()),
        ]))
        .respond_with(api_ok_raw(agent_version_json()))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let created = client
        .agents_create_draft(&draft_create_form())
        .await
        .unwrap();
    assert_eq!(created.version, "1.0");
}

/// `agents_update_draft` must PUT the exact 5-field GA urlencoded form.
#[tokio::test]
async fn test_agents_update_draft_uses_ga_form() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/v3/admin/ai/agents/draft"))
        .and(ExactForm(vec![
            ("namespaceId".into(), "ns-1".into()),
            ("agentName".into(), "my-agent".into()),
            ("version".into(), "1.0".into()),
            ("callInterfaces".into(), "[]".into()),
            ("changeDescription".into(), "edit".into()),
        ]))
        .respond_with(api_ok_raw(agent_version_json()))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let form = AgentDraftUpdateForm {
        namespace_id: "ns-1".to_string(),
        agent_name: "my-agent".to_string(),
        version: "1.0".to_string(),
        call_interfaces: Some("[]".to_string()),
        change_description: Some("edit".to_string()),
    };
    let updated = client.agents_update_draft(&form).await.unwrap();
    assert_eq!(updated.status, "online");
}

/// `agents_update_labels` must PUT `labels` as ONE JSON-string form value.
/// A mutant that does not JSON-ify the map (e.g. flattened keys) changes the
/// body so `ExactForm` no longer matches.
#[tokio::test]
async fn test_agents_update_labels_uses_ga_form() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/v3/admin/ai/agents/labels"))
        .and(ExactForm(vec![
            ("namespaceId".into(), "ns-1".into()),
            ("agentName".into(), "my-agent".into()),
            ("labels".into(), "{\"stable\":\"1.0\"}".into()),
        ]))
        .respond_with(api_ok_raw(agent_summary_json()))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let mut labels = HashMap::new();
    labels.insert("stable".to_string(), "1.0".to_string());
    let summary = client
        .agents_update_labels("ns-1", "my-agent", &labels)
        .await
        .unwrap();
    assert_eq!(summary.agent_name, "my-agent");
    // GA returns AgentSummary; unknown fields stay reachable through `extra`.
    assert!(summary.extra.contains_key("serverOnlyField"));
}

/// `agents_update_agent` must PUT the exact 9-field GA form with NO version.
#[tokio::test]
async fn test_agents_update_agent_uses_ga_form() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/v3/admin/ai/agents"))
        .and(ExactForm(vec![
            ("namespaceId".into(), "ns-1".into()),
            ("agentName".into(), "my-agent".into()),
            ("displayName".into(), "My Agent".into()),
            ("description".into(), "desc".into()),
            ("iconUrl".into(), "http://icon".into()),
            ("provider".into(), "{}".into()),
            ("tags".into(), "[]".into()),
            ("extensions".into(), "{}".into()),
            ("status".into(), "online".into()),
        ]))
        .respond_with(api_ok_raw(agent_summary_json()))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let form = AgentUpdateForm {
        namespace_id: "ns-1".to_string(),
        agent_name: "my-agent".to_string(),
        display_name: Some("My Agent".to_string()),
        description: Some("desc".to_string()),
        icon_url: Some("http://icon".to_string()),
        provider: Some("{}".to_string()),
        tags: Some("[]".to_string()),
        extensions: Some("{}".to_string()),
        status: Some("online".to_string()),
    };
    let summary = client.agents_update_agent(&form).await.unwrap();
    assert_eq!(summary.status, "online");
}

/// `agents_delete_agent` must DELETE with ONLY namespaceId/agentName in the
/// query (no version) and report success through the envelope code.
#[tokio::test]
async fn test_agents_delete_agent_uses_query() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/v3/admin/ai/agents"))
        .and(ExactQuery(vec![
            ("namespaceId".into(), "ns-1".into()),
            ("agentName".into(), "my-agent".into()),
        ]))
        .respond_with(api_ok_raw(serde_json::json!(null)))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    assert!(
        client
            .agents_delete_agent("ns-1", "my-agent")
            .await
            .unwrap()
    );
}

/// A failing envelope code is still a failed delete (error-code path).
#[tokio::test]
async fn test_agents_delete_agent_error_code_is_false() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/v3/admin/ai/agents"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "code": 500, "message": "denied", "data": null
        })))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    assert!(
        !client
            .agents_delete_agent("ns-1", "my-agent")
            .await
            .unwrap()
    );
}

/// `agents_get_runtime_endpoints` must GET the new path and forward all four
/// query fields; the response is decoded tolerantly into the GA DTO.
#[tokio::test]
async fn test_agents_get_runtime_endpoints_forwards_all_query_fields() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ai/agents/runtime-endpoints"))
        .and(ExactQuery(vec![
            ("namespaceId".into(), "ns-1".into()),
            ("agentName".into(), "my-agent".into()),
            ("protocol".into(), "JSONRPC".into()),
            ("version".into(), "1.0".into()),
        ]))
        .respond_with(api_ok_raw(runtime_endpoint_snapshot_json("1.0")))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let snapshot = client
        .agents_get_runtime_endpoints("ns-1", "my-agent", Some("JSONRPC"), Some("1.0"))
        .await
        .unwrap();
    assert_eq!(snapshot.version, "1.0");
    assert_eq!(snapshot.agent_name, "my-agent");
    assert!(snapshot.extra.contains_key("batataSpecific"));
}

/// With `version: None` the wrapper must omit the `version` query key entirely.
#[tokio::test]
async fn test_agents_get_runtime_endpoints_omits_none_version() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ai/agents/runtime-endpoints"))
        .and(ExactQuery(vec![
            ("namespaceId".into(), "ns-1".into()),
            ("agentName".into(), "my-agent".into()),
            ("protocol".into(), "JSONRPC".into()),
        ]))
        .respond_with(api_ok_raw(runtime_endpoint_snapshot_json("9.9")))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let snapshot = client
        .agents_get_runtime_endpoints("ns-1", "my-agent", Some("JSONRPC"), None)
        .await
        .unwrap();
    assert_eq!(snapshot.version, "9.9");
}

/// The admin endpoint now returns GA's flat `RuntimeEndpointSnapshot`; the
/// wrapper must decode the `endpointSets` tree rather than leaving it raw.
#[tokio::test]
async fn test_agents_get_runtime_endpoints_decodes_ga_snapshot() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/admin/ai/agents/runtime-endpoints"))
        .respond_with(api_ok_raw(ga_runtime_endpoint_snapshot_json()))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let snapshot = client
        .agents_get_runtime_endpoints("public", "demo", Some("a2a"), Some("1.0.0"))
        .await
        .unwrap();
    assert_eq!(snapshot.namespace_id, "public");
    assert_eq!(snapshot.agent_name, "demo");
    assert_eq!(snapshot.version, "1.0.0");
    assert!(
        snapshot.extra.is_empty(),
        "GA data is flat, not a console view"
    );
    let call = snapshot.call_interface.expect("callInterface must decode");
    assert_eq!(call.protocol, "a2a");
    assert_eq!(call.endpoint_sets.len(), 1);
    assert_eq!(call.endpoint_sets[0].source, "RUNTIME");
    assert_eq!(
        call.endpoint_sets[0].last_updated_time,
        Some(1763000000000i64)
    );
    let endpoint = &call.endpoint_sets[0].endpoints[0];
    assert_eq!(endpoint.uri, "http://127.0.0.1:8080/a2a");
    assert_eq!(endpoint.healthy, Some(true));
    assert_eq!(endpoint.enabled, Some(true));
    assert_eq!(endpoint.bindings[0].runtime_version, "1.0.6");
}

/// New write wrapper surfaces a transport error (error-code path).
#[tokio::test]
async fn test_agents_update_agent_http_error_is_err() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/v3/admin/ai/agents"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(&server)
        .await;

    let client = make_client(&server).await;
    let form = AgentUpdateForm {
        namespace_id: "ns-1".to_string(),
        agent_name: "my-agent".to_string(),
        ..Default::default()
    };
    assert!(client.agents_update_agent(&form).await.is_err());
}
