mod common;

use std::sync::Arc;

use actix_web::{test, web, App};
use batata_plugin_apollo::persistence::traits::ApolloPersistenceService;

async fn spawn_app() -> (Arc<dyn ApolloPersistenceService>, ()) {
    let p = common::make_embedded_persistence().await;
    (p, ())
}

macro_rules! req {
    ($app:expr, $m:ident, $uri:expr $(, $body:expr)?) => {{
        let b = test::TestRequest::$m();
        #[allow(unused_mut)]
        let mut b = b.uri($uri);
        $( b = b.set_payload($body); b = b.insert_header(("content-type","application/json")); )?
        let r = b.to_request();
        test::call_service(&$app, r).await
    }};
}

#[actix_web::test]
async fn p5_batch2_end_to_end_in_process() {
    use batata_plugin_apollo::api::dto::{ItemDTO, NamespaceDTO};
    use batata_plugin_apollo::service::{AppService, ItemService, NamespaceService, ReleaseService};
    use batata_plugin_apollo::api::dto::{AppDTO, ItemChangeSets};

    let (p, _) = spawn_app().await;

    // ---- seed via services ----
    let app_id = format!("b2t-{}", chrono::Utc::now().timestamp_millis());
    AppService::new(p.clone())
        .create(AppDTO {
            app_id: app_id.clone(), name: "T".into(), org_id: "T".into(), org_name: "T".into(),
            owner_name: "t".into(), owner_email: "t@t".into(),
            data_change_created_by: Some("t".into()),
            data_change_last_modified_by: None,
            data_change_created_time: None, data_change_last_time: None,
        })
        .await
        .unwrap();
    // bootstrap default cluster + application ns (upstream semantics)
    AppService::new(p.clone()).bootstrap_default_namespace(&app_id, "t").await.unwrap();

    NamespaceService::new(p.clone())
        .create(&app_id, "default",
            NamespaceDTO { app_id: app_id.clone(), cluster_name: "default".into(),
                namespace_name: "nsz".into(), format: Some("properties".into()),
                is_public: Some(false), comment: None,
                data_change_created_by: Some("t".into()), data_change_last_modified_by: None,
                data_change_last_time: None, data_change_created_time: None })
        .await.unwrap();
    let item = |k:&str,v:&str| ItemDTO{ id:None,key:k.into(),value:v.into(),r#type:None,comment:None,line_num:None,data_change_created_by:Some("t".into()),data_change_last_modified_by:None,data_change_last_time:None,data_change_created_time:None};
    let isvc = ItemService::new(p.clone());
    isvc.create(&app_id,"default","nsz",item("k1","v1")).await.unwrap();
    isvc.create(&app_id,"default","nsz",item("k2","v2")).await.unwrap();
    ReleaseService::new(p.clone()).publish(&app_id,"default","nsz","r1",None,"t",false).await.unwrap();

    // ---- in-process HTTP ----
    let app = test::init_service(
        App::new().app_data(web::Data::new(p.clone()))
            .configure(batata_plugin_apollo::route::configure_routes),
    ).await;
    let _ = std::mem::replace(&mut (), ());

    // PITEM-003 bulk PUT: k1 changed, k3 added, k2 removed
    let resp = req!(app, put, &format!("/openapi/v1/envs/DEV/apps/{}/clusters/default/namespaces/nsz/items", app_id),
        r#"{"text":"k1=changed\nk3=v3","operator":"t"}"#);
    assert_eq!(resp.status(), 200, "bulk PUT should succeed");
    let bytes = actix_web::body::to_bytes(resp.into_body()).await.unwrap_or_default();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    let _ = body;
    let items = isvc.list(&app_id,"default","nsz").await.unwrap();
    let got: Vec<(String,String)> = items.iter().map(|i|(i.key.clone(),i.value.clone())).collect();
    assert!(got.contains(&("k1".into(),"changed".into())), "{:?}", got);
    assert!(got.contains(&("k3".into(),"v3".into())));
    assert!(!got.iter().any(|(k,_)| k=="k2"), "k2 must be removed by bulk text");

    // PITEM-011 validation
    let resp = req!(app, post, &format!("/openapi/v1/envs/DEV/apps/{}/clusters/default/namespaces/nsz/items/validation", app_id),
        r#"{"text":"a=1\nb = two"}"#);
    assert_eq!(resp.status(), 200);
    let resp = req!(app, post, &format!("/openapi/v1/envs/DEV/apps/{}/clusters/default/namespaces/nsz/items/validation", app_id),
        r#"{"text":"broken_line"}"#);
    assert_eq!(resp.status(), 400);

    // PITEM-012 revocation → back to release state {k1:v1,k2:v2}
    let resp = req!(app, post, &format!("/openapi/v1/envs/DEV/apps/{}/clusters/default/namespaces/nsz/items/revocation?operator=t", app_id));
    assert_eq!(resp.status(), 200);
    let items = isvc.list(&app_id,"default","nsz").await.unwrap();
    let m: std::collections::HashMap<String,String> = items.iter().map(|i|(i.key.clone(),i.value.clone())).collect();
    assert_eq!(m.get("k1").map(String::as_str), Some("v1"));
    assert_eq!(m.get("k2").map(String::as_str), Some("v2"));
    assert!(!m.contains_key("k3"));

    // PITEM-007 encodedItems GET
    use base64::Engine;
    let k = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b"k1");
    let resp = req!(app, get, &format!("/openapi/v1/envs/DEV/apps/{}/clusters/default/namespaces/nsz/encodedItems/{}", app_id, k));
    assert_eq!(resp.status(), 200);

    // PITEM-008 branch items (create branch first)
    let resp = req!(app, post, &format!("/openapi/v1/envs/DEV/apps/{}/clusters/default/namespaces/nsz/branches", app_id), "{}");
    assert_eq!(resp.status(), 200);
    let bytes = actix_web::body::to_bytes(resp.into_body()).await.unwrap_or_default();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    let branch = body["clusterName"].as_str().unwrap().to_string();
    let resp = req!(app, get, &format!("/openapi/v1/envs/DEV/apps/{}/clusters/default/namespaces/nsz/branches/{}/items", app_id, branch));
    assert_eq!(resp.status(), 200);

    // ADM-019 / PORT-025 publish_info + releases/status
    let resp = req!(app, get, &format!("/apps/{}/namespaces/publish_info", app_id));
    assert_eq!(resp.status(), 200);
    let resp = req!(app, get, &format!("/openapi/v1/apps/{}/namespaces/releases/status", app_id));
    assert_eq!(resp.status(), 200);

    // PORT-003/004
    for uri in ["/openapi/v1/apps/authorized", "/openapi/v1/apps/by-self"] {
        let resp = req!(app, get, uri);
        assert_eq!(resp.status(), 200, "{}", uri);
    }

    // ADM-017 find-by-item
    let resp = req!(app, get, &format!("/namespaces/find-by-item?itemKey=k1&size=5"));
    assert_eq!(resp.status(), 200);
    let bytes = actix_web::body::to_bytes(resp.into_body()).await.unwrap_or_default();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    assert!(body["total"].as_u64().unwrap_or(0) >= 1);

    // PORT-026 missing-namespaces
    let resp = req!(app, get, &format!("/openapi/v1/envs/DEV/apps/{}/clusters/shadowx/missing-namespaces", app_id));
    assert_eq!(resp.status(), 200);

    // ADM-018 associated-public-namespace: no public ns seeded with that name → 404
    let resp = req!(app, get, &format!("/apps/{}/clusters/default/namespaces/nsz/associated-public-namespace", app_id));
    assert_eq!(resp.status(), 404);

    // ADM-011 unique
    let resp = req!(app, get, &format!("/apps/{}/cluster/default/unique", app_id));
    assert_eq!(resp.status(), 200);
    let _ = ItemChangeSets{ create_items: vec![], update_items: vec![], delete_items: vec![] };
}
