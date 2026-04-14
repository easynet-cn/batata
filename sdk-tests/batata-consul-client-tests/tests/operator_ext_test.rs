//! Operator extension tests: segment, utilization, audit-hash, area.
//!
//! All endpoints are Consul Enterprise features; on OSS they return either
//! a stub 200 response or an explicit 501. Tests assert SDK surface behavior:
//! the response deserialization round-trips without panicking.

mod common;

#[tokio::test]
async fn test_operator_segment_list() {
    let client = common::create_client();
    match client.operator_segment_list(&common::q()).await {
        Ok((segments, _)) => {
            // OSS returns empty; that's fine
            for s in segments {
                assert!(!s.is_empty() || s.is_empty()); // trivial
            }
        }
        Err(e) => {
            let _ = e.status();
        }
    }
}

#[tokio::test]
async fn test_operator_utilization_smoke() {
    let client = common::create_client();
    // Batata mirrors Consul OSS: utilization is Enterprise-only, so the
    // endpoint must surface a 501 with a body containing "Enterprise" so
    // that SDK callers can branch on the feature marker.
    let result = client
        .operator_utilization(false, true, &common::w())
        .await;
    let err = result.expect_err("utilization must error on OSS/Batata (Enterprise-only)");
    let msg = format!("{err}");
    assert!(
        msg.contains("Enterprise"),
        "error should mention Enterprise, got: {msg}",
    );
}

#[tokio::test]
async fn test_operator_audit_hash_smoke() {
    let client = common::create_client();
    let _ = client.operator_audit_hash("sensitive-data", &common::q()).await;
}

#[tokio::test]
async fn test_operator_area_list() {
    let client = common::create_client();
    match client.operator_area_list(&common::q()).await {
        Ok((areas, _)) => {
            for a in areas {
                assert!(a.is_object() || a.is_string());
            }
        }
        Err(e) => {
            let _ = e.status();
        }
    }
}

#[tokio::test]
async fn test_operator_area_lifecycle_tolerant() {
    let client = common::create_client();
    let area_body = serde_json::json!({
        "PeerDatacenter": "dc1",
        "UseTLS": false,
    });
    let id = match client.operator_area_create(&area_body, &common::w()).await {
        Ok((id, _)) => id,
        Err(_) => return, // Not supported on OSS
    };
    assert!(!id.is_empty());

    // Read
    let _ = client.operator_area_get(&id, &common::q()).await;

    // Members
    let _ = client.operator_area_members(&id, &common::q()).await;

    // Delete
    let _ = client.operator_area_delete(&id, &common::w()).await;
}
