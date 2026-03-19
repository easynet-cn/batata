//! Connection handler tests
//!
//! Tests for gRPC connection management request/response serialization

use batata_api::remote::model::{
    ConnectionSetupRequest, HealthCheckRequest, RequestTrait, ServerCheckRequest,
};

#[test]
fn test_connection_setup_request_serialization() {
    let mut req = ConnectionSetupRequest::new();
    req.client_version = "3.1.0".to_string();
    req.tenant = "public".to_string();
    req.labels.insert("source".to_string(), "sdk".to_string());
    req.labels
        .insert("appName".to_string(), "my-app".to_string());

    let payload = req.build_server_push_payload();
    assert_eq!(
        payload.metadata.as_ref().unwrap().r#type,
        "ConnectionSetupRequest"
    );
    assert!(payload.body.is_some());

    let deserialized = ConnectionSetupRequest::from(&payload);
    assert_eq!(deserialized.client_version, "3.1.0");
    assert_eq!(deserialized.tenant, "public");
    assert_eq!(deserialized.labels.get("source"), Some(&"sdk".to_string()));
    assert_eq!(
        deserialized.labels.get("appName"),
        Some(&"my-app".to_string())
    );
}

#[test]
fn test_health_check_request_serialization() {
    let req = HealthCheckRequest::new();

    let payload = req.build_server_push_payload();
    assert_eq!(
        payload.metadata.as_ref().unwrap().r#type,
        "HealthCheckRequest"
    );
    assert!(payload.body.is_some());

    // Round-trip: should deserialize without error
    let deserialized = HealthCheckRequest::from(&payload);
    // HealthCheckRequest has no additional fields beyond internal_request
    assert_eq!(deserialized.request_type(), "HealthCheckRequest");
}

#[test]
fn test_server_check_request_serialization() {
    let req = ServerCheckRequest::new();

    let payload = req.build_server_push_payload();
    assert_eq!(
        payload.metadata.as_ref().unwrap().r#type,
        "ServerCheckRequest"
    );
    assert!(payload.body.is_some());

    // Round-trip: should deserialize without error
    let deserialized = ServerCheckRequest::from(&payload);
    assert_eq!(deserialized.request_type(), "ServerCheckRequest");
}
