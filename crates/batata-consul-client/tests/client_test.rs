use batata_consul_client::*;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn make_client(server: &MockServer) -> ConsulClient {
    let config = ConsulClientConfig::new(&server.uri()).with_token("test-token");
    ConsulClient::new(config).unwrap()
}

// ==================== KV Tests ====================

#[tokio::test]
async fn test_kv_get() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/kv/my-key"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!([{
                    "Key": "my-key",
                    "Value": "aGVsbG8=",
                    "CreateIndex": 100,
                    "ModifyIndex": 200,
                    "LockIndex": 0,
                    "Flags": 0
                }]))
                .append_header("X-Consul-Index", "200"),
        )
        .mount(&server)
        .await;

    let (pair, meta) = client
        .kv_get("my-key", &QueryOptions::default())
        .await
        .unwrap();
    let pair = pair.unwrap();
    assert_eq!(pair.key, "my-key");
    assert_eq!(pair.value_str().unwrap(), "hello");
    assert_eq!(meta.last_index, 200);
}

#[tokio::test]
async fn test_kv_get_not_found() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/kv/missing"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let (pair, _) = client
        .kv_get("missing", &QueryOptions::default())
        .await
        .unwrap();
    assert!(pair.is_none());
}

#[tokio::test]
async fn test_kv_list() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/kv/prefix/"))
        .and(query_param("recurse", ""))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {"Key": "prefix/a", "Value": "YQ==", "CreateIndex": 1, "ModifyIndex": 1, "LockIndex": 0, "Flags": 0},
                {"Key": "prefix/b", "Value": "Yg==", "CreateIndex": 2, "ModifyIndex": 2, "LockIndex": 0, "Flags": 0}
            ])),
        )
        .mount(&server)
        .await;

    let (pairs, _) = client
        .kv_list("prefix/", &QueryOptions::default())
        .await
        .unwrap();
    assert_eq!(pairs.len(), 2);
    assert_eq!(pairs[0].key, "prefix/a");
}

#[tokio::test]
async fn test_kv_keys() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/kv/prefix/"))
        .and(query_param("keys", ""))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!(["prefix/a", "prefix/b"])),
        )
        .mount(&server)
        .await;

    let (keys, _) = client
        .kv_keys("prefix/", "", &QueryOptions::default())
        .await
        .unwrap();
    assert_eq!(keys, vec!["prefix/a", "prefix/b"]);
}

#[tokio::test]
async fn test_kv_put() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("PUT"))
        .and(path("/v1/kv/test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_string("true"))
        .mount(&server)
        .await;

    use base64::Engine;
    let pair = KVPair {
        key: "test-key".to_string(),
        value: Some(base64::engine::general_purpose::STANDARD.encode("test-value")),
        ..Default::default()
    };

    let (ok, _) = client
        .kv_put(&pair, &WriteOptions::default())
        .await
        .unwrap();
    assert!(ok);
}

#[tokio::test]
async fn test_kv_delete() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("DELETE"))
        .and(path("/v1/kv/test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_string("true"))
        .mount(&server)
        .await;

    let (ok, _) = client
        .kv_delete("test-key", &WriteOptions::default())
        .await
        .unwrap();
    assert!(ok);
}

#[tokio::test]
async fn test_kv_delete_tree() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("DELETE"))
        .and(path("/v1/kv/prefix/"))
        .and(query_param("recurse", ""))
        .respond_with(ResponseTemplate::new(200).set_body_string("true"))
        .mount(&server)
        .await;

    let (ok, _) = client
        .kv_delete_tree("prefix/", &WriteOptions::default())
        .await
        .unwrap();
    assert!(ok);
}

#[tokio::test]
async fn test_kv_cas() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("PUT"))
        .and(path("/v1/kv/cas-key"))
        .and(query_param("cas", "42"))
        .respond_with(ResponseTemplate::new(200).set_body_string("true"))
        .mount(&server)
        .await;

    use base64::Engine;
    let pair = KVPair {
        key: "cas-key".to_string(),
        modify_index: 42,
        value: Some(base64::engine::general_purpose::STANDARD.encode("new-value")),
        ..Default::default()
    };

    let (ok, _) = client
        .kv_cas(&pair, &WriteOptions::default())
        .await
        .unwrap();
    assert!(ok);
}

#[tokio::test]
async fn test_kv_acquire_release() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("PUT"))
        .and(path("/v1/kv/lock-key"))
        .and(query_param("acquire", "session-123"))
        .respond_with(ResponseTemplate::new(200).set_body_string("true"))
        .mount(&server)
        .await;

    let pair = KVPair {
        key: "lock-key".to_string(),
        session: Some("session-123".to_string()),
        ..Default::default()
    };

    let (ok, _) = client
        .kv_acquire(&pair, &WriteOptions::default())
        .await
        .unwrap();
    assert!(ok);
}

// ==================== Agent Tests ====================

#[tokio::test]
async fn test_agent_services() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/agent/services"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "web": {
                "ID": "web",
                "Service": "web",
                "Tags": ["v1"],
                "Port": 8080,
                "Address": "10.0.0.1",
                "EnableTagOverride": false
            }
        })))
        .mount(&server)
        .await;

    let (services, _) = client
        .agent_services(&QueryOptions::default())
        .await
        .unwrap();
    assert_eq!(services.len(), 1);
    let svc = services.get("web").unwrap();
    assert_eq!(svc.id, "web");
    assert_eq!(svc.port, 8080);
}

