//! Config listener tests ported from NacosConfigListenerTest.java
//!
//! Tests: add/remove listener, multiple listeners, rapid changes,
//! namespace/group isolation, non-existent config, duplicate listener,
//! large config notification.
//!
//! Requires a running Batata server.

mod common;

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use batata_client::config::listener::{ConfigChangeListener, ConfigResponse, FnConfigChangeListener};
use tokio::sync::Notify;
use tokio::time::sleep;

/// NCL-001: Add listener and receive notification
#[tokio::test]
async fn test_add_config_listener() {
    common::init_tracing();
    let svc = common::create_config_service().await.unwrap();
    let id = common::test_id();
    let data_id = format!("listener-add-{}", id);
    let content = "test.value=1";

    let notify = Arc::new(Notify::new());
    let received = Arc::new(tokio::sync::Mutex::new(String::new()));

    let n = notify.clone();
    let r = received.clone();
    let listener: Arc<dyn ConfigChangeListener> = Arc::new(FnConfigChangeListener::new(move |info: ConfigResponse| {
        *r.blocking_lock() = info.content.clone();
        n.notify_one();
    }));

    svc.add_listener(&data_id, "DEFAULT_GROUP", "", listener.clone())
        .await
        .unwrap();
    sleep(Duration::from_millis(500)).await;

    let published = svc
        .publish_config(&data_id, "DEFAULT_GROUP", "", content)
        .await
        .unwrap();
    assert!(published);

    let got = tokio::time::timeout(Duration::from_secs(10), notify.notified())
        .await
        .is_ok();
    assert!(got, "Listener should receive notification within 10s");
    assert_eq!(*received.lock().await, content);

    // Cleanup
    svc.remove_specific_listener(&data_id, "DEFAULT_GROUP", "", &listener)
        .await
        .unwrap();
    svc.remove_config(&data_id, "DEFAULT_GROUP", "").await.ok();
}

/// NCL-002: Remove listener stops notifications
#[tokio::test]
async fn test_remove_config_listener() {
    common::init_tracing();
    let svc = common::create_config_service().await.unwrap();
    let id = common::test_id();
    let data_id = format!("listener-remove-{}", id);

    let call_count = Arc::new(AtomicU32::new(0));
    let first_notify = Arc::new(Notify::new());

    let cc = call_count.clone();
    let fn_ = first_notify.clone();
    let listener: Arc<dyn ConfigChangeListener> = Arc::new(FnConfigChangeListener::new(move |_: ConfigResponse| {
        let prev = cc.fetch_add(1, Ordering::SeqCst);
        if prev == 0 {
            fn_.notify_one();
        }
    }));

    svc.add_listener(&data_id, "DEFAULT_GROUP", "", listener.clone())
        .await
        .unwrap();
    sleep(Duration::from_millis(500)).await;

    svc.publish_config(&data_id, "DEFAULT_GROUP", "", "first=value")
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(10), first_notify.notified())
        .await
        .ok();

    let count_after_first = call_count.load(Ordering::SeqCst);
    assert!(count_after_first >= 1, "Should receive at least 1 notification");

    // Remove listener
    svc.remove_specific_listener(&data_id, "DEFAULT_GROUP", "", &listener)
        .await
        .unwrap();
    sleep(Duration::from_millis(500)).await;

    // Publish again — should NOT trigger
    svc.publish_config(&data_id, "DEFAULT_GROUP", "", "second=value")
        .await
        .unwrap();
    sleep(Duration::from_secs(3)).await;

    let count_after_second = call_count.load(Ordering::SeqCst);
    assert_eq!(
        count_after_first, count_after_second,
        "No additional notifications after listener removal"
    );

    svc.remove_config(&data_id, "DEFAULT_GROUP", "").await.ok();
}

