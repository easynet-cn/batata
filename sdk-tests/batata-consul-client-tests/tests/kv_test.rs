mod common;

use base64::Engine;
use batata_consul_client::KVPair;

/// Helper to create a KVPair with a UTF-8 value (base64-encoded).
fn kv_pair(key: &str, value: &str) -> KVPair {
    KVPair {
        key: key.to_string(),
        value: Some(base64::engine::general_purpose::STANDARD.encode(value.as_bytes())),
        ..Default::default()
    }
}

#[tokio::test]
#[ignore]
async fn test_kv_put_and_get() {
    let client = common::create_client();
    let key = format!("test/kv/put-get/{}", common::test_id());

    // Put a key
    let (ok, _) = client
        .kv_put(&kv_pair(&key, "hello-world"), &common::w())
        .await
        .unwrap();
    assert!(ok, "kv_put should succeed");

    // Get it back
    let (pair, _) = client.kv_get(&key, &common::q()).await.unwrap();
    let pair = pair.expect("kv_get should return Some for existing key");
    assert_eq!(pair.key, key);
    assert_eq!(pair.value_str().unwrap(), "hello-world");
    assert!(pair.create_index > 0, "create_index should be positive");
    assert!(pair.modify_index > 0, "modify_index should be positive");

    // Cleanup
    let _ = client.kv_delete(&key, &common::w()).await;
}

#[tokio::test]
#[ignore]
async fn test_kv_delete() {
    let client = common::create_client();
    let key = format!("test/kv/delete/{}", common::test_id());

    // Put then delete
    let (ok, _) = client
        .kv_put(&kv_pair(&key, "to-be-deleted"), &common::w())
        .await
        .unwrap();
    assert!(ok, "kv_put should succeed");

    let (ok, _) = client.kv_delete(&key, &common::w()).await.unwrap();
    assert!(ok, "kv_delete should succeed");

    // Verify gone
    let (pair, _) = client.kv_get(&key, &common::q()).await.unwrap();
    assert!(pair.is_none(), "key should be gone after delete");
}

#[tokio::test]
#[ignore]
async fn test_kv_list() {
    let client = common::create_client();
    let prefix = format!("test/kv/list/{}", common::test_id());

    // Put 3 keys under the same prefix
    for i in 0..3 {
        let key = format!("{}/key-{}", prefix, i);
        let (ok, _) = client
            .kv_put(&kv_pair(&key, &format!("val-{}", i)), &common::w())
            .await
            .unwrap();
        assert!(ok, "kv_put for key-{} should succeed", i);
    }

    // List all under prefix
    let (pairs, _) = client.kv_list(&prefix, &common::q()).await.unwrap();
    assert_eq!(pairs.len(), 3, "should list 3 keys under prefix");

    // Verify each key belongs to the prefix
    for pair in &pairs {
        assert!(
            pair.key.starts_with(&prefix),
            "key '{}' should start with prefix '{}'",
            pair.key,
            prefix
        );
        assert!(pair.value_str().is_some(), "each key should have a value");
    }

    // Cleanup
    let _ = client.kv_delete_tree(&prefix, &common::w()).await;
}

#[tokio::test]
#[ignore]
async fn test_kv_keys() {
    let client = common::create_client();
    let prefix = format!("test/kv/keys/{}", common::test_id());

    for i in 0..3 {
        let key = format!("{}/item-{}", prefix, i);
        let (ok, _) = client
            .kv_put(&kv_pair(&key, "x"), &common::w())
            .await
            .unwrap();
        assert!(ok, "kv_put for item-{} should succeed", i);
    }

    // Get keys only (no values)
    let (keys, _) = client
        .kv_keys(&format!("{}/", prefix), "", &common::q())
        .await
        .unwrap();
    assert_eq!(keys.len(), 3, "should return 3 keys");
    for key in &keys {
        assert!(
            key.starts_with(&prefix),
            "key '{}' should start with prefix '{}'",
            key,
            prefix
        );
    }

    // Cleanup
    let _ = client.kv_delete_tree(&prefix, &common::w()).await;
}

#[tokio::test]
#[ignore]
async fn test_kv_cas() {
    let client = common::create_client();
    let key = format!("test/kv/cas/{}", common::test_id());

    // Initial put
    let (ok, _) = client
        .kv_put(&kv_pair(&key, "initial"), &common::w())
        .await
        .unwrap();
    assert!(ok);

    // Get to obtain modify_index
    let (pair, _) = client.kv_get(&key, &common::q()).await.unwrap();
    let pair = pair.expect("key should exist");
    let correct_index = pair.modify_index;

    // CAS with correct index should succeed
    let mut cas_pair = kv_pair(&key, "updated-via-cas");
    cas_pair.modify_index = correct_index;
    let (ok, _) = client.kv_cas(&cas_pair, &common::w()).await.unwrap();
    assert!(ok, "CAS with correct modify_index should succeed");

    // Verify value was updated
    let (pair, _) = client.kv_get(&key, &common::q()).await.unwrap();
    let pair = pair.expect("key should still exist");
    assert_eq!(pair.value_str().unwrap(), "updated-via-cas");

    // CAS with wrong (stale) index should fail
    let mut stale_pair = kv_pair(&key, "should-not-apply");
    stale_pair.modify_index = correct_index; // stale index
    let (ok, _) = client.kv_cas(&stale_pair, &common::w()).await.unwrap();
    assert!(!ok, "CAS with stale modify_index should fail");

    // Verify value unchanged
    let (pair, _) = client.kv_get(&key, &common::q()).await.unwrap();
    assert_eq!(pair.unwrap().value_str().unwrap(), "updated-via-cas");

    // Cleanup
    let _ = client.kv_delete(&key, &common::w()).await;
}

