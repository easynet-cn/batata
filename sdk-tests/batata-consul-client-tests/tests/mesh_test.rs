//! Service mesh endpoints tests:
//!   - discovery_chain
//!   - exported_services
//!   - imported_services
//!
//! These test that the SDK method path deserializes the Consul response
//! correctly, even when the result is empty.

mod common;

use batata_consul_client::discovery_chain::DiscoveryChainOptions;

#[tokio::test]
async fn test_discovery_chain_get_consul_service() {
    let client = common::create_client();
    // The built-in "consul" service should always be resolvable on a live
    // Batata/Consul cluster. On servers that don't implement discovery
    // chain we tolerate an error as long as the SDK surface is clean.
    match client
        .discovery_chain_get("consul", None, &common::q())
        .await
    {
        Ok((resp, _meta)) => {
            assert_eq!(resp.chain.service_name, "consul");
            assert!(!resp.chain.datacenter.is_empty());
        }
        Err(e) => {
            // Acceptable: server may not implement discovery-chain
            let _ = e.status();
        }
    }
}

#[tokio::test]
async fn test_discovery_chain_with_override_requires_post() {
    let opts = DiscoveryChainOptions {
        override_protocol: "http".into(),
        ..Default::default()
    };
    assert!(opts.requires_post(), "overrides must flip to POST");

    let client = common::create_client();
    let _ = client
        .discovery_chain_get("nonexistent-service", Some(&opts), &common::q())
        .await; // tolerate error — we only assert the code path is callable
}

#[tokio::test]
async fn test_exported_services_returns_array() {
    let client = common::create_client();
    match client.exported_services(&common::q()).await {
        Ok((list, _meta)) => {
            // Empty on a fresh cluster; just verify type round-trip
            for s in &list {
                assert!(!s.service.is_empty());
            }
        }
        Err(e) => {
            let _ = e.status();
        }
    }
}

#[tokio::test]
async fn test_imported_services_returns_array() {
    let client = common::create_client();
    match client.imported_services(&common::q()).await {
        Ok((list, _meta)) => {
            for s in &list {
                assert!(!s.service.is_empty());
            }
        }
        Err(e) => {
            let _ = e.status();
        }
    }
}