/// NCL-003: Multiple listeners all receive notification
#[tokio::test]
async fn test_multiple_listeners() {
    common::init_tracing();
    let svc = common::create_config_service().await.unwrap();
    let id = common::test_id();
    let data_id = format!("listener-multi-{}", id);
    let content = "multi.test=value";

    let count = Arc::new(AtomicU32::new(0));
    let all_done = Arc::new(Notify::new());

    let mut listeners = Vec::new();
    for _ in 0..3 {
        let c = count.clone();
        let a = all_done.clone();
        let listener: Arc<dyn ConfigChangeListener> = Arc::new(FnConfigChangeListener::new(move |_: ConfigResponse| {
            if c.fetch_add(1, Ordering::SeqCst) == 2 {
                a.notify_one();
            }
        }));
        svc.add_listener(&data_id, "DEFAULT_GROUP", "", listener.clone())
            .await
            .unwrap();
        listeners.push(listener);
    }

    sleep(Duration::from_millis(500)).await;
    svc.publish_config(&data_id, "DEFAULT_GROUP", "", content)
        .await
        .unwrap();

    let all_received = tokio::time::timeout(Duration::from_secs(15), all_done.notified())
        .await
        .is_ok();
    assert!(all_received, "All 3 listeners should be notified");
    assert_eq!(count.load(Ordering::SeqCst), 3);

    for l in &listeners {
        svc.remove_specific_listener(&data_id, "DEFAULT_GROUP", "", l)
            .await
            .unwrap();
    }
    svc.remove_config(&data_id, "DEFAULT_GROUP", "").await.ok();
}

/// NCL-005: Listener receives multiple updates
#[tokio::test]
async fn test_listener_multiple_updates() {
    common::init_tracing();
    let svc = common::create_config_service().await.unwrap();
    let id = common::test_id();
    let data_id = format!("listener-updates-{}", id);

    let count = Arc::new(AtomicU32::new(0));
    let last_value = Arc::new(tokio::sync::Mutex::new(String::new()));

    let c = count.clone();
    let lv = last_value.clone();
    let listener: Arc<dyn ConfigChangeListener> = Arc::new(FnConfigChangeListener::new(move |info: ConfigResponse| {
        c.fetch_add(1, Ordering::SeqCst);
        *lv.blocking_lock() = info.content.clone();
    }));

    svc.add_listener(&data_id, "DEFAULT_GROUP", "", listener.clone())
        .await
        .unwrap();
    sleep(Duration::from_millis(500)).await;

    for i in 0..5 {
        svc.publish_config(&data_id, "DEFAULT_GROUP", "", &format!("update={}", i))
            .await
            .unwrap();
        sleep(Duration::from_millis(1500)).await;
    }
    sleep(Duration::from_secs(5)).await;

    let final_count = count.load(Ordering::SeqCst);
    assert!(final_count >= 1, "Should receive at least 1 notification");
    assert_eq!(*last_value.lock().await, "update=4");

    svc.remove_specific_listener(&data_id, "DEFAULT_GROUP", "", &listener)
        .await
        .unwrap();
    svc.remove_config(&data_id, "DEFAULT_GROUP", "").await.ok();
}

/// NCL-010: Rapid config changes — at least one notification, last value correct
#[tokio::test]
async fn test_listener_rapid_changes() {
    common::init_tracing();
    let svc = common::create_config_service().await.unwrap();
    let id = common::test_id();
    let data_id = format!("listener-rapid-{}", id);

    let count = Arc::new(AtomicU32::new(0));
    let last_value = Arc::new(tokio::sync::Mutex::new(String::new()));

    let c = count.clone();
    let lv = last_value.clone();
    let listener: Arc<dyn ConfigChangeListener> = Arc::new(FnConfigChangeListener::new(move |info: ConfigResponse| {
        c.fetch_add(1, Ordering::SeqCst);
        *lv.blocking_lock() = info.content.clone();
    }));

    svc.add_listener(&data_id, "DEFAULT_GROUP", "", listener.clone())
        .await
        .unwrap();
    sleep(Duration::from_millis(500)).await;

    for i in 0..10 {
        svc.publish_config(&data_id, "DEFAULT_GROUP", "", &format!("rapid.change={}", i))
            .await
            .unwrap();
        sleep(Duration::from_millis(500)).await;
    }
    sleep(Duration::from_secs(5)).await;

    let final_count = count.load(Ordering::SeqCst);
    assert!(final_count >= 1, "At least 1 notification (some may be coalesced)");
    assert_eq!(*last_value.lock().await, "rapid.change=9");

    svc.remove_specific_listener(&data_id, "DEFAULT_GROUP", "", &listener)
        .await
        .unwrap();
    svc.remove_config(&data_id, "DEFAULT_GROUP", "").await.ok();
}

