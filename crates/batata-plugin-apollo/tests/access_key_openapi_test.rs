mod common;

use std::sync::Arc;

use actix_web::{App, http::StatusCode, test, web};
use batata_plugin_apollo::api::dto::{AccessKeyDTO, ConsumerDTO, RoleDTO, UserDTO};
use batata_plugin_apollo::auth::permission::permission_type_ids;
use batata_plugin_apollo::model::config::AuthConfig;
use batata_plugin_apollo::persistence::traits::{
    AccessKeyPersistence, ApolloPersistenceService, ConsumerPersistence, ConsumerRolePersistence,
    ConsumerTokenPersistence, PermissionPersistence, RolePersistence, UserPersistence,
};
use batata_plugin_apollo::service::{AccessKeyService, UserTokenService};

fn auth_config() -> web::Data<AuthConfig> {
    web::Data::new(AuthConfig {
        openapi_auth_enabled: true,
        ..AuthConfig::default()
    })
}

fn access_keys_uri(app_id: &str) -> String {
    format!("/openapi/v1/apps/{app_id}/envs/DEV/accesskeys")
}

async fn make_consumer_with_app_admin(
    p: Arc<dyn ApolloPersistenceService>,
    app_id: &str,
) -> (i64, String) {
    let consumer = ConsumerPersistence::create_consumer(
        &p,
        ConsumerDTO {
            id: None,
            app_id: app_id.to_string(),
            name: "access-key consumer".into(),
            org_id: "org".into(),
            org_name: "org".into(),
            owner_name: "owner".into(),
            owner_email: "owner@example.com".into(),
            data_change_created_by: Some("tester".into()),
            data_change_created_time: None,
        },
    )
    .await
    .unwrap();
    let token = ConsumerTokenPersistence::create_consumer_token(&p, consumer.id, "tester")
        .await
        .unwrap()
        .token;

    let role = RolePersistence::create_role(
        &p,
        RoleDTO {
            id: None,
            role_name: format!("AssignRole+{app_id}"),
            role_type: 0,
            target_id: app_id.to_string(),
            data_change_created_by: Some("tester".into()),
            data_change_created_time: None,
        },
    )
    .await
    .unwrap();
    let permission = PermissionPersistence::create_permission(
        &p,
        permission_type_ids::ASSIGN_ROLE,
        app_id,
        "tester",
    )
    .await
    .unwrap();
    RolePersistence::assign_role_permission(&p, role.id, permission.id, "tester")
        .await
        .unwrap();
    ConsumerRolePersistence::assign_role_to_consumer(&p, consumer.id, role.id, "tester")
        .await
        .unwrap();
    (consumer.id, token)
}

#[actix_web::test]
async fn pat_env_scope_controls_access_key_list() {
    let p = common::make_embedded_persistence().await;
    let app_id = format!("ak-list-{}", chrono::Utc::now().timestamp_millis());
    let tokens = UserTokenService::new(p.clone());
    let (pro_only, _) = tokens
        .create_with_scope(
            "apollo",
            "pro-only",
            "apollo",
            None,
            r#"{"operations":["app:manage-role"],"envs":["PRO"]}"#,
        )
        .await
        .unwrap();
    let (dev_only, _) = tokens
        .create_with_scope(
            "apollo",
            "dev-only",
            "apollo",
            None,
            r#"{"operations":["app:manage-role"],"envs":["DEV"]}"#,
        )
        .await
        .unwrap();

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(p))
            .app_data(auth_config())
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let denied = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&access_keys_uri(&app_id))
            .insert_header(("X-Apollo-Token", pro_only))
            .to_request(),
    )
    .await;
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);

    let allowed = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&access_keys_uri(&app_id))
            .insert_header(("X-Apollo-Token", dev_only))
            .to_request(),
    )
    .await;
    assert_eq!(allowed.status(), StatusCode::OK);
    let _: Vec<AccessKeyDTO> = test::read_body_json(allowed).await;
}

#[actix_web::test]
async fn pat_without_manage_role_operation_cannot_list_access_keys() {
    let p = common::make_embedded_persistence().await;
    let app_id = format!("ak-forbidden-{}", chrono::Utc::now().timestamp_millis());
    let (token, _) = UserTokenService::new(p.clone())
        .create_with_scope(
            "apollo",
            "read-only",
            "apollo",
            None,
            r#"{"operations":["config:read"],"envs":["DEV"]}"#,
        )
        .await
        .unwrap();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(p))
            .app_data(auth_config())
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&access_keys_uri(&app_id))
            .insert_header(("X-Apollo-Token", token))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[actix_web::test]
