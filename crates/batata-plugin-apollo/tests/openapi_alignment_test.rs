//! HTTP-level assertions for the Apollo v0.3.13 path alignment round.
//!
//! Every case here fails if a newly registered route is removed: the request
//! would either 404/405 (missing route) or return stale data (missing handler
//! behaviour). Existing batata paths are kept as aliases and are not removed.

mod common;

use std::sync::Arc;

use actix_web::{App, test, web};
use batata_plugin_apollo::persistence::shared::{StoredApp, StoredNamespace};
use batata_plugin_apollo::persistence::traits::{
    ApolloPersistenceService, AppPersistence, NamespacePersistence,
};
use serde_json::{Value, json};

macro_rules! call {
    ($app:expr, $req:expr) => {{
        let resp = test::call_service(&$app, $req).await;
        let status = resp.status().as_u16();
        let body = test::read_body(resp).await;
        let value = serde_json::from_slice::<Value>(&body).unwrap_or(Value::Null);
        (status, value)
    }};
}

async fn seed_app_namespace(p: &Arc<dyn ApolloPersistenceService>, app_id: &str) {
    AppPersistence::create(
        &**p,
        StoredApp {
            app_id: app_id.to_string(),
            name: app_id.to_string(),
            org_id: "1".to_string(),
            org_name: "default".to_string(),
            owner_name: "apollo".to_string(),
            owner_email: "apollo@example.com".to_string(),
            is_deleted: false,
            deleted_at: 0,
            data_change_created_by: "apollo".to_string(),
            data_change_created_time: 0,
            data_change_last_modified_by: None,
            data_change_last_time: None,
        },
    )
    .await
    .expect("create app");
    NamespacePersistence::create(
        &**p,
        StoredNamespace {
            id: 0,
            app_id: app_id.to_string(),
            cluster_name: "default".to_string(),
            namespace_name: "application".to_string(),
            format: "properties".to_string(),
            is_public: false,
            comment: None,
            is_deleted: false,
            deleted_at: 0,
            data_change_created_by: "apollo".to_string(),
            data_change_created_time: 0,
            data_change_last_modified_by: None,
            data_change_last_time: None,
        },
    )
    .await
    .expect("create namespace");
}

#[actix_web::test]
async fn batch_item_verbs_are_registered_and_change_real_items() {
    let p = common::make_embedded_persistence().await;
    seed_app_namespace(&p, "batch-app").await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(p))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;
    let base = "/openapi/v1/envs/DEV/apps/batch-app/clusters/default/namespaces/application/items";

    let (status, _) = call!(
        app,
        test::TestRequest::post()
            .uri(&format!("{base}/batch-create?operator=alice"))
            .set_json(json!([
                { "key": "k1", "value": "v1" },
                { "key": "k2", "value": "v2" }
            ]))
            .to_request()
    );
    assert_eq!(status, 200, "POST items/batch-create must be registered");

    let (status, body) = call!(app, test::TestRequest::get().uri(base).to_request());
    assert_eq!(status, 200);
    let content = body["content"].as_array().expect("item content array");
    assert_eq!(
        content.len(),
        2,
        "both batch-created items must be persisted"
    );

    let (status, _) = call!(
        app,
        test::TestRequest::put()
            .uri(&format!("{base}/batch-update?operator=alice"))
            .set_json(json!([{ "key": "k1", "value": "v1-updated" }]))
            .to_request()
    );
    assert_eq!(status, 200, "PUT items/batch-update must be registered");

    let (_, body) = call!(app, test::TestRequest::get().uri(base).to_request());
    let k1 = body["content"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["key"] == "k1")
        .expect("k1 exists");
    assert_eq!(
        k1["value"], "v1-updated",
        "batch update must persist values"
    );

    let (status, _) = call!(
        app,
        test::TestRequest::post()
            .uri(&format!("{base}/batch-delete?operator=alice"))
            .set_json(json!(["k2"]))
            .to_request()
    );
    assert_eq!(status, 200, "POST items/batch-delete must be registered");

    let (_, body) = call!(app, test::TestRequest::get().uri(base).to_request());
    let keys: Vec<&str> = body["content"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item["key"].as_str())
        .collect();
    assert_eq!(
        keys,
        vec!["k1"],
        "batch delete must remove the selected key"
    );

    let (status, _) = call!(
        app,
        test::TestRequest::post()
            .uri(&format!("{base}/batch-create"))
            .set_json(json!([]))
            .to_request()
    );
    assert_eq!(status, 400, "upstream rejects an empty item list");
}

