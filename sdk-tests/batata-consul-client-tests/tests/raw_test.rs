//! Raw HTTP escape-hatch tests.
//!
//! Use the SDK's `raw_query` / `raw_write` / `raw_delete` against well-known
//! endpoints that the typed wrappers already cover, then assert equivalent
//! data is returned.

mod common;

use serde_json::Value;

#[tokio::test]
async fn test_raw_query_status_leader() {
    let client = common::create_client();
    // /v1/status/leader returns a JSON-encoded string like "ip:port"
    let (leader, _meta): (Value, _) = client
        .raw_query("/v1/status/leader", &common::q())
        .await
        .unwrap();
    assert!(leader.is_string(), "leader must be a JSON string");
}

#[tokio::test]
async fn test_raw_query_matches_typed_catalog() {
    let client = common::create_client();
    // Typed path
    let (typed, _) = client.catalog_services(&common::q()).await.unwrap();
    // Raw path on the same endpoint
    let (raw, _): (Value, _) = client
        .raw_query("/v1/catalog/services", &common::q())
        .await
        .unwrap();
    // Both should list at least the default "consul" service
    assert!(raw.is_object(), "raw response must be a JSON object");
    let raw_map = raw.as_object().unwrap();
    assert_eq!(
        typed.len(),
        raw_map.len(),
        "typed and raw responses must have the same entry count"
    );
}

#[tokio::test]
async fn test_raw_write_and_delete_kv() {
    let client = common::create_client();
    let key = format!("test/raw/{}", common::test_id());
    let path = format!("/v1/kv/{}", key);

    // Write a value. PUT with body "hello" — response is `true`/`false` boolean.
    let (ok, _meta): (bool, _) = client
        .raw_write(&path, Some(&"hello"), &common::w())
        .await
        .unwrap();
    assert!(ok, "raw_write to KV must succeed");

    // Read back via typed SDK to verify
    let (pair, _) = client.kv_get(&key, &common::q()).await.unwrap();
    assert!(pair.is_some(), "key should exist after raw_write");

    // Delete via raw
    let _meta = client.raw_delete(&path, &common::w()).await.unwrap();

    let (pair, _) = client.kv_get(&key, &common::q()).await.unwrap();
    assert!(pair.is_none(), "key should be gone after raw_delete");
}