#[tokio::test]
async fn test_agent_service_register() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("PUT"))
        .and(path("/v1/agent/service/register"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let reg = AgentServiceRegistration {
        name: "my-service".to_string(),
        port: Some(8080),
        tags: Some(vec!["v1".to_string()]),
        ..Default::default()
    };

    client
        .agent_service_register(&reg, &WriteOptions::default())
        .await
        .unwrap();
}

#[tokio::test]
async fn test_agent_service_deregister() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("PUT"))
        .and(path("/v1/agent/service/deregister/my-service"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    client
        .agent_service_deregister("my-service", &WriteOptions::default())
        .await
        .unwrap();
}

#[tokio::test]
async fn test_agent_checks() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/agent/checks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "service:web": {
                "Node": "node1",
                "CheckID": "service:web",
                "Name": "Service 'web' check",
                "Status": "passing",
                "Notes": "",
                "Output": "HTTP GET http://localhost:8080: 200 OK",
                "ServiceID": "web",
                "ServiceName": "web",
                "CreateIndex": 1,
                "ModifyIndex": 2
            }
        })))
        .mount(&server)
        .await;

    let (checks, _) = client.agent_checks(&QueryOptions::default()).await.unwrap();
    assert_eq!(checks.len(), 1);
    let check = checks.get("service:web").unwrap();
    assert_eq!(check.status, "passing");
}

#[tokio::test]
async fn test_agent_members() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/agent/members"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "Name": "node1",
                "Addr": "10.0.0.1",
                "Port": 8301,
                "Status": 1,
                "ProtocolMin": 1,
                "ProtocolMax": 5,
                "ProtocolCur": 2,
                "DelegateMin": 2,
                "DelegateMax": 5,
                "DelegateCur": 4
            }
        ])))
        .mount(&server)
        .await;

    let (members, _) = client
        .agent_members(false, &QueryOptions::default())
        .await
        .unwrap();
    assert_eq!(members.len(), 1);
    assert_eq!(members[0].name, "node1");
}

#[tokio::test]
async fn test_agent_self() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/agent/self"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"Config": {"Datacenter": "dc1"}})),
        )
        .mount(&server)
        .await;

    let (info, _) = client.agent_self(&QueryOptions::default()).await.unwrap();
    assert_eq!(info["Config"]["Datacenter"], "dc1");
}

#[tokio::test]
async fn test_agent_join() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("PUT"))
        .and(path("/v1/agent/join/10.0.0.2"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    client
        .agent_join("10.0.0.2", false, &WriteOptions::default())
        .await
        .unwrap();
}

#[tokio::test]
async fn test_agent_leave() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("PUT"))
        .and(path("/v1/agent/leave"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    client.agent_leave(&WriteOptions::default()).await.unwrap();
}

#[tokio::test]
async fn test_agent_force_leave() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("PUT"))
        .and(path("/v1/agent/force-leave/bad-node"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    client
        .agent_force_leave("bad-node", &WriteOptions::default())
        .await
        .unwrap();
}

#[tokio::test]
async fn test_agent_maintenance() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("PUT"))
        .and(path("/v1/agent/maintenance"))
        .and(query_param("enable", "true"))
        .and(query_param("reason", "upgrade"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    client
        .agent_node_maintenance(true, "upgrade", &WriteOptions::default())
        .await
        .unwrap();
}

// ==================== Health Tests ====================

#[tokio::test]
async fn test_health_node() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/health/node/node1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "Node": "node1",
                "CheckID": "serfHealth",
                "Name": "Serf Health Status",
                "Status": "passing",
                "Notes": "",
                "Output": "Agent alive",
                "ServiceID": "",
                "ServiceName": "",
                "CreateIndex": 1,
                "ModifyIndex": 2
            }
        ])))
        .mount(&server)
        .await;

    let (checks, _) = client
        .health_node("node1", &QueryOptions::default())
        .await
        .unwrap();
    assert_eq!(checks.len(), 1);
    assert_eq!(checks[0].status, "passing");
}

#[tokio::test]
async fn test_health_service() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/health/service/web"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "Node": {
                    "Node": "node1",
                    "Address": "10.0.0.1",
                    "CreateIndex": 1,
                    "ModifyIndex": 2
                },
                "Service": {
                    "ID": "web",
                    "Service": "web",
                    "Port": 8080,
                    "Address": "10.0.0.1",
                    "EnableTagOverride": false
                },
                "Checks": []
            }
        ])))
        .mount(&server)
        .await;

    let (entries, _) = client
        .health_service("web", "", false, &QueryOptions::default())
        .await
        .unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].service.port, 8080);
}

#[tokio::test]
async fn test_health_service_passing_only() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/health/service/web"))
        .and(query_param("passing", ""))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    let (entries, _) = client
        .health_service("web", "", true, &QueryOptions::default())
        .await
        .unwrap();
    assert_eq!(entries.len(), 0);
}

#[tokio::test]
async fn test_health_state() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/health/state/critical"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    let (checks, _) = client
        .health_state("critical", &QueryOptions::default())
        .await
        .unwrap();
    assert_eq!(checks.len(), 0);
}

#[tokio::test]
async fn test_health_checks() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/health/checks/web"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!([{
                "Node": "node1",
                "CheckID": "service:web",
                "Name": "Web check",
                "Status": "passing",
                "Notes": "",
                "Output": "OK",
                "ServiceID": "web",
                "ServiceName": "web",
                "CreateIndex": 1,
                "ModifyIndex": 2
            }])),
        )
        .mount(&server)
        .await;

    let (checks, _) = client
        .health_checks("web", &QueryOptions::default())
        .await
        .unwrap();
    assert_eq!(checks.len(), 1);
}

// ==================== Catalog Tests ====================

