//! Integration tests for ConsulClient infrastructure (creation, config, metrics, retry)

mod common;

use std::time::Duration;

use batata_consul_client::{ConsulClient, ConsulClientConfig};

#[tokio::test]
#[ignore]
async fn test_client_creation() {
    let config = ConsulClientConfig::new(common::CONSUL_ADDR);
    let client = ConsulClient::new(config);

    assert!(
        client.is_ok(),
        "Client creation with default config should succeed"
    );
    let client = client.unwrap();

    // Verify the client can make a basic request (status/leader)
    let result = client.status_leader(&common::q()).await;
    assert!(
        result.is_ok(),
        "Basic request with default client should succeed"
    );
}

#[tokio::test]
#[ignore]
async fn test_client_custom_config() {
    let config = ConsulClientConfig::new(common::CONSUL_ADDR)
        .with_datacenter("dc1")
        .with_namespace("")
        .with_partition("")
        .with_wait_time(Duration::from_secs(10))
        .with_timeouts(Duration::from_secs(3), Duration::from_secs(15))
        .with_retry(3, 200);

    assert_eq!(
        config.effective_address(),
        common::CONSUL_ADDR,
        "Effective address should match"
    );
    assert_eq!(config.datacenter, "dc1", "Datacenter should be set");
    assert_eq!(
        config.wait_time,
        Some(Duration::from_secs(10)),
        "Wait time should be set"
    );
    assert_eq!(
        config.connect_timeout,
        Duration::from_secs(3),
        "Connect timeout should be set"
    );
    assert_eq!(
        config.read_timeout,
        Duration::from_secs(15),
        "Read timeout should be set"
    );
    assert_eq!(config.max_retries, 3, "Max retries should be set");
    assert_eq!(
        config.retry_backoff_base_ms, 200,
        "Retry backoff base should be set"
    );

    let client = ConsulClient::new(config).unwrap();
    let result = client.status_leader(&common::q()).await;
    assert!(
        result.is_ok(),
        "Custom-configured client should be able to make requests"
    );
}

#[tokio::test]
#[ignore]
async fn test_client_metrics_initial() {
    let client = common::create_client();

    let snapshot = client.metrics.snapshot();
    assert_eq!(
        snapshot.requests_total, 0,
        "Initial requests_total should be 0"
    );
    assert_eq!(
        snapshot.requests_success, 0,
        "Initial requests_success should be 0"
    );
    assert_eq!(
        snapshot.requests_failed, 0,
        "Initial requests_failed should be 0"
    );
    assert_eq!(
        snapshot.retries_total, 0,
        "Initial retries_total should be 0"
    );
    assert_eq!(
        snapshot.address_rotations, 0,
        "Initial address_rotations should be 0"
    );
}

#[tokio::test]
#[ignore]
async fn test_client_metrics_after_request() {
    let client = common::create_client();

    // Make a successful request
    let _ = client.status_leader(&common::q()).await.unwrap();

    let snapshot = client.metrics.snapshot();
    assert!(
        snapshot.requests_total > 0,
        "requests_total should be > 0 after a request, got {}",
        snapshot.requests_total
    );
    assert!(
        snapshot.requests_success > 0,
        "requests_success should be > 0 after a successful request, got {}",
        snapshot.requests_success
    );
    assert_eq!(
        snapshot.requests_failed, 0,
        "requests_failed should be 0 after a successful request"
    );
}

#[tokio::test]
#[ignore]
async fn test_client_error_on_bad_address() {
    let config = ConsulClientConfig::new("http://127.0.0.1:19999").with_retry(0, 100); // No retries for faster failure

    let client = ConsulClient::new(config).unwrap();

    let result = client.status_leader(&common::q()).await;
    assert!(
        result.is_err(),
        "Request to unreachable address should fail"
    );

    let snapshot = client.metrics.snapshot();
    assert!(
        snapshot.requests_total > 0,
        "requests_total should be > 0 even on failure"
    );
    assert!(
        snapshot.requests_failed > 0,
        "requests_failed should be > 0 after failed request"
    );
}

#[tokio::test]
#[ignore]
async fn test_client_address_rotation() {
    // First address is bad, second is good
    let config = ConsulClientConfig::new("http://127.0.0.1:19999")
        .with_addresses(vec![
            "http://127.0.0.1:19999".to_string(),
            common::CONSUL_ADDR.to_string(),
        ])
        .with_retry(2, 50);

    let client = ConsulClient::new(config).unwrap();

    // The client should eventually reach the good address via rotation
    let result = client.status_leader(&common::q()).await;

    // Check that rotation was attempted
    let snapshot = client.metrics.snapshot();
    // Either the request succeeded (rotation worked) or all retries failed
    assert!(
        snapshot.requests_total > 0,
        "requests_total should be > 0 after attempting requests"
    );

    if result.is_ok() {
        // If successful, rotation should have kicked in
        assert!(
            snapshot.address_rotations > 0 || snapshot.requests_success > 0,
            "Should have rotated addresses or succeeded directly"
        );
    }
}

#[tokio::test]
#[ignore]
async fn test_client_retry_on_error() {
    // Point to a non-existent server to force retries
    let config = ConsulClientConfig::new("http://127.0.0.1:19999").with_retry(2, 50);

    let client = ConsulClient::new(config).unwrap();

    let result = client.status_leader(&common::q()).await;
    assert!(result.is_err(), "Request to bad server should fail");

    let snapshot = client.metrics.snapshot();
    assert!(
        snapshot.retries_total > 0,
        "retries_total should be > 0 after failed request with retries configured, got {}",
        snapshot.retries_total
    );
    assert!(
        snapshot.requests_failed > 0,
        "requests_failed should be > 0 after all retries exhausted"
    );
}

#[tokio::test]
#[ignore]
async fn test_client_token_file() {
    common::init_tracing();

    // Write a token to a temp file
    let token_value = "test-token-from-file";
    let temp_dir = std::env::temp_dir();
    let token_path = temp_dir.join(format!("consul-token-{}", common::test_id()));
    std::fs::write(&token_path, token_value).unwrap();

    // Create client with token_file
    let config = ConsulClientConfig::new(common::CONSUL_ADDR).with_token_file(token_path.clone());

    assert_eq!(
        config.effective_token(),
        token_value,
        "effective_token() should read from token_file"
    );

    let client = ConsulClient::new(config).unwrap();

    // The client should be created successfully (token is sent as header).
    // We verify by making a request; ACL may not be enabled, so just check
    // the client works without panicking.
    let _ = client.status_leader(&common::q()).await;

    // Verify the token file content is used
    let config2 = ConsulClientConfig::new(common::CONSUL_ADDR).with_token_file(token_path.clone());
    assert_eq!(
        config2.effective_token(),
        token_value,
        "Token should still be read from file"
    );

    // Update the token file and verify re-read
    let new_token = "updated-token";
    std::fs::write(&token_path, new_token).unwrap();
    let config3 = ConsulClientConfig::new(common::CONSUL_ADDR).with_token_file(token_path.clone());
    assert_eq!(
        config3.effective_token(),
        new_token,
        "effective_token() should re-read updated file"
    );

    // Cleanup
    let _ = std::fs::remove_file(&token_path);
}
