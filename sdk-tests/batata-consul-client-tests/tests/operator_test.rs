//! Integration tests for the Operator API (Raft, Autopilot, Keyring, Usage)

mod common;

#[tokio::test]
async fn test_operator_raft_config() {
    common::init_tracing();
    let client = common::create_client();

    let (raft_config, meta) = client.operator_raft_config(&common::q()).await.unwrap();

    assert!(
        meta.last_index > 0,
        "QueryMeta last_index should be positive"
    );
    assert!(
        !raft_config.servers.is_empty(),
        "Raft configuration should have at least one server"
    );

    // At least one server must be the leader in a healthy cluster
    let has_leader = raft_config.servers.iter().any(|s| s.leader);
    assert!(has_leader, "At least one Raft server should be the leader");

    let leader = raft_config.servers.iter().find(|s| s.leader).unwrap();
    assert!(!leader.id.is_empty(), "Leader ID should not be empty");
    assert!(
        !leader.node.is_empty(),
        "Leader node name should not be empty"
    );
    assert!(
        !leader.address.is_empty(),
        "Leader address should not be empty"
    );
}

#[tokio::test]
async fn test_operator_autopilot_get_config() {
    common::init_tracing();
    let client = common::create_client();

    let (config, _meta) = client
        .operator_autopilot_get_configuration(&common::q())
        .await
        .unwrap();

    // Batata may not return X-Consul-Index header for operator endpoints
    // Autopilot has sensible defaults; verify some fields are present
    assert!(
        !config.last_contact_threshold.is_empty(),
        "last_contact_threshold should have a default value"
    );
}

#[tokio::test]
async fn test_operator_autopilot_set_config() {
    common::init_tracing();
    let client = common::create_client();

    // Read current config
    let (mut config, _) = client
        .operator_autopilot_get_configuration(&common::q())
        .await
        .unwrap();

    let original_cleanup = config.cleanup_dead_servers;

    // Toggle cleanup_dead_servers
    config.cleanup_dead_servers = !original_cleanup;
    client
        .operator_autopilot_set_configuration(&config, &common::w())
        .await
        .unwrap();

    // Verify the change
    let (updated, _) = client
        .operator_autopilot_get_configuration(&common::q())
        .await
        .unwrap();
    assert_eq!(
        updated.cleanup_dead_servers, !original_cleanup,
        "cleanup_dead_servers should be toggled"
    );

    // Restore original value
    config.cleanup_dead_servers = original_cleanup;
    client
        .operator_autopilot_set_configuration(&config, &common::w())
        .await
        .unwrap();
}

#[tokio::test]
async fn test_operator_autopilot_health() {
    common::init_tracing();
    let client = common::create_client();

    let (health, _meta) = client
        .operator_autopilot_health(&common::q())
        .await
        .unwrap();

    // Batata may not return X-Consul-Index header for operator endpoints
    assert!(health.healthy, "Autopilot health should be healthy");
    assert!(
        !health.servers.is_empty(),
        "Autopilot health should list at least one server"
    );

    let first_server = &health.servers[0];
    assert!(
        !first_server.id.is_empty(),
        "Server health ID should not be empty"
    );
    assert!(
        !first_server.name.is_empty(),
        "Server health name should not be empty"
    );
}

#[tokio::test]
async fn test_operator_autopilot_state() {
    common::init_tracing();
    let client = common::create_client();

    let (state, _meta) = client.operator_autopilot_state(&common::q()).await.unwrap();

    // Batata may not return X-Consul-Index header for operator endpoints
    assert!(state.is_object(), "Autopilot state should be a JSON object");
    // The state object should contain a "Healthy" field
    assert!(
        state.get("Healthy").is_some(),
        "Autopilot state should contain a 'Healthy' field"
    );
}

#[tokio::test]
async fn test_operator_keyring_list() {
    common::init_tracing();
    let client = common::create_client();

    let result = client.operator_keyring_list(&common::q()).await;

    // Keyring may not be enabled on all configurations.
    // If it succeeds, validate the response structure.
    match result {
        Ok((keyrings, _meta)) => {
            // If keyrings are available, each entry should have a datacenter
            for kr in &keyrings {
                assert!(
                    !kr.datacenter.is_empty(),
                    "Keyring response datacenter should not be empty"
                );
                assert!(
                    kr.num_nodes >= 0,
                    "Keyring num_nodes should be non-negative"
                );
            }
        }
        Err(e) => {
            // Keyring not enabled is acceptable
            let msg = format!("{}", e);
            assert!(
                msg.contains("keyring") || msg.contains("404") || msg.contains("500"),
                "Error should be related to keyring not being enabled, got: {}",
                msg
            );
        }
    }
}

#[tokio::test]
async fn test_operator_usage() {
    common::init_tracing();
    let client = common::create_client();

    let (usage, _meta) = client.operator_usage(&common::q()).await.unwrap();

    // Batata may not return X-Consul-Index header for operator endpoints
    assert!(usage.is_object(), "Usage response should be a JSON object");
}

#[tokio::test]
async fn test_operator_raft_transfer_leader() {
    common::init_tracing();
    let client = common::create_client();

    // In a single-node cluster, leadership transfer may be a no-op or error.
    // We just verify the API call is accepted.
    let result = client.operator_raft_transfer_leader(&common::w()).await;

    match result {
        Ok(_meta) => {
            // Transfer accepted (may be a no-op in single-node cluster)
        }
        Err(e) => {
            // Single-node cluster cannot transfer leadership; this is expected
            let msg = format!("{}", e);
            assert!(
                msg.contains("cannot transfer")
                    || msg.contains("no peer")
                    || msg.contains("500")
                    || msg.contains("leader"),
                "Error should be about inability to transfer in single-node mode, got: {}",
                msg
            );
        }
    }
}