#[tokio::test]
async fn test_catalog_datacenters() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/catalog/datacenters"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!(["dc1", "dc2"])))
        .mount(&server)
        .await;

    let (dcs, _) = client
        .catalog_datacenters(&QueryOptions::default())
        .await
        .unwrap();
    assert_eq!(dcs, vec!["dc1", "dc2"]);
}

#[tokio::test]
async fn test_catalog_nodes() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/catalog/nodes"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "Node": "node1",
                "Address": "10.0.0.1",
                "CreateIndex": 1,
                "ModifyIndex": 2
            }
        ])))
        .mount(&server)
        .await;

    let (nodes, _) = client
        .catalog_nodes(&QueryOptions::default())
        .await
        .unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].node, "node1");
}

#[tokio::test]
async fn test_catalog_services() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/catalog/services"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "consul": [],
            "web": ["v1", "production"]
        })))
        .mount(&server)
        .await;

    let (services, _) = client
        .catalog_services(&QueryOptions::default())
        .await
        .unwrap();
    assert_eq!(services.len(), 2);
    assert_eq!(services["web"], vec!["v1", "production"]);
}

#[tokio::test]
async fn test_catalog_service() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/catalog/service/web"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "Node": "node1",
                "Address": "10.0.0.1",
                "ServiceID": "web-1",
                "ServiceName": "web",
                "ServiceAddress": "10.0.0.1",
                "ServicePort": 8080,
                "ServiceEnableTagOverride": false,
                "CreateIndex": 1,
                "ModifyIndex": 2
            }
        ])))
        .mount(&server)
        .await;

    let (services, _) = client
        .catalog_service("web", "", &QueryOptions::default())
        .await
        .unwrap();
    assert_eq!(services.len(), 1);
    assert_eq!(services[0].service_port, 8080);
}

#[tokio::test]
async fn test_catalog_node() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/catalog/node/node1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "Node": {
                "Node": "node1",
                "Address": "10.0.0.1",
                "CreateIndex": 1,
                "ModifyIndex": 2
            },
            "Services": {}
        })))
        .mount(&server)
        .await;

    let (node, _) = client
        .catalog_node("node1", &QueryOptions::default())
        .await
        .unwrap();
    let node = node.unwrap();
    assert_eq!(node.node.unwrap().node, "node1");
}

#[tokio::test]
async fn test_catalog_register() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("PUT"))
        .and(path("/v1/catalog/register"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let reg = CatalogRegistration {
        node: "node1".to_string(),
        address: "10.0.0.1".to_string(),
        ..Default::default()
    };

    client
        .catalog_register(&reg, &WriteOptions::default())
        .await
        .unwrap();
}

#[tokio::test]
async fn test_catalog_deregister() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("PUT"))
        .and(path("/v1/catalog/deregister"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let dereg = CatalogDeregistration {
        node: "node1".to_string(),
        ..Default::default()
    };

    client
        .catalog_deregister(&dereg, &WriteOptions::default())
        .await
        .unwrap();
}

// ==================== Session Tests ====================

#[tokio::test]
async fn test_session_create() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("PUT"))
        .and(path("/v1/session/create"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"ID": "session-abc-123"})),
        )
        .mount(&server)
        .await;

    let entry = SessionEntry {
        name: Some("my-session".to_string()),
        ttl: Some("30s".to_string()),
        ..Default::default()
    };

    let (id, _) = client
        .session_create(&entry, &WriteOptions::default())
        .await
        .unwrap();
    assert_eq!(id, "session-abc-123");
}

#[tokio::test]
async fn test_session_destroy() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("PUT"))
        .and(path("/v1/session/destroy/session-abc-123"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    client
        .session_destroy("session-abc-123", &WriteOptions::default())
        .await
        .unwrap();
}

#[tokio::test]
async fn test_session_info() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/session/info/session-abc-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "ID": "session-abc-123",
                "Name": "my-session",
                "Node": "node1",
                "CreateIndex": 100,
                "ModifyIndex": 100
            }
        ])))
        .mount(&server)
        .await;

    let (sessions, _) = client
        .session_info("session-abc-123", &QueryOptions::default())
        .await
        .unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id.as_deref(), Some("session-abc-123"));
}

#[tokio::test]
async fn test_session_list() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/session/list"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    let (sessions, _) = client.session_list(&QueryOptions::default()).await.unwrap();
    assert_eq!(sessions.len(), 0);
}

#[tokio::test]
async fn test_session_renew() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("PUT"))
        .and(path("/v1/session/renew/session-abc-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "ID": "session-abc-123",
                "Name": "my-session",
                "CreateIndex": 100,
                "ModifyIndex": 100
            }
        ])))
        .mount(&server)
        .await;

    let (sessions, _) = client
        .session_renew("session-abc-123", &WriteOptions::default())
        .await
        .unwrap();
    assert_eq!(sessions.len(), 1);
}

#[tokio::test]
async fn test_session_node() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/session/node/node1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    let (sessions, _) = client
        .session_node("node1", &QueryOptions::default())
        .await
        .unwrap();
    assert_eq!(sessions.len(), 0);
}

// ==================== Status Tests ====================

#[tokio::test]
async fn test_status_leader() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/status/leader"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!("10.0.0.1:8300")))
        .mount(&server)
        .await;

    let (leader, _) = client
        .status_leader(&QueryOptions::default())
        .await
        .unwrap();
    assert_eq!(leader, "10.0.0.1:8300");
}

#[tokio::test]
async fn test_status_peers() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/status/peers"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!(["10.0.0.1:8300", "10.0.0.2:8300"])),
        )
        .mount(&server)
        .await;

    let (peers, _) = client.status_peers(&QueryOptions::default()).await.unwrap();
    assert_eq!(peers.len(), 2);
}

// ==================== Event Tests ====================

