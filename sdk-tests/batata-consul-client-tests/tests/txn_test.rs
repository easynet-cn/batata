//! Tests for the transaction API (batata-consul-client).
//!
//! Covers KV verbs plus the round-5 additions for Node/Service/Check
//! transaction operations. All calls go through `ConsulClient::txn`
//! (no raw HTTP).

mod common;

use base64::Engine;
use batata_consul_client::model::{
    TxnCheck, TxnKVOp, TxnNode, TxnOp, TxnService,
};

fn b64(s: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(s.as_bytes())
}

#[tokio::test]
async fn test_txn_kv_set_and_get() {
    let client = common::create_client();
    let key = format!("test/txn/kv/{}", common::test_id());
    let ops = vec![
        TxnOp::kv_set(&key, b64("txn-value")),
        TxnOp::kv_get(&key),
    ];
    let (ok, resp, _meta) = client.txn(&ops, &common::w()).await.unwrap();
    assert!(ok, "txn should succeed, errors={:?}", resp.errors);
    // First result is from set (may or may not include KV), second is from get.
    assert!(
        !resp.results.is_empty(),
        "results should not be empty after kv set+get"
    );
    let retrieved = resp.results.iter().find_map(|r| r.kv.clone()).expect("kv result");
    assert_eq!(retrieved.key, key);
    // Cleanup
    let _ = client.kv_delete(&key, &common::w()).await;
}

#[tokio::test]
async fn test_txn_kv_cas_fails_on_stale_index() {
    let client = common::create_client();
    let key = format!("test/txn/cas/{}", common::test_id());
    // Seed a value
    let (_ok, _resp, _meta) = client
        .txn(&[TxnOp::kv_set(&key, b64("v1"))], &common::w())
        .await
        .unwrap();
    // Now attempt CAS with index=0 (which requires non-existence → should fail
    // because the key now exists).
    let (ok, resp, _) = client
        .txn(
            &[TxnOp {
                kv: Some(TxnKVOp {
                    verb: "cas".into(),
                    key: key.clone(),
                    value: Some(b64("v2")),
                    flags: 0,
                    index: 0,
                    session: None,
                }),
                ..Default::default()
            }],
            &common::w(),
        )
        .await
        .unwrap();
    assert!(!ok, "stale CAS should fail; errors={:?}", resp.errors);
    assert!(!resp.errors.is_empty(), "errors should be populated");
    let _ = client.kv_delete(&key, &common::w()).await;
}

#[tokio::test]
async fn test_txn_kv_delete_tree() {
    let client = common::create_client();
    let prefix = format!("test/txn/tree/{}", common::test_id());
    let ops = vec![
        TxnOp::kv_set(format!("{}/a", prefix), b64("A")),
        TxnOp::kv_set(format!("{}/b", prefix), b64("B")),
        TxnOp::kv_set(format!("{}/c", prefix), b64("C")),
    ];
    let (ok, _, _) = client.txn(&ops, &common::w()).await.unwrap();
    assert!(ok);

    // Delete-tree in a transaction
    let del_op = TxnOp {
        kv: Some(TxnKVOp {
            verb: "delete-tree".into(),
            key: prefix.clone(),
            value: None,
            flags: 0,
            index: 0,
            session: None,
        }),
        ..Default::default()
    };
    let (ok, _, _) = client.txn(&[del_op], &common::w()).await.unwrap();
    assert!(ok, "delete-tree should succeed");

    // Verify all gone
    let (pairs, _) = client.kv_list(&prefix, &common::q()).await.unwrap();
    assert!(pairs.is_empty(), "no keys should remain under prefix");
}

#[tokio::test]
async fn test_txn_node_verb_accepted() {
    // Consul supports Node verbs even on OSS. Batata's plugin accepts them
    // as pass-through. Verify the op round-trips through the wire format.
    let client = common::create_client();
    let node = TxnNode {
        node: format!("test-node-{}", common::test_id()),
        address: "127.0.0.1".into(),
        ..Default::default()
    };
    let op = TxnOp::node_set(node.clone());
    let (_ok, resp, _meta) = client.txn(&[op], &common::w()).await.unwrap();
    // Either the server successfully echoed it, or it returned an error;
    // either way the response deserialization must succeed (wire compat).
    assert!(resp.results.len() + resp.errors.len() >= 1);
}

#[tokio::test]
async fn test_txn_service_verb_accepted() {
    let client = common::create_client();
    let svc = TxnService {
        id: format!("svc-{}", common::test_id()),
        service: "compat-service".into(),
        address: "127.0.0.1".into(),
        port: 8080,
        ..Default::default()
    };
    let op = TxnOp::service_set("test-node", svc);
    let (_ok, resp, _meta) = client.txn(&[op], &common::w()).await.unwrap();
    assert!(resp.results.len() + resp.errors.len() >= 1);
}

#[tokio::test]
async fn test_txn_check_verb_accepted() {
    let client = common::create_client();
    let check = TxnCheck {
        node: "test-node".into(),
        check_id: format!("chk-{}", common::test_id()),
        name: "compat check".into(),
        status: "passing".into(),
        ..Default::default()
    };
    let op = TxnOp::check_set(check);
    let (_ok, resp, _meta) = client.txn(&[op], &common::w()).await.unwrap();
    assert!(resp.results.len() + resp.errors.len() >= 1);
}

#[tokio::test]
async fn test_txn_mixed_kv_node_service() {
    // A single transaction with one op of each high-level kind exercises
    // the full TxnOp wire format end-to-end.
    let client = common::create_client();
    let key = format!("test/txn/mixed/{}", common::test_id());
    let ops = vec![
        TxnOp::kv_set(&key, b64("mixed")),
        TxnOp::node_set(TxnNode {
            node: format!("mixed-node-{}", common::test_id()),
            address: "127.0.0.1".into(),
            ..Default::default()
        }),
        TxnOp::service_set(
            "mixed-node",
            TxnService {
                service: "mixed-svc".into(),
                port: 9999,
                ..Default::default()
            },
        ),
    ];
    let (_ok, resp, _meta) = client.txn(&ops, &common::w()).await.unwrap();
    assert!(!resp.results.is_empty() || !resp.errors.is_empty());
    let _ = client.kv_delete(&key, &common::w()).await;
}

#[tokio::test]
async fn test_txn_max_ops_limit() {
    // Builder enforces maxTxnOps=128 locally before sending.
    let client = common::create_client();
    let mut ops = Vec::with_capacity(200);
    for i in 0..200 {
        ops.push(TxnOp::kv_get(format!("does-not-exist/{}", i)));
    }
    let err = client.txn(&ops, &common::w()).await.unwrap_err();
    let msg = format!("{}", err);
    assert!(
        msg.contains("too many operations") || msg.contains("max"),
        "expected limit error, got: {}",
        msg
    );
}
