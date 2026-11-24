mod common;

use actix_web::{App, http::StatusCode, test, web};
use batata_common::datetime::{DateTimeExt, Duration, NaiveDateTimeExt, Utc};
use batata_plugin_apollo::api::dto::UserDTO;
use batata_plugin_apollo::model::config::AuthConfig;
use batata_plugin_apollo::service::{
    AuditLogService, UserService, UserTokenPolicy, UserTokenService,
};
use serde_json::{Value, json};

fn auth_config(openapi_auth_enabled: bool) -> web::Data<AuthConfig> {
    web::Data::new(AuthConfig {
        openapi_auth_enabled,
        ..AuthConfig::default()
    })
}

fn portal_user(username: &str) -> UserDTO {
    UserDTO {
        id: None,
        username: username.to_string(),
        password: "pw".to_string(),
        email: None,
        enabled: true,
        data_change_created_by: None,
        data_change_created_time: None,
    }
}

#[tokio::test]
async fn password_login_issues_pat_that_works_for_ui_and_current_endpoints() {
    let persistence = common::make_embedded_persistence().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(persistence.clone()))
            .app_data(auth_config(true))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/openapi/v1/user/login")
            .set_json(json!({ "username": "apollo", "password": "apollo" }))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let token = body["tokenValue"]
        .as_str()
        .expect("login response must contain the plaintext token");
    assert!(
        token.starts_with("apollo_pat_"),
        "expected an upstream-style PAT, got {token}"
    );
    assert_eq!(body["userId"], "apollo");
    assert_eq!(body["authType"], "USER_TOKEN");

    for uri in [
        "/openapi/v1/user-tokens/current",
        "/openapi/v1/user-tokens/current/capabilities",
        "/openapi/v1/user-tokens/whoami",
    ] {
        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(uri)
                .insert_header(("X-Apollo-Token", token))
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK, "{uri}");
        let body: Value = test::read_body_json(response).await;
        assert_eq!(body["authType"], "USER_TOKEN");
        assert_eq!(body["userId"], "apollo");
        assert_eq!(body["denyAll"], false);
        assert_eq!(body["allOperations"], true);
    }

    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/openapi/v1/users")
            .insert_header(("X-Apollo-Token", token))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let (logs, _) = AuditLogService::new(persistence)
        .list(None, None, None, None, 1, 100)
        .await
        .unwrap();
    assert!(
        logs.iter().any(|log| log.op_name == "IssueUserToken"),
        "login must be audited: {logs:?}"
    );
}

#[tokio::test]
async fn wrong_password_and_missing_token_are_rejected() {
    let persistence = common::make_embedded_persistence().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(persistence))
            .app_data(auth_config(true))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/openapi/v1/user/login")
            .set_json(json!({ "username": "apollo", "password": "wrong" }))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let result = test::try_call_service(
        &app,
        test::TestRequest::get()
            .uri("/openapi/v1/user-tokens/current")
            .to_request(),
    )
    .await;
    let error = result.expect_err("middleware must reject a missing user token");
    assert!(error.to_string().contains("user token"));
}

