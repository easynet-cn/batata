// Test for V1 Auth API compatibility with Nacos SDK
//
// This test verifies that the /v1/auth/users/login endpoint works correctly
// for backward compatibility with the Nacos SDK.

use actix_web::{test, web, App};
use batata_server::model::common::{AppState, Configuration};

#[actix_web::test]
async fn test_v1_auth_login_success() {
    // Initialize test app state
    let configuration = Configuration::new().expect("Failed to load test configuration");
    let cluster_manager = None;
    let config_subscriber_manager: std::sync::Arc<dyn batata_common::ConfigSubscriptionService> = std::sync::Arc::new(batata_core::ConfigSubscriberManager::new());
    let console_datasource = None;
    let oauth_service = None;
    let persistence = None;
    let health_check_manager = None;

    let app_state = std::sync::Arc::new(AppState {
        configuration,
        cluster_manager,
        config_subscriber_manager,
        console_datasource,
        oauth_service,
        auth_plugin: None,
        persistence,
        health_check_manager,
        raft_node: None,
        server_status: std::sync::Arc::new(batata_server_common::ServerStatusManager::new()),
        control_plugin: None,
        encryption_service: None,
        plugin_state_providers: vec![],
    });

    // Create test app
    let app = test::init_service(
        App::new()
            .app_data(web::Data::from(app_state))
            .service(batata_server::auth::v3::route::v1_routes()),
    )
    .await;

    // Note: In real tests, we would create the user in the database first
    // For now, we just test the endpoint exists and returns proper format

    // Test V1 login endpoint
    let req = test::TestRequest::post()
        .uri("/v1/auth/users/login?username=test_user")
        .set_form(&serde_json::json!({"password": "test_password"}))
        .to_request();

    let resp = test::call_service(&app, req).await;

    // The endpoint should exist (may return 403 if user doesn't exist, but that's expected)
    let status = resp.status();
    assert_ne!(
        status,
        actix_web::http::StatusCode::NOT_FOUND,
        "V1 auth login endpoint should exist"
    );
    // Without a real user, we expect 403 Forbidden
    assert_eq!(
        status,
        actix_web::http::StatusCode::FORBIDDEN,
        "V1 auth login with invalid user should return 403, got {}",
        status,
    );
}

#[actix_web::test]
async fn test_v3_auth_login_success() {
    // Initialize test app state
    let configuration = Configuration::new().expect("Failed to load test configuration");
    let cluster_manager = None;
    let config_subscriber_manager: std::sync::Arc<dyn batata_common::ConfigSubscriptionService> = std::sync::Arc::new(batata_core::ConfigSubscriberManager::new());
    let console_datasource = None;
    let oauth_service = None;
    let persistence = None;
    let health_check_manager = None;

    let app_state = std::sync::Arc::new(AppState {
        configuration,
        cluster_manager,
        config_subscriber_manager,
        console_datasource,
        oauth_service,
        auth_plugin: None,
        persistence,
        health_check_manager,
        raft_node: None,
        server_status: std::sync::Arc::new(batata_server_common::ServerStatusManager::new()),
        control_plugin: None,
        encryption_service: None,
        plugin_state_providers: vec![],
    });

    // Create test app
    let app = test::init_service(
        App::new()
            .app_data(web::Data::from(app_state))
            .service(batata_server::auth::v3::route::routes()),
    )
    .await;

    // Test V3 login endpoint
    let req = test::TestRequest::post()
        .uri("/v3/auth/user/login?username=test_user")
        .set_form(&serde_json::json!({"password": "test_password"}))
        .to_request();
    
    let resp = test::call_service(&app, req).await;
    
    // The endpoint should exist
    let status = resp.status();
    assert_ne!(
        status,
        actix_web::http::StatusCode::NOT_FOUND,
        "V3 auth login endpoint should exist"
    );
    // Without a real user, we expect 403 Forbidden
    assert_eq!(
        status,
        actix_web::http::StatusCode::FORBIDDEN,
        "V3 auth login with invalid user should return 403, got {}",
        status,
    );
}
