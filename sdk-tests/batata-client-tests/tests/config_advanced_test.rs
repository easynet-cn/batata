use md5::Digest;
// Advanced Config Service Functional Tests
//
// Tests for config types, large content, concurrency, special characters,
// empty content, multi-tenant, multiple listeners, and CAS with type.
// Requires a running Batata server.

mod common;

use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};

use batata_client::config::listener::{ConfigChangeListener, ConfigResponse};

const TENANT: &str = "";
const GROUP: &str = "DEFAULT_GROUP";

// ==================== Config Types ====================

#[tokio::test]
async fn test_publish_config_with_all_types() {
    common::init_tracing();
    let svc = common::create_config_service().await.unwrap();
    let id = common::test_id();

    let type_cases: Vec<(&str, String, &str)> = vec![
        ("yaml", format!("cfg-type-yaml-{id}.yaml"), "server:\n  port: 8080\n  name: test"),
        ("json", format!("cfg-type-json-{id}.json"), r#"{"server":{"port":8080,"name":"test"}}"#),
        ("xml", format!("cfg-type-xml-{id}.xml"), "<server><port>8080</port><name>test</name></server>"),
        (
            "properties",
            format!("cfg-type-props-{id}.properties"),
            "server.port=8080\nserver.name=test",
        ),
        ("text", format!("cfg-type-text-{id}.txt"), "plain text content here"),
        ("html", format!("cfg-type-html-{id}.html"), "<html><body><h1>Test</h1></body></html>"),
        ("toml", format!("cfg-type-toml-{id}.toml"), "[server]\nport = 8080\nname = \"test\""),
    ];

    for (config_type, ref data_id, content) in &type_cases {
        let ok = svc
            .publish_config_with_type(data_id, GROUP, TENANT, content, config_type)
            .await
            .unwrap();
        assert!(ok, "publish_config_with_type should succeed for type={config_type}");

        let result = svc.get_config(data_id, GROUP, TENANT).await.unwrap();
        assert_eq!(
            result, *content,
            "retrieved config content should match for type={config_type}"
        );
    }

    // Cleanup
    for (_, ref data_id, _) in &type_cases {
        svc.remove_config(data_id, GROUP, TENANT).await.unwrap();
    }
    svc.shutdown().await;
}

// ==================== Large Content ====================

#[tokio::test]
async fn test_config_large_content() {
    common::init_tracing();
    let svc = common::create_config_service().await.unwrap();

    let data_id = format!("cfg-large-{}", common::test_id());
    // Generate 100KB content
    let large_content: String = "abcdefghij".repeat(10_240); // 10 chars * 10240 = 102400 bytes
    assert_eq!(large_content.len(), 102_400, "content should be 100KB");

    let ok = svc
        .publish_config(&data_id, GROUP, TENANT, &large_content)
        .await
        .unwrap();
    assert!(ok, "publish of 100KB config should succeed");

    let result = svc.get_config(&data_id, GROUP, TENANT).await.unwrap();
    assert_eq!(result.len(), large_content.len(), "retrieved content length should match 100KB");
    assert_eq!(result, large_content, "retrieved content should match exactly");

    svc.remove_config(&data_id, GROUP, TENANT).await.unwrap();
    svc.shutdown().await;
}

// ==================== Concurrent Publish ====================

#[tokio::test]
async fn test_config_concurrent_publish() {
    common::init_tracing();
    let svc = common::create_config_service().await.unwrap();
    let id = common::test_id();

    let mut handles = Vec::new();
    let data_ids: Vec<String> = (0..5).map(|i| format!("cfg-conc-{id}-{i}")).collect();

    for (i, data_id) in data_ids.iter().enumerate() {
        let svc_clone = svc.clone();
        let did = data_id.clone();
        let content = format!("concurrent-value-{i}");
        handles.push(tokio::spawn(async move {
            svc_clone
                .publish_config(&did, GROUP, TENANT, &content)
                .await
        }));
    }

    // Wait for all publishes to complete
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok(), "concurrent publish should not error");
        assert!(result.unwrap(), "concurrent publish should return true");
    }

    // Verify all values are correct
    for (i, data_id) in data_ids.iter().enumerate() {
        let result = svc.get_config(data_id, GROUP, TENANT).await.unwrap();
        assert_eq!(
            result,
            format!("concurrent-value-{i}"),
            "config {data_id} should have correct value after concurrent publish"
        );
    }

    // Cleanup
    for data_id in &data_ids {
        svc.remove_config(data_id, GROUP, TENANT).await.unwrap();
    }
    svc.shutdown().await;
}