#[tokio::test]
async fn explicit_invalid_token_fails_even_when_global_auth_is_disabled() {
    let persistence = common::make_embedded_persistence().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(persistence))
            .app_data(auth_config(false))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let result = test::try_call_service(
        &app,
        test::TestRequest::get()
            .uri("/openapi/v1/apps")
            .insert_header(("X-Apollo-Token", "apollo_pat_deadbeef_deadbeef"))
            .to_request(),
    )
    .await;
    let error = result.expect_err("an explicitly presented token must be validated");
    assert!(error.to_string().contains("Invalid or expired user token"));

    // No credential at all keeps the legacy openapi behavior.
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/openapi/v1/apps")
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn pat_scope_is_anded_with_superadmin_rbac() {
    let persistence = common::make_embedded_persistence().await;
    let service = UserTokenService::new(persistence.clone());
    // The user is still Apollo's superadmin, but this PAT may only read config.
    let (token, _) = service
        .create_with_scope(
            "apollo",
            "read-only",
            "apollo",
            None,
            r#"{"operations":["config:read"]}"#,
        )
        .await
        .unwrap();

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(persistence))
            .app_data(auth_config(true))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/openapi/v1/users")
            .insert_header(("X-Apollo-Token", token))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn pat_scope_limits_env_cluster_and_namespace_on_data_paths() {
    let persistence = common::make_embedded_persistence().await;
    let service = UserTokenService::new(persistence.clone());
    let (token, _) = service
        .create_with_scope(
            "apollo",
            "dev-application",
            "apollo",
            None,
            r#"{"operations":["config:read","config:modify"],"envs":["DEV"],"namespaces":[{"appId":"app-a","namespaceName":"application"}]}"#,
        )
        .await
        .unwrap();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(persistence))
            .app_data(auth_config(true))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let base = "/openapi/v1/envs/{env}/apps/app-a/clusters/default/namespaces/{ns}/items";
    for (env, ns, method, forbidden) in [
        ("dev", "application", "GET", false),
        ("DEV", "application", "GET", false),
        ("PRO", "application", "GET", true),
        ("DEV", "other", "GET", true),
        ("DEV", "application", "PUT", false),
        ("PRO", "application", "PUT", true),
    ] {
        let uri = base.replace("{env}", env).replace("{ns}", ns);
        let request = match method {
            "PUT" => test::TestRequest::put()
                .uri(&uri)
                .insert_header(("X-Apollo-Token", token.clone()))
                .set_json(json!({}))
                .to_request(),
            _ => test::TestRequest::get()
                .uri(&uri)
                .insert_header(("X-Apollo-Token", token.clone()))
                .to_request(),
        };
        let result = test::try_call_service(&app, request).await;
        if forbidden {
            let error = result.expect_err("out-of-scope resource must be rejected by middleware");
            assert!(error.to_string().contains("scope"), "{error}");
        } else {
            // In-scope requests pass the middleware; handler may 404 when the
            // test database has no such app.
            let response =
                result.unwrap_or_else(|error| panic!("unexpected middleware error: {error}"));
            assert_ne!(response.status(), StatusCode::FORBIDDEN, "{method} {uri}");
        }
    }
}

#[tokio::test]
async fn pat_scope_limits_permission_targets_even_for_superadmin() {
    let persistence = common::make_embedded_persistence().await;
    let service = UserTokenService::new(persistence.clone());
    let (token, _) = service
        .create_with_scope(
            "apollo",
            "dev-only",
            "apollo",
            None,
            r#"{"operations":["config:modify"],"envs":["DEV"]}"#,
        )
        .await
        .unwrap();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(persistence))
            .app_data(auth_config(true))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let uri = |env: &str| {
        format!(
            "/openapi/v1/apps/app-a/envs/{env}/namespaces/application/permissions/ModifyNamespace"
        )
    };
    for (env, expected) in [("DEV", true), ("PRO", false), ("dev", true)] {
        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&uri(env))
                .insert_header(("X-Apollo-Token", token.clone()))
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK, "env={env}");
        let body: Value = test::read_body_json(response).await;
        assert_eq!(body["hasPermission"], expected, "env={env}");
    }
}

#[tokio::test]
async fn rate_limited_pat_returns_429_after_budget_is_exhausted() {
    let persistence = common::make_embedded_persistence().await;
    let service = UserTokenService::new(persistence.clone());
    let (token, model) = service
        .create_with_options("apollo", "limited", "apollo", None, "{}", 1)
        .await
        .unwrap();
    assert_eq!(model.rate_limit, 1);

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(persistence))
            .app_data(auth_config(true))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    for expected in ["user token", "Too Many Requests"] {
        let result = test::try_call_service(
            &app,
            test::TestRequest::get()
                .uri("/openapi/v1/user-tokens/whoami")
                .insert_header(("X-Apollo-Token", token.clone()))
                .to_request(),
        )
        .await;
        if expected == "user token" {
            assert!(result.is_ok(), "first request must be admitted");
        } else {
            let error = result.expect_err("exhausted rate limit must be rejected");
            assert!(error.to_string().contains(expected), "{error}");
        }
    }
}