#[tokio::test]
async fn test_event_fire() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("PUT"))
        .and(path("/v1/event/fire/deploy"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "ID": "event-123",
            "Name": "deploy",
            "Version": 1,
            "LTime": 42
        })))
        .mount(&server)
        .await;

    let (event, _) = client
        .event_fire("deploy", None, "", "", "", &WriteOptions::default())
        .await
        .unwrap();
    assert_eq!(event.name, "deploy");
    assert_eq!(event.id.as_deref(), Some("event-123"));
}

#[tokio::test]
async fn test_event_list() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/event/list"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"ID": "event-1", "Name": "deploy", "Version": 1, "LTime": 1},
            {"ID": "event-2", "Name": "deploy", "Version": 1, "LTime": 2}
        ])))
        .mount(&server)
        .await;

    let (events, _) = client
        .event_list("", &QueryOptions::default())
        .await
        .unwrap();
    assert_eq!(events.len(), 2);
}

#[tokio::test]
async fn test_event_list_filtered() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/event/list"))
        .and(query_param("name", "deploy"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    let (events, _) = client
        .event_list("deploy", &QueryOptions::default())
        .await
        .unwrap();
    assert_eq!(events.len(), 0);
}

// ==================== Blocking Query Tests ====================

#[tokio::test]
async fn test_blocking_query_headers() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/catalog/services"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({}))
                .append_header("X-Consul-Index", "42")
                .append_header("X-Consul-KnownLeader", "true")
                .append_header("X-Consul-LastContact", "5"),
        )
        .mount(&server)
        .await;

    let (_, meta) = client
        .catalog_services(&QueryOptions::default())
        .await
        .unwrap();
    assert_eq!(meta.last_index, 42);
    assert!(meta.known_leader);
    assert_eq!(meta.last_contact, 5);
}

#[tokio::test]
async fn test_query_with_datacenter() {
    let server = MockServer::start().await;
    let config = ConsulClientConfig::new(&server.uri())
        .with_token("test-token")
        .with_datacenter("dc2");
    let client = ConsulClient::new(config).unwrap();

    Mock::given(method("GET"))
        .and(path("/v1/catalog/services"))
        .and(query_param("dc", "dc2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .mount(&server)
        .await;

    client
        .catalog_services(&QueryOptions::default())
        .await
        .unwrap();
}

// ==================== Error Tests ====================

#[tokio::test]
async fn test_api_error() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/catalog/services"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&server)
        .await;

    let result = client.catalog_services(&QueryOptions::default()).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        ConsulError::Api { status, message } => {
            assert_eq!(status, 500);
            assert_eq!(message, "Internal Server Error");
        }
        e => panic!("Expected Api error, got: {:?}", e),
    }
}

#[tokio::test]
async fn test_not_found() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/catalog/node/missing"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let (node, _) = client
        .catalog_node("missing", &QueryOptions::default())
        .await
        .unwrap();
    assert!(node.is_none());
}

// ==================== Model Tests ====================

#[test]
fn test_kv_pair_value_decode() {
    let pair = KVPair {
        key: "test".to_string(),
        value: Some("aGVsbG8gd29ybGQ=".to_string()),
        ..Default::default()
    };

    assert_eq!(pair.value_str().unwrap(), "hello world");
    assert_eq!(pair.value_bytes().unwrap(), b"hello world");
}

#[test]
fn test_kv_pair_empty_value() {
    let pair = KVPair {
        key: "test".to_string(),
        value: None,
        ..Default::default()
    };

    assert!(pair.value_str().is_none());
    assert!(pair.value_bytes().is_none());
}

#[test]
fn test_query_options_default() {
    let opts = QueryOptions::default();
    assert_eq!(opts.wait_index, 0);
    assert!(opts.datacenter.is_empty());
    assert!(!opts.require_consistent);
    assert!(!opts.allow_stale);
}

#[test]
fn test_write_options_default() {
    let opts = WriteOptions::default();
    assert!(opts.datacenter.is_empty());
    assert!(opts.token.is_empty());
}

#[test]
fn test_consul_client_config_builder() {
    let config = ConsulClientConfig::new("http://localhost:8500")
        .with_token("my-token")
        .with_datacenter("dc1")
        .with_namespace("default")
        .with_partition("default");

    assert_eq!(config.address, "http://localhost:8500");
    assert_eq!(config.token, "my-token");
    assert_eq!(config.datacenter, "dc1");
    assert_eq!(config.namespace, "default");
    assert_eq!(config.partition, "default");
}

#[test]
fn test_agent_service_registration_json() {
    let reg = AgentServiceRegistration {
        name: "web".to_string(),
        port: Some(8080),
        tags: Some(vec!["v1".to_string()]),
        check: Some(AgentServiceCheck {
            http: Some("http://localhost:8080/health".to_string()),
            interval: Some("10s".to_string()),
            ..Default::default()
        }),
        ..Default::default()
    };

    let json = serde_json::to_value(&reg).unwrap();
    assert_eq!(json["Name"], "web");
    assert_eq!(json["Port"], 8080);
    assert_eq!(json["Tags"], serde_json::json!(["v1"]));
    assert!(json["Check"]["HTTP"].is_string());
}

#[test]
fn test_session_entry_json() {
    let entry = SessionEntry {
        name: Some("my-session".to_string()),
        ttl: Some("30s".to_string()),
        behavior: Some("release".to_string()),
        ..Default::default()
    };

    let json = serde_json::to_value(&entry).unwrap();
    assert_eq!(json["Name"], "my-session");
    assert_eq!(json["TTL"], "30s");
    assert_eq!(json["Behavior"], "release");
}

// ==================== ACL Token Tests ====================