/// NCL-011: Listener for non-existent config fires when config is created
#[tokio::test]
async fn test_listener_for_non_existent_config() {
    common::init_tracing();
    let svc = common::create_config_service().await.unwrap();
    let id = common::test_id();
    let data_id = format!("listener-nonexist-{}", id);
    let content = "created.later=true";

    let notify = Arc::new(Notify::new());
    let received = Arc::new(tokio::sync::Mutex::new(String::new()));

    let n = notify.clone();
    let r = received.clone();
    let listener: Arc<dyn ConfigChangeListener> = Arc::new(FnConfigChangeListener::new(move |info: ConfigResponse| {
        *r.blocking_lock() = info.content.clone();
        n.notify_one();
    }));

    // Add listener BEFORE config exists
    svc.add_listener(&data_id, "DEFAULT_GROUP", "", listener.clone())
        .await
        .unwrap();
    sleep(Duration::from_millis(500)).await;

    // Now create the config
    svc.publish_config(&data_id, "DEFAULT_GROUP", "", content)
        .await
        .unwrap();

    let got = tokio::time::timeout(Duration::from_secs(10), notify.notified())
        .await
        .is_ok();
    assert!(got, "Listener should fire when config is created");
    assert_eq!(*received.lock().await, content);

    svc.remove_specific_listener(&data_id, "DEFAULT_GROUP", "", &listener)
        .await
        .unwrap();
    svc.remove_config(&data_id, "DEFAULT_GROUP", "").await.ok();
}

/// NCL-013: Large config notification (100KB)
#[tokio::test]
async fn test_large_config_notification() {
    common::init_tracing();
    let svc = common::create_config_service().await.unwrap();
    let id = common::test_id();
    let data_id = format!("listener-large-{}", id);

    // Build ~100KB content
    let mut content = String::new();
    for i in 0..10000 {
        content.push_str(&format!("property{}=value{}\n", i, i));
    }

    let notify = Arc::new(Notify::new());
    let received_len = Arc::new(AtomicU32::new(0));

    let n = notify.clone();
    let rl = received_len.clone();
    let listener: Arc<dyn ConfigChangeListener> = Arc::new(FnConfigChangeListener::new(move |info: ConfigResponse| {
        rl.store(info.content.len() as u32, Ordering::SeqCst);
        n.notify_one();
    }));

    svc.add_listener(&data_id, "DEFAULT_GROUP", "", listener.clone())
        .await
        .unwrap();
    sleep(Duration::from_millis(500)).await;

    svc.publish_config(&data_id, "DEFAULT_GROUP", "", &content)
        .await
        .unwrap();

    let got = tokio::time::timeout(Duration::from_secs(15), notify.notified())
        .await
        .is_ok();
    assert!(got, "Large config notification should be received");
    assert_eq!(received_len.load(Ordering::SeqCst), content.len() as u32);

    svc.remove_specific_listener(&data_id, "DEFAULT_GROUP", "", &listener)
        .await
        .unwrap();
    svc.remove_config(&data_id, "DEFAULT_GROUP", "").await.ok();
}

/// NCL-008: Listener in custom group (isolation)
#[tokio::test]
async fn test_listener_in_custom_group() {
    common::init_tracing();
    let svc = common::create_config_service().await.unwrap();
    let id = common::test_id();
    let data_id = format!("listener-group-{}", id);
    let content = "custom.group=value";

    let notify = Arc::new(Notify::new());
    let received = Arc::new(tokio::sync::Mutex::new(String::new()));

    let n = notify.clone();
    let r = received.clone();
    let listener: Arc<dyn ConfigChangeListener> = Arc::new(FnConfigChangeListener::new(move |info: ConfigResponse| {
        *r.blocking_lock() = info.content.clone();
        n.notify_one();
    }));

    svc.add_listener(&data_id, "CUSTOM_GROUP", "", listener.clone())
        .await
        .unwrap();
    sleep(Duration::from_millis(500)).await;

    svc.publish_config(&data_id, "CUSTOM_GROUP", "", content)
        .await
        .unwrap();

    let got = tokio::time::timeout(Duration::from_secs(10), notify.notified())
        .await
        .is_ok();
    assert!(got, "Custom group listener should fire");
    assert_eq!(*received.lock().await, content);

    // Verify NOT in DEFAULT_GROUP
    let default_config = svc.get_config(&data_id, "DEFAULT_GROUP", "").await;
    assert!(
        default_config.is_err(),
        "Config should NOT be in DEFAULT_GROUP"
    );

    svc.remove_specific_listener(&data_id, "CUSTOM_GROUP", "", &listener)
        .await
        .unwrap();
    svc.remove_config(&data_id, "CUSTOM_GROUP", "").await.ok();
}
