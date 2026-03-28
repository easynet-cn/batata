mod common;

use batata_consul_client::SessionEntry;

#[tokio::test]
#[ignore]
async fn test_session_create_and_destroy() {
    let client = common::create_client();

    let entry = SessionEntry {
        name: Some(format!("session-create-{}", common::test_id())),
        ttl: Some("30s".to_string()),
        ..Default::default()
    };

    // Create
    let (session_id, _) = client.session_create(&entry, &common::w()).await.unwrap();
    assert!(!session_id.is_empty(), "session ID should not be empty");

    // Verify it exists via info
    let (entries, _) = client
        .session_info(&session_id, &common::q())
        .await
        .unwrap();
    assert_eq!(
        entries.len(),
        1,
        "session_info should return exactly 1 entry"
    );
    assert_eq!(entries[0].id.as_deref(), Some(session_id.as_str()));

    // Destroy
    let _ = client
        .session_destroy(&session_id, &common::w())
        .await
        .unwrap();

    // Verify gone
    let (entries, _) = client
        .session_info(&session_id, &common::q())
        .await
        .unwrap();
    assert!(entries.is_empty(), "session should be gone after destroy");
}

#[tokio::test]
#[ignore]
async fn test_session_info() {
    let client = common::create_client();
    let name = format!("session-info-{}", common::test_id());

    let entry = SessionEntry {
        name: Some(name.clone()),
        ttl: Some("30s".to_string()),
        ..Default::default()
    };
    let (session_id, _) = client.session_create(&entry, &common::w()).await.unwrap();

    let (entries, _meta) = client
        .session_info(&session_id, &common::q())
        .await
        .unwrap();
    assert_eq!(entries.len(), 1, "should return 1 session");

    let session = &entries[0];
    assert_eq!(session.id.as_deref(), Some(session_id.as_str()));
    assert_eq!(
        session.name.as_deref(),
        Some(name.as_str()),
        "session name should match"
    );
    assert!(session.node.is_some(), "session should have a node");
    assert!(
        !session.node.as_deref().unwrap_or("").is_empty(),
        "node should not be empty"
    );
    assert!(session.create_index > 0, "create_index should be positive");

    // Cleanup
    let _ = client.session_destroy(&session_id, &common::w()).await;
}

#[tokio::test]
#[ignore]
async fn test_session_list() {
    let client = common::create_client();
    let mut session_ids = Vec::new();

    // Create 2 sessions
    for i in 0..2 {
        let entry = SessionEntry {
            name: Some(format!("session-list-{}-{}", i, common::test_id())),
            ttl: Some("30s".to_string()),
            ..Default::default()
        };
        let (id, _) = client.session_create(&entry, &common::w()).await.unwrap();
        assert!(!id.is_empty());
        session_ids.push(id);
    }

    // List all sessions
    let (entries, _) = client.session_list(&common::q()).await.unwrap();
    assert!(
        entries.len() >= 2,
        "should list at least 2 sessions, got {}",
        entries.len()
    );

    // Verify our sessions are in the list
    for sid in &session_ids {
        let found = entries
            .iter()
            .any(|e| e.id.as_deref() == Some(sid.as_str()));
        assert!(found, "session {} should be in the list", sid);
    }

    // Cleanup
    for sid in &session_ids {
        let _ = client.session_destroy(sid, &common::w()).await;
    }
}

#[tokio::test]
#[ignore]
async fn test_session_node() {
    let client = common::create_client();

    let entry = SessionEntry {
        name: Some(format!("session-node-{}", common::test_id())),
        ttl: Some("30s".to_string()),
        ..Default::default()
    };
    let (session_id, _) = client.session_create(&entry, &common::w()).await.unwrap();

    // Get session info to find the node name
    let (entries, _) = client
        .session_info(&session_id, &common::q())
        .await
        .unwrap();
    assert_eq!(entries.len(), 1);
    let node_name = entries[0].node.clone().expect("session should have a node");
    assert!(!node_name.is_empty());

    // Query sessions by node
    let (node_sessions, _) = client.session_node(&node_name, &common::q()).await.unwrap();
    assert!(
        !node_sessions.is_empty(),
        "node should have at least one session"
    );

    let found = node_sessions
        .iter()
        .any(|e| e.id.as_deref() == Some(session_id.as_str()));
    assert!(
        found,
        "our session should appear in the node's session list"
    );

    // Cleanup
    let _ = client.session_destroy(&session_id, &common::w()).await;
}

#[tokio::test]
#[ignore]
async fn test_session_renew() {
    let client = common::create_client();

    let entry = SessionEntry {
        name: Some(format!("session-renew-{}", common::test_id())),
        ttl: Some("30s".to_string()),
        ..Default::default()
    };
    let (session_id, _) = client.session_create(&entry, &common::w()).await.unwrap();

    // Renew the session
    let (renewed, _) = client
        .session_renew(&session_id, &common::w())
        .await
        .unwrap();
    assert!(!renewed.is_empty(), "renew should return the session entry");
    assert_eq!(
        renewed[0].id.as_deref(),
        Some(session_id.as_str()),
        "renewed session ID should match"
    );

    // Verify it is still alive
    let (entries, _) = client
        .session_info(&session_id, &common::q())
        .await
        .unwrap();
    assert_eq!(entries.len(), 1, "session should still exist after renew");

    // Cleanup
    let _ = client.session_destroy(&session_id, &common::w()).await;
}

#[tokio::test]
#[ignore]
async fn test_session_create_no_checks() {
    let client = common::create_client();

    let entry = SessionEntry {
        name: Some(format!("session-no-checks-{}", common::test_id())),
        ttl: Some("30s".to_string()),
        ..Default::default()
    };

    let (session_id, _) = client
        .session_create_no_checks(&entry, &common::w())
        .await
        .unwrap();
    assert!(!session_id.is_empty(), "session ID should not be empty");

    // Verify the session exists and has the correct name
    let (entries, _) = client
        .session_info(&session_id, &common::q())
        .await
        .unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0].name.as_deref(),
        Some(entry.name.as_deref().unwrap()),
        "session name should match"
    );

    // Cleanup
    let _ = client.session_destroy(&session_id, &common::w()).await;
}

#[tokio::test]
#[ignore]
async fn test_session_with_behavior() {
    let client = common::create_client();

    let entry = SessionEntry {
        name: Some(format!("session-behavior-{}", common::test_id())),
        behavior: Some("delete".to_string()),
        ttl: Some("30s".to_string()),
        ..Default::default()
    };

    let (session_id, _) = client.session_create(&entry, &common::w()).await.unwrap();
    assert!(!session_id.is_empty(), "session ID should not be empty");

    // Verify the session exists with the specified behavior
    let (entries, _) = client
        .session_info(&session_id, &common::q())
        .await
        .unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0].behavior.as_deref(),
        Some("delete"),
        "session behavior should be 'delete'"
    );

    // Cleanup
    let _ = client.session_destroy(&session_id, &common::w()).await;
}
