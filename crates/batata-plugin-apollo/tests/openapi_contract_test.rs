//! Contract tests for the portal OpenAPI surface.
//!
//! These are deliberately HTTP-level: every case below reproduces a defect that
//! was only visible at runtime (stub handlers, DTO field drift, a route alias
//! registered on the wrong path). Reverting any guard must turn the
//! corresponding test red.

mod common;

use std::sync::Arc;

use actix_web::{App, test, web};
use batata_plugin_apollo::persistence::traits::ApolloPersistenceService;
use serde_json::{Value, json};

// `actix_http::Request` is not a direct dependency of this test crate, so the
// helpers are macros: they keep the concrete request/service types inferred.
macro_rules! make_app {
    () => {{
        let p: Arc<dyn ApolloPersistenceService> = common::make_embedded_persistence().await;
        test::init_service(
            App::new()
                .app_data(web::Data::new(p))
                .configure(batata_plugin_apollo::route::configure_routes),
        )
        .await
    }};
}

macro_rules! call {
    ($app:expr, $req:expr) => {{
        let resp = test::call_service(&$app, $req).await;
        let status = resp.status().as_u16();
        let body = test::read_body(resp).await;
        let value = serde_json::from_slice::<Value>(&body).unwrap_or(Value::Null);
        (status, value)
    }};
}

/// PMISC-017: upstream `find-all-config` returns a bare list built from real
/// storage. The former handlers returned `{}` / `{content:[],total:0}` and
/// rejected POST/DELETE with 405.
#[actix_web::test]
async fn server_config_crud_uses_upstream_paths_and_real_storage() {
    let app = make_app!();
    let key = format!("cfg-{}", chrono::Utc::now().timestamp_millis());

    let (status, _) = call!(
        app,
        test::TestRequest::post()
            .uri("/openapi/v1/server/portal-db/config")
            .set_json(json!({ "key": key, "value": "v1", "comment": "c" }))
            .to_request()
    );
    assert_eq!(
        status, 200,
        "POST /server/portal-db/config must be registered"
    );

    for uri in [
        "/openapi/v1/server/portal-db/config/find-all-config".to_string(),
        "/openapi/v1/server/portal-db/config/find-all".to_string(),
        "/openapi/v1/server/portal-db/config".to_string(),
        "/openapi/v1/server/envs/DEV/config-db/config/find-all-config".to_string(),
    ] {
        let (status, body) = call!(app, test::TestRequest::get().uri(&uri).to_request());
        assert_eq!(status, 200, "GET {uri}");
        let list = body
            .as_array()
            .unwrap_or_else(|| panic!("GET {uri} must return an array"));
        assert!(
            list.iter().any(|c| c["key"] == key && c["value"] == "v1"),
            "GET {uri} must read the persisted config, got {body:?}"
        );
    }

    // @Valid ServerConfig: a blank key must be rejected so the create modal
    // stays open instead of reporting success.
    let (status, _) = call!(
        app,
        test::TestRequest::post()
            .uri("/openapi/v1/server/portal-db/config")
            .set_json(json!({ "key": "", "value": "" }))
            .to_request()
    );
    assert_eq!(status, 400, "blank key/value must be rejected");

    let (status, _) = call!(
        app,
        test::TestRequest::delete()
            .uri(&format!("/openapi/v1/server/portal-db/config?key={key}"))
            .to_request()
    );
    assert_eq!(
        status, 200,
        "DELETE /server/portal-db/config must be registered"
    );

    let (_, body) = call!(
        app,
        test::TestRequest::get()
            .uri("/openapi/v1/server/portal-db/config/find-all-config")
            .to_request()
    );
    assert!(
        !body
            .as_array()
            .expect("list is an array")
            .iter()
            .any(|c| c["key"] == key),
        "deleted config must disappear"
    );
}

/// PMISC-013: upstream `UserTokenCreateRequest.name` + `tokenValue`, camelCase
/// list summaries, POST revoke/rotate, and never leak `token_hash`.
#[actix_web::test]
async fn user_tokens_use_upstream_dto_and_hide_the_hash() {
    let app = make_app!();

    let (status, created) = call!(
        app,
        test::TestRequest::post()
            .uri("/openapi/v1/user-tokens")
            .set_json(json!({ "name": "e2e-token", "expires": "2026-12-31" }))
            .to_request()
    );
    assert_eq!(status, 200);
    assert!(
        created["tokenValue"].is_string(),
        "create must return tokenValue"
    );
    assert_eq!(created["name"], "e2e-token");
    assert!(
        !created.to_string().contains("token_hash"),
        "create response leaked token_hash"
    );
    let id = created["id"].as_i64().expect("create returns id");

    let (status, list) = call!(
        app,
        test::TestRequest::get()
            .uri("/openapi/v1/user-tokens")
            .to_request()
    );
    assert_eq!(status, 200);
    let tokens = list.as_array().expect("list returns array");
    let entry = tokens
        .iter()
        .find(|t| t["id"].as_i64() == Some(id))
        .expect("new token listed");
    assert_eq!(entry["name"], "e2e-token");
    assert!(entry["expires"].is_string());
    assert!(
        !list.to_string().contains("token_hash"),
        "list response leaked token_hash"
    );

    let (status, rotated) = call!(
        app,
        test::TestRequest::post()
            .uri(&format!("/openapi/v1/user-tokens/{id}/rotate"))
            .to_request()
    );
    assert_eq!(status, 200, "upstream uses POST for rotate");
    assert!(
        rotated["tokenValue"].is_string(),
        "rotate re-issues the value"
    );
    assert_eq!(rotated["name"], "e2e-token", "rotate keeps the token name");

    let (status, _) = call!(
        app,
        test::TestRequest::post()
            .uri(&format!("/openapi/v1/user-tokens/{id}/revoke"))
            .to_request()
    );
    assert_eq!(status, 200, "upstream uses POST for revoke");
}

