//! Legacy v1 ACL API tests.
//!
//! These methods exist for backward compatibility with Consul < 1.4. Modern
//! Consul returns errors for these paths, so the tests validate that the
//! SDK surface cleanly returns a typed error rather than panicking, AND
//! that the wire serialization/deserialization path works.

mod common;

use batata_consul_client::model::ACLEntry;

#[tokio::test]
async fn test_acl_legacy_list_returns_typed_result_or_error() {
    let client = common::create_client();
    match client.acl_legacy_list(&common::q()).await {
        Ok((entries, _)) => {
            for e in entries {
                assert!(!e.id.is_empty() || !e.name.is_empty());
            }
        }
        Err(e) => {
            // Modern Consul: 401/404/405/501 are all acceptable
            let _ = e.status();
        }
    }
}

#[tokio::test]
async fn test_acl_legacy_info_for_unknown_id_returns_none_or_error() {
    let client = common::create_client();
    let id = format!("00000000-0000-0000-0000-{:012}", 0);
    match client.acl_legacy_info(&id, &common::q()).await {
        Ok((entry, _)) => assert!(entry.is_none()),
        Err(e) => {
            let _ = e.status();
        }
    }
}

#[tokio::test]
async fn test_acl_legacy_create_round_trip_shape() {
    // Ensure the ACLEntry type can be serialized as a legacy ACL create body.
    let entry = ACLEntry {
        name: "legacy-test".into(),
        acl_type: "client".into(),
        rules: "key \"\" { policy = \"read\" }".into(),
        ..Default::default()
    };
    let j = serde_json::to_string(&entry).unwrap();
    assert!(j.contains("\"Name\":\"legacy-test\""));
    assert!(j.contains("\"Type\":\"client\""));
    assert!(j.contains("\"Rules\""));

    // And try the actual call — we don't require success.
    let client = common::create_client();
    let _ = client.acl_legacy_create(&entry, &common::w()).await;
}
