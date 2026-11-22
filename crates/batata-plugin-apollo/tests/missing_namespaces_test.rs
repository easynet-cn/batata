use batata_common::datetime::DateTimeExt;
mod common;

use std::sync::Arc;

use actix_web::{App, http::StatusCode, test, web};
use batata_plugin_apollo::api::dto::{AppDTO, AppNamespaceDTO, ConsumerDTO, RoleDTO};
use batata_plugin_apollo::auth::permission::permission_type_ids;
use batata_plugin_apollo::model::config::AuthConfig;
use batata_plugin_apollo::persistence::traits::{
    ApolloPersistenceService, AppNamespacePersistence, ConsumerPersistence,
    ConsumerRolePersistence, ConsumerTokenPersistence, PermissionPersistence, RolePersistence,
};
use batata_plugin_apollo::service::{AppService, NamespaceService, UserTokenService};

fn auth_config() -> web::Data<AuthConfig> {
    web::Data::new(AuthConfig {
        openapi_auth_enabled: true,
        ..AuthConfig::default()
    })
}

async fn seed_namespaces(p: Arc<dyn ApolloPersistenceService>, app_id: &str) {
    AppService::new(p.clone())
        .create(AppDTO {
            app_id: app_id.to_string(),
            name: "Missing namespaces app".into(),
            org_id: "org".into(),
            org_name: "org".into(),
            owner_name: "tester".into(),
            owner_email: "tester@example.com".into(),
            data_change_created_by: Some("tester".into()),
            data_change_last_modified_by: None,
            data_change_created_time: None,
            data_change_last_time: None,
        })
        .await
        .unwrap();
    AppService::new(p.clone())
        .bootstrap_default_namespace(app_id, "tester")
        .await
        .unwrap();
    // Inserted in ascending name order so persistence's newest-first listing
    // returns `zeta, beta`; the handler's explicit sort must restore upstream order.
    for name in ["beta", "zeta"] {
        // Persistence-only seed: AppNamespaceService also instantiates the
        // namespace under each root cluster, which would leave nothing missing.
        AppNamespacePersistence::create_app_namespace(
            &p,
            AppNamespaceDTO {
                id: None,
                name: name.into(),
                app_id: app_id.to_string(),
                format: "properties".into(),
                is_public: false,
                comment: String::new(),
                data_change_created_by: Some("tester".into()),
                data_change_created_time: None,
            },
        )
        .await
        .unwrap();
    }
}

fn missing_uri(app_id: &str) -> String {
    format!("/openapi/v1/apps/{app_id}/envs/DEV/clusters/default/missing-namespaces")
}

#[actix_web::test]
async fn pat_metadata_scope_controls_missing_namespaces_get() {
    let p = common::make_embedded_persistence().await;
    let app_id = format!(
        "mn-get-{}",
        batata_common::datetime::Utc::now().timestamp_millis()
    );
    seed_namespaces(p.clone(), &app_id).await;

    let tokens = UserTokenService::new(p.clone());
    let (without_metadata, _) = tokens
        .create_with_scope(
            "apollo",
            "no-metadata",
            "apollo",
            None,
            r#"{"operations":["user:manage"]}"#,
        )
        .await
        .unwrap();
    let (with_metadata, _) = tokens
        .create_with_scope(
            "apollo",
            "with-metadata",
            "apollo",
            None,
            r#"{"operations":["config:read"]}"#,
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
            .uri(&missing_uri(&app_id))
            .insert_header(("X-Apollo-Token", without_metadata))
            .to_request(),
    )
    .await;
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);

    let allowed = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&missing_uri(&app_id))
            .insert_header(("X-Apollo-Token", with_metadata))
            .to_request(),
    )
    .await;
    assert_eq!(allowed.status(), StatusCode::OK);
    let body: Vec<String> = test::read_body_json(allowed).await;
    assert!(body.windows(2).all(|pair| pair[0] <= pair[1]));
    assert!(body.contains(&"beta".to_string()));
    assert!(body.contains(&"zeta".to_string()));
}