#[tokio::test]
async fn authenticated_pat_refreshes_last_used_fields() {
    let persistence = common::make_embedded_persistence().await;
    let service = UserTokenService::new(persistence.clone());
    let (token, model) = service
        .create("apollo", "tracked", "apollo", None)
        .await
        .unwrap();
    assert!(model.last_used_time.is_none());

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(persistence))
            .app_data(auth_config(true))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/openapi/v1/user-tokens/whoami")
            .insert_header(("X-Apollo-Token", token))
            .insert_header(("User-Agent", "batata-test-agent"))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let tokens = service.list("apollo").await.unwrap();
    let updated = tokens.iter().find(|item| item.id == model.id).unwrap();
    assert!(updated.last_used_time.is_some());
    assert_eq!(
        updated.last_used_user_agent.as_deref(),
        Some("batata-test-agent")
    );
    assert_eq!(
        updated.data_change_last_modified_by.as_deref(),
        Some("apollo")
    );
}

#[tokio::test]
async fn authorization_bearer_is_accepted_as_a_user_token() {
    let persistence = common::make_embedded_persistence().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(persistence))
            .app_data(auth_config(true))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/openapi/v1/user/login")
            .set_json(json!({ "username": "apollo", "password": "apollo" }))
            .to_request(),
    )
    .await;
    let body: Value = test::read_body_json(response).await;
    let token = body["tokenValue"].as_str().unwrap();

    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/openapi/v1/user-tokens/whoami")
            .insert_header(("Authorization", format!("Bearer {token}")))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn placeholder_authorization_token_is_ignored_when_global_auth_is_disabled() {
    let persistence = common::make_embedded_persistence().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(persistence))
            .app_data(auth_config(false))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    // SDK suites send a placeholder consumer token ("admin") that has no row
    // in apollo_consumer_token. With OpenAPI auth off this must stay open.
    let response = test::try_call_service(
        &app,
        test::TestRequest::get()
            .uri("/openapi/v1/apps/app-a/namespaces/application/items")
            .insert_header(("Authorization", "admin"))
            .to_request(),
    )
    .await
    .expect("placeholder token must not be rejected when auth is disabled");
    assert_ne!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn disabling_the_owner_immediately_invalidates_an_issued_pat() {
    let persistence = common::make_embedded_persistence().await;
    let users = UserService::new(persistence.clone());
    users.create(portal_user("agent-owner")).await.unwrap();
    let (token, _) = UserTokenService::new(persistence.clone())
        .create("agent-owner", "owner-pat", "agent-owner", None)
        .await
        .unwrap();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(persistence))
            .app_data(auth_config(true))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/openapi/v1/user-tokens/whoami")
            .insert_header(("X-Apollo-Token", token.clone()))
            .to_request(),
    )
    .await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "enabled owner is admitted"
    );

    users.set_enabled("agent-owner", false).await.unwrap();
    let result = test::try_call_service(
        &app,
        test::TestRequest::get()
            .uri("/openapi/v1/user-tokens/whoami")
            .insert_header(("X-Apollo-Token", token))
            .to_request(),
    )
    .await;
    let error = result.expect_err("disabled owner must invalidate the token");
    assert!(error.to_string().contains("disabled"), "{error}");
}