fn sample_token_json() -> serde_json::Value {
    serde_json::json!({
        "AccessorID": "accessor-123",
        "SecretID": "secret-456",
        "Description": "test token",
        "Policies": [{"ID": "policy-1", "Name": "global-management"}],
        "Roles": [{"ID": "role-1", "Name": "admin"}],
        "Local": false,
        "CreateTime": "2024-01-01T00:00:00Z",
        "CreateIndex": 10,
        "ModifyIndex": 20
    })
}

#[tokio::test]
async fn test_acl_bootstrap() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("PUT"))
        .and(path("/v1/acl/bootstrap"))
        .respond_with(ResponseTemplate::new(200).set_body_json(sample_token_json()))
        .mount(&server)
        .await;

    let (token, _) = client
        .acl_bootstrap(&WriteOptions::default())
        .await
        .unwrap();
    assert_eq!(token.accessor_id, "accessor-123");
    assert_eq!(token.secret_id.as_deref(), Some("secret-456"));
}

#[tokio::test]
async fn test_acl_token_create() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("PUT"))
        .and(path("/v1/acl/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(sample_token_json()))
        .mount(&server)
        .await;

    let token = ACLToken {
        description: "my token".to_string(),
        policies: Some(vec![ACLTokenPolicyLink {
            id: "policy-1".to_string(),
            name: "global-management".to_string(),
        }]),
        ..Default::default()
    };

    let (created, _) = client
        .acl_token_create(&token, &WriteOptions::default())
        .await
        .unwrap();
    assert_eq!(created.accessor_id, "accessor-123");
}

#[tokio::test]
async fn test_acl_token_read() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/acl/token/accessor-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(sample_token_json()))
        .mount(&server)
        .await;

    let (token, _) = client
        .acl_token_read("accessor-123", &QueryOptions::default())
        .await
        .unwrap();
    assert_eq!(token.accessor_id, "accessor-123");
    assert_eq!(token.description, "test token");
}

#[tokio::test]
async fn test_acl_token_read_self() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/acl/token/self"))
        .respond_with(ResponseTemplate::new(200).set_body_json(sample_token_json()))
        .mount(&server)
        .await;

    let (token, _) = client
        .acl_token_read_self(&QueryOptions::default())
        .await
        .unwrap();
    assert_eq!(token.accessor_id, "accessor-123");
}

#[tokio::test]
async fn test_acl_token_update() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("PUT"))
        .and(path("/v1/acl/token/accessor-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(sample_token_json()))
        .mount(&server)
        .await;

    let token = ACLToken {
        accessor_id: "accessor-123".to_string(),
        description: "updated".to_string(),
        ..Default::default()
    };

    let (updated, _) = client
        .acl_token_update(&token, &WriteOptions::default())
        .await
        .unwrap();
    assert_eq!(updated.accessor_id, "accessor-123");
}

#[tokio::test]
async fn test_acl_token_clone() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("PUT"))
        .and(path("/v1/acl/token/accessor-123/clone"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "AccessorID": "accessor-789",
            "SecretID": "secret-new",
            "Description": "cloned token",
            "CreateIndex": 30,
            "ModifyIndex": 30
        })))
        .mount(&server)
        .await;

    let (cloned, _) = client
        .acl_token_clone("accessor-123", "cloned token", &WriteOptions::default())
        .await
        .unwrap();
    assert_eq!(cloned.accessor_id, "accessor-789");
    assert_eq!(cloned.description, "cloned token");
}

#[tokio::test]
async fn test_acl_token_delete() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("DELETE"))
        .and(path("/v1/acl/token/accessor-123"))
        .respond_with(ResponseTemplate::new(200).set_body_string("true"))
        .mount(&server)
        .await;

    let (ok, _) = client
        .acl_token_delete("accessor-123", &WriteOptions::default())
        .await
        .unwrap();
    assert!(ok);
}

#[tokio::test]
async fn test_acl_token_list() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/acl/tokens"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "AccessorID": "accessor-1",
                "Description": "token 1",
                "Local": false,
                "CreateIndex": 1,
                "ModifyIndex": 1
            },
            {
                "AccessorID": "accessor-2",
                "Description": "token 2",
                "Local": true,
                "CreateIndex": 2,
                "ModifyIndex": 2
            }
        ])))
        .mount(&server)
        .await;

    let (tokens, _) = client
        .acl_token_list(&QueryOptions::default())
        .await
        .unwrap();
    assert_eq!(tokens.len(), 2);
    assert_eq!(tokens[0].accessor_id, "accessor-1");
    assert!(tokens[1].local);
}

#[tokio::test]
async fn test_acl_token_list_filtered() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/acl/tokens"))
        .and(query_param("authmethod", "kubernetes"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    let filter = ACLTokenFilterOptions {
        auth_method: Some("kubernetes".to_string()),
        ..Default::default()
    };

    let (tokens, _) = client
        .acl_token_list_filtered(&filter, &QueryOptions::default())
        .await
        .unwrap();
    assert_eq!(tokens.len(), 0);
}

// ==================== ACL Policy Tests ====================

fn sample_policy_json() -> serde_json::Value {
    serde_json::json!({
        "ID": "policy-abc",
        "Name": "node-read",
        "Description": "Read-only node access",
        "Rules": "node_prefix \"\" { policy = \"read\" }",
        "CreateIndex": 10,
        "ModifyIndex": 20
    })
}

#[tokio::test]
async fn test_acl_policy_create() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("PUT"))
        .and(path("/v1/acl/policy"))
        .respond_with(ResponseTemplate::new(200).set_body_json(sample_policy_json()))
        .mount(&server)
        .await;

    let policy = ACLPolicy {
        name: "node-read".to_string(),
        rules: "node_prefix \"\" { policy = \"read\" }".to_string(),
        ..Default::default()
    };

    let (created, _) = client
        .acl_policy_create(&policy, &WriteOptions::default())
        .await
        .unwrap();
    assert_eq!(created.id, "policy-abc");
    assert_eq!(created.name, "node-read");
}

