//! Direct coverage for the public surface of `BatataAiService` (TG-03).
//!
//! The inline unit tests in `src/ai.rs` only exercise DTO serialization. These
//! tests call every public method against a wiremock server and assert the
//! request wire, the success payload, the error path and the local caches.

use std::sync::Arc;

use batata_client::{BatataAiService, BatataHttpClient, HttpClientConfig};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn make_service(server: &MockServer) -> BatataAiService {
    // Identity auth keeps these tests off the login endpoint; without it the
    // client would try to authenticate and mask the wire assertions.
    let config = HttpClientConfig {
        server_addrs: vec![server.uri()],
        server_identity_key: "serverIdentity".to_string(),
        server_identity_value: "test-identity".to_string(),
        ..Default::default()
    };
    let http_client = BatataHttpClient::new_without_auth(config).unwrap();
    BatataAiService::new(Arc::new(http_client))
}

fn ok(data: serde_json::Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "code": 0,
        "message": "success",
        "data": data
    }))
}

fn fail(data: serde_json::Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "code": 500,
        "message": "boom",
        "data": data
    }))
}

fn mcp_detail() -> serde_json::Value {
    serde_json::json!({
        "mcpName": "my-mcp",
        "namespaceId": "ns-1",
        "version": "1.0",
        "description": "demo",
        "enabled": true,
        "protocol": "stdio",
        "serverType": "local",
        "tools": [],
        "metadata": {}
    })
}

fn mcp_page() -> serde_json::Value {
    serde_json::json!({
        "totalCount": 1,
        "pageNumber": 1,
        "pagesAvailable": 1,
        "pageItems": [{ "mcpName": "my-mcp", "namespaceId": "ns-1" }]
    })
}

fn agent_card() -> serde_json::Value {
    serde_json::json!({
        "agentName": "my-agent",
        "namespaceId": "ns-1",
        "version": "1.0",
        "description": "demo",
        "enabled": true,
        "capabilities": { "streaming": true },
        "skills": [],
        "metadata": {},
        "securityRequirements": [{ "oauth": ["read"] }]
    })
}

fn agent_page() -> serde_json::Value {
    serde_json::json!({
        "totalCount": 1,
        "pageNumber": 1,
        "pagesAvailable": 1,
        "pageItems": [agent_card()]
    })
}

/// `get_mcp_server`, `list_mcp_servers`, `get_agent_card` and
/// `list_agent_cards` must hit their console paths with the exact query shape,
/// and the two getters must populate their caches. A changed path or query
/// turns the wiremock request into a 404, so this is the wire mutant guard.
#[tokio::test]
async fn test_ai_service_read_paths_and_cache() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/console/ai/mcp"))
        .and(query_param("namespaceId", "ns-1"))
        .and(query_param("mcpName", "my-mcp"))
        .respond_with(ok(mcp_detail()))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v3/console/ai/mcp/list"))
        .and(query_param("namespaceId", "ns-1"))
        .and(query_param("pageNo", "2"))
        .and(query_param("pageSize", "5"))
        .respond_with(ok(mcp_page()))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v3/console/ai/a2a"))
        .and(query_param("namespaceId", "ns-1"))
        .and(query_param("agentName", "my-agent"))
        .respond_with(ok(agent_card()))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v3/console/ai/a2a/list"))
        .and(query_param("namespaceId", "ns-1"))
        .and(query_param("pageNo", "3"))
        .and(query_param("pageSize", "10"))
        .respond_with(ok(agent_page()))
        .mount(&server)
        .await;

    let service = make_service(&server).await;

    // Cache accessors start empty.
    assert_eq!(service.mcp_cache_size(), 0);
    assert_eq!(service.agent_cache_size(), 0);
    assert!(service.get_cached_mcp_server("ns-1", "my-mcp").is_none());
    assert!(service.get_cached_agent_card("ns-1", "my-agent").is_none());

    let detail = service.get_mcp_server("ns-1", "my-mcp").await.unwrap();
    assert_eq!(detail.mcp_name, "my-mcp");
    assert_eq!(detail.version, "1.0");
    let mcp_page = service.list_mcp_servers("ns-1", 2, 5).await.unwrap();
    assert_eq!(mcp_page.total_count, 1);
    assert_eq!(mcp_page.page_items[0].mcp_name, "my-mcp");

    let card = service.get_agent_card("ns-1", "my-agent").await.unwrap();
    assert_eq!(card.agent_name, "my-agent");
    assert!(card.capabilities.streaming);
    assert_eq!(card.security_requirements.len(), 1);
    let agent_page = service.list_agent_cards("ns-1", 3, 10).await.unwrap();
    assert_eq!(agent_page.total_count, 1);
    assert_eq!(agent_page.page_items[0].agent_name, "my-agent");

    // Both getters must have cached their payloads.
    assert_eq!(service.mcp_cache_size(), 1);
    assert_eq!(service.agent_cache_size(), 1);
    assert_eq!(
        service
            .get_cached_mcp_server("ns-1", "my-mcp")
            .unwrap()
            .version,
        "1.0"
    );
    assert_eq!(
        service
            .get_cached_agent_card("ns-1", "my-agent")
            .unwrap()
            .version,
        "1.0"
    );
    assert!(service.get_cached_mcp_server("other", "my-mcp").is_none());
    assert!(service.get_cached_agent_card("other", "my-agent").is_none());

    service.clear_cache();
    assert_eq!(service.mcp_cache_size(), 0);
    assert_eq!(service.agent_cache_size(), 0);
    assert!(service.get_cached_mcp_server("ns-1", "my-mcp").is_none());
    assert!(service.get_cached_agent_card("ns-1", "my-agent").is_none());
}

/// A non-zero `code` must be surfaced as an error by all four read methods,
/// and must not populate the caches.
#[tokio::test]
async fn test_ai_service_error_paths() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/console/ai/mcp"))
        .respond_with(fail(mcp_detail()))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v3/console/ai/mcp/list"))
        .respond_with(fail(serde_json::json!({})))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v3/console/ai/a2a"))
        .respond_with(fail(serde_json::json!({ "agentName": "" })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v3/console/ai/a2a/list"))
        .respond_with(fail(serde_json::json!({})))
        .mount(&server)
        .await;

    let service = make_service(&server).await;
    // The error must come from the non-zero `code`, not from a missing mock or
    // an auth failure, so assert the server message is threaded through.
    for error in [
        service
            .get_mcp_server("ns-1", "my-mcp")
            .await
            .expect_err("non-zero code must fail"),
        service
            .list_mcp_servers("ns-1", 1, 20)
            .await
            .expect_err("non-zero code must fail"),
        service
            .get_agent_card("ns-1", "my-agent")
            .await
            .expect_err("non-zero code must fail"),
        service
            .list_agent_cards("ns-1", 1, 20)
            .await
            .expect_err("non-zero code must fail"),
    ] {
        assert!(
            error.to_string().contains("boom"),
            "unexpected error: {error}"
        );
    }
    assert_eq!(service.mcp_cache_size(), 0);
    assert_eq!(service.agent_cache_size(), 0);
}