#[actix_web::test]
async fn pat_create_namespace_scope_controls_missing_namespaces_post() {
    let p = common::make_embedded_persistence().await;
    let app_id = format!(
        "mn-post-{}",
        batata_common::datetime::Utc::now().timestamp_millis()
    );
    seed_namespaces(p.clone(), &app_id).await;

    let tokens = UserTokenService::new(p.clone());
    let (dev, _) = tokens
        .create_with_scope(
            "apollo",
            "dev-create",
            "apollo",
            None,
            r#"{"operations":["namespace:create"],"envs":["DEV"]}"#,
        )
        .await
        .unwrap();
    let (pro, _) = tokens
        .create_with_scope(
            "apollo",
            "pro-create",
            "apollo",
            None,
            r#"{"operations":["namespace:create"],"envs":["PRO"]}"#,
        )
        .await
        .unwrap();
    let (read_only, _) = tokens
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
            .app_data(web::Data::new(p.clone()))
            .app_data(auth_config())
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    for (token, expected) in [
        (&pro, StatusCode::FORBIDDEN),
        (&read_only, StatusCode::FORBIDDEN),
    ] {
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri(&missing_uri(&app_id))
                .insert_header(("X-Apollo-Token", token.clone()))
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), expected);
    }

    let created = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&missing_uri(&app_id))
            .insert_header(("X-Apollo-Token", dev))
            .to_request(),
    )
    .await;
    assert_eq!(created.status(), StatusCode::OK);
    assert!(test::read_body(created).await.is_empty());

    let namespaces = NamespaceService::new(p)
        .list(&app_id, "default")
        .await
        .unwrap();
    let names: Vec<_> = namespaces.into_iter().map(|ns| ns.namespace_name).collect();
    assert!(names.contains(&"beta".to_string()));
    assert!(names.contains(&"zeta".to_string()));
}

#[actix_web::test]
async fn consumer_create_namespace_role_controls_missing_namespaces_post() {
    let p = common::make_embedded_persistence().await;
    let app_id = format!(
        "mn-consumer-{}",
        batata_common::datetime::Utc::now().timestamp_millis()
    );
    seed_namespaces(p.clone(), &app_id).await;

    async fn make_consumer(
        p: Arc<dyn ApolloPersistenceService>,
        app_id: &str,
        name: &str,
    ) -> (i64, String) {
        let consumer = ConsumerPersistence::create_consumer(
            &p,
            ConsumerDTO {
                id: None,
                app_id: app_id.to_string(),
                name: name.into(),
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
        (consumer.id, token)
    }

    let (consumer_id, consumer_token) = make_consumer(p.clone(), &app_id, "consumer").await;

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(p.clone()))
            .app_data(auth_config())
            .configure(batata_plugin_apollo::route::configure_routes),
    )
    .await;

    // Without the CreateNamespace role the request is rejected before operator
    // resolution, matching upstream's @PreAuthorize guard.
    let denied = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{}?operator=apollo", missing_uri(&app_id)))
            .insert_header(("Authorization", consumer_token.clone()))
            .to_request(),
    )
    .await;
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);

    let role = RolePersistence::create_role(
        &p,
        RoleDTO {
            id: None,
            role_name: format!("CreateNamespace+{app_id}"),
            role_type: 0,
            target_id: app_id.clone(),
            data_change_created_by: Some("tester".into()),
            data_change_created_time: None,
        },
    )
    .await
    .unwrap();
    let permission = PermissionPersistence::create_permission(
        &p,
        permission_type_ids::CREATE_NAMESPACE,
        &app_id,
        "tester",
    )
    .await
    .unwrap();
    RolePersistence::assign_role_permission(&p, role.id, permission.id, "tester")
        .await
        .unwrap();
    ConsumerRolePersistence::assign_role_to_consumer(&p, consumer_id, role.id, "tester")
        .await
        .unwrap();

    let missing_operator = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&missing_uri(&app_id))
            .insert_header(("Authorization", consumer_token.clone()))
            .to_request(),
    )
    .await;
    assert_eq!(missing_operator.status(), StatusCode::BAD_REQUEST);

    let allowed = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{}?operator=apollo", missing_uri(&app_id)))
            .insert_header(("Authorization", consumer_token))
            .to_request(),
    )
    .await;
    assert_eq!(allowed.status(), StatusCode::OK);
    assert!(test::read_body(allowed).await.is_empty());
}
