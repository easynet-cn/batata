mod common;

use std::sync::Arc;

use batata_plugin_apollo::api::dto::*;
use batata_plugin_apollo::persistence::shared::{StoredItem, StoredNamespace, StoredRelease};
use batata_plugin_apollo::persistence::traits::{
    ApolloPersistenceService, AppNamespacePersistence, AuditPersistence, ConsumerPersistence,
    ConsumerTokenPersistence, FavoritePersistence, InstanceConfigPersistence, ItemPersistence,
    NamespacePersistence, PermissionPersistence, ReleaseHistoryPersistence, ReleasePersistence,
    RolePersistence, ServerConfigPersistence,
};

/// Generic persistence contract test. Runs against any `Arc<dyn ApolloPersistenceService>`
/// (embedded RocksDB or SQL), covering the portal/admin entities (Tier 2) plus the
/// core `Namespace`/`Item`/`Release` traits with the newly added `list_all`,
/// `list_deleted_items`, and release-history methods.
async fn suite(p: Arc<dyn ApolloPersistenceService>) {
    // ---- AppNamespace ----
    let ns = AppNamespacePersistence::create_app_namespace(
        &p,
        AppNamespaceDTO {
            id: None,
            name: "appns1".into(),
            app_id: "app1".into(),
            format: "properties".into(),
            is_public: true,
            comment: "c".into(),
            data_change_created_by: Some("tester".into()),
            data_change_created_time: None,
        },
    )
    .await
    .unwrap();
    assert!(ns.id > 0, "app_namespace id should be assigned");
    assert!(
        AppNamespacePersistence::get_app_namespace(&p, "app1", "appns1")
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        !AppNamespacePersistence::list_app_namespace_by_app(&p, "app1")
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        AppNamespacePersistence::list_public_app_namespace(&p)
            .await
            .unwrap()
            .iter()
            .any(|m| m.name == "appns1")
    );
    AppNamespacePersistence::delete_app_namespace(&p, "app1", "appns1", "tester")
        .await
        .unwrap();
    assert!(
        AppNamespacePersistence::get_app_namespace(&p, "app1", "appns1")
            .await
            .unwrap()
            .is_none()
    );

    // ---- Audit ----
    let audit = AuditPersistence::create_audit(
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
    assert!(audit.id > 0);
    let (audit_list, audit_total) = AuditPersistence::list_audit(&p, 1, 10).await.unwrap();
    assert!(audit_total >= 1 && !audit_list.is_empty());
    assert!(
        !AuditPersistence::list_audit_by_entity(&p, "app", "1")
            .await
            .unwrap()
            .is_empty()
    );

    // ---- Consumer + ConsumerToken ----
    // NOTE: `apollo_consumer.app_id` carries a UNIQUE constraint and the
    // consumer is never deleted, so the app id must be unique per run to keep
    // the suite rerunnable against a persistent SQL database.
    let consumer_app_id = format!("app1-{}", chrono::Utc::now().timestamp_millis());
    let consumer = ConsumerPersistence::create_consumer(
        &p,
        ConsumerDTO {
            id: None,
            app_id: consumer_app_id.clone(),
            name: "c1".into(),
            org_id: "o".into(),
            org_name: "on".into(),
            owner_name: "ow".into(),
            owner_email: "e@x".into(),
            data_change_created_by: Some("tester".into()),
            data_change_created_time: None,
        },
    )
    .await
    .unwrap();
    assert!(consumer.id > 0);
    assert!(ConsumerPersistence::get_consumer(&p, consumer.id).await.unwrap().is_some());
    assert!(!ConsumerPersistence::list_consumers(&p).await.unwrap().is_empty());
    assert!(
        ConsumerPersistence::get_consumer_by_app(&p, &consumer_app_id)
            .await
            .unwrap()
            .is_some()
    );

    let token = ConsumerTokenPersistence::create_consumer_token(&p, consumer.id, "tester")
        .await
        .unwrap();
    assert!(!token.token.is_empty());
    assert!(
        !ConsumerTokenPersistence::list_tokens_by_consumer(&p, consumer.id)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        ConsumerTokenPersistence::get_consumer_token_by_token(&p, &token.token)
            .await
            .unwrap()
            .is_some()
    );
    ConsumerTokenPersistence::delete_consumer_token(&p, token.id)
        .await
        .unwrap();
    assert!(
        ConsumerTokenPersistence::get_consumer_token_by_token(&p, &token.token)
            .await
            .unwrap()
            .is_none()
    );

    // ---- Permission + Role (+role_permission/user_role) ----
    // Same rerun-safety note as the consumer: (target_id, permission_type) is
    // UNIQUE and the permission is never deleted, so the target must be
    // unique per run.
    let perm_target = format!("target1-{}", chrono::Utc::now().timestamp_millis());
    let perm = PermissionPersistence::create_permission(&p, 1, &perm_target, "tester")
        .await
        .unwrap();
    assert!(perm.id > 0);
    assert!(
        !PermissionPersistence::list_permission_by_target(&p, &perm_target)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        !PermissionPersistence::list_permission_by_type(&p, 1)
            .await
            .unwrap()
            .is_empty()
    );

    let role = RolePersistence::create_role(
        &p,
        RoleDTO {
            id: None,
            role_name: "r1".into(),
            role_type: 1,
            target_id: "app1".into(),
            data_change_created_by: Some("tester".into()),
            data_change_created_time: None,
        },
    )
    .await
    .unwrap();
    assert!(role.id > 0);
    assert!(RolePersistence::get_role(&p, role.id).await.unwrap().is_some());
    assert!(
        !RolePersistence::list_role_by_target(&p, "app1")
            .await
            .unwrap()
            .is_empty()
    );
    RolePersistence::assign_role_permission(&p, role.id, perm.id, "tester")
        .await
        .unwrap();
    assert!(
        RolePersistence::list_role_permissions(&p, role.id)
            .await
            .unwrap()
            .contains(&perm.id)
    );
    RolePersistence::assign_role_to_user(&p, "user1", role.id, "tester")
        .await
        .unwrap();
    assert!(
        !RolePersistence::list_user_roles(&p, "user1")
            .await
            .unwrap()
            .is_empty()
    );
    RolePersistence::remove_role_permission(&p, role.id, perm.id)
        .await
        .unwrap();
    assert!(
        !RolePersistence::list_role_permissions(&p, role.id)
            .await
            .unwrap()
            .contains(&perm.id)
    );
    RolePersistence::remove_role_from_user(&p, "user1", role.id)
        .await
        .unwrap();
    RolePersistence::delete_role(&p, role.id).await.unwrap();

    // ---- Favorite ----
    let fav = FavoritePersistence::create_favorite(
        &p,
        FavoriteDTO {
            id: None,
            user_id: "u1".into(),
            app_id: "app1".into(),
            data_change_created_by: Some("tester".into()),
            data_change_created_time: None,
        },
    )
    .await
    .unwrap();
    assert!(fav.id > 0);
    assert!(
        !FavoritePersistence::list_favorite_by_user(&p, "u1")
            .await
            .unwrap()
            .is_empty()
    );
    FavoritePersistence::delete_favorite(&p, fav.id, "u1")
        .await
        .unwrap();

    // ---- ServerConfig ----
    let sc = ServerConfigPersistence::create_server_config(
        &p,
        ServerConfigDTO {
            key: "sc1".into(),
            value: "v1".into(),
            comment: Some("c".into()),
            data_change_created_by: Some("tester".into()),
            data_change_created_time: None,
        },
    )
    .await
    .unwrap();
    assert!(sc.id > 0);
    assert!(
        ServerConfigPersistence::get_server_config(&p, "sc1")
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        !ServerConfigPersistence::list_server_config(&p)
            .await
            .unwrap()
            .is_empty()
    );
    ServerConfigPersistence::update_server_config(&p, "sc1", "v2", "tester")
        .await
        .unwrap();
    assert_eq!(
        ServerConfigPersistence::get_server_config(&p, "sc1")
            .await
            .unwrap()
            .unwrap()
            .value,
        "v2"
    );
    ServerConfigPersistence::delete_server_config(&p, "sc1", "tester")
        .await
        .unwrap();
    assert!(
        ServerConfigPersistence::get_server_config(&p, "sc1")
            .await
            .unwrap()
            .is_none()
    );

    // ---- InstanceConfig ----
    let ic = InstanceConfigPersistence::create_or_update_instance_config(
        &p,
        InstanceConfigDTO {
            id: None,
            config_app_id: None,
        instance_id: 1,
            namespace_name: "ns".into(),
            cluster_name: "default".into(),
            release_key: "rk".into(),
            configurations: Some("{}".into()),
            data_change_created_by: Some("tester".into()),
            data_change_created_time: None,
            data_change_last_time: None,
        },
    )
    .await
    .unwrap();
    assert!(ic.id > 0);
    assert!(
        !InstanceConfigPersistence::get_instance_config_by_instance(&p, 1)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        !InstanceConfigPersistence::list_instance_config_by_app_cluster(&p, "x", "default", "x")
            .await
            .unwrap()
            .is_empty()
    );

    // ---- Namespace / Item / Release (core) + list_all / list_deleted_items ----
    // `apollo_namespace` is UNIQUE on (app_id, cluster, name, deleted_at) and
    // the namespace is never deleted, so the app id must be unique per run.
    let ns_app_id = format!("nsapp-{}", chrono::Utc::now().timestamp_millis());
    let namespace = NamespacePersistence::create(
        &p,
        StoredNamespace {
            id: 0,
            app_id: ns_app_id,
            cluster_name: "default".into(),
            namespace_name: "ns1".into(),
            format: "properties".into(),
            is_public: false,
            comment: None,
            is_deleted: false,
            deleted_at: 0,
            data_change_created_by: "tester".into(),
            data_change_created_time: 0,
            data_change_last_modified_by: None,
            data_change_last_time: None,
        },
    )
    .await
    .unwrap();
    assert!(namespace.id > 0);

    let item = ItemPersistence::create(
        &p,
        StoredItem {
            id: 0,
            namespace_id: namespace.id,
            key: "k1".into(),
            r#type: 0,
            value: "v1".into(),
            comment: None,
            line_num: 0,
            is_deleted: false,
            deleted_at: 0,
            data_change_created_by: "tester".into(),
            data_change_created_time: 0,
            data_change_last_modified_by: None,
            data_change_last_time: None,
        },
    )
    .await
    .unwrap();
    assert!(item.id > 0);

    ItemPersistence::delete(&p, item.id).await.unwrap();
    let deleted = ItemPersistence::list_deleted_items(&p, namespace.id)
        .await
        .unwrap();
    assert!(
        deleted.iter().any(|i| i.id == item.id),
        "soft-deleted item should appear in list_deleted_items"
    );

    let all_ns = NamespacePersistence::list_all(&p).await.unwrap();
    assert!(
        all_ns.iter().any(|n| n.id == namespace.id),
        "list_all should include the created namespace"
    );

    // `apollo_release.release_key` is UNIQUE and the release is never
    // deleted — timestamp it so reruns against a persistent DB don't collide.
    let release_key = format!("rk1-{}", chrono::Utc::now().timestamp_millis());
    let release = ReleasePersistence::create(
        &p,
        StoredRelease {
            id: 0,
            release_key,
            name: "r".into(),
            comment: None,
            app_id: "app1".into(),
            cluster_name: "default".into(),
            namespace_name: "ns".into(),
            configurations: "{}".into(),
            release_id: Some(1),
            is_abandoned: false,
            is_deleted: false,
            deleted_at: 0,
            data_change_created_by: "tester".into(),
            data_change_created_time: 0,
            data_change_last_modified_by: None,
            data_change_last_time: None,
        },
    )
    .await
    .unwrap();
    assert!(release.id > 0);

    // ---- ReleaseHistory ----
    let hist = ReleaseHistoryPersistence::record_release_history(
        &p, "app1", "default", "ns", "", 1, 0, 0, "ctx", "tester",
    )
    .await
    .unwrap();
    assert!(hist.id > 0);
    let (hist_list, hist_total) =
        ReleaseHistoryPersistence::find_release_history(&p, "app1", "default", "ns", 1, 10)
            .await
            .unwrap();
    assert!(
        hist_total >= 1 && !hist_list.is_empty(),
        "release history should be recorded"
    );
}

#[tokio::test]
async fn embedded_portal_persistence() {
    let p = common::make_embedded_persistence().await;
    suite(p).await;
}

#[tokio::test]
async fn sql_portal_persistence() {
    if let Some(p) = common::make_sql_persistence().await {
        suite(p).await;
    }
}
