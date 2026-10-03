//! Route-level tests for portal extension endpoints: audit logs, consumer
//! extensions, user tokens, server config, and the config supplementary
//! endpoints `/services/meta` and `/notifications`.

mod common;

use actix_web::{test, web, App};
use batata_plugin_apollo::api::dto::{AuditDTO, ConsumerDTO};
use batata_plugin_apollo::persistence::traits::{
    AuditPersistence, ConsumerPersistence, ConsumerTokenPersistence,
};

// ===== Audit log extension endpoints =====

#[actix_web::test]
async fn audit_properties_returns_entity_types() {
    let p = common::make_embedded_persistence().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(p))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/openapi/v1/apollo/audit/properties")
            .to_request(),
    )
    .await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    let arr = body.as_array().expect("properties returns an array");
    assert!(arr.contains(&serde_json::json!("APP")));
    assert!(arr.contains(&serde_json::json!("NAMESPACE")));
}

#[actix_web::test]
async fn audit_logs_returns_paged_envelope() {
    let p = common::make_embedded_persistence().await;
    AuditPersistence::create_audit(
        &p,
        AuditDTO {
            id: None,
            audit_key: "k1".into(),
            entity_name: "app".into(),
            entity_id: "1".into(),
            op_name: "create".into(),
            op_time: "2026-01-01T00:00:00".into(),
            op_by: "tester".into(),
            op_client_ip: "127.0.0.1".into(),
            detail: Some("d".into()),
            data_change_created_by: Some("tester".into()),
        },
    )
    .await
    .unwrap();

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(p))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/openapi/v1/logs?page=0&size=20")
            .to_request(),
    )
    .await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["page"], 0);
    assert_eq!(body["size"], 20);
    assert!(body["total"].as_u64().unwrap() >= 1);
    assert!(!body["content"].as_array().unwrap().is_empty());
}

#[actix_web::test]
async fn audit_logs_op_name_returns_envelope() {
    let p = common::make_embedded_persistence().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(p))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/openapi/v1/logs/opName?page=0&size=10")
            .to_request(),
    )
    .await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.get("content").is_some());
    assert!(body.get("total").is_some());
}

#[actix_web::test]
async fn audit_trace_returns_array() {
    let p = common::make_embedded_persistence().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(p))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/openapi/v1/trace?traceId=abc")
            .to_request(),
    )
    .await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.is_array());
}

#[actix_web::test]
async fn audit_logs_field_returns_envelope() {
    let p = common::make_embedded_persistence().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(p))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/openapi/v1/logs/dataInfluences/field?page=0&size=10")
            .to_request(),
    )
    .await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.get("content").is_some());
}

#[actix_web::test]
async fn audit_logs_search_returns_envelope() {
    let p = common::make_embedded_persistence().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(p))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/openapi/v1/logs/by-name-or-type-or-operator?page=0&size=10")
            .to_request(),
    )
    .await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.get("content").is_some());
}

// ===== Consumer extension endpoints =====

#[actix_web::test]
async fn consumer_by_app_id_returns_consumer() {
    let p = common::make_embedded_persistence().await;
    let app_id = format!("cons-{}", chrono::Utc::now().timestamp_millis());
    ConsumerPersistence::create_consumer(
        &p,
        ConsumerDTO {
            id: None,
            app_id: app_id.clone(),
            name: "Test Consumer".into(),
            org_id: "org1".into(),
            org_name: "Org1".into(),
            owner_name: "owner".into(),
            owner_email: "owner@test.com".into(),
            data_change_created_by: Some("tester".into()),
            data_change_created_time: None,
        },
    )
    .await
    .unwrap();

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(p))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/openapi/v1/consumers/by-appId?appId={}", app_id))
            .to_request(),
    )
    .await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["appId"], app_id);
}

#[actix_web::test]
async fn consumer_by_app_id_not_found() {
    let p = common::make_embedded_persistence().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(p))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/openapi/v1/consumers/by-appId?appId=nonexistent")
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), 404);
}

#[actix_web::test]
async fn consumer_tokens_by_app_id_returns_tokens() {
    let p = common::make_embedded_persistence().await;
    let app_id = format!("constok-{}", chrono::Utc::now().timestamp_millis());
    let consumer = ConsumerPersistence::create_consumer(
        &p,
        ConsumerDTO {
            id: None,
            app_id: app_id.clone(),
            name: "Token Consumer".into(),
            org_id: "org1".into(),
            org_name: "Org1".into(),
            owner_name: "owner".into(),
            owner_email: "owner@test.com".into(),
            data_change_created_by: Some("tester".into()),
            data_change_created_time: None,
        },
    )
    .await
    .unwrap();
    ConsumerTokenPersistence::create_consumer_token(&p, consumer.id, "tester")
        .await
        .unwrap();

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(p))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/openapi/v1/consumer-tokens/by-appId?appId={}", app_id))
            .to_request(),
    )
    .await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    let arr = body.as_array().expect("consumer tokens returns an array");
    assert_eq!(arr.len(), 1);
}

#[actix_web::test]
async fn consumer_tokens_by_app_id_empty_for_missing() {
    let p = common::make_embedded_persistence().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(p))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/openapi/v1/consumer-tokens/by-appId?appId=missing")
            .to_request(),
    )
    .await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.as_array().unwrap().is_empty());
}

#[actix_web::test]
async fn assign_consumer_role_returns_ok() {
    let p = common::make_embedded_persistence().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(p))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/openapi/v1/consumers/sometoken/assign-role")
            .set_json(serde_json::json!({ "role": "Admin" }))
            .to_request(),
    )
    .await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["status"], "ok");
}

