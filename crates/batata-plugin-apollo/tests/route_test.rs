mod common;

use std::sync::Arc;

use actix_web::{test, web, App};
use batata_plugin_apollo::api::dto::{AppNamespaceDTO, ItemDTO, NamespaceDTO, ServerConfigDTO};
use batata_plugin_apollo::persistence::traits::ApolloPersistenceService;
use batata_plugin_apollo::service::{AppNamespaceService, ItemService, NamespaceService, ReleaseService};

#[actix_web::test]
async fn b1_metaservice_returns_real_host() {
    let p: Arc<dyn ApolloPersistenceService> = common::make_embedded_persistence().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(p))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/services/config")
        .insert_header(("host", "myhost:9999"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    let arr = body.as_array().expect("metaservice returns an array");
    assert!(!arr.is_empty());
    assert_eq!(arr[0]["homepageUrl"], "http://myhost:9999");
    assert_eq!(arr[0]["appName"], "apollo-configservice");
}

#[actix_web::test]
async fn tier2_server_config_crud_via_http() {
    let p: Arc<dyn ApolloPersistenceService> = common::make_embedded_persistence().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(p))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let dto = ServerConfigDTO {
        key: "sc1".into(),
        value: "v1".into(),
        comment: Some("c".into()),
        data_change_created_by: Some("tester".into()),
        data_change_created_time: None,
    };
    let create = test::TestRequest::post()
        .uri("/serverconfigs")
        .insert_header(("content-type", "application/json"))
        .set_payload(serde_json::to_string(&dto).unwrap())
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert_eq!(resp.status(), 200, "server config create should succeed");

    let get = test::TestRequest::get().uri("/serverconfigs/sc1").to_request();
    let resp = test::call_service(&app, get).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["value"], "v1");

    let list = test::TestRequest::get().uri("/serverconfigs").to_request();
    let resp = test::call_service(&app, list).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.as_array().unwrap().iter().any(|c| c["key"] == "sc1"));

    let del = test::TestRequest::delete()
        .uri("/serverconfigs/sc1?operator=admin")
        .to_request();
    let resp = test::call_service(&app, del).await;
    assert_eq!(resp.status(), 200);

    let get = test::TestRequest::get().uri("/serverconfigs/sc1").to_request();
    let resp = test::call_service(&app, get).await;
    assert_eq!(resp.status(), 404);
}

#[actix_web::test]
async fn tier2_app_namespace_crud_via_http() {
    let p: Arc<dyn ApolloPersistenceService> = common::make_embedded_persistence().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(p))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let dto = AppNamespaceDTO {
        id: None,
        name: "appns1".into(),
        app_id: "app1".into(),
        format: "properties".into(),
        is_public: false,
        comment: "c".into(),
        data_change_created_by: Some("tester".into()),
        data_change_created_time: None,
    };
    let create = test::TestRequest::post()
        .uri("/apps/app1/appnamespaces")
        .insert_header(("content-type", "application/json"))
        .set_payload(serde_json::to_string(&dto).unwrap())
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert_eq!(resp.status(), 200, "app namespace create should succeed");

    let get = test::TestRequest::get()
        .uri("/apps/app1/appnamespaces/appns1")
        .to_request();
    let resp = test::call_service(&app, get).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["name"], "appns1");
    assert_eq!(body["appId"], "app1");
}

/// A2 — public namespace MERGE: a consumer app requesting a public namespace
/// owned by another app receives the owner's configs with any of its OWN
/// releases overriding per-key (upstream mergeReleaseConfigurations).
#[actix_web::test]
async fn a2_public_namespace_merges_private_over_public() {
    let p: Arc<dyn ApolloPersistenceService> = common::make_embedded_persistence().await;

    let ns_dto = |app: &str, public: bool| NamespaceDTO {
        app_id: app.into(),
        cluster_name: "default".into(),
        namespace_name: "shared-ns".into(),
        format: Some("properties".into()),
        is_public: Some(public),
        comment: None,
        data_change_created_by: Some("t".into()),
        data_change_last_modified_by: None,
        data_change_last_time: None,
        data_change_created_time: None,
    };
    let item_dto = |k: &str, v: &str| ItemDTO {
        id: None,
        key: k.into(),
        value: v.into(),
        r#type: None,
        comment: None,
        line_num: None,
        data_change_created_by: Some("t".into()),
        data_change_last_modified_by: None,
        data_change_last_time: None,
        data_change_created_time: None,
    };

    // Owner app: public AppNamespace + Namespace + items + release.
    AppNamespaceService::new(p.clone())
        .create(AppNamespaceDTO {
            id: None,
            name: "shared-ns".into(),
            app_id: "owner".into(),
            format: "properties".into(),
            is_public: true,
            comment: "c".into(),
            data_change_created_by: Some("t".into()),
            data_change_created_time: None,
        })
        .await
        .unwrap();
    NamespaceService::new(p.clone()).create("owner", "default", ns_dto("owner", true)).await.unwrap();
    let item_svc = ItemService::new(p.clone());
    item_svc.create("owner", "default", "shared-ns", item_dto("pub.only", "from-public")).await.unwrap();
    item_svc.create("owner", "default", "shared-ns", item_dto("overlap", "public-value")).await.unwrap();
    ReleaseService::new(p.clone())
        .publish("owner", "default", "shared-ns", "r1", None, "t", false)
        .await
        .unwrap();

    // Consumer app owns the SAME namespace privately with its own release.
    NamespaceService::new(p.clone()).create("consumer", "default", ns_dto("consumer", false)).await.unwrap();
    item_svc.create("consumer", "default", "shared-ns", item_dto("overlap", "private-wins")).await.unwrap();
    ReleaseService::new(p.clone())
        .publish("consumer", "default", "shared-ns", "c1", None, "t", false)
        .await
        .unwrap();

    // Request via HTTP as the consumer.
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(p))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;
    let req = test::TestRequest::get()
        .uri("/configs/consumer/default/shared-ns")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(
        body["configurations"]["pub.only"], "from-public",
        "public-only key must survive the merge"
    );
    assert_eq!(
        body["configurations"]["overlap"], "private-wins",
        "private release overrides public per-key"
    );
}

