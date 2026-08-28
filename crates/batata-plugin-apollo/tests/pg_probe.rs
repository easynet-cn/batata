//! Minimal empirical probe: does Insert::exec() return a real
//! last_insert_id on PostgreSQL in sea-orm 2.0.2?
//!
//! Run: TEST_DATABASE_URL="postgres://batata:batata@127.0.0.1:5433/batata_test" \
//!      cargo test -p batata-plugin-apollo --test pg_probe -- --nocapture --ignored

#![allow(dead_code)]

use sea_orm::ConnectionTrait;
use sea_orm_migration::MigratorTrait;

use batata_plugin_apollo::persistence::shared::{StoredCluster, StoredNamespace};
use batata_plugin_apollo::persistence::sql::SqlApolloPersistence;
use batata_plugin_apollo::persistence::traits::{
    ApolloPersistenceService, ClusterPersistence, NamespacePersistence,
};

fn pg_url() -> String {
    std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://batata:batata@127.0.0.1:5433/batata_test".into())
}

#[tokio::test]
#[ignore]
async fn probe_pg_last_insert_id_and_namespace_create() {
    let url = pg_url();
    let conn = sea_orm::Database::connect(url.as_str()).await.unwrap();
    batata_plugin_apollo::migration::ApolloMigrator::up(&conn, None).await.unwrap();

    // Raw driver-level check of the exact statement pattern used by the
    // original code path.
    let res = sea_orm::Statement::from_sql_and_values(
        conn.get_database_backend(),
        r#"INSERT INTO apollo_cluster (app_id, name, parent_cluster_id, is_deleted, deleted_at, data_change_created_by, data_change_created_time) VALUES ($1,$2,$3,$4,$5,$6,$7) RETURNING id"#,
        [
            "probe-app".into(),
            "probe-cluster".into(),
            0i32.into(),
            false.into(),
            0i64.into(),
            "t".into(),
            chrono::Utc::now().naive_utc().into(),
        ],
    );
    let qr = conn.query_one_raw(res).await.unwrap().expect("row returned");
    use sea_orm::QueryResult;
    let raw_id: i32 = qr.try_get::<i32>("", "id").unwrap();
    println!("RAW RETURNING id = {raw_id}");

    // Now via the trait implementation (original exec()+find_by_id code).
    let p = SqlApolloPersistence::new(conn);
    let now = chrono::Utc::now().timestamp_millis();
    let cluster = ClusterPersistence::create(
        &p,
        StoredCluster {
            id: 0,
            name: "default".into(),
            app_id: format!("probe-app-{}", now),
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
    println!("CLUSTER id via service = {}", cluster.id);
    assert!(cluster.id > 0);

    let ns = NamespacePersistence::create(
        &p,
        StoredNamespace {
            id: 0,
            app_id: cluster.app_id.clone(),
            cluster_name: "default".into(),
            namespace_name: "application".into(),
            format: "properties".into(),
            is_public: false,
            comment: None,
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
    println!("NAMESPACE id via service = {} (app={})", ns.id, ns.app_id);
    assert!(ns.id > 0, "namespace id should be positive");
}