#[tokio::test]
#[ignore]
async fn test_kv_acquire_release() {
    let client = common::create_client();
    let key = format!("test/kv/lock/{}", common::test_id());

    // Create a session for locking
    let session_entry = batata_consul_client::SessionEntry {
        name: Some("kv-lock-test".to_string()),
        ttl: Some("30s".to_string()),
        ..Default::default()
    };
    let (session_id, _) = client
        .session_create(&session_entry, &common::w())
        .await
        .unwrap();
    assert!(!session_id.is_empty(), "session ID should not be empty");

    // Acquire lock
    let mut lock_pair = kv_pair(&key, "locked-value");
    lock_pair.session = Some(session_id.clone());
    let (ok, _) = client.kv_acquire(&lock_pair, &common::w()).await.unwrap();
    assert!(ok, "acquire lock should succeed");

    // Verify session is set on the key
    let (pair, _) = client.kv_get(&key, &common::q()).await.unwrap();
    let pair = pair.expect("locked key should exist");
    assert_eq!(
        pair.session.as_deref(),
        Some(session_id.as_str()),
        "session should be set on key"
    );

    // Release lock
    let release_pair = KVPair {
        key: key.clone(),
        session: Some(session_id.clone()),
        ..Default::default()
    };
    let (ok, _) = client
        .kv_release(&release_pair, &common::w())
        .await
        .unwrap();
    assert!(ok, "release lock should succeed");

    // Verify session is cleared
    let (pair, _) = client.kv_get(&key, &common::q()).await.unwrap();
    let pair = pair.expect("key should still exist after release");
    assert!(
        pair.session.is_none() || pair.session.as_deref() == Some(""),
        "session should be cleared after release"
    );

    // Cleanup
    let _ = client.session_destroy(&session_id, &common::w()).await;
    let _ = client.kv_delete(&key, &common::w()).await;
}

#[tokio::test]
#[ignore]
async fn test_kv_delete_tree() {
    let client = common::create_client();
    let prefix = format!("test/kv/tree/{}", common::test_id());

    // Put 3 keys
    for i in 0..3 {
        let key = format!("{}/node-{}", prefix, i);
        let (ok, _) = client
            .kv_put(&kv_pair(&key, &format!("data-{}", i)), &common::w())
            .await
            .unwrap();
        assert!(ok);
    }

    // Verify all exist
    let (pairs, _) = client.kv_list(&prefix, &common::q()).await.unwrap();
    assert_eq!(pairs.len(), 3, "should have 3 keys before delete_tree");

    // Delete tree
    let (ok, _) = client.kv_delete_tree(&prefix, &common::w()).await.unwrap();
    assert!(ok, "kv_delete_tree should succeed");

    // Verify all gone
    let (pairs, _) = client.kv_list(&prefix, &common::q()).await.unwrap();
    assert!(
        pairs.is_empty(),
        "all keys should be gone after delete_tree"
    );
}

#[tokio::test]
#[ignore]
async fn test_kv_flags() {
    let client = common::create_client();
    let key = format!("test/kv/flags/{}", common::test_id());

    let mut pair = kv_pair(&key, "flagged-value");
    pair.flags = 42;
    let (ok, _) = client.kv_put(&pair, &common::w()).await.unwrap();
    assert!(ok, "kv_put with flags should succeed");

    let (result, _) = client.kv_get(&key, &common::q()).await.unwrap();
    let result = result.expect("key should exist");
    assert_eq!(result.flags, 42, "flags should be preserved");
    assert_eq!(result.value_str().unwrap(), "flagged-value");

    // Cleanup
    let _ = client.kv_delete(&key, &common::w()).await;
}

#[tokio::test]
#[ignore]
async fn test_kv_large_value() {
    let client = common::create_client();
    let key = format!("test/kv/large/{}", common::test_id());

    // Create a 100KB value
    let large_value = "X".repeat(100 * 1024);
    let (ok, _) = client
        .kv_put(&kv_pair(&key, &large_value), &common::w())
        .await
        .unwrap();
    assert!(ok, "kv_put with 100KB value should succeed");

    let (pair, _) = client.kv_get(&key, &common::q()).await.unwrap();
    let pair = pair.expect("large key should exist");
    let retrieved = pair.value_str().unwrap();
    assert_eq!(
        retrieved.len(),
        large_value.len(),
        "retrieved value length should match"
    );
    assert_eq!(
        retrieved, large_value,
        "retrieved value should match exactly"
    );

    // Cleanup
    let _ = client.kv_delete(&key, &common::w()).await;
}
