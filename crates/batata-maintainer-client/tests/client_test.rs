// Integration tests for MaintainerClient using wiremock HTTP mock server

use std::collections::HashMap;

use batata_client::HttpClientConfig;
use batata_maintainer_client::MaintainerClient;
use batata_maintainer_client::model::{
    AgentCard, AgentCardBasicInfo, ClusterInfo, ConfigCloneInfo, Instance, McpServerBasicInfo,
    SameConfigPolicy,
};
use wiremock::matchers::{method, path};
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
        .and(path("/v3/admin/core/namespace/exist"))
        .respond_with(api_ok(&true))
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
                "triggerFlag": false
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
        .service_create("", "DEFAULT_GROUP", "new-svc", 0.8, "", "")
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
