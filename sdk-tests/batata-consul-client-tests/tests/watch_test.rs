//! Integration tests for the Watch API (blocking query helpers)

mod common;

use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};
use std::time::Duration;

use base64::Engine;
use batata_consul_client::watch::{watch, watch_key, WatchConfig};
use batata_consul_client::{KVPair, QueryOptions};

fn b64(s: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(s.as_bytes())
}

#[tokio::test]
async fn test_watch_kv_change() {
    common::init_tracing();
    let client = Arc::new(common::create_client());
    let id = common::test_id();
    let key = format!("test/watch/{}", id);

    // Write initial value
    let pair = KVPair {
        key: key.clone(),
        value: Some(b64("initial")),
        ..Default::default()
    };
    let (ok, _) = client.kv_put(&pair, &common::w()).await.unwrap();
    assert!(ok, "Initial KV put should succeed");

    let call_count = Arc::new(AtomicU32::new(0));
    let call_count_clone = call_count.clone();
    let key_clone = key.clone();
    let client_clone = client.clone();

    // Start watch in background with a short wait time and max_errors to auto-stop
    let watch_handle = tokio::spawn(async move {
        let config = WatchConfig {
            wait_time: Duration::from_secs(2),
            error_retry_delay: Duration::from_millis(100),
            max_errors: 3,
        };

        watch_key(
            client_clone,
            key_clone,
            config,
            QueryOptions::default(),
            move |kv_opt, _meta| {
                call_count_clone.fetch_add(1, Ordering::SeqCst);
                if let Some(kv) = &kv_opt {
                    assert_eq!(
                        kv.key,
                        format!("test/watch/{}", id),
                        "Watched key should match"
                    );
                }
            },
        )
        .await;
    });

    // Give the watch a moment to start its first blocking query
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Update the value to trigger the watch
    let pair2 = KVPair {
        key: key.clone(),
        value: Some(b64("updated")),
        ..Default::default()
    };
    let (ok2, _) = client.kv_put(&pair2, &common::w()).await.unwrap();
    assert!(ok2, "KV update should succeed");

    // Wait for the watch to pick up the change
    tokio::time::sleep(Duration::from_secs(3)).await;

    let count = call_count.load(Ordering::SeqCst);
    assert!(
        count >= 1,
        "Watch handler should have been called at least once, got {}",
        count
    );

    // Cleanup: abort the watch task and delete the key
    watch_handle.abort();
    let _ = client.kv_delete(&key, &common::w()).await;
}

#[tokio::test]
async fn test_watch_config_defaults() {
    let config = WatchConfig::default();

    assert_eq!(
        config.wait_time,
        Duration::from_secs(300),
        "Default wait_time should be 5 minutes (300s)"
    );
    assert_eq!(
        config.error_retry_delay,
        Duration::from_secs(1),
        "Default error_retry_delay should be 1 second"
    );
    assert_eq!(
        config.max_errors, 0,
        "Default max_errors should be 0 (unlimited)"
    );
}

#[tokio::test]
async fn test_watch_error_recovery() {
    common::init_tracing();

    // Create a client pointing to a bad address to force errors
    let bad_config = batata_consul_client::ConsulClientConfig::new("http://127.0.0.1:19999");
    let bad_client = Arc::new(batata_consul_client::ConsulClient::new(bad_config).unwrap());

    let error_count = Arc::new(AtomicU32::new(0));
    let error_count_clone = error_count.clone();

    let max_errors = 3u32;
    let config = WatchConfig {
        wait_time: Duration::from_secs(1),
        error_retry_delay: Duration::from_millis(50),
        max_errors,
    };

    // Watch should stop after max_errors consecutive errors
    watch(
        config,
        QueryOptions::default(),
        |opts| {
            let client = bad_client.clone();
            let err_count = error_count_clone.clone();
            async move {
                err_count.fetch_add(1, Ordering::SeqCst);
                client.kv_get("nonexistent", &opts).await
            }
        },
        |_data: Option<KVPair>, _meta| {
            // Should never be called since all queries fail
            panic!("Handler should not be called on error");
        },
    )
    .await;

    let final_count = error_count.load(Ordering::SeqCst);
    assert!(
        final_count >= max_errors,
        "Watch should have attempted at least {} queries before stopping, got {}",
        max_errors,
        final_count
    );
}