#[tokio::test]
async fn test_acl_policy_read() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/acl/policy/policy-abc"))
        .respond_with(ResponseTemplate::new(200).set_body_json(sample_policy_json()))
        .mount(&server)
        .await;

    let (policy, _) = client
        .acl_policy_read("policy-abc", &QueryOptions::default())
        .await
        .unwrap();
    assert_eq!(policy.name, "node-read");
}

#[tokio::test]
async fn test_acl_policy_read_by_name() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/acl/policy/name/node-read"))
        .respond_with(ResponseTemplate::new(200).set_body_json(sample_policy_json()))
        .mount(&server)
        .await;

    let (policy, _) = client
        .acl_policy_read_by_name("node-read", &QueryOptions::default())
        .await
        .unwrap();
    assert_eq!(policy.id, "policy-abc");
}

#[tokio::test]
async fn test_acl_policy_update() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("PUT"))
        .and(path("/v1/acl/policy/policy-abc"))
        .respond_with(ResponseTemplate::new(200).set_body_json(sample_policy_json()))
        .mount(&server)
        .await;

    let policy = ACLPolicy {
        id: "policy-abc".to_string(),
        name: "node-read".to_string(),
        rules: "node_prefix \"\" { policy = \"read\" }".to_string(),
        ..Default::default()
    };

    let (updated, _) = client
        .acl_policy_update(&policy, &WriteOptions::default())
        .await
        .unwrap();
    assert_eq!(updated.id, "policy-abc");
}

#[tokio::test]
async fn test_acl_policy_delete() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("DELETE"))
        .and(path("/v1/acl/policy/policy-abc"))
        .respond_with(ResponseTemplate::new(200).set_body_string("true"))
        .mount(&server)
        .await;

    let (ok, _) = client
        .acl_policy_delete("policy-abc", &WriteOptions::default())
        .await
        .unwrap();
    assert!(ok);
}

#[tokio::test]
async fn test_acl_policy_list() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/acl/policies"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"ID": "p1", "Name": "global-management", "Description": "Built-in", "CreateIndex": 1, "ModifyIndex": 1},
            {"ID": "p2", "Name": "node-read", "Description": "Node read", "CreateIndex": 2, "ModifyIndex": 2}
        ])))
        .mount(&server)
        .await;

    let (policies, _) = client
        .acl_policy_list(&QueryOptions::default())
        .await
        .unwrap();
    assert_eq!(policies.len(), 2);
}

// ==================== ACL Role Tests ====================

fn sample_role_json() -> serde_json::Value {
    serde_json::json!({
        "ID": "role-abc",
        "Name": "admin-role",
        "Description": "Admin access",
        "Policies": [{"ID": "p1", "Name": "global-management"}],
        "CreateIndex": 10,
        "ModifyIndex": 20
    })
}

#[tokio::test]
async fn test_acl_role_create() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("PUT"))
        .and(path("/v1/acl/role"))
        .respond_with(ResponseTemplate::new(200).set_body_json(sample_role_json()))
        .mount(&server)
        .await;

    let role = ACLRole {
        name: "admin-role".to_string(),
        ..Default::default()
    };

    let (created, _) = client
        .acl_role_create(&role, &WriteOptions::default())
        .await
        .unwrap();
    assert_eq!(created.id, "role-abc");
}

#[tokio::test]
async fn test_acl_role_read() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/acl/role/role-abc"))
        .respond_with(ResponseTemplate::new(200).set_body_json(sample_role_json()))
        .mount(&server)
        .await;

    let (role, _) = client
        .acl_role_read("role-abc", &QueryOptions::default())
        .await
        .unwrap();
    assert_eq!(role.name, "admin-role");
}

#[tokio::test]
async fn test_acl_role_read_by_name() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/acl/role/name/admin-role"))
        .respond_with(ResponseTemplate::new(200).set_body_json(sample_role_json()))
        .mount(&server)
        .await;

    let (role, _) = client
        .acl_role_read_by_name("admin-role", &QueryOptions::default())
        .await
        .unwrap();
    assert_eq!(role.id, "role-abc");
}

#[tokio::test]
async fn test_acl_role_update() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("PUT"))
        .and(path("/v1/acl/role/role-abc"))
        .respond_with(ResponseTemplate::new(200).set_body_json(sample_role_json()))
        .mount(&server)
        .await;

    let role = ACLRole {
        id: "role-abc".to_string(),
        name: "admin-role".to_string(),
        ..Default::default()
    };

    let (updated, _) = client
        .acl_role_update(&role, &WriteOptions::default())
        .await
        .unwrap();
    assert_eq!(updated.id, "role-abc");
}

#[tokio::test]
async fn test_acl_role_delete() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("DELETE"))
        .and(path("/v1/acl/role/role-abc"))
        .respond_with(ResponseTemplate::new(200).set_body_string("true"))
        .mount(&server)
        .await;

    let (ok, _) = client
        .acl_role_delete("role-abc", &WriteOptions::default())
        .await
        .unwrap();
    assert!(ok);
}

#[tokio::test]
async fn test_acl_role_list() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/acl/roles"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"ID": "r1", "Name": "admin", "Description": "Admin", "CreateIndex": 1, "ModifyIndex": 1}
        ])))
        .mount(&server)
        .await;

    let (roles, _) = client
        .acl_role_list(&QueryOptions::default())
        .await
        .unwrap();
    assert_eq!(roles.len(), 1);
    assert_eq!(roles[0].name, "admin");
}

