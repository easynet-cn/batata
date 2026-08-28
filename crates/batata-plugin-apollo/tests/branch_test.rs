mod common;

use std::sync::Arc;

use batata_plugin_apollo::api::dto::{GrayReleaseRuleDTO, ItemDTO, NamespaceDTO};
use batata_plugin_apollo::persistence::traits::{
    ApolloPersistenceService, GrayReleasePersistence,
};
use batata_plugin_apollo::service::{
    GrayReleaseRuleService, ItemService, NamespaceBranchService, NamespaceService, ReleaseService,
};

async fn setup(p: &Arc<dyn ApolloPersistenceService>) {
    use batata_plugin_apollo::persistence::shared::StoredCluster;
    use batata_plugin_apollo::persistence::traits::ClusterPersistence;
    // The default cluster row is created together with an app in real flows.
    let now = chrono::Utc::now().timestamp_millis();
    ClusterPersistence::create(
        p,
        StoredCluster {
            id: 0,
            name: "default".into(),
            app_id: "app1".into(),
            parent_cluster_id: 0,
            is_deleted: false,
            deleted_at: 0,
            data_change_created_by: "t".into(),
            data_change_created_time: now,
            data_change_last_modified_by: None,
            data_change_last_time: Some(now),
        },
    )
    .await
    .unwrap();

    let ns_svc = NamespaceService::new(p.clone());
    ns_svc
        .create(
            "app1",
            "default",
            NamespaceDTO {
                app_id: "app1".into(),
                cluster_name: "default".into(),
                namespace_name: "ns1".into(),
                format: Some("properties".into()),
                is_public: Some(false),
                comment: None,
                data_change_created_by: Some("t".into()),
                data_change_last_modified_by: None,
                data_change_last_time: None,
                data_change_created_time: None,
            },
        )
        .await
        .unwrap();
    let item_svc = ItemService::new(p.clone());
    let mk = |k: &str, v: &str| ItemDTO {
        id: None,
        key: k.into(),
        value: v.into(),
        r#type: None,
        comment: None,
        line_num: None,
        data_change_created_by: Some("t".into()),
        data_change_last_modified_by: None,
        data_change_last_time: None,
        data_change_created_time: None,
    };
    item_svc.create("app1", "default", "ns1", mk("k1", "base")).await.unwrap();
    ReleaseService::new(p.clone())
        .publish("app1", "default", "ns1", "r1", None, "t", false)
        .await
        .unwrap();
}

fn item(k: &str, v: &str) -> ItemDTO {
    ItemDTO {
        id: None,
        key: k.into(),
        value: v.into(),
        r#type: None,
        comment: None,
        line_num: None,
        data_change_created_by: Some("t".into()),
        data_change_last_modified_by: None,
        data_change_last_time: None,
        data_change_created_time: None,
    }
}

#[tokio::test]
async fn branch_lifecycle_create_master_merge_and_rollback() {
    let p: Arc<dyn ApolloPersistenceService> = common::make_embedded_persistence().await;
    setup(&p).await;
    let branch_svc = NamespaceBranchService::new(p.clone());
    let release_svc = ReleaseService::new(p.clone());
    let item_svc = ItemService::new(p.clone());

    // 1) createBranch — child cluster + namespace entities.
    let branch_ns = branch_svc.create_branch("app1", "default", "ns1", "t").await.unwrap();
    assert_ne!(branch_ns.cluster_name, "default");
    assert!(branch_ns.cluster_name.contains('-'), "timestamp-hex branch name");

    // Idempotent: second call returns the SAME branch.
    let again = branch_svc.create_branch("app1", "default", "ns1", "t").await.unwrap();
    assert_eq!(again.cluster_name, branch_ns.cluster_name);

    // Child cluster has parent_cluster_id > 0 and mirrors the parent ns name.
    use batata_plugin_apollo::persistence::traits::ClusterPersistence;
    let clusters = ClusterPersistence::list(&*p, "app1").await.unwrap();
    let child = clusters.iter().find(|c| c.name == branch_ns.cluster_name).unwrap();
    assert!(child.parent_cluster_id > 0);

    // 2) Author an edit on the BRANCH namespace then gray-publish the branch.
    item_svc
        .create("app1", &branch_ns.cluster_name, "ns1", item("k1", "gray-edit"))
        .await
        .unwrap();

    // Active rule so has_active_branch / repointing works.
    GrayReleaseRuleService::new(p.clone())
        .create(GrayReleaseRuleDTO {
            id: None,
            app_id: "app1".into(),
            cluster_name: "default".into(),
            namespace_name: "ns1".into(),
            branch_name: branch_ns.cluster_name.clone(),
            rules: Some(r#"[{"clientAppId":"app1","clientIpList":["*"],"clientLabelList":["*"]}]"#.into()),
            release_id: 0,
            branch_status: Some(1),
            priority: None,
            data_change_created_by: Some("t".into()),
            data_change_last_modified_by: None,
            data_change_created_time: None,
        })
        .await
        .unwrap();

    // Branch publish via the openapi gray-release route handler service path:
    // simulate by publishing through ReleaseService on the branch cluster.
    release_svc
        .publish("app1", &branch_ns.cluster_name, "ns1", "gray-r1", None, "t", false)
        .await
        .unwrap();
    let branch_cfg = branch_svc
        .branch_latest_configurations("app1", &branch_ns.cluster_name, "ns1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(branch_cfg.get("k1").map(String::as_str), Some("gray-edit"));

    // 3) Master publishes again → auto MASTER_NORMAL_RELEASE_MERGE_TO_GRAY:
    //    new key appears on branch, branch's own k1 edit survives.
    item_svc
        .create("app1", "default", "ns1", item("k2", "master-new"))
        .await
        .unwrap();
    release_svc
        .publish("app1", "default", "ns1", "r2", None, "t", false)
        .await
        .unwrap();
    let after_auto = branch_svc
        .branch_latest_configurations("app1", &branch_ns.cluster_name, "ns1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        after_auto.get("k1").map(String::as_str),
        Some("gray-edit"),
        "branch's own modification must survive the master merge"
    );
    assert_eq!(
        after_auto.get("k2").map(String::as_str),
        Some("master-new"),
        "new master key must propagate into the branch"
    );

    // 4) Rollback is ABANDON-ONLY: no new release row created; previous
    //    becomes effective. Count rows before/after.
    let before_count = <dyn GrayReleasePersistence>::list_by_app(&*p, "app1").await.unwrap().len();
    let (releases_before, _) = release_svc
        .find_active_releases("app1", "default", "ns1", 0, 100)
        .await
        .unwrap();
    release_svc.rollback("app1", "default", "ns1", releases_before[0].id.unwrap(), "t").await.unwrap();
    let (releases_after, _) = release_svc
        .find_active_releases("app1", "default", "ns1", 0, 100)
        .await
        .unwrap();
    assert_eq!(releases_before.len(), releases_after.len() + 1, "exactly one release abandoned");
    let cfgs: std::collections::HashMap<String, String> = serde_json::from_str(
        releases_after[0].configurations.as_deref().unwrap_or("{}"),
    )
    .unwrap_or_default();
    assert_eq!(
        cfgs.get("k2").map(String::as_str),
        None,
        "effective release is the previous one (without k2)"
    );
    assert_eq!(before_count, <dyn GrayReleasePersistence>::list_by_app(&*p, "app1").await.unwrap().len());
}