// ==================== Special Characters ====================

#[tokio::test]
async fn test_config_special_characters() {
    common::init_tracing();
    let svc = common::create_config_service().await.unwrap();

    let data_id = format!("cfg-special-{}", common::test_id());
    let content = "unicode: \u{1F600}\u{1F680}\u{2764}\nnewline1\nnewline2\rtab\there\nquotes: \"double\" 'single'\nbackslash: C:\\path\\to\\file\nnull-like: \\0\nCJK: \u{4F60}\u{597D}\u{4E16}\u{754C}";

    let ok = svc
        .publish_config(&data_id, GROUP, TENANT, content)
        .await
        .unwrap();
    assert!(ok, "publish with special characters should succeed");

    let result = svc.get_config(&data_id, GROUP, TENANT).await.unwrap();
    assert_eq!(result, content, "retrieved content should preserve special characters exactly");
    assert!(
        result.contains('\u{1F600}'),
        "content should contain emoji (grinning face)"
    );
    assert!(
        result.contains("\u{4F60}\u{597D}"),
        "content should contain CJK characters"
    );
    assert!(result.contains('\n'), "content should contain newlines");

    svc.remove_config(&data_id, GROUP, TENANT).await.unwrap();
    svc.shutdown().await;
}

// ==================== Empty Content ====================

#[tokio::test]
async fn test_config_empty_content() {
    common::init_tracing();
    let svc = common::create_config_service().await.unwrap();

    let data_id = format!("cfg-empty-{}", common::test_id());

    // Publish non-empty first, then overwrite with empty
    let ok = svc
        .publish_config(&data_id, GROUP, TENANT, "initial-value")
        .await
        .unwrap();
    assert!(ok, "initial publish should succeed");

    // Empty content should be rejected (consistent with Nacos behavior)
    let result = svc.publish_config(&data_id, GROUP, TENANT, "").await;
    assert!(result.is_err(), "publish with empty content should fail");

    // Original content should still be present
    let content = svc.get_config(&data_id, GROUP, TENANT).await.unwrap();
    assert_eq!(content, "initial-value", "original content should be unchanged");

    svc.remove_config(&data_id, GROUP, TENANT).await.unwrap();
    svc.shutdown().await;
}

// ==================== Multi-Tenant ====================

#[tokio::test]
async fn test_config_multi_tenant() {
    common::init_tracing();
    let svc = common::create_config_service().await.unwrap();

    let data_id = format!("cfg-tenant-{}", common::test_id());
    let tenant_public = "";
    let tenant_custom = "custom_ns";

    // Publish to public namespace
    let ok = svc
        .publish_config(&data_id, GROUP, tenant_public, "public-value")
        .await
        .unwrap();
    assert!(ok, "publish to public namespace should succeed");

    // Publish to custom namespace with same data_id
    let ok = svc
        .publish_config(&data_id, GROUP, tenant_custom, "custom-value")
        .await
        .unwrap();
    assert!(ok, "publish to custom namespace should succeed");

    // Read from public namespace
    let result_public = svc.get_config(&data_id, GROUP, tenant_public).await.unwrap();
    assert_eq!(
        result_public, "public-value",
        "public namespace should have its own value"
    );

    // Read from custom namespace
    let result_custom = svc.get_config(&data_id, GROUP, tenant_custom).await.unwrap();
    assert_eq!(
        result_custom, "custom-value",
        "custom namespace should have its own value"
    );

    // Verify they are isolated: values are different
    assert_ne!(
        result_public, result_custom,
        "configs in different namespaces should be independent"
    );

    // Cleanup both
    svc.remove_config(&data_id, GROUP, tenant_public).await.unwrap();
    svc.remove_config(&data_id, GROUP, tenant_custom).await.unwrap();
    svc.shutdown().await;
}

// ==================== Multiple Listeners ====================

