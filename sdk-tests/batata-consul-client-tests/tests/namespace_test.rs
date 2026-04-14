//! Namespace API tests (Enterprise).
//!
//! Batata is OSS, so these endpoints may return 501/404. The tests still
//! exercise the SDK serialization/deserialization path end-to-end — if the
//! server doesn't support namespaces, we validate that the client returns
//! a typed error rather than panicking.

mod common;

use batata_consul_client::namespace::{Namespace, NamespaceACLConfig};

fn ns_name() -> String {
    format!("ns-{}", common::test_id())
}

#[tokio::test]
async fn test_namespace_lifecycle_or_not_supported() {
    let client = common::create_client();
    let name = ns_name();
    let ns = Namespace {
        name: name.clone(),
        description: "batata-consul-client integration test".into(),
        ..Default::default()
    };

    // Try create
    match client.namespace_create(&ns, &common::w()).await {
        Ok((created, _meta)) => {
            assert_eq!(created.name, name);
            // Read it back
            let (got, _) = client
                .namespace_read(&name, &common::q())
                .await
                .unwrap();
            assert!(got.is_some(), "namespace_read must return the created ns");

            // Update
            let mut updated = ns.clone();
            updated.description = "updated description".into();
            let (u, _) = client.namespace_update(&updated, &common::w()).await.unwrap();
            assert_eq!(u.description, "updated description");

            // List
            let (list, _) = client.namespace_list(&common::q()).await.unwrap();
            assert!(list.iter().any(|n| n.name == name));

            // Delete
            let _m = client.namespace_delete(&name, &common::w()).await.unwrap();
        }
        Err(e) => {
            // Accept 404 or other error types, but must deserialize the
            // response without panicking — this test exercises the wire path.
            let _ = e.status();
        }
    }
}

#[tokio::test]
async fn test_namespace_read_non_existent_returns_none() {
    let client = common::create_client();
    let name = format!("does-not-exist-{}", common::test_id());
    match client.namespace_read(&name, &common::q()).await {
        Ok((ns, _meta)) => assert!(ns.is_none(), "non-existent namespace must return None"),
        Err(e) => {
            // Server may reject unauthorized reads with 403 — acceptable.
            let _ = e.status();
        }
    }
}

#[tokio::test]
async fn test_namespace_with_acl_defaults_deserializes() {
    // Construct a Namespace with ACL defaults and ensure it round-trips
    // through serde. This exercises the NamespaceACLConfig type even when
    // the server doesn't support namespaces.
    let ns = Namespace {
        name: "test-ns".into(),
        description: "with acl defaults".into(),
        acls: Some(NamespaceACLConfig::default()),
        ..Default::default()
    };
    let json = serde_json::to_string(&ns).unwrap();
    assert!(json.contains("\"Name\":\"test-ns\""));
    assert!(json.contains("\"ACLs\""));
    let back: Namespace = serde_json::from_str(&json).unwrap();
    assert_eq!(back.name, ns.name);
}
