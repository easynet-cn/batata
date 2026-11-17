mod common;

use actix_web::{App, http::StatusCode, test, web};
use batata_plugin_apollo::model::config::AuthConfig;
use batata_plugin_apollo::service::{AuditLogService, UserTokenService};
use serde_json::{Value, json};

fn auth_config(openapi_auth_enabled: bool) -> web::Data<AuthConfig> {
    web::Data::new(AuthConfig {
        openapi_auth_enabled,
        ..AuthConfig::default()
    })
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
