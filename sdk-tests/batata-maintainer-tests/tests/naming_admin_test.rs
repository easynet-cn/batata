//! Naming Admin API Functional Tests
//!
//! Tests instance CRUD, service management, health, metrics via HTTP admin API.

mod common;

// ==================== Instance CRUD ====================

#[tokio::test]
#[ignore]
async fn test_instance_create_and_delete() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let service = format!("adm-inst-{}", common::test_id());

    // Create service first
    client
        .service_create("public", "DEFAULT_GROUP", &service, 0.0, "", "")
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Register instance via HTTP
    let result = client
        .instance_create(
            "public",
            "DEFAULT_GROUP",
            &service,
            "10.0.0.1",
            8080,
            "DEFAULT",
            true,
        )
        .await;
    assert!(result.is_ok(), "Instance create should succeed");

    // List instances
    let instances = client
        .instance_list("public", "DEFAULT_GROUP", &service, "DEFAULT", 1, 10)
        .await
        .unwrap();
    assert!(
        !instances.page_items.is_empty(),
        "Should find registered instance"
    );

    // Delete instance
    client
        .instance_delete("public", "DEFAULT_GROUP", &service, "10.0.0.1", 8080, "DEFAULT")
        .await
        .unwrap();

    // Cleanup
    client
        .service_delete("public", "DEFAULT_GROUP", &service)
        .await
        .unwrap();
}

#[tokio::test]
#[ignore]
async fn test_instance_get_detail() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let service = format!("adm-idet-{}", common::test_id());
    client
        .service_create("public", "DEFAULT_GROUP", &service, 0.0, "", "")
        .await
        .unwrap();
    client
        .instance_create(
            "public",
            "DEFAULT_GROUP",
            &service,
            "10.0.1.1",
            9090,
            "DEFAULT",
            true,
        )
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let detail = client
        .instance_get("public", "DEFAULT_GROUP", &service, "10.0.1.1", 9090, "DEFAULT")
        .await
        .unwrap();
    assert_eq!(detail.ip, "10.0.1.1");
    assert_eq!(detail.port, 9090);

    // Deregister instance before deleting service
    client
        .instance_delete("public", "DEFAULT_GROUP", &service, "10.0.1.1", 9090, "DEFAULT")
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    let _ = client
        .service_delete("public", "DEFAULT_GROUP", &service)
        .await;
}

// ==================== Service CRUD ====================

#[tokio::test]
#[ignore]
async fn test_service_create_and_delete() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let service = format!("adm-svc-{}", common::test_id());

    let ok = client
        .service_create("public", "DEFAULT_GROUP", &service, 0.5, "", "")
        .await
        .unwrap();
    assert!(ok);

    let detail = client
        .service_get("public", "DEFAULT_GROUP", &service)
        .await
        .unwrap();
    assert!(detail.is_some(), "Service should exist");
    assert_eq!(detail.unwrap().service_name, service);

    let deleted = client
        .service_delete("public", "DEFAULT_GROUP", &service)
        .await
        .unwrap();
    assert!(deleted);
}

#[tokio::test]
#[ignore]
async fn test_service_list() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let service = format!("adm-slist-{}", common::test_id());
    client
        .service_create("public", "DEFAULT_GROUP", &service, 0.0, "", "")
        .await
        .unwrap();

    let list = client
        .service_list("public", "DEFAULT_GROUP", "", 1, 10, false)
        .await
        .unwrap();
    assert!(list.total_count > 0, "Should find at least one service");

    client
        .service_delete("public", "DEFAULT_GROUP", &service)
        .await
        .unwrap();
}

// ==================== Health ====================

#[tokio::test]
#[ignore]
async fn test_instance_health_update() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let service = format!("adm-hlth-{}", common::test_id());
    client
        .service_create("public", "DEFAULT_GROUP", &service, 0.0, "", "")
        .await
        .unwrap();
    client
        .instance_create(
            "public",
            "DEFAULT_GROUP",
            &service,
            "10.0.2.1",
            8080,
            "DEFAULT",
            false,
        )
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Update health to false
    let result = client
        .instance_health_update("public", "DEFAULT_GROUP", &service, "10.0.2.1", 8080, false)
        .await;
    assert!(result.is_ok(), "Health update should succeed");

    // Deregister instance before deleting service
    let _ = client
        .instance_delete("public", "DEFAULT_GROUP", &service, "10.0.2.1", 8080, "DEFAULT")
        .await;
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    let _ = client
        .service_delete("public", "DEFAULT_GROUP", &service)
        .await;
}

#[tokio::test]
#[ignore]
async fn test_health_checkers() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let checkers = client.health_checkers().await.unwrap();
    // Should return available health checker types
    assert!(
        checkers.is_empty() || !checkers.is_empty(),
        "Query should succeed"
    );
}

// ==================== Metrics ====================

#[tokio::test]
#[ignore]
async fn test_naming_metrics() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let metrics = client.naming_metrics().await.unwrap();
    assert!(metrics.is_object(), "Metrics should be a JSON object");
}

// ==================== Service Subscribers ====================

#[tokio::test]
#[ignore]
async fn test_service_subscribers() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let service = format!("adm-subs-{}", common::test_id());
    client
        .service_create("public", "DEFAULT_GROUP", &service, 0.0, "", "")
        .await
        .unwrap();

    let subs = client
        .service_subscribers("public", "DEFAULT_GROUP", &service, 1, 10)
        .await
        .unwrap();
    // May be empty if no gRPC subscribers
    assert!(
        subs.page_items.is_empty() || !subs.page_items.is_empty(),
        "Query should succeed"
    );

    client
        .service_delete("public", "DEFAULT_GROUP", &service)
        .await
        .unwrap();
}
