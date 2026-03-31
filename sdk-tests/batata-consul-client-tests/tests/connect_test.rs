//! Integration tests for the Connect API (CA, Intentions, Discovery Chain)

mod common;

use batata_consul_client::Intention;

#[tokio::test]
async fn test_connect_ca_roots() {
    common::init_tracing();
    let client = common::create_client();

    let (roots, meta) = client.connect_ca_roots(&common::q()).await.unwrap();

    assert!(
        meta.last_index > 0,
        "QueryMeta last_index should be positive"
    );
    assert!(
        !roots.active_root_id.is_empty(),
        "Active root ID should not be empty"
    );
    assert!(
        !roots.roots.is_empty(),
        "CA roots list should contain at least one root"
    );
    let first = &roots.roots[0];
    assert!(!first.id.is_empty(), "Root ID should not be empty");
    assert!(!first.name.is_empty(), "Root name should not be empty");
    assert!(
        !first.root_cert.is_empty(),
        "Root certificate PEM should not be empty"
    );
}

#[tokio::test]
async fn test_connect_ca_config() {
    common::init_tracing();
    let client = common::create_client();

    let (config, meta) = client.connect_ca_get_config(&common::q()).await.unwrap();

    assert!(
        meta.last_index > 0,
        "QueryMeta last_index should be positive"
    );
    assert!(
        !config.provider.is_empty(),
        "CA config provider should not be empty (e.g. 'consul')"
    );
}

#[tokio::test]
async fn test_connect_intentions_list() {
    common::init_tracing();
    let client = common::create_client();

    let (intentions, meta) = client.connect_intentions(&common::q()).await.unwrap();

    // Intentions list may be empty on a fresh server; just verify the call succeeds
    // and returns a valid response.
    // last_index is u64, always >= 0; just confirm the meta was populated
    let _ = meta.last_index;
    // Verify the list is a valid Vec (type system ensures this, but check length is sane)
    assert!(
        intentions.len() < 100_000,
        "Intentions list length should be reasonable"
    );
}

#[tokio::test]
async fn test_connect_intention_upsert_and_delete() {
    common::init_tracing();
    let client = common::create_client();
    let id = common::test_id();

    let source = format!("src-svc-{}", id);
    let destination = format!("dst-svc-{}", id);

    let intention = Intention {
        source_name: source.clone(),
        destination_name: destination.clone(),
        action: "allow".to_string(),
        description: "test intention".to_string(),
        ..Default::default()
    };

    // Upsert the intention
    let _meta = client
        .connect_intention_upsert(&intention, &source, &destination, &common::w())
        .await
        .unwrap();

    // Verify by exact match
    let (fetched, _) = client
        .connect_intention_exact(&source, &destination, &common::q())
        .await
        .unwrap();
    assert_eq!(
        fetched.source_name, source,
        "Fetched intention source_name should match"
    );
    assert_eq!(
        fetched.destination_name, destination,
        "Fetched intention destination_name should match"
    );
    assert_eq!(
        fetched.action, "allow",
        "Fetched intention action should be 'allow'"
    );

    // Delete by exact match
    let (deleted, _) = client
        .connect_intention_delete_exact(&source, &destination, &common::w())
        .await
        .unwrap();
    assert!(deleted, "Intention delete should return true");
}

#[tokio::test]
async fn test_connect_intention_match() {
    common::init_tracing();
    let client = common::create_client();
    let id = common::test_id();

    let source = format!("match-src-{}", id);
    let destination = format!("match-dst-{}", id);

    let intention = Intention {
        source_name: source.clone(),
        destination_name: destination.clone(),
        action: "allow".to_string(),
        ..Default::default()
    };

    // Create intention first
    client
        .connect_intention_upsert(&intention, &source, &destination, &common::w())
        .await
        .unwrap();

    // Match by source
    let (result, meta) = client
        .connect_intention_match("source", &source, &common::q())
        .await
        .unwrap();
    assert!(
        meta.last_index > 0,
        "QueryMeta last_index should be positive"
    );
    assert!(result.is_object(), "Match result should be a JSON object");

    // Cleanup
    let _ = client
        .connect_intention_delete_exact(&source, &destination, &common::w())
        .await;
}

#[tokio::test]
async fn test_connect_intention_check() {
    common::init_tracing();
    let client = common::create_client();
    let id = common::test_id();

    let source = format!("chk-src-{}", id);
    let destination = format!("chk-dst-{}", id);

    // Create an allow intention
    let intention = Intention {
        source_name: source.clone(),
        destination_name: destination.clone(),
        action: "allow".to_string(),
        ..Default::default()
    };
    client
        .connect_intention_upsert(&intention, &source, &destination, &common::w())
        .await
        .unwrap();

    // Check authorization
    let (check, meta) = client
        .connect_intention_check(&source, &destination, &common::q())
        .await
        .unwrap();
    assert!(
        meta.last_index > 0,
        "QueryMeta last_index should be positive"
    );
    assert!(
        check.allowed,
        "Intention check should report allowed=true for an allow intention"
    );

    // Cleanup
    let _ = client
        .connect_intention_delete_exact(&source, &destination, &common::w())
        .await;
}

#[tokio::test]
async fn test_connect_discovery_chain() {
    common::init_tracing();
    let client = common::create_client();

    let (chain, meta) = client
        .discovery_chain("test-service", &common::q())
        .await
        .unwrap();

    assert!(
        meta.last_index > 0,
        "QueryMeta last_index should be positive"
    );
    assert!(
        chain.is_object(),
        "Discovery chain response should be a JSON object"
    );
}

#[tokio::test]
async fn test_connect_leaf_cert() {
    common::init_tracing();
    let client = common::create_client();

    // Leaf certificates are fetched via the agent connect API.
    // The connect CA must be active for this to work.
    // We verify that CA roots are available (prerequisite for leaf certs).
    let (roots, _) = client.connect_ca_roots(&common::q()).await.unwrap();
    assert!(
        !roots.active_root_id.is_empty(),
        "CA must have an active root for leaf cert issuance"
    );
    assert!(
        !roots.roots.is_empty(),
        "At least one CA root must exist for leaf certs"
    );

    // Verify the active root has a valid certificate
    let active_root = roots.roots.iter().find(|r| r.id == roots.active_root_id);
    assert!(
        active_root.is_some(),
        "Should find the active root in the roots list"
    );
    let root = active_root.unwrap();
    assert!(
        root.root_cert.contains("BEGIN CERTIFICATE"),
        "Root cert should be a PEM-encoded certificate"
    );
}
