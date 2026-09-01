mod common;

use std::sync::Arc;

use actix_web::{test, web, App};
use batata_plugin_apollo::api::dto::{AppDTO, ItemChangeSets, ItemDTO, NamespaceDTO};
use batata_plugin_apollo::persistence::traits::ApolloPersistenceService;
use batata_plugin_apollo::service::{
    AppService, ItemService, ItemSetService, NamespaceService, ReleaseService,
};

/// Builds an item DTO matching the shape used by the plugin tests.
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

/// Seeds an application plus one custom namespace in cluster `default`.
async fn seed(p: Arc<dyn ApolloPersistenceService>, app_id: &str, ns: &str) {
    AppService::new(p.clone())
        .create(AppDTO {
            app_id: app_id.to_string(),
            name: "T".into(),
            org_id: "T".into(),
            org_name: "T".into(),
            owner_name: "t".into(),
            owner_email: "t@t".into(),
            data_change_created_by: Some("t".into()),
            data_change_last_modified_by: None,
            data_change_created_time: None,
            data_change_last_time: None,
        })
        .await
        .unwrap();
    AppService::new(p.clone())
        .bootstrap_default_namespace(app_id, "t")
        .await
        .unwrap();
    NamespaceService::new(p.clone())
        .create(
            app_id,
            "default",
            NamespaceDTO {
                app_id: app_id.to_string(),
                cluster_name: "default".into(),
                namespace_name: ns.into(),
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

/// F-APO-ITEM-006 — `items/deleted` reports the items deleted since the latest
/// active release, not the set of currently soft-deleted items.
#[actix_web::test]
async fn item006_deleted_items_are_scoped_to_latest_release() {
    let p = common::make_embedded_persistence().await;
    let app_id = format!("del-{}", chrono::Utc::now().timestamp_millis());
    let ns = "nsd";
    seed(p.clone(), &app_id, ns).await;

    let isvc = ItemService::new(p.clone());
    isvc.create(&app_id, "default", ns, item("k1", "v1"))
        .await
        .unwrap();
    isvc.create(&app_id, "default", ns, item("k2", "v2"))
        .await
        .unwrap();

    // Publish so there is a "last release" timestamp to compare commits against.
    ReleaseService::new(p.clone())
        .publish(&app_id, "default", ns, "r1", None, "t", false)
        .await
        .unwrap();

    // Delete through `update_set` so a commit carrying the change sets is written.
    ItemSetService::new(p.clone())
        .update_set(
            &app_id,
            "default",
            ns,
            ItemChangeSets {
                create_items: vec![],
                update_items: vec![],
                delete_items: vec![item("k1", "v1")],
            },
        )
        .await
        .unwrap();

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(p.clone()))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let uri = format!(
        "/apps/{}/clusters/default/namespaces/{}/items/deleted",
        app_id, ns
    );
    let resp = test::call_service(&app, test::TestRequest::get().uri(&uri).to_request()).await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    let deleted = body.as_array().expect("items/deleted returns an array");
    assert_eq!(deleted.len(), 1, "k1 was deleted after the publish");
    assert_eq!(deleted[0]["key"], "k1");

    // Publishing again moves the boundary forward, so nothing is "deleted since".
    ReleaseService::new(p.clone())
        .publish(&app_id, "default", ns, "r2", None, "t", false)
        .await
        .unwrap();
    let resp = test::call_service(&app, test::TestRequest::get().uri(&uri).to_request()).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    let deleted = body.as_array().expect("items/deleted returns an array");
    assert!(
        deleted.is_empty(),
        "nothing is reported once the deletion precedes the latest publish"
    );
}

/// F-APO-ITEM-011 — the standard search path wraps matches in a `PageDTO`
/// (`{total, content, page, size}`) instead of returning a bare array.
#[actix_web::test]
async fn item011_key_and_value_search_returns_page_dto() {
    let p = common::make_embedded_persistence().await;
    let app_id = format!("srch-{}", chrono::Utc::now().timestamp_millis());
    let ns = "nss";
    seed(p.clone(), &app_id, ns).await;

    let isvc = ItemService::new(p.clone());
    isvc.create(&app_id, "default", ns, item("alpha", "1"))
        .await
        .unwrap();
    isvc.create(&app_id, "default", ns, item("beta", "2"))
        .await
        .unwrap();

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(p.clone()))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/items-search/key-and-value?key=alpha")
            .to_request(),
    )
    .await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;

    assert!(body.get("content").is_some(), "PageDTO carries `content`");
    assert!(body.get("total").is_some(), "PageDTO carries `total`");
    assert_eq!(body["page"], 0, "PageDTO defaults to page 0");
    assert_eq!(body["size"], 20, "PageDTO defaults to size 20");

    let content = body["content"].as_array().expect("content is an array");
    assert_eq!(content.len(), 1, "only `alpha` matches");
    assert_eq!(content[0]["key"], "alpha");
}