// ===== User token endpoints =====

#[actix_web::test]
async fn user_tokens_create_and_list() {
    let p = common::make_embedded_persistence().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(p))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    // create
    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/openapi/v1/user-tokens")
            .set_json(serde_json::json!({ "description": "my token" }))
            .to_request(),
    )
    .await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.get("token").is_some(), "create returns plaintext token");
    assert!(body.get("model").is_some());
    let token_id = body["model"]["id"].as_i64().unwrap();

    // list contains the new token
    let resp = test::call_service(
        &app,
        test::TestRequest::get().uri("/openapi/v1/user-tokens").to_request(),
    )
    .await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    let arr = body.as_array().expect("list returns array");
    assert!(arr.iter().any(|t| t["id"].as_i64() == Some(token_id)));
}

#[actix_web::test]
async fn user_tokens_revoke() {
    let p = common::make_embedded_persistence().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(p))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/openapi/v1/user-tokens")
            .set_json(serde_json::json!({ "description": "revoke me" }))
            .to_request(),
    )
    .await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    let token_id = body["model"]["id"].as_i64().unwrap();

    let resp = test::call_service(
        &app,
        test::TestRequest::put()
            .uri(&format!("/openapi/v1/user-tokens/{}/revoke", token_id))
            .to_request(),
    )
    .await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["status"], "ok");
}

#[actix_web::test]
async fn user_tokens_rotate() {
    let p = common::make_embedded_persistence().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(p))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/openapi/v1/user-tokens")
            .set_json(serde_json::json!({ "description": "rotate me" }))
            .to_request(),
    )
    .await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    let token_id = body["model"]["id"].as_i64().unwrap();

    let resp = test::call_service(
        &app,
        test::TestRequest::put()
            .uri(&format!("/openapi/v1/user-tokens/{}/rotate", token_id))
            .to_request(),
    )
    .await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.get("token").is_some());
    assert!(body.get("model").is_some());
}

#[actix_web::test]
async fn user_tokens_delete() {
    let p = common::make_embedded_persistence().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(p))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/openapi/v1/user-tokens")
            .set_json(serde_json::json!({ "description": "delete me" }))
            .to_request(),
    )
    .await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    let token_id = body["model"]["id"].as_i64().unwrap();

    let resp = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri(&format!("/openapi/v1/user-tokens/{}", token_id))
            .to_request(),
    )
    .await;
    assert!(resp.status().is_success());
}

#[actix_web::test]
async fn user_tokens_capabilities() {
    let p = common::make_embedded_persistence().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(p))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/openapi/v1/user-tokens/capabilities")
            .to_request(),
    )
    .await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["tokenSupported"], true);
    assert_eq!(body["tokenRotationSupported"], true);
}

#[actix_web::test]
async fn admin_user_tokens_crud() {
    let p = common::make_embedded_persistence().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(p))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    // create via admin endpoint
    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/openapi/v1/users/apollo/tokens")
            .set_json(serde_json::json!({ "userId": "apollo", "description": "admin token" }))
            .to_request(),
    )
    .await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.get("token").is_some());
    let token_id = body["model"]["id"].as_i64().unwrap();

    // list
    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/openapi/v1/users/apollo/tokens")
            .to_request(),
    )
    .await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["id"].as_i64() == Some(token_id)));

    // revoke
    let resp = test::call_service(
        &app,
        test::TestRequest::put()
            .uri(&format!("/openapi/v1/users/apollo/tokens/{}/revoke", token_id))
            .to_request(),
    )
    .await;
    assert!(resp.status().is_success());
}

// ===== Server config endpoints =====

#[actix_web::test]
async fn server_portal_db_config_returns_empty() {
    let p = common::make_embedded_persistence().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(p))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/openapi/v1/server/portal-db/config")
            .to_request(),
    )
    .await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.is_object());
}

#[actix_web::test]
async fn server_portal_db_config_find_all_returns_empty() {
    let p = common::make_embedded_persistence().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(p))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/openapi/v1/server/portal-db/config/find-all")
            .to_request(),
    )
    .await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["content"], serde_json::json!([]));
    assert_eq!(body["total"], 0);
}

#[actix_web::test]
async fn server_env_config_db_config_returns_empty() {
    let p = common::make_embedded_persistence().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(p))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/openapi/v1/server/envs/DEV/config-db/config")
            .to_request(),
    )
    .await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.is_object());
}

#[actix_web::test]
async fn server_env_config_db_config_find_all_returns_empty() {
    let p = common::make_embedded_persistence().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(p))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/openapi/v1/server/envs/DEV/config-db/config/find-all")
            .to_request(),
    )
    .await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["content"], serde_json::json!([]));
}

// ===== Config supplementary endpoints =====

#[actix_web::test]
async fn services_meta_returns_configservice_instances() {
    let p = common::make_embedded_persistence().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(p))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/services/meta")
            .insert_header(("host", "meta:8080"))
            .to_request(),
    )
    .await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    let arr = body.as_array().expect("services/meta returns an array");
    assert!(!arr.is_empty());
    assert_eq!(arr[0]["appName"], "apollo-configservice");
}

#[actix_web::test]
async fn notifications_v1_returns_not_modified_without_namespaces() {
    let p = common::make_embedded_persistence().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(p))
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/notifications?appId=app1&cluster=default&namespaceName=")
            .to_request(),
    )
    .await;
    // v1 returns 304 NotModified when no namespace names are provided
    assert_eq!(resp.status(), 304);
}