#[actix_web::test]
async fn upstream_path_aliases_are_reachable() {
    let p = common::make_embedded_persistence().await;
    seed_app_namespace(&p, "alias-app").await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(p))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let diff_body = json!({
        "namespaceName": "application",
        "syncItems": { "createItems": [], "updateItems": [], "deleteItems": [] },
        "syncToNamespaces": []
    });
    let (status, diff_response) = call!(
        app,
        test::TestRequest::post()
            .uri("/openapi/v1/envs/DEV/apps/alias-app/clusters/default/namespaces/application/items/diff")
            .set_json(&diff_body)
            .to_request()
    );
    assert_eq!(
        status, 200,
        "upstream items/diff path must be registered: {diff_response:?}"
    );

    let (status, body) = call!(
        app,
        test::TestRequest::get()
            .uri("/openapi/v1/envs/DEV/apps/alias-app/clusters/default/namespaces/application/instances")
            .to_request()
    );
    assert_eq!(status, 200, "upstream instance count path: {body:?}");

    for uri in [
        "/openapi/v1/envs/DEV/instances/by-release?releaseId=1",
        "/openapi/v1/envs/DEV/instances/by-namespace?appId=alias-app&clusterName=default&namespaceName=application",
        "/openapi/v1/envs/DEV/instances/by-namespace-and-releases-not-in?appId=alias-app&clusterName=default&namespaceName=application&releaseIds=1",
        "/openapi/v1/envs/DEV/apps/alias-app/clusters/default/namespaces/permissions/ModifyNamespace",
        "/openapi/v1/envs/DEV/apps/alias-app/clusters/default/namespaces/role-users",
        "/openapi/v1/envs/DEV/apps/alias-app/clusters/default/roles/Master",
    ] {
        let (status, body) = call!(app, test::TestRequest::get().uri(uri).to_request());
        assert_ne!(status, 405, "GET {uri} must accept GET");
        if uri.contains("by-release?releaseId") || uri.contains("releases-not-in") {
            // The handler-level 404 proves the route was reached; an
            // unregistered path would return an empty 404 body.
            assert!(
                body.get("message").is_some(),
                "GET {uri} must reach the release lookup handler"
            );
        } else {
            assert_ne!(status, 404, "GET {uri} must be a registered route");
        }
    }

    let (status, _) = call!(
        app,
        test::TestRequest::post()
            .uri("/openapi/v1/envs/DEV/apps/alias-app/clusters/default/namespaces/application/branches/gray")
            .set_json(json!({
                "releaseTitle": "merge",
                "releasedBy": "apollo",
                "createItems": [], "updateItems": [], "deleteItems": []
            }))
            .to_request()
    );
    assert_ne!(status, 405, "upstream mergeBranch POST must be registered");
}

#[::core::prelude::v1::test]
fn action_catalog_exposes_the_new_alignment_operations() {
    use batata_plugin_apollo::auth::action_catalog::ACTION_CATALOG;

    assert!(
        ACTION_CATALOG.len() >= 92,
        "catalog must cover every aligned operation"
    );
    for id in [
        "item.batch_create",
        "item.batch_update",
        "item.batch_delete",
        "release.gray_del",
        "appnamespace.deletion_check",
        "role.app_assign",
        "role.app_remove",
        "role.app_master_assign",
        "role.app_master_remove",
        "role.create_application_remove",
        "item.diff_upstream",
        "instance.count_upstream",
        "instance.by_release_upstream",
        "instance.by_namespace_upstream",
        "instance.not_in_upstream",
        "branch.merge_upstream",
        "permission.cluster_namespace_upstream",
        "role.cluster_namespace_users_upstream",
        "role.cluster_assign_upstream",
        "role.cluster_remove_upstream",
    ] {
        assert!(
            ACTION_CATALOG.iter().any(|action| action.id == id),
            "missing action catalog entry: {id}"
        );
    }
}

#[actix_web::test]
async fn deletion_check_and_role_endpoints_are_registered() {
    let p = common::make_embedded_persistence().await;
    seed_app_namespace(&p, "role-app").await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(p))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let (status, body) = call!(
        app,
        test::TestRequest::get()
            .uri("/openapi/v1/apps/role-app/appnamespaces/application/deletion-check")
            .to_request()
    );
    assert_eq!(status, 200, "deletion-check must be registered: {body:?}");
    assert_eq!(body["appId"], "role-app");
    assert_eq!(
        body["environments"].as_array().map(Vec::len),
        Some(4),
        "upstream deletion-check reports every environment"
    );

    for (method, uri, body) in [
        (
            "POST",
            "/openapi/v1/apps/role-app/roles/ModifyNamespace",
            json!("alice"),
        ),
        (
            "DELETE",
            "/openapi/v1/apps/role-app/roles/ModifyNamespace?user=alice",
            json!(null),
        ),
        (
            "POST",
            "/openapi/v1/apps/role-app/roles/master",
            json!("alice"),
        ),
        (
            "DELETE",
            "/openapi/v1/apps/role-app/roles/master?user=alice",
            json!(null),
        ),
        (
            "DELETE",
            "/openapi/v1/system/roles/create-application?userId=alice",
            json!(null),
        ),
    ] {
        let request = if method == "POST" {
            test::TestRequest::post()
                .uri(uri)
                .set_json(body)
                .to_request()
        } else {
            test::TestRequest::delete().uri(uri).to_request()
        };
        let (status, response) = call!(app, request);
        assert_ne!(status, 404, "{method} {uri} must be a registered route");
        assert_ne!(status, 405, "{method} {uri} must accept {method}");
        let _ = response;
    }
}