#[tokio::test]
async fn test_config_listener_multiple() {
    common::init_tracing();
    let svc = common::create_config_service().await.unwrap();

    let data_id = format!("cfg-multi-listen-{}", common::test_id());
    let counter1 = Arc::new(AtomicU32::new(0));
    let counter2 = Arc::new(AtomicU32::new(0));
    let received_content1 = Arc::new(std::sync::Mutex::new(String::new()));
    let received_content2 = Arc::new(std::sync::Mutex::new(String::new()));

    struct TrackingListener {
        counter: Arc<AtomicU32>,
        content: Arc<std::sync::Mutex<String>>,
    }

    impl ConfigChangeListener for TrackingListener {
        fn receive_config_info(&self, info: ConfigResponse) {
            self.counter.fetch_add(1, Ordering::SeqCst);
            if let Ok(mut c) = self.content.lock() {
                *c = info.content.clone();
            }
        }
    }

    // Publish initial config
    svc.publish_config(&data_id, GROUP, TENANT, "initial")
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Add first listener via get_config_and_sign_listener
    let content = svc
        .get_config_and_sign_listener(
            &data_id,
            GROUP,
            TENANT,
            Arc::new(TrackingListener {
                counter: counter1.clone(),
                content: received_content1.clone(),
            }),
        )
        .await
        .unwrap();
    assert_eq!(content, "initial", "get_config_and_sign_listener should return current value");

    // Add second listener (replaces the first one on the same key)
    svc.add_listener(
        &data_id,
        GROUP,
        TENANT,
        Arc::new(TrackingListener {
            counter: counter2.clone(),
            content: received_content2.clone(),
        }),
    )
    .await
    .unwrap();

    // Update config to trigger listeners
    svc.publish_config(&data_id, GROUP, TENANT, "updated-multi")
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // At least one listener should have been notified
    let total = counter1.load(Ordering::SeqCst) + counter2.load(Ordering::SeqCst);
    assert!(
        total >= 1,
        "at least one listener should have been notified, got counter1={}, counter2={}",
        counter1.load(Ordering::SeqCst),
        counter2.load(Ordering::SeqCst)
    );

    svc.remove_listener(&data_id, GROUP, TENANT).await.unwrap();
    svc.remove_config(&data_id, GROUP, TENANT).await.unwrap();
    svc.shutdown().await;
}

// ==================== CAS With Type ====================

#[tokio::test]
async fn test_config_cas_with_type() {
    common::init_tracing();
    let svc = common::create_config_service().await.unwrap();

    let data_id = format!("cfg-cas-type-{}.json", common::test_id());
    let initial_content = r#"{"version":1}"#;
    let updated_content = r#"{"version":2}"#;

    // Initial publish with type
    let ok = svc
        .publish_config_with_type(&data_id, GROUP, TENANT, initial_content, "json")
        .await
        .unwrap();
    assert!(ok, "initial publish with type should succeed");

    // Get content and compute MD5
    let content = svc.get_config(&data_id, GROUP, TENANT).await.unwrap();
    assert_eq!(content, initial_content, "initial content should match");
    let md5 = const_hex::encode(md5::Md5::digest(content.as_bytes()));

    // CAS update with correct MD5 and type
    let ok = svc
        .publish_config_cas_with_type(&data_id, GROUP, TENANT, updated_content, &md5, "json")
        .await
        .unwrap();
    assert!(ok, "CAS with correct MD5 and type should succeed");

    // Verify updated content
    let result = svc.get_config(&data_id, GROUP, TENANT).await.unwrap();
    assert_eq!(result, updated_content, "content should be updated after CAS");

    // CAS with wrong MD5 should fail
    let result = svc
        .publish_config_cas_with_type(&data_id, GROUP, TENANT, r#"{"version":3}"#, "wrong_md5", "json")
        .await;
    assert!(result.is_err(), "CAS with wrong MD5 should fail");

    // Content should still be version 2
    let result = svc.get_config(&data_id, GROUP, TENANT).await.unwrap();
    assert_eq!(
        result, updated_content,
        "content should remain unchanged after failed CAS"
    );

    svc.remove_config(&data_id, GROUP, TENANT).await.unwrap();
    svc.shutdown().await;
}