#[tokio::test]
async fn test_watch_key_helper() {
    common::init_tracing();
    let client = Arc::new(common::create_client());
    let id = common::test_id();
    let key = format!("test/watch-key-helper/{}", id);

    // Write initial value
    let pair = KVPair {
        key: key.clone(),
        value: Some(b64("hello")),
        ..Default::default()
    };
    let (ok, _) = client.kv_put(&pair, &common::w()).await.unwrap();
    assert!(ok, "Initial KV put should succeed");

    let received = Arc::new(AtomicU32::new(0));
    let received_clone = received.clone();

    let config = WatchConfig {
        wait_time: Duration::from_secs(2),
        error_retry_delay: Duration::from_millis(100),
        max_errors: 3,
    };

    let client_clone = client.clone();
    let key_clone = key.clone();
    let handle = tokio::spawn(async move {
        watch_key(
            client_clone,
            key_clone,
            config,
            QueryOptions::default(),
            move |_kv_opt, _meta| {
                received_clone.fetch_add(1, Ordering::SeqCst);
            },
        )
        .await;
    });

    // Give it time to start
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Update to trigger watch
    let pair2 = KVPair {
        key: key.clone(),
        value: Some(b64("world")),
        ..Default::default()
    };
    client.kv_put(&pair2, &common::w()).await.unwrap();

    tokio::time::sleep(Duration::from_secs(3)).await;

    let count = received.load(Ordering::SeqCst);
    assert!(
        count >= 1,
        "watch_key handler should have been called at least once, got {}",
        count
    );

    handle.abort();
    let _ = client.kv_delete(&key, &common::w()).await;
}

#[tokio::test]
async fn test_watch_index_tracking() {
    common::init_tracing();
    let client = common::create_client();
    let id = common::test_id();
    let key = format!("test/watch-index/{}", id);

    // Write a value and capture the index
    let pair = KVPair {
        key: key.clone(),
        value: Some(b64("v1")),
        ..Default::default()
    };
    let (ok, _) = client.kv_put(&pair, &common::w()).await.unwrap();
    assert!(ok, "First KV put should succeed");

    let (kv_opt, meta1) = client.kv_get(&key, &common::q()).await.unwrap();
    assert!(kv_opt.is_some(), "Key should exist after put");
    let index1 = meta1.last_index;
    assert!(index1 > 0, "First index should be positive");

    // Write a second value
    let pair2 = KVPair {
        key: key.clone(),
        value: Some(b64("v2")),
        ..Default::default()
    };
    let (ok2, _) = client.kv_put(&pair2, &common::w()).await.unwrap();
    assert!(ok2, "Second KV put should succeed");

    let (kv_opt2, meta2) = client.kv_get(&key, &common::q()).await.unwrap();
    assert!(kv_opt2.is_some(), "Key should still exist");
    let index2 = meta2.last_index;
    assert!(
        index2 > index1,
        "Index should advance after KV update: {} > {}",
        index2,
        index1
    );

    // Cleanup
    let _ = client.kv_delete(&key, &common::w()).await;
}

#[tokio::test]
async fn test_watch_cancel() {
    common::init_tracing();
    let client = Arc::new(common::create_client());
    let id = common::test_id();
    let key = format!("test/watch-cancel/{}", id);

    let config = WatchConfig {
        wait_time: Duration::from_secs(2),
        error_retry_delay: Duration::from_millis(100),
        max_errors: 0, // unlimited
    };

    let client_clone = client.clone();
    let key_clone = key.clone();
    let handle = tokio::spawn(async move {
        watch_key(
            client_clone,
            key_clone,
            config,
            QueryOptions::default(),
            |_kv_opt, _meta| {},
        )
        .await;
    });

    // Give the watch time to start
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Verify the task is still running (not finished)
    assert!(!handle.is_finished(), "Watch task should still be running");

    // Abort (drop) the watch task
    handle.abort();

    // Wait a moment and verify it stopped
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        handle.is_finished(),
        "Watch task should be finished after abort"
    );
}