// ==================== ACL Auth Method Tests ====================

fn sample_auth_method_json() -> serde_json::Value {
    serde_json::json!({
        "Name": "kubernetes",
        "Type": "kubernetes",
        "DisplayName": "K8s Auth",
        "Description": "Kubernetes auth method",
        "CreateIndex": 10,
        "ModifyIndex": 20
    })
}

#[tokio::test]
async fn test_acl_auth_method_create() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("PUT"))
        .and(path("/v1/acl/auth-method"))
        .respond_with(ResponseTemplate::new(200).set_body_json(sample_auth_method_json()))
        .mount(&server)
        .await;

    let method_def = ACLAuthMethod {
        name: "kubernetes".to_string(),
        method_type: "kubernetes".to_string(),
        ..Default::default()
    };

    let (created, _) = client
        .acl_auth_method_create(&method_def, &WriteOptions::default())
        .await
        .unwrap();
    assert_eq!(created.name, "kubernetes");
    assert_eq!(created.method_type, "kubernetes");
}

#[tokio::test]
async fn test_acl_auth_method_read() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/acl/auth-method/kubernetes"))
        .respond_with(ResponseTemplate::new(200).set_body_json(sample_auth_method_json()))
        .mount(&server)
        .await;

    let (am, _) = client
        .acl_auth_method_read("kubernetes", &QueryOptions::default())
        .await
        .unwrap();
    assert_eq!(am.name, "kubernetes");
}

#[tokio::test]
async fn test_acl_auth_method_update() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("PUT"))
        .and(path("/v1/acl/auth-method/kubernetes"))
        .respond_with(ResponseTemplate::new(200).set_body_json(sample_auth_method_json()))
        .mount(&server)
        .await;

    let method_def = ACLAuthMethod {
        name: "kubernetes".to_string(),
        method_type: "kubernetes".to_string(),
        description: Some("Updated".to_string()),
        ..Default::default()
    };

    let (updated, _) = client
        .acl_auth_method_update(&method_def, &WriteOptions::default())
        .await
        .unwrap();
    assert_eq!(updated.name, "kubernetes");
}

#[tokio::test]
async fn test_acl_auth_method_delete() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("DELETE"))
        .and(path("/v1/acl/auth-method/kubernetes"))
        .respond_with(ResponseTemplate::new(200).set_body_string("true"))
        .mount(&server)
        .await;

    let (ok, _) = client
        .acl_auth_method_delete("kubernetes", &WriteOptions::default())
        .await
        .unwrap();
    assert!(ok);
}

#[tokio::test]
async fn test_acl_auth_method_list() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/acl/auth-methods"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"Name": "kubernetes", "Type": "kubernetes", "CreateIndex": 1, "ModifyIndex": 1}
        ])))
        .mount(&server)
        .await;

    let (methods, _) = client
        .acl_auth_method_list(&QueryOptions::default())
        .await
        .unwrap();
    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0].name, "kubernetes");
}

// ==================== ACL Binding Rule Tests ====================

fn sample_binding_rule_json() -> serde_json::Value {
    serde_json::json!({
        "ID": "rule-abc",
        "Description": "K8s binding",
        "AuthMethod": "kubernetes",
        "Selector": "serviceaccount.namespace==default",
        "BindType": "service",
        "BindName": "${serviceaccount.name}",
        "CreateIndex": 10,
        "ModifyIndex": 20
    })
}

#[tokio::test]
async fn test_acl_binding_rule_create() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("PUT"))
        .and(path("/v1/acl/binding-rule"))
        .respond_with(ResponseTemplate::new(200).set_body_json(sample_binding_rule_json()))
        .mount(&server)
        .await;

    let rule = ACLBindingRule {
        auth_method: "kubernetes".to_string(),
        bind_type: "service".to_string(),
        bind_name: "${serviceaccount.name}".to_string(),
        ..Default::default()
    };

    let (created, _) = client
        .acl_binding_rule_create(&rule, &WriteOptions::default())
        .await
        .unwrap();
    assert_eq!(created.id, "rule-abc");
    assert_eq!(created.bind_type, "service");
}

#[tokio::test]
async fn test_acl_binding_rule_read() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/acl/binding-rule/rule-abc"))
        .respond_with(ResponseTemplate::new(200).set_body_json(sample_binding_rule_json()))
        .mount(&server)
        .await;

    let (rule, _) = client
        .acl_binding_rule_read("rule-abc", &QueryOptions::default())
        .await
        .unwrap();
    assert_eq!(rule.auth_method, "kubernetes");
}

#[tokio::test]
async fn test_acl_binding_rule_update() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("PUT"))
        .and(path("/v1/acl/binding-rule/rule-abc"))
        .respond_with(ResponseTemplate::new(200).set_body_json(sample_binding_rule_json()))
        .mount(&server)
        .await;

    let rule = ACLBindingRule {
        id: "rule-abc".to_string(),
        auth_method: "kubernetes".to_string(),
        bind_type: "service".to_string(),
        bind_name: "${serviceaccount.name}".to_string(),
        ..Default::default()
    };

    let (updated, _) = client
        .acl_binding_rule_update(&rule, &WriteOptions::default())
        .await
        .unwrap();
    assert_eq!(updated.id, "rule-abc");
}

#[tokio::test]
async fn test_acl_binding_rule_delete() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("DELETE"))
        .and(path("/v1/acl/binding-rule/rule-abc"))
        .respond_with(ResponseTemplate::new(200).set_body_string("true"))
        .mount(&server)
        .await;

    let (ok, _) = client
        .acl_binding_rule_delete("rule-abc", &WriteOptions::default())
        .await
        .unwrap();
    assert!(ok);
}

