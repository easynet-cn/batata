mod common;

use std::sync::Arc;

use batata_plugin_apollo::api::dto::{
    ConsumerDTO, NewAuditLog, NewAuditLogDataInfluence, NewConsumerAudit, NewUserTokenAudit,
    RoleDTO, UserDTO,
};
use batata_plugin_apollo::auth::permission::{
    Identity, PermissionResolver, permission_type, permission_type_id, permission_type_ids,
};
use batata_plugin_apollo::persistence::traits::{
    ApolloPersistenceService, AuditLogPersistence, AuthorityPersistence, ConsumerAuditPersistence,
    ConsumerPersistence, ConsumerRolePersistence, PermissionPersistence, RolePersistence,
    UserPersistence, UserTokenAuditPersistence,
};
use batata_plugin_apollo::service::{
    AuditLogService, ConsumerAuditService, PortalBootstrapService, UserTokenAuditService,
};

#[tokio::test]
async fn authorities_and_audit_tables_have_real_crud() {
    let p = common::make_embedded_persistence().await;

    let authority = AuthorityPersistence::assign_authority(&p, "alice", "ROLE_TEST")
        .await
        .unwrap();
    assert!(authority.id > 0);
    assert!(
        AuthorityPersistence::has_authority(&p, "alice", "ROLE_TEST")
            .await
            .unwrap()
    );
    assert_eq!(
        AuthorityPersistence::list_authorities(&p, "alice")
            .await
            .unwrap(),
        vec!["ROLE_TEST".to_string()]
    );
    AuthorityPersistence::remove_authority(&p, "alice", "ROLE_TEST")
        .await
        .unwrap();
    assert!(
        !AuthorityPersistence::has_authority(&p, "alice", "ROLE_TEST")
            .await
            .unwrap()
    );

    let log = AuditLogPersistence::create_audit_log(
        &p,
        NewAuditLog {
            trace_id: "trace-1".into(),
            span_id: "span-1".into(),
            operator: "alice".into(),
            op_type: "ManageUser".into(),
            op_name: "CreateUser".into(),
            description: Some("created bob".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(log.span_id, "span-1");
    let influence = AuditLogPersistence::create_data_influence(
        &p,
        NewAuditLogDataInfluence {
            span_id: "span-1".into(),
            entity_id: "bob".into(),
            entity_name: "User".into(),
            field_name: Some("enabled".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert!(influence.id > 0);

    let (rows, total) = AuditLogPersistence::list_audit_logs(
        &p,
        Some("CreateUser"),
        Some("alice"),
        Some("trace-1"),
        Some("bob"),
        1,
        10,
    )
    .await
    .unwrap();
    assert_eq!(total, 1);
    assert_eq!(rows.len(), 1);
    let influences = AuditLogPersistence::list_influences(&p, &["span-1".to_string()])
        .await
        .unwrap();
    assert_eq!(influences.len(), 1);
    assert_eq!(influences[0].influence_entity_id, "bob");

    let token_audit = UserTokenAuditPersistence::create_user_token_audit(
        &p,
        NewUserTokenAudit {
            token_id: Some(7),
            user_id: "alice".into(),
            uri: "/openapi/v1/users".into(),
            method: "GET".into(),
        },
    )
    .await
    .unwrap();
    assert!(token_audit.id > 0);
    assert_eq!(
        UserTokenAuditPersistence::list_user_token_audits(&p, 7)
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        UserTokenAuditPersistence::list_user_token_audits_by_user(&p, "alice")
            .await
            .unwrap()
            .len(),
        1
    );

    let consumer_audit = ConsumerAuditPersistence::create_consumer_audit(
        &p,
        NewConsumerAudit {
            consumer_id: 9,
            op_name: "AssignRole".into(),
            op_by: "alice".into(),
        },
    )
    .await
    .unwrap();
    assert!(consumer_audit.id > 0);
    assert_eq!(
        ConsumerAuditPersistence::list_consumer_audits(&p, 9)
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn audit_log_service_joins_influences() {
    let p = common::make_embedded_persistence().await;
    let service = AuditLogService::new(p.clone());
    let created = service
        .record(
            NewAuditLog {
                trace_id: "trace-audit-service".into(),
                span_id: "span-audit-service".into(),
                operator: "alice".into(),
                op_name: "ChangeUserEnabled".into(),
                ..Default::default()
            },
            vec![NewAuditLogDataInfluence {
                span_id: String::new(),
                entity_id: "bob".into(),
                entity_name: "User".into(),
                field_name: Some("enabled".into()),
                old_value: Some("true".into()),
                new_value: Some("false".into()),
                ..Default::default()
            }],
        )
        .await
        .unwrap();
    assert_eq!(created.data_influences.len(), 1);

    let (rows, total) = service
        .list(Some("ChangeUserEnabled"), None, None, None, 1, 20)
        .await
        .unwrap();
    assert_eq!(total, 1);
    assert_eq!(rows[0].data_influences[0].entity_id, "bob");

    let token_service = UserTokenAuditService::new(p.clone());
    token_service
        .record(NewUserTokenAudit {
            token_id: Some(11),
            user_id: "alice".into(),
            uri: "/openapi/v1/users".into(),
            method: "POST".into(),
        })
        .await
        .unwrap();
    assert_eq!(token_service.list_by_token(11).await.unwrap().len(), 1);

    let consumer_service = ConsumerAuditService::new(p);
    consumer_service
        .record(NewConsumerAudit {
            consumer_id: 12,
            op_name: "CreateConsumer".into(),
            op_by: "alice".into(),
        })
        .await
        .unwrap();
    assert_eq!(consumer_service.list(12).await.unwrap().len(), 1);
}

#[tokio::test]
async fn bootstrap_seeds_admin_identity_and_rbac() {
    let p: Arc<dyn ApolloPersistenceService> = common::make_embedded_persistence().await;
    let bootstrap = PortalBootstrapService::new(p.clone());
    bootstrap.ensure_initialized().await.unwrap();
    // A second call must be a no-op, not a duplicate-row failure.
    bootstrap.ensure_initialized().await.unwrap();

    let resolver = PermissionResolver::new(p);
    let identity = Identity::User(UserDTO {
        id: None,
        username: "apollo".into(),
        password: String::new(),
        email: None,
        enabled: true,
        data_change_created_by: None,
        data_change_created_time: None,
    });
    assert!(
        resolver
            .has_permission(&identity, permission_type::CREATE_APPLICATION, "SystemRole")
            .await
            .unwrap()
    );
    assert!(
        !resolver
            .has_permission(&identity, permission_type::CREATE_CLUSTER, "SystemRole")
            .await
            .unwrap()
    );
    assert_eq!(
        permission_type_id(permission_type::MANAGE_USERS),
        Some(permission_type_ids::MANAGE_USERS)
    );
}

#[tokio::test]
async fn consumer_roles_grant_permissions_to_consumer_identity() {
    let p = common::make_embedded_persistence().await;
    let consumer = ConsumerPersistence::create_consumer(
        &p,
        ConsumerDTO {
            id: None,
            app_id: "app-a".into(),
            name: "consumer-a".into(),
            org_id: "1".into(),
            org_name: "default".into(),
            owner_name: "owner".into(),
            owner_email: "owner@example.com".into(),
            data_change_created_by: Some("system".into()),
            data_change_created_time: None,
        },
    )
    .await
    .unwrap();
    let role = RolePersistence::create_role(
        &p,
        RoleDTO {
            id: None,
            role_name: "ModifyNamespace+app-a".into(),
            role_type: 0,
            target_id: "app-a".into(),
            data_change_created_by: Some("system".into()),
            data_change_created_time: None,
        },
    )
    .await
    .unwrap();
    let permission = PermissionPersistence::create_permission(
        &p,
        permission_type_ids::MODIFY_NAMESPACE,
        "app-a+application",
        "system",
    )
    .await
    .unwrap();
    RolePersistence::assign_role_permission(&p, role.id, permission.id, "system")
        .await
        .unwrap();
    ConsumerRolePersistence::assign_role_to_consumer(&p, consumer.id, role.id, "system")
        .await
        .unwrap();

    let resolver = PermissionResolver::new(p);
    let identity = Identity::Consumer {
        consumer_id: consumer.id,
        app_id: Some("app-a".into()),
    };
    assert!(
        resolver
            .has_permission(
                &identity,
                permission_type::MODIFY_NAMESPACE,
                "app-a+application"
            )
            .await
            .unwrap()
    );
    assert!(
        !resolver
            .has_permission(&identity, permission_type::MODIFY_NAMESPACE, "app-a+other")
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn seeded_password_fits_the_apollo_users_column() {
    let persistence = common::make_embedded_persistence().await;
    PortalBootstrapService::new(persistence.clone())
        .ensure_initialized()
        .await
        .unwrap();

    let user = UserPersistence::get_user(&persistence, "apollo")
        .await
        .unwrap()
        .expect("bootstrap admin must exist");
    assert_eq!(
        user.password.len(),
        64,
        "upstream password column is varchar(64)"
    );
    assert!(user.password.bytes().all(|byte| byte.is_ascii_hexdigit()));
}