/// PMISC-012: `POST /users?isCreate=true` creates, plain `POST /users` updates,
/// and the list payload is keyed by `username` (upstream `UserPO`).
#[actix_web::test]
async fn users_is_create_flag_and_username_contract() {
    let app = make_app!();
    let username = format!("u-{}", chrono::Utc::now().timestamp_millis());

    let payload = json!({
        "username": username,
        "password": "Passw0rd!",
        "email": "u@batata.local",
        "enabled": true,
    });

    let (status, _) = call!(
        app,
        test::TestRequest::post()
            .uri("/openapi/v1/users?isCreate=true")
            .set_json(payload.clone())
            .to_request()
    );
    assert_eq!(status, 200, "create with isCreate=true");

    let (_, users) = call!(
        app,
        test::TestRequest::get()
            .uri("/openapi/v1/users")
            .to_request()
    );
    let entry = users
        .as_array()
        .expect("users is an array")
        .iter()
        .find(|u| u["username"] == username)
        .unwrap_or_else(|| panic!("created user must be listed: {users:?}"));
    assert_eq!(entry["enabled"], true);
    assert!(entry.get("userId").is_none(), "list must use username");

    // Duplicate creation is rejected by UserPersistence.
    let (status, _) = call!(
        app,
        test::TestRequest::post()
            .uri("/openapi/v1/users?isCreate=true")
            .set_json(payload.clone())
            .to_request()
    );
    assert_eq!(status, 400, "duplicate user must be rejected");

    // Without isCreate the same endpoint updates instead of erroring.
    let (status, _) = call!(
        app,
        test::TestRequest::post()
            .uri("/openapi/v1/users")
            .set_json(json!({
                "username": username,
                "password": "NewPassw0rd!",
                "enabled": true,
            }))
            .to_request()
    );
    assert_eq!(status, 200, "POST /users without isCreate updates");

    let (status, _) = call!(
        app,
        test::TestRequest::put()
            .uri("/openapi/v1/users/enabled")
            .set_json(json!({ "username": username, "enabled": false }))
            .to_request()
    );
    assert_eq!(status, 200, "PUT /users/enabled");

    let (_, users) = call!(
        app,
        test::TestRequest::get()
            .uri("/openapi/v1/users")
            .to_request()
    );
    let entry = users
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["username"] == username)
        .expect("user still listed");
    assert_eq!(entry["enabled"], false, "enabled change must persist");

    let (status, _) = call!(
        app,
        test::TestRequest::delete()
            .uri(&format!("/openapi/v1/users/{username}"))
            .to_request()
    );
    assert_eq!(status, 200);
}

/// The UI uses upstream's `ns_roles` spelling; assignments must actually be
/// persisted via `RoleService` rather than answered by an empty stub.
#[actix_web::test]
async fn cluster_ns_roles_alias_assigns_and_revokes_real_members() {
    let app = make_app!();
    let base = "/openapi/v1/apps/a1/envs/DEV/clusters/c1";
    let user = "e2e-role-user";

    let (status, body) = call!(
        app,
        test::TestRequest::get()
            .uri(&format!("{base}/ns_roles/ModifyNamespacesInCluster"))
            .to_request()
    );
    assert_eq!(status, 200, "ns_roles GET must not 404");
    assert!(body.is_array(), "role members are a bare array");

    let (status, _) = call!(
        app,
        test::TestRequest::post()
            .uri(&format!("{base}/ns_roles/ModifyNamespacesInCluster"))
            .set_json(json!(user))
            .to_request()
    );
    assert_eq!(status, 200, "assign must succeed");

    // Both spellings must resolve to the same role.
    for path in ["ns_roles", "roles"] {
        let (status, body) = call!(
            app,
            test::TestRequest::get()
                .uri(&format!("{base}/{path}/ModifyNamespacesInCluster"))
                .to_request()
        );
        assert_eq!(status, 200, "GET {path}");
        let members = body.as_array().expect("array of members");
        assert!(
            members.iter().any(|m| m["userId"] == user),
            "assigned user must be listed on {path}: {body:?}"
        );
    }

    let (status, _) = call!(
        app,
        test::TestRequest::delete()
            .uri(&format!(
                "{base}/ns_roles/ModifyNamespacesInCluster?user={user}"
            ))
            .to_request()
    );
    assert_eq!(status, 200, "revoke must succeed");

    let (_, body) = call!(
        app,
        test::TestRequest::get()
            .uri(&format!("{base}/ns_roles/ModifyNamespacesInCluster"))
            .to_request()
    );
    assert!(
        !body.as_array().unwrap().iter().any(|m| m["userId"] == user),
        "revoked user must disappear: {body:?}"
    );
}