async fn consumer_create_access_key_uses_operator_query() {
    let p = common::make_embedded_persistence().await;
    let app_id = format!("ak-create-{}", chrono::Utc::now().timestamp_millis());
    let (_, token) = make_consumer_with_app_admin(p.clone(), &app_id).await;
    UserPersistence::create_user(
        &p,
        UserDTO {
            id: None,
            username: "api-operator".into(),
            password: "a".repeat(64),
            email: None,
            enabled: true,
            data_change_created_by: Some("tester".into()),
            data_change_created_time: None,
        },
    )
    .await
    .unwrap();

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(p.clone()))
            .app_data(auth_config())
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let missing = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&access_keys_uri(&app_id))
            .insert_header(("Authorization", token.clone()))
            .to_request(),
    )
    .await;
    assert_eq!(missing.status(), StatusCode::BAD_REQUEST);

    let created = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!(
                "{}?operator=api-operator",
                access_keys_uri(&app_id)
            ))
            .insert_header(("Authorization", token))
            .to_request(),
    )
    .await;
    assert_eq!(created.status(), StatusCode::OK);
    let key: AccessKeyDTO = test::read_body_json(created).await;
    assert_eq!(key.data_change_created_by.as_deref(), Some("api-operator"));
    assert_eq!(key.app_id, app_id);
}

#[actix_web::test]
async fn pat_activation_uses_current_user_and_mode() {
    let p = common::make_embedded_persistence().await;
    let app_id = format!("ak-enable-{}", chrono::Utc::now().timestamp_millis());
    let (token, _) = UserTokenService::new(p.clone())
        .create_with_scope(
            "apollo",
            "manage-role",
            "apollo",
            None,
            r#"{"operations":["app:manage-role"],"envs":["DEV"]}"#,
        )
        .await
        .unwrap();
    let created = AccessKeyService::new(p.clone())
        .create(&app_id, "seed")
        .await
        .unwrap();
    let id = created.id.unwrap();

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(p.clone()))
            .app_data(auth_config())
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    let default_mode = test::call_service(
        &app,
        test::TestRequest::put()
            .uri(&format!(
                "/openapi/v1/apps/{app_id}/envs/DEV/accesskeys/{id}/activation"
            ))
            .insert_header(("X-Apollo-Token", token.clone()))
            .to_request(),
    )
    .await;
    assert_eq!(default_mode.status(), StatusCode::OK);
    let keys = AccessKeyPersistence::get_by_app(&*p, &app_id)
        .await
        .unwrap();
    let stored = keys.iter().find(|key| key.id == id).unwrap();
    assert!(stored.is_enabled);
    assert_eq!(stored.mode, 0);
    assert_eq!(
        stored.data_change_last_modified_by.as_deref(),
        Some("apollo")
    );

    let observer_mode = test::call_service(
        &app,
        test::TestRequest::put()
            .uri(&format!(
                "/openapi/v1/apps/{app_id}/envs/DEV/accesskeys/{id}/activation?mode=1"
            ))
            .insert_header(("X-Apollo-Token", token))
            .to_request(),
    )
    .await;
    assert_eq!(observer_mode.status(), StatusCode::OK);
    let keys = AccessKeyPersistence::get_by_app(&*p, &app_id)
        .await
        .unwrap();
    let stored = keys.iter().find(|key| key.id == id).unwrap();
    assert_eq!(stored.mode, 1);
}

#[actix_web::test]
async fn consumer_deactivation_requires_operator() {
    let p = common::make_embedded_persistence().await;
    let app_id = format!("ak-disable-{}", chrono::Utc::now().timestamp_millis());
    let (_, token) = make_consumer_with_app_admin(p.clone(), &app_id).await;
    UserPersistence::create_user(
        &p,
        UserDTO {
            id: None,
            username: "api-operator".into(),
            password: "a".repeat(64),
            email: None,
            enabled: true,
            data_change_created_by: Some("tester".into()),
            data_change_created_time: None,
        },
    )
    .await
    .unwrap();
    let created = AccessKeyService::new(p.clone())
        .create(&app_id, "seed")
        .await
        .unwrap();
    let id = created.id.unwrap();

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(p.clone()))
            .app_data(auth_config())
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;
    let uri = format!("/openapi/v1/apps/{app_id}/envs/DEV/accesskeys/{id}/deactivation");

    let missing = test::call_service(
        &app,
        test::TestRequest::put()
            .uri(&uri)
            .insert_header(("Authorization", token.clone()))
            .to_request(),
    )
    .await;
    assert_eq!(missing.status(), StatusCode::BAD_REQUEST);

    let allowed = test::call_service(
        &app,
        test::TestRequest::put()
            .uri(&format!("{uri}?operator=api-operator"))
            .insert_header(("Authorization", token))
            .to_request(),
    )
    .await;
    assert_eq!(allowed.status(), StatusCode::OK);
    // The embedded store hides disabled keys from `get_by_app`, mirroring
    // upstream's effective "not usable" state after deactivation.
    let keys = AccessKeyPersistence::get_by_app(&*p, &app_id)
        .await
        .unwrap();
    assert!(keys.iter().all(|key| key.id != id));
}