#[tokio::test]
async fn test_acl_binding_rule_list() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/acl/binding-rules"))
        .and(query_param("authmethod", "kubernetes"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!([sample_binding_rule_json()])),
        )
        .mount(&server)
        .await;

    let (rules, _) = client
        .acl_binding_rule_list("kubernetes", &QueryOptions::default())
        .await
        .unwrap();
    assert_eq!(rules.len(), 1);
}

// ==================== ACL Login/Logout Tests ====================

#[tokio::test]
async fn test_acl_login() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("POST"))
        .and(path("/v1/acl/login"))
        .respond_with(ResponseTemplate::new(200).set_body_json(sample_token_json()))
        .mount(&server)
        .await;

    let params = ACLLoginParams {
        auth_method: "kubernetes".to_string(),
        bearer_token: "k8s-jwt-token".to_string(),
        ..Default::default()
    };

    let (token, _) = client
        .acl_login(&params, &WriteOptions::default())
        .await
        .unwrap();
    assert_eq!(token.accessor_id, "accessor-123");
}

#[tokio::test]
async fn test_acl_logout() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("PUT"))
        .and(path("/v1/acl/logout"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    client.acl_logout(&WriteOptions::default()).await.unwrap();
}

// ==================== ACL Replication Tests ====================

#[tokio::test]
async fn test_acl_replication() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/acl/replication"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "Enabled": true,
            "Running": true,
            "SourceDatacenter": "dc1",
            "ReplicationType": "tokens",
            "ReplicatedIndex": 100,
            "ReplicatedRoleIndex": 50,
            "ReplicatedTokenIndex": 75,
            "LastErrorMessage": ""
        })))
        .mount(&server)
        .await;

    let (status, _) = client
        .acl_replication(&QueryOptions::default())
        .await
        .unwrap();
    assert!(status.enabled);
    assert!(status.running);
    assert_eq!(status.source_datacenter, "dc1");
    assert_eq!(status.replicated_index, 100);
}

// ==================== ACL Templated Policy Tests ====================

#[tokio::test]
async fn test_acl_templated_policy_list() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/acl/templated-policies"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "builtin/service": {
                "TemplateName": "builtin/service",
                "Schema": "",
                "Template": "service \"${name}\" { policy = \"write\" }",
                "Description": "Service identity"
            }
        })))
        .mount(&server)
        .await;

    let (tps, _) = client
        .acl_templated_policy_list(&QueryOptions::default())
        .await
        .unwrap();
    assert_eq!(tps.len(), 1);
    assert!(tps.contains_key("builtin/service"));
}

#[tokio::test]
async fn test_acl_templated_policy_read() {
    let server = MockServer::start().await;
    let client = make_client(&server).await;

    Mock::given(method("GET"))
        .and(path("/v1/acl/templated-policy/name/builtin/service"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "TemplateName": "builtin/service",
            "Schema": "",
            "Template": "service \"${name}\" { policy = \"write\" }",
            "Description": "Service identity policy"
        })))
        .mount(&server)
        .await;

    let (tp, _) = client
        .acl_templated_policy_read("builtin/service", &QueryOptions::default())
        .await
        .unwrap();
    assert_eq!(tp.template_name, "builtin/service");
}

// ==================== ACL Model Tests ====================

#[test]
fn test_acl_token_json_serialization() {
    let token = ACLToken {
        accessor_id: "abc".to_string(),
        description: "test".to_string(),
        policies: Some(vec![ACLTokenPolicyLink {
            id: "p1".to_string(),
            name: "policy-1".to_string(),
        }]),
        local: true,
        ..Default::default()
    };

    let json = serde_json::to_value(&token).unwrap();
    assert_eq!(json["AccessorID"], "abc");
    assert_eq!(json["Local"], true);
    assert_eq!(json["Policies"][0]["ID"], "p1");
    // SecretID should not appear when None
    assert!(json.get("SecretID").is_none());
}

#[test]
fn test_acl_policy_json_serialization() {
    let policy = ACLPolicy {
        name: "test-policy".to_string(),
        rules: "key_prefix \"\" { policy = \"read\" }".to_string(),
        ..Default::default()
    };

    let json = serde_json::to_value(&policy).unwrap();
    assert_eq!(json["Name"], "test-policy");
    assert_eq!(json["Rules"], "key_prefix \"\" { policy = \"read\" }");
}

#[test]
fn test_acl_binding_rule_json_serialization() {
    let rule = ACLBindingRule {
        auth_method: "kubernetes".to_string(),
        bind_type: "service".to_string(),
        bind_name: "web".to_string(),
        selector: "serviceaccount.namespace==default".to_string(),
        ..Default::default()
    };

    let json = serde_json::to_value(&rule).unwrap();
    assert_eq!(json["AuthMethod"], "kubernetes");
    assert_eq!(json["BindType"], "service");
    assert_eq!(json["Selector"], "serviceaccount.namespace==default");
}

#[test]
fn test_acl_login_params_json() {
    let params = ACLLoginParams {
        auth_method: "kubernetes".to_string(),
        bearer_token: "my-jwt".to_string(),
        meta: Some({
            let mut m = std::collections::HashMap::new();
            m.insert("pod".to_string(), "web-pod-1".to_string());
            m
        }),
    };

    let json = serde_json::to_value(&params).unwrap();
    assert_eq!(json["AuthMethod"], "kubernetes");
    assert_eq!(json["BearerToken"], "my-jwt");
    assert_eq!(json["Meta"]["pod"], "web-pod-1");
}