#[tokio::test]
async fn created_token_is_bound_to_delegating_user_and_lists_real_scope() {
    let persistence = common::make_embedded_persistence().await;
    UserService::new(persistence.clone())
        .create(portal_user("agent-owner"))
        .await
        .unwrap();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(persistence))
            .app_data(auth_config(true))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/openapi/v1/user/login")
            .set_json(json!({ "username": "agent-owner", "password": "pw" }))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let login: Value = test::read_body_json(response).await;
    let parent = login["tokenValue"].as_str().unwrap();

    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/openapi/v1/user-tokens")
            .insert_header(("X-Apollo-Token", parent))
            .set_json(json!({
                "name": "child",
                "userId": "apollo",
                "scopes": "{\"operations\":[\"config:read\"],\"appIds\":[\"app-a\"]}"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let created: Value = test::read_body_json(response).await;
    assert_eq!(
        created["model"]["userId"], "agent-owner",
        "upstream binds the token to the delegating user, not the request body"
    );
    let child_id = created["model"]["id"].as_i64().unwrap();

    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/openapi/v1/user-tokens")
            .insert_header(("X-Apollo-Token", parent))
            .to_request(),
    )
    .await;
    let listed: Value = test::read_body_json(response).await;
    let child = listed
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["id"].as_i64() == Some(child_id))
        .expect("child token listed");
    assert_eq!(child["operations"][0], "config:read");
    assert_eq!(child["appIds"][0], "app-a");
}

#[tokio::test]
async fn rotation_preserves_scope_rate_limit_expiry_and_owner() {
    let persistence = common::make_embedded_persistence().await;
    let service = UserTokenService::new(persistence.clone());
    let expires = Utc::now() + Duration::days(10);
    let (old_token, old_model) = service
        .create_with_options(
            "apollo",
            "restricted",
            "apollo",
            Some(expires),
            r#"{"operations":["config:read"],"envs":["DEV"]}"#,
            3,
        )
        .await
        .unwrap();
    let persistence_check = persistence.clone();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(persistence))
            .app_data(auth_config(true))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/openapi/v1/user-tokens/{}/rotate", old_model.id))
            .insert_header(("X-Apollo-Token", old_token.clone()))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let rotated: Value = test::read_body_json(response).await;
    assert_eq!(rotated["model"]["userId"], "apollo");
    assert_eq!(rotated["model"]["name"], "restricted");
    assert_eq!(rotated["model"]["rateLimit"], 3);
    let new_id = rotated["model"]["id"].as_i64().unwrap();
    let new_token = rotated["tokenValue"].as_str().unwrap();
    let persisted = UserTokenService::new(persistence_check)
        .list("apollo")
        .await
        .unwrap()
        .into_iter()
        .find(|token| token.id == new_id)
        .expect("rotated token persisted");
    assert_eq!(
        persisted.expires, old_model.expires,
        "rotation must not refresh expiry"
    );
    assert_eq!(persisted.rate_limit, 3);
    assert_eq!(persisted.user_id, "apollo");
    assert_eq!(
        persisted.scopes.as_deref(),
        Some(r#"{"operations":["config:read"],"envs":["DEV"]}"#)
    );

    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/openapi/v1/user-tokens/current")
            .insert_header(("X-Apollo-Token", new_token))
            .to_request(),
    )
    .await;
    let current: Value = test::read_body_json(response).await;
    assert_eq!(current["operations"][0], "config:read");
    assert_eq!(current["rateLimit"], 3);
    assert_eq!(current["allOperations"], false);
    assert_eq!(current["allEnvs"], false);
    assert_eq!(current["envs"][0], "DEV");

    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/openapi/v1/users")
            .insert_header(("X-Apollo-Token", new_token))
            .to_request(),
    )
    .await;
    assert_eq!(
        response.status(),
        StatusCode::FORBIDDEN,
        "rotation must not grant user management"
    );

    let result = test::try_call_service(
        &app,
        test::TestRequest::get()
            .uri("/openapi/v1/user-tokens/whoami")
            .insert_header(("X-Apollo-Token", old_token))
            .to_request(),
    )
    .await;
    assert!(result.is_err(), "the rotated-out token must stop working");
}

