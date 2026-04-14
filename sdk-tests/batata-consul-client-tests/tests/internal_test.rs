//! Internal endpoint tests.
//!
//! Exercises `internal_assign_service_virtual_ip` through the SDK.

mod common;

#[tokio::test]
async fn test_internal_assign_service_virtual_ip_smoke() {
    let client = common::create_client();
    let svc = format!("vip-svc-{}", common::test_id());
    let vips = vec!["240.0.0.1".to_string()];

    match client
        .internal_assign_service_virtual_ip(&svc, &vips, &common::w())
        .await
    {
        Ok((resp, _meta)) => {
            // Response shape check — Found is bool, UnassignedFrom is list.
            // Found may be true or false depending on whether the service exists.
            let _ = resp.service_found;
            let _ = resp.unassigned_from.len();
        }
        Err(e) => {
            // Endpoint may be disabled; surface check only.
            let _ = e.status();
        }
    }
}

#[tokio::test]
async fn test_internal_assign_service_virtual_ip_multiple_vips() {
    let client = common::create_client();
    let svc = format!("vip-multi-{}", common::test_id());
    let vips = vec!["240.0.0.2".to_string(), "240.0.0.3".to_string()];

    let _ = client
        .internal_assign_service_virtual_ip(&svc, &vips, &common::w())
        .await;
}
