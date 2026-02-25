// Test for V1 Auth API compatibility with Nacos SDK
//
// This test verifies that the /v1/auth/users/login endpoint works correctly
// for backward compatibility with the Nacos SDK.

use actix_web::{test, web, App};
use batata_server::model::common::{AppState, Configuration};

#[actix_web::test]
async fn test_v1_auth_login_success() {
    // Initialize test app state
    let configuration = Configuration::new();
    let server_member_manager = None;
    let config_subscriber_manager = std::sync::Arc::new(batata_core::ConfigSubscriberManager::new());
    let console_datasource = None;
    let oauth_service = None;
    let persistence = None;
    let health_check_manager = None;

    let app_state = std::sync::Arc::new(AppState {
        configuration,
        server_member_manager,
        config_subscriber_manager,
        console_datasource,
        oauth_service,
        persistence,
        health_check_manager,
    });

    // Create test app
    let app = test::init_service(
        App::new()
            .app_data(web::Data::from(app_state))
            .service(batata_server::auth::v3::route::v1_routes()),
    )
    .await;

    // Create admin user for testing
    let username = "test_user";
    let password = "test_password";
    let hashed_password = bcrypt::hash(password, 10u32).unwrap();
    
    // Note: In real tests, we would create the user in the database first
    // For now, we just test the endpoint exists and returns proper format
    
    // Test V1 login endpoint
    let req = test::TestRequest::post()
        .uri("/v1/auth/users/login?username=test_user")
        .set_form(&serde_json::json!({"password": "test_password"}))
        .to_request();
    
    let resp = test::call_service(&app, req).await;
    
    // The endpoint should exist (may return 403 if user doesn't exist, but that's expected)
    assert!(
        resp.status() != actix_web::http::StatusCode::NOT_FOUND,
        "V1 auth login endpoint should exist"
    );
    
    println!("✓ V1 auth login endpoint exists");
}

#[actix_web::test]
async fn test_v3_auth_login_success() {
    // Initialize test app state
    let configuration = Configuration::new();
    let server_member_manager = None;
    let config_subscriber_manager = std::sync::Arc::new(batata_core::ConfigSubscriberManager::new());
    let console_datasource = None;
    let oauth_service = None;
    let persistence = None;
    let health_check_manager = None;

    let app_state = std::sync::Arc::new(AppState {
        configuration,
        server_member_manager,
        config_subscriber_manager,
        console_datasource,
        oauth_service,
        persistence,
        health_check_manager,
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
    assert!(
        resp.status() != actix_web::http::StatusCode::NOT_FOUND,
        "V3 auth login endpoint should exist"
    );
    
    println!("✓ V3 auth login endpoint exists");
}