#[tokio::test]
async fn a_token_cannot_revoke_another_users_token() {
    let persistence = common::make_embedded_persistence().await;
    let users = UserService::new(persistence.clone());
    users.create(portal_user("alice")).await.unwrap();
    users.create(portal_user("bob")).await.unwrap();
    let service = UserTokenService::new(persistence.clone());
    let (alice_token, alice_model) = service
        .create("alice", "alice-pat", "alice", None)
        .await
        .unwrap();
    let (bob_token, _) = service.create("bob", "bob-pat", "bob", None).await.unwrap();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(persistence))
            .app_data(auth_config(true))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!(
                "/openapi/v1/user-tokens/{}/revoke",
                alice_model.id
            ))
            .insert_header(("X-Apollo-Token", bob_token))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/openapi/v1/user-tokens/whoami")
            .insert_header(("X-Apollo-Token", alice_token))
            .to_request(),
    )
    .await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "the other owner's token stays active"
    );
}

#[tokio::test]
async fn rotation_rejects_an_inactive_token() {
    let persistence = common::make_embedded_persistence().await;
    let service = UserTokenService::new(persistence.clone());
    let (_revoked_token, model) = service
        .create("apollo", "revoked", "apollo", None)
        .await
        .unwrap();
    let (operator_token, _) = service
        .create("apollo", "operator", "apollo", None)
        .await
        .unwrap();
    service.revoke(model.id, "apollo").await.unwrap();
    // The revoked credential cannot authenticate; rotate through another PAT
    // of the same owner, as upstream would through a Portal session.
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(persistence))
            .app_data(auth_config(true))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/openapi/v1/user-tokens/{}/rotate", model.id))
            .insert_header(("X-Apollo-Token", operator_token))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let error: Value = test::read_body_json(response).await;
    assert_eq!(error["message"], "Token is not active");
}

#[tokio::test]
async fn capability_document_lists_only_actions_allowed_by_the_token() {
    let persistence = common::make_embedded_persistence().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(persistence.clone()))
            .app_data(auth_config(true))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/openapi/v1/user/login")
            .set_json(json!({ "username": "apollo", "password": "apollo" }))
            .to_request(),
    )
    .await;
    let login: Value = test::read_body_json(response).await;
    let admin_token = login["tokenValue"].as_str().unwrap();

    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/openapi/v1/user-tokens/capabilities")
            .insert_header(("X-Apollo-Token", admin_token))
            .to_request(),
    )
    .await;
    let options: Value = test::read_body_json(response).await;
    assert_eq!(options["tokenSupported"], true);
    assert_eq!(options["defaultExpireDays"], 90);
    assert_eq!(options["maxExpireDays"], 365);
    let operations = options["operations"].as_array().unwrap();
    for expected in [
        "config:read",
        "config:modify",
        "config:release",
        "app:create",
        "user:manage",
        "system:admin",
    ] {
        assert!(
            operations.iter().any(|op| op == expected),
            "missing {expected}"
        );
    }

    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/openapi/v1/user-tokens/whoami")
            .insert_header(("X-Apollo-Token", admin_token))
            .to_request(),
    )
    .await;
    let admin: Value = test::read_body_json(response).await;
    let actions = admin["actions"].as_array().expect("actions catalog");
    assert!(actions.len() >= 60, "catalog too small: {}", actions.len());
    let ids: Vec<&str> = actions.iter().map(|a| a["id"].as_str().unwrap()).collect();
    for expected in [
        "item.list",
        "release.create",
        "user.create",
        "access-key.create",
        "branch.merge",
    ] {
        assert!(ids.contains(&expected), "catalog missing {expected}");
    }
    for action in actions {
        assert!(action["method"].is_string());
        assert!(action["path"].as_str().unwrap().starts_with("/openapi/v1/"));
        assert!(action["grantedOperations"].is_array());
        assert!(action["resourceScope"].is_string());
        assert!(action["description"].is_string());
    }

    let restricted = UserTokenService::new(persistence)
        .create_with_scope(
            "apollo",
            "read-only",
            "apollo",
            None,
            r#"{"operations":["config:read"]}"#,
        )
        .await
        .unwrap()
        .0;
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/openapi/v1/user-tokens/whoami")
            .insert_header(("X-Apollo-Token", restricted))
            .to_request(),
    )
    .await;
    let limited: Value = test::read_body_json(response).await;
    let ids: Vec<&str> = limited["actions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&"item.list"));
    for forbidden in [
        "item.create",
        "release.create",
        "user.create",
        "access-key.create",
    ] {
        assert!(
            !ids.contains(&forbidden),
            "read token must not see {forbidden}"
        );
    }
}

