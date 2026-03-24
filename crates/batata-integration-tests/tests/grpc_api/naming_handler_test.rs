//! Naming (Service Discovery) gRPC handler tests
//!
//! Tests for naming-related request/response serialization round-trips

use std::collections::HashMap;

use batata_api::naming::model::{
    BatchInstanceRequest, Instance, InstanceRequest, ServiceQueryRequest, SubscribeServiceRequest,
};
use batata_api::remote::model::RequestTrait;

fn make_test_instance() -> Instance {
    Instance {
        instance_id: "10.0.0.1#8080#DEFAULT#DEFAULT_GROUP@@my-service".to_string(),
        ip: "10.0.0.1".to_string(),
        port: 8080,
        weight: 1.0,
        healthy: true,
        enabled: true,
        ephemeral: true,
        cluster_name: "DEFAULT".to_string(),
        service_name: "my-service".to_string(),
        metadata: HashMap::from([("env".to_string(), "prod".to_string())]),
        register_source: batata_api::naming::RegisterSource::Batata,
    }
}

#[test]
fn test_instance_request_serialization() {
    let mut req = InstanceRequest::new();
    req.naming_request.namespace = "public".to_string();
    req.naming_request.service_name = "my-service".to_string();
    req.naming_request.group_name = "DEFAULT_GROUP".to_string();
    req.r#type = "registerInstance".to_string();
    req.instance = make_test_instance();

    let payload = req.build_server_push_payload();
    assert_eq!(payload.metadata.as_ref().unwrap().r#type, "InstanceRequest");

    let deserialized = InstanceRequest::from(&payload);
    assert_eq!(deserialized.naming_request.namespace, "public");
    assert_eq!(deserialized.naming_request.service_name, "my-service");
    assert_eq!(deserialized.r#type, "registerInstance");
    assert_eq!(deserialized.instance.ip, "10.0.0.1");
    assert_eq!(deserialized.instance.port, 8080);
    assert!(deserialized.instance.healthy);
    assert!(deserialized.instance.ephemeral);
    assert_eq!(
        deserialized.instance.metadata.get("env"),
        Some(&"prod".to_string())
    );
}

#[test]
fn test_service_query_request_serialization() {
    let mut req = ServiceQueryRequest::new();
    req.naming_request.namespace = "tenant-1".to_string();
    req.naming_request.service_name = "order-service".to_string();
    req.naming_request.group_name = "DEFAULT_GROUP".to_string();
    req.cluster = "DEFAULT".to_string();
    req.healthy_only = true;
    req.udp_port = 9999;

    let payload = req.build_server_push_payload();
    assert_eq!(
        payload.metadata.as_ref().unwrap().r#type,
        "ServiceQueryRequest"
    );

    let deserialized = ServiceQueryRequest::from(&payload);
    assert_eq!(deserialized.naming_request.namespace, "tenant-1");
    assert_eq!(deserialized.naming_request.service_name, "order-service");
    assert_eq!(deserialized.cluster, "DEFAULT");
    assert!(deserialized.healthy_only);
    assert_eq!(deserialized.udp_port, 9999);
}

#[test]
fn test_subscribe_request_serialization() {
    let mut req = SubscribeServiceRequest::new();
    req.naming_request.namespace = "public".to_string();
    req.naming_request.service_name = "payment-service".to_string();
    req.naming_request.group_name = "DEFAULT_GROUP".to_string();
    req.subscribe = true;
    req.clusters = "DEFAULT,BACKUP".to_string();

    let payload = req.build_server_push_payload();
    assert_eq!(
        payload.metadata.as_ref().unwrap().r#type,
        "SubscribeServiceRequest"
    );

    let deserialized = SubscribeServiceRequest::from(&payload);
    assert_eq!(deserialized.naming_request.namespace, "public");
    assert_eq!(deserialized.naming_request.service_name, "payment-service");
    assert!(deserialized.subscribe);
    assert_eq!(deserialized.clusters, "DEFAULT,BACKUP");
}

#[test]
fn test_batch_instance_request_serialization() {
    let mut req = BatchInstanceRequest::new();
    req.naming_request.namespace = "public".to_string();
    req.naming_request.service_name = "batch-service".to_string();
    req.naming_request.group_name = "DEFAULT_GROUP".to_string();
    req.r#type = "registerInstance".to_string();

    let mut inst1 = make_test_instance();
    inst1.ip = "10.0.0.1".to_string();
    inst1.port = 8080;

    let mut inst2 = make_test_instance();
    inst2.ip = "10.0.0.2".to_string();
    inst2.port = 8081;

    req.instances = vec![inst1, inst2];

    let payload = req.build_server_push_payload();
    assert_eq!(
        payload.metadata.as_ref().unwrap().r#type,
        "BatchInstanceRequest"
    );

    let deserialized = BatchInstanceRequest::from(&payload);
    assert_eq!(deserialized.naming_request.namespace, "public");
    assert_eq!(deserialized.r#type, "registerInstance");
    assert_eq!(deserialized.instances.len(), 2);
    assert_eq!(deserialized.instances[0].ip, "10.0.0.1");
    assert_eq!(deserialized.instances[1].ip, "10.0.0.2");
    assert_eq!(deserialized.instances[1].port, 8081);
}

// Integration test scenario: Full service discovery lifecycle
#[tokio::test]
#[ignore = "requires running gRPC server"]
async fn test_service_discovery_lifecycle_grpc() {
    // 1. Register instances via InstanceRequestHandler
    // 2. Query service via ServiceQueryRequestHandler
    // 3. Subscribe to service via SubscribeServiceRequestHandler
    // 4. Update instance and verify notification
    // 5. Deregister instance via InstanceRequestHandler
}