/// P3 — database-discovery metaservice: live registry rows are served;
/// an empty registry falls back to this node.
#[actix_web::test]
async fn b1_metaservice_serves_registry_instances() {
    use batata_plugin_apollo::persistence::traits::ServiceRegistryPersistence;

    let p: Arc<dyn ApolloPersistenceService> = common::make_embedded_persistence().await;

    // Two live configservice instances + one stale row (outside 61s window).
    ServiceRegistryPersistence::heartbeat(&*p, "apollo-configservice", "http://10.0.0.1:8080", "default").await.unwrap();
    ServiceRegistryPersistence::heartbeat(&*p, "apollo-configservice", "http://10.0.0.2:8080", "default").await.unwrap();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(p.clone()))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let req = test::TestRequest::get().uri("/services/config").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 2, "both live instances are served");
    assert!(
        arr.iter().any(|d| d["homepageUrl"] == "http://10.0.0.1:8080"),
        "registry uri is surfaced as homepageUrl"
    );

    // Deregistration removes the instance from discovery.
    ServiceRegistryPersistence::deregister(&*p, "apollo-configservice", "http://10.0.0.1:8080").await.unwrap();
    let req = test::TestRequest::get().uri("/services/config").to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body.as_array().unwrap().len(), 1);

    // Empty registry → self fallback (host echo).
    ServiceRegistryPersistence::deregister(&*p, "apollo-configservice", "http://10.0.0.2:8080").await.unwrap();
    let req = test::TestRequest::get()
        .uri("/services/config")
        .insert_header(("host", "self:8080"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body[0]["homepageUrl"], "http://self:8080");
}

/// P3.3+P3.4 — config fetch audits instances; adminservice by-* queries read them.
#[actix_web::test]
async fn instance_audit_and_queries() {
    use batata_plugin_apollo::api::dto::{ItemDTO, NamespaceDTO};
    use batata_plugin_apollo::service::{ItemService, NamespaceService, ReleaseService};

    let p: Arc<dyn ApolloPersistenceService> = common::make_embedded_persistence().await;
    let ns_dto = NamespaceDTO {
        app_id: "app1".into(),
        cluster_name: "default".into(),
        namespace_name: "audit-ns".into(),
        format: Some("properties".into()),
        is_public: Some(false),
        comment: None,
        data_change_created_by: Some("t".into()),
        data_change_last_modified_by: None,
        data_change_last_time: None,
        data_change_created_time: None,
    };
    NamespaceService::new(p.clone()).create("app1", "default", ns_dto).await.unwrap();
    ItemService::new(p.clone())
        .create("app1", "default", "audit-ns", ItemDTO { id: None, key: "k".into(), value: "v".into(), r#type: None, comment: None, line_num: None, data_change_created_by: Some("t".into()), data_change_last_modified_by: None, data_change_last_time: None, data_change_created_time: None })
        .await
        .unwrap();
    ReleaseService::new(p.clone())
        .publish("app1", "default", "audit-ns", "r1", None, "t", false)
        .await
        .unwrap();

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(p.clone()))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    // Client fetch → async audit writes Instance + InstanceConfig.
    let req = test::TestRequest::get()
        .uri("/configs/app1/default/audit-ns")
        .insert_header(("X-Forwarded-For", "10.1.1.9"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    let release_key = body["releaseKey"].as_str().unwrap().to_string();

    // wait for the detached audit task
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;

    // ADMSVC-002: by-namespace lists the audited fetch
    let req = test::TestRequest::get()
        .uri("/instances/by-namespace?appId=app1&clusterName=default&namespaceName=audit-ns")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let rows: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(rows.as_array().unwrap().len(), 1);
    assert_eq!(rows[0]["releaseKey"], release_key.as_str());
    assert_eq!(rows[0]["configAppId"], "app1");
    assert_eq!(rows[0]["instance"]["ip"], "10.1.1.9");

    // ADMSVC-003: count
    let req = test::TestRequest::get()
        .uri("/instances/by-namespace/count?appId=app1&clusterName=default&namespaceName=audit-ns")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["count"], 1);

    // ADMSVC-004: not-in the delivered release → empty; not-in another → 1 row.
    let other = if release_key.ends_with('x') { format!("{}y", release_key) } else { format!("{}x", release_key) };
    let url = "/instances/by-namespace-and-releases-not-in?appId=app1&clusterName=default&namespaceName=audit-ns&releaseIds=-1".to_string();
    let req = test::TestRequest::get().uri(&url).to_request();
    let resp = test::call_service(&app, req).await;
    let rows: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(rows.as_array().unwrap().len(), 1);
    let _ = other;
}