#[tokio::test]
async fn token_expiry_policy_is_configurable_and_enforced() {
    let persistence = common::make_embedded_persistence().await;
    let default_policy = UserTokenPolicy::default();
    assert_eq!(default_policy.default_expire_days, 90);
    assert_eq!(default_policy.max_expire_days, 365);
    let (_, default_model) = UserTokenService::new(persistence.clone())
        .create("apollo", "default-expiry", "apollo", None)
        .await
        .unwrap();
    let default_now = Utc::now().timestamp();
    let default_expires = default_model.expires.and_utc().timestamp();
    assert!(
        default_expires >= default_now + 89 * 86_400
            && default_expires <= default_now + 91 * 86_400,
        "upstream default expiry is 90 days: {default_expires}"
    );

    let service = UserTokenService::with_policy(
        persistence.clone(),
        UserTokenPolicy {
            default_expire_days: 10,
            max_expire_days: 20,
        },
    );
    let (_, model) = service
        .create("apollo", "short", "apollo", None)
        .await
        .unwrap();
    let now = Utc::now().timestamp();
    let expires = model.expires.and_utc().timestamp();
    assert!(
        expires >= now + 9 * 86_400 && expires <= now + 11 * 86_400,
        "expires={expires}"
    );

    let error = service
        .create(
            "apollo",
            "too-long",
            "apollo",
            Some(Utc::now() + Duration::days(30)),
        )
        .await
        .unwrap_err();
    assert!(
        error.to_string().contains("exceeds max allowed days:20"),
        "{error}"
    );
    let error = service
        .create(
            "apollo",
            "past",
            "apollo",
            Some(Utc::now() - Duration::days(1)),
        )
        .await
        .unwrap_err();
    assert!(
        error.to_string().contains("must be in the future"),
        "{error}"
    );

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(persistence.clone()))
            .app_data(web::Data::new(AuthConfig {
                user_token_default_expire_days: 11,
                user_token_max_expire_days: 21,
                ..AuthConfig::default()
            }))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/openapi/v1/user-tokens")
            .set_json(json!({ "name": "configured" }))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let created: Value = test::read_body_json(response).await;
    let id = created["model"]["id"].as_i64().unwrap();
    let configured = service
        .list("apollo")
        .await
        .unwrap()
        .into_iter()
        .find(|token| token.id == id)
        .expect("configured token persisted");
    let configured_expiry = configured.expires.and_utc().timestamp();
    let now = Utc::now().timestamp();
    assert!(
        configured_expiry >= now + 10 * 86_400 && configured_expiry <= now + 12 * 86_400,
        "configured default was ignored: {configured_expiry}"
    );
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/openapi/v1/user-tokens")
            .to_request(),
    )
    .await;
    let listed: Value = test::read_body_json(response).await;
    let entry = listed
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"].as_i64() == Some(id))
        .unwrap();
    let expires_text = entry["expires"].as_str().unwrap();
    assert!(expires_text.contains("T"), "{expires_text}");
}
