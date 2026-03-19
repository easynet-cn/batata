//! Configuration gRPC handler tests
//!
//! Tests for config-related request/response serialization round-trips

use batata_api::config::model::{
    ConfigBatchListenRequest, ConfigListenContext, ConfigPublishRequest, ConfigQueryRequest,
    ConfigRemoveRequest,
};
use batata_api::remote::model::RequestTrait;

#[test]
fn test_config_query_request_serialization() {
    let mut req = ConfigQueryRequest::new();
    req.config_request.data_id = "test-config.yaml".to_string();
    req.config_request.group = "DEFAULT_GROUP".to_string();
    req.config_request.tenant = "public".to_string();
    req.tag = "v1".to_string();

    // Serialize to Payload
    let payload = req.build_server_push_payload();
    assert!(payload.metadata.is_some());
    assert_eq!(
        payload.metadata.as_ref().unwrap().r#type,
        "ConfigQueryRequest"
    );
    assert!(payload.body.is_some());

    // Deserialize from Payload
    let deserialized = ConfigQueryRequest::from(&payload);
    assert_eq!(deserialized.config_request.data_id, "test-config.yaml");
    assert_eq!(deserialized.config_request.group, "DEFAULT_GROUP");
    assert_eq!(deserialized.config_request.tenant, "public");
    assert_eq!(deserialized.tag, "v1");
}

#[test]
fn test_config_publish_request_serialization() {
    let mut req = ConfigPublishRequest::new();
    req.config_request.data_id = "app.properties".to_string();
    req.config_request.group = "PROD_GROUP".to_string();
    req.config_request.tenant = "tenant-1".to_string();
    req.content = "server.port=8080\napp.name=test".to_string();
    req.cas_md5 = "abc123".to_string();
    req.addition_map
        .insert("type".to_string(), "properties".to_string());

    let payload = req.build_server_push_payload();
    assert_eq!(
        payload.metadata.as_ref().unwrap().r#type,
        "ConfigPublishRequest"
    );

    let deserialized = ConfigPublishRequest::from(&payload);
    assert_eq!(deserialized.config_request.data_id, "app.properties");
    assert_eq!(deserialized.config_request.group, "PROD_GROUP");
    assert_eq!(deserialized.content, "server.port=8080\napp.name=test");
    assert_eq!(deserialized.cas_md5, "abc123");
    assert_eq!(
        deserialized.addition_map.get("type"),
        Some(&"properties".to_string())
    );
}

#[test]
fn test_config_remove_request_serialization() {
    let mut req = ConfigRemoveRequest::new();
    req.config_request.data_id = "old-config.yaml".to_string();
    req.config_request.group = "DEFAULT_GROUP".to_string();
    req.config_request.tenant = "public".to_string();
    req.tag = "deprecated".to_string();

    let payload = req.build_server_push_payload();
    assert_eq!(
        payload.metadata.as_ref().unwrap().r#type,
        "ConfigRemoveRequest"
    );

    let deserialized = ConfigRemoveRequest::from(&payload);
    assert_eq!(deserialized.config_request.data_id, "old-config.yaml");
    assert_eq!(deserialized.config_request.group, "DEFAULT_GROUP");
    assert_eq!(deserialized.tag, "deprecated");
}

#[test]
fn test_config_batch_listen_request_serialization() {
    let mut req = ConfigBatchListenRequest::new();
    req.listen = true;
    req.config_listen_contexts = vec![
        ConfigListenContext {
            group: "DEFAULT_GROUP".to_string(),
            data_id: "config-1.yaml".to_string(),
            md5: "md5hash1".to_string(),
            tenant: "public".to_string(),
        },
        ConfigListenContext {
            group: "DEFAULT_GROUP".to_string(),
            data_id: "config-2.yaml".to_string(),
            md5: "md5hash2".to_string(),
            tenant: "public".to_string(),
        },
    ];

    let payload = req.build_server_push_payload();
    assert_eq!(
        payload.metadata.as_ref().unwrap().r#type,
        "ConfigBatchListenRequest"
    );

    let deserialized = ConfigBatchListenRequest::from(&payload);
    assert!(deserialized.listen);
    assert_eq!(deserialized.config_listen_contexts.len(), 2);
    assert_eq!(
        deserialized.config_listen_contexts[0].data_id,
        "config-1.yaml"
    );
    assert_eq!(deserialized.config_listen_contexts[0].md5, "md5hash1");
    assert_eq!(
        deserialized.config_listen_contexts[1].data_id,
        "config-2.yaml"
    );
}

// Integration test scenario: Full config lifecycle
#[tokio::test]
#[ignore = "requires running gRPC server"]
async fn test_config_lifecycle_grpc() {
    // 1. Publish config via ConfigPublishHandler
    // 2. Query config via ConfigQueryHandler
    // 3. Register listener via ConfigBatchListenHandler
    // 4. Update config and verify notification
    // 5. Remove config via ConfigRemoveHandler
}
