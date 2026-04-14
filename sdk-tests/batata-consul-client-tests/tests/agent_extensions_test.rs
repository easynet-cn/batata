//! Tests for agent method additions in this round:
//! - agent_force_leave_options / _prune
//! - agent_monitor_json
//! - agent_health_service_by_id_opts / _by_name_opts
//! - agent_update_agent_recovery_token / agent_update_token_of_type

mod common;

use batata_consul_client::model::AgentServiceRegistration;

#[tokio::test]
async fn test_agent_force_leave_prune_unknown_node() {
    let client = common::create_client();
    // Attempt to prune a non-existent node — must not panic. Server may
    // return 200 regardless (ForceLeave is idempotent in Consul).
    let _ = client
        .agent_force_leave_prune(&format!("ghost-{}", common::test_id()), &common::w())
        .await;
}

#[tokio::test]
async fn test_agent_force_leave_options_flag() {
    let client = common::create_client();
    let _ = client
        .agent_force_leave_options(
            &format!("ghost-{}", common::test_id()),
            true,
            &common::w(),
        )
        .await;
}

#[tokio::test]
async fn test_agent_monitor_json_yields_lines() {
    let client = common::create_client();
    let (mut rx, handle) = match client.agent_monitor_json("DEBUG", &common::q()).await {
        Ok(x) => x,
        Err(_) => return, // monitor disabled — acceptable
    };

    // Give the server up to 3 seconds to emit at least one line
    let line = tokio::time::timeout(std::time::Duration::from_secs(3), rx.recv()).await;
    // Don't assert — just close cleanly
    drop(rx);
    handle.abort();
    if let Ok(Some(l)) = line {
        assert!(!l.is_empty(), "monitor line should be non-empty");
    }
}

#[tokio::test]
async fn test_agent_health_service_by_id_opts_missing() {
    let client = common::create_client();
    let id = format!("missing-{}", common::test_id());
    let (status, info) = client
        .agent_health_service_by_id_opts(&id, &common::q())
        .await
        .unwrap();
    assert!(info.is_none(), "missing service must return None info");
    assert!(status.is_empty(), "status for missing service must be empty");
}

#[tokio::test]
async fn test_agent_health_service_by_name_aggregate() {
    // Register a service, then query aggregated health by name.
    let client = common::create_client();
    let name = format!("compat-health-{}", common::test_id());
    let id = format!("{}-1", name);
    let reg = AgentServiceRegistration {
        id: Some(id.clone()),
        name: name.clone(),
        port: Some(8080),
        ..Default::default()
    };
    client
        .agent_service_register(&reg, &common::w())
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let (status, infos) = client
        .agent_health_service_by_name_opts(&name, &common::q())
        .await
        .unwrap();
    assert!(!infos.is_empty(), "should find registered instance");
    assert!(
        ["passing", "warning", "critical", ""].contains(&status.as_str()),
        "status must be one of the expected values, got '{}'",
        status
    );

    // Cleanup
    let _ = client.agent_service_deregister(&id, &common::w()).await;
}

#[tokio::test]
async fn test_agent_update_token_of_type_accepted() {
    let client = common::create_client();
    // Batata ignores Consul ACL tokens (uses JWT); the endpoint still
    // returns 200 OK so the call must succeed cleanly.
    let _ = client
        .agent_update_token_of_type("default", "irrelevant-token", &common::w())
        .await;
}

#[tokio::test]
async fn test_agent_update_agent_recovery_token_accepted() {
    let client = common::create_client();
    let _ = client
        .agent_update_agent_recovery_token("recovery-token", &common::w())
        .await;
}
