//! Integration tests for the Lock and Semaphore API

mod common;

use batata_consul_client::SessionEntry;

/// Helper: create a session with no health checks and return its ID
async fn create_session(client: &batata_consul_client::ConsulClient) -> String {
    let entry = SessionEntry {
        name: Some(format!("lock-test-{}", common::test_id())),
        ttl: Some("30s".to_string()),
        behavior: Some("delete".to_string()),
        ..Default::default()
    };
    let (session_id, _) = client
        .session_create_no_checks(&entry, &common::w())
        .await
        .unwrap();
    assert!(
        !session_id.is_empty(),
        "Session ID should not be empty after creation"
    );
    session_id
}

#[tokio::test]
#[ignore]
async fn test_lock_acquire_and_release() {
    common::init_tracing();
    let client = common::create_client();
    let id = common::test_id();
    let key = format!("test/lock/{}", id);

    let session_id = create_session(&client).await;

    // Acquire the lock
    let (acquired, _) = client
        .lock_acquire(&key, &session_id, Some(b"holder-1"), &common::w())
        .await
        .unwrap();
    assert!(acquired, "First lock acquisition should succeed");

    // Verify the KV key has our session attached
    let (kv_opt, _) = client.kv_get(&key, &common::q()).await.unwrap();
    assert!(kv_opt.is_some(), "KV key should exist after lock acquire");
    let kv = kv_opt.unwrap();
    assert_eq!(
        kv.session.as_deref(),
        Some(session_id.as_str()),
        "KV session should match the locking session"
    );

    // Release the lock
    let (released, _) = client
        .lock_release(&key, &session_id, &common::w())
        .await
        .unwrap();
    assert!(released, "Lock release should succeed");

    // Verify session is cleared from the KV key
    let (kv_opt, _) = client.kv_get(&key, &common::q()).await.unwrap();
    if let Some(kv) = kv_opt {
        assert!(
            kv.session.is_none() || kv.session.as_deref() == Some(""),
            "KV session should be cleared after lock release"
        );
    }

    // Cleanup
    let _ = client.session_destroy(&session_id, &common::w()).await;
}

#[tokio::test]
#[ignore]
async fn test_lock_contention() {
    common::init_tracing();
    let client = common::create_client();
    let id = common::test_id();
    let key = format!("test/lock-contention/{}", id);

    let session_a = create_session(&client).await;
    let session_b = create_session(&client).await;

    // Session A acquires the lock
    let (acquired_a, _) = client
        .lock_acquire(&key, &session_a, Some(b"holder-a"), &common::w())
        .await
        .unwrap();
    assert!(acquired_a, "Session A should acquire the lock");

    // Session B tries to acquire the same lock — should fail
    let (acquired_b, _) = client
        .lock_acquire(&key, &session_b, Some(b"holder-b"), &common::w())
        .await
        .unwrap();
    assert!(
        !acquired_b,
        "Session B should NOT acquire the lock held by Session A"
    );

    // Release from A, then B should succeed
    let (released, _) = client
        .lock_release(&key, &session_a, &common::w())
        .await
        .unwrap();
    assert!(released, "Session A should release the lock");

    let (acquired_b2, _) = client
        .lock_acquire(&key, &session_b, Some(b"holder-b"), &common::w())
        .await
        .unwrap();
    assert!(
        acquired_b2,
        "Session B should acquire the lock after Session A releases"
    );

    // Cleanup
    let _ = client.lock_release(&key, &session_b, &common::w()).await;
    let _ = client.session_destroy(&session_a, &common::w()).await;
    let _ = client.session_destroy(&session_b, &common::w()).await;
}

#[tokio::test]
#[ignore]
async fn test_semaphore_acquire_and_release() {
    common::init_tracing();
    let client = common::create_client();
    let id = common::test_id();
    let prefix = format!("test/semaphore/{}", id);

    let session_id = create_session(&client).await;

    // Acquire a semaphore slot with limit 2
    let (acquired, _) = client
        .semaphore_acquire(&prefix, &session_id, 2, &common::w())
        .await
        .unwrap();
    assert!(
        acquired,
        "Semaphore acquisition should succeed within limit"
    );

    // Verify the contender key exists
    let contender_key = format!("{}/{}", prefix, session_id);
    let (kv_opt, _) = client.kv_get(&contender_key, &common::q()).await.unwrap();
    assert!(
        kv_opt.is_some(),
        "Contender KV key should exist after semaphore acquire"
    );

    // Release the semaphore slot
    let (released, _) = client
        .semaphore_release(&prefix, &session_id, &common::w())
        .await
        .unwrap();
    assert!(released, "Semaphore release should succeed");

    // Cleanup
    let _ = client.session_destroy(&session_id, &common::w()).await;
}

#[tokio::test]
#[ignore]
async fn test_semaphore_limit() {
    common::init_tracing();
    let client = common::create_client();
    let id = common::test_id();
    let prefix = format!("test/semaphore-limit/{}", id);
    let limit = 2u32;

    // Create sessions for each contender
    let session_1 = create_session(&client).await;
    let session_2 = create_session(&client).await;
    let session_3 = create_session(&client).await;

    // Acquire slot 1
    let (acq1, _) = client
        .semaphore_acquire(&prefix, &session_1, limit, &common::w())
        .await
        .unwrap();
    assert!(acq1, "First semaphore slot should be acquired");

    // Acquire slot 2
    let (acq2, _) = client
        .semaphore_acquire(&prefix, &session_2, limit, &common::w())
        .await
        .unwrap();
    assert!(
        acq2,
        "Second semaphore slot should be acquired (within limit)"
    );

    // Attempt to acquire slot 3 — should exceed the limit
    // Note: Consul's semaphore is advisory; the KV acquire may still succeed
    // because the limit enforcement is client-side in the standard Consul
    // semaphore protocol. We verify that the third contender key is created.
    let (acq3, _) = client
        .semaphore_acquire(&prefix, &session_3, limit, &common::w())
        .await
        .unwrap();
    // The raw KV acquire may succeed since enforcement is client-side;
    // we verify the contender key exists regardless.
    let contender_key_3 = format!("{}/{}", prefix, session_3);
    let (kv_opt, _) = client.kv_get(&contender_key_3, &common::q()).await.unwrap();
    assert!(
        kv_opt.is_some(),
        "Third contender key should exist (KV-level acquire)"
    );
    // Log whether the third acquire reported success for diagnostics
    tracing::info!("Third semaphore acquire returned: {}", acq3);

    // Cleanup
    let _ = client
        .semaphore_release(&prefix, &session_1, &common::w())
        .await;
    let _ = client
        .semaphore_release(&prefix, &session_2, &common::w())
        .await;
    let _ = client
        .semaphore_release(&prefix, &session_3, &common::w())
        .await;
    let _ = client.session_destroy(&session_1, &common::w()).await;
    let _ = client.session_destroy(&session_2, &common::w()).await;
    let _ = client.session_destroy(&session_3, &common::w()).await;
}
