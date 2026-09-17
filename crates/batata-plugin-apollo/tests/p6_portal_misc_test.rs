mod common;

use std::sync::Arc;

use actix_web::{test, web, App};
use batata_plugin_apollo::api::dto::{ItemChangeSets, ItemDTO, NamespaceDTO};
use batata_plugin_apollo::persistence::traits::ApolloPersistenceService;
use batata_plugin_apollo::service::{AppService, ConfigSyncService, ItemService, NamespaceService};

fn item(k: &str, v: &str) -> ItemDTO {
    ItemDTO {
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
    }
}

async fn seed(p: Arc<dyn ApolloPersistenceService>, app_id: &str, ns: &str) {
    AppService::new(p.clone())
        .create(batata_plugin_apollo::api::dto::AppDTO {
            app_id: app_id.to_string(),
            name: "T".into(),
            org_id: "T".into(),
            org_name: "T".into(),
            owner_name: "t".into(),
            owner_email: "t@t".into(),
            data_change_created_by: Some("t".into()),
            data_change_last_modified_by: None,
            data_change_last_time: None,
            data_change_created_time: None,
        })
        .await
        .unwrap();
    for target in [ns, "target-ns"] {
        NamespaceService::new(p.clone())
            .create(
                app_id,
                "default",
                NamespaceDTO {
                    app_id: app_id.to_string(),
                    cluster_name: "default".into(),
                    namespace_name: target.into(),
                    format: Some("properties".into()),
                    is_public: Some(false),
                    comment: None,
                    data_change_created_by: Some("t".into()),
                    data_change_last_modified_by: None,
                    data_change_last_time: None,
                    data_change_created_time: None,
                },
            )
            .await
            .unwrap();
    }
}

/// PITEM-009 — `items/diff` returns, per target namespace, the create/update/
/// delete item lists that `synchronize` would apply.
#[actix_web::test]
async fn pitem009_diff_produces_per_namespace_changesets() {
    let p = common::make_embedded_persistence().await;
    let app_id = format!("diff-{}", chrono::Utc::now().timestamp_millis());
    seed(p.clone(), &app_id, "src").await;

    let isvc = ItemService::new(p.clone());
    isvc.create(&app_id, "default", "src", item("shared", "v0"))
        .await
        .unwrap();
    isvc.create(&app_id, "default", "target-ns", item("shared", "vold"))
        .await
        .unwrap();

    let svc = ConfigSyncService::new(p.clone());
    let diffs = svc
        .compare(
            &app_id,
            "default",
            "src",
            &["target-ns".to_string()],
            &ItemChangeSets {
                create_items: vec![item("new", "n")],
                update_items: vec![item("shared", "v1")],
                delete_items: vec![item("gone", "x")],
            },
        )
        .await
        .unwrap();

    assert_eq!(diffs.len(), 1, "one diff per target namespace");
    let d = &diffs[0];
    assert_eq!(d.namespace_name, "target-ns");
    // `new` does not exist in target -> create
    assert_eq!(d.create_items.len(), 1);
    assert_eq!(d.create_items[0].key, "new");
    // `shared` exists in target with a different value -> update
    assert_eq!(d.update_items.len(), 1);
    assert_eq!(d.update_items[0].key, "shared");
    // `gone` does not exist in target -> nothing to delete
    assert!(d.delete_items.is_empty());
}

/// PITEM-010 — `PUT .../items` synchronizes the change sets into the target
/// namespace, returning the resulting item list.
#[actix_web::test]
async fn pitem010_synchronize_applies_changesets() {
    let p = common::make_embedded_persistence().await;
    let app_id = format!("sync-{}", chrono::Utc::now().timestamp_millis());
    seed(p.clone(), &app_id, "src").await;

    let isvc = ItemService::new(p.clone());
    isvc.create(&app_id, "default", "src", item("k", "v"))
        .await
        .unwrap();

    let svc = ConfigSyncService::new(p.clone());
    let results = svc
        .synchronize(
            &app_id,
            "default",
            "src",
            &["target-ns".to_string()],
            &ItemChangeSets {
                create_items: vec![item("synced", "yes")],
                update_items: vec![],
                delete_items: vec![],
            },
            "t",
        )
        .await
        .unwrap();

    assert_eq!(results.len(), 1);
    let keys: Vec<&str> = results[0].iter().map(|i| i.key.as_str()).collect();
    assert!(keys.contains(&"synced"), "target namespace now has `synced`");
}

/// PMISC-001 — the openapi instance endpoint returns a paged envelope, even
/// when there are no instances yet (single-tenant: empty content, total 0).
#[actix_web::test]
async fn pmisc001_instances_returns_paged_envelope() {
    let p = common::make_embedded_persistence().await;
    let app_id = format!("inst-{}", chrono::Utc::now().timestamp_millis());
    seed(p.clone(), &app_id, "ns").await;

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(p.clone()))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let uri = format!(
        "/openapi/v1/apps/{}/envs/DEV/clusters/default/namespaces/ns/instances?page=1&size=20",
        app_id
    );
    let resp = test::call_service(&app, test::TestRequest::get().uri(&uri).to_request()).await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["page"], 1);
    assert_eq!(body["size"], 20);
    assert_eq!(body["total"], 0);
    assert!(body["content"].as_array().unwrap().is_empty());
}

/// PMISC-007/008/009/010/011/016 — single-tenant degraded permission/user/system
/// endpoints return the expected upstream-shaped payloads.
#[actix_web::test]
async fn pmisc_degraded_endpoints() {
    let p = common::make_embedded_persistence().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(p.clone()))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let cases = [
        ("/openapi/v1/permissions/root", "hasRootPermission"),
        ("/openapi/v1/apps/demo/permissions/CREATE_NAMESPACE", "hasPermission"),
        ("/openapi/v1/apps/demo/roles/ModifyNamespace", "roleName"),
        // NOTE: /organizations is a LIST endpoint (List<OrganizationDTO>);
        // it is asserted separately below against its array shape.
        ("/openapi/v1/user", "username"),
        ("/openapi/v1/system-info", "apolloVersion"),
    ];
    for (uri, key) in cases {
        let resp = test::call_service(&app, test::TestRequest::get().uri(uri).to_request()).await;
        assert!(resp.status().is_success(), "GET {} should succeed", uri);
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body.get(key).is_some(), "GET {} carries `{}`", uri, key);
    }

    // organizations returns an array of one default org
    let resp = test::call_service(
        &app,
        test::TestRequest::get().uri("/openapi/v1/organizations").to_request(),
    )
    .await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    let arr = body.as_array().expect("organizations returns an array");
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["orgName"], "default");
}

/// PORT-009 — env-cluster-info returns one entry per canonical env, each with
/// the app's root clusters.
#[actix_web::test]
async fn port009_env_cluster_info() {
    let p = common::make_embedded_persistence().await;
    let app_id = format!("envc-{}", chrono::Utc::now().timestamp_millis());
    seed(p.clone(), &app_id, "ns").await;

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(p.clone()))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let uri = format!("/openapi/v1/apps/{}/env-cluster-info", app_id);
    let resp = test::call_service(&app, test::TestRequest::get().uri(&uri).to_request()).await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    let envs = body["envClusterInfo"].as_array().expect("envClusterInfo is an array");
    assert_eq!(envs.len(), 4, "DEV/FAT/UAT/PRO");
    assert_eq!(envs[0]["env"], "DEV");
    assert!(!envs[0]["clusters"].as_array().unwrap().is_empty(), "app has a root cluster");
}
