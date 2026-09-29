//! Real-database migration smoke test.
//!
//! Applies every migration to a live database and asserts that the AI resource
//! tables were created. This is the check that catches dialect differences
//! between MySQL and PostgreSQL (auto-increment vs. serial, LONGTEXT vs. TEXT,
//! timestamp defaults, index length limits) — `cargo test` alone only proves the
//! Rust compiles.
//!
//! Ignored by default: it needs a reachable database.
//!
//! Prepare a clean database first (drops everything in it):
//!
//! ```bash
//! podman exec mysql mysql -uroot -pdevterry -e \
//!   "DROP DATABASE IF EXISTS batata_ai_test; CREATE DATABASE batata_ai_test;"
//! podman exec -e PGPASSWORD=devterry postgres psql -U postgres \
//!   -c "DROP DATABASE IF EXISTS batata_ai_test;" -c "CREATE DATABASE batata_ai_test;"
//! ```
//!
//! Then run:
//!
//! ```bash
//! DATABASE_URL="mysql://root:devterry@127.0.0.1:3306/batata_ai_test" \
//!   cargo test -p batata-migration --test migration_smoke -- --ignored --nocapture
//!
//! DATABASE_URL="postgres://postgres:devterry@127.0.0.1:5432/batata_ai_test" \
//!   cargo test -p batata-migration --test migration_smoke -- --ignored --nocapture
//! ```

use batata_migration::{ConnectionTrait, Migrator};
use sea_orm_migration::sea_orm::{
    ConnectOptions, Database, DatabaseBackend, DatabaseConnection, Statement,
};
use sea_orm_migration::MigratorTrait;

/// Tables the AI module relies on; all five come from the upstream Nacos schema.
/// Connect to the test database, failing fast with an actionable message.
///
/// A stopped Podman container leaves the forwarded port *accepting* TCP while
/// nothing completes the handshake, so a plain `Database::connect` blocks for
/// the default ~30s and then reports a pool timeout.
async fn connect_database(url: &str) -> DatabaseConnection {
    use std::time::Duration;
    let mut options = ConnectOptions::new(url.to_string());
    options
        .connect_timeout(Duration::from_secs(3))
        .acquire_timeout(Duration::from_secs(3));
    match Database::connect(options).await {
        Ok(connection) => connection,
        Err(error) => panic!(
            "cannot reach the test database at {url}: {error}\n\
             hint: containers do not come back on their own after a machine \
             restart or a podman machine stop — try `podman start mysql postgres`"
        ),
    }
}

const AI_TABLES: [&str; 5] = [
    "ai_resource",
    "ai_resource_version",
    "ai_resource_search_document",
    "ai_resource_search_chunk",
    "ai_resource_task",
];

/// Return true when `table` is present in the current schema.
async fn table_exists(db: &DatabaseConnection, table: &str) -> bool {
    let scope = match db.get_database_backend() {
        DatabaseBackend::MySql => "table_schema = DATABASE()",
        _ => "table_schema = current_schema()",
    };
    // `table` is a fixed literal from AI_TABLES, so inline it rather than
    // fighting MySQL `?` vs PostgreSQL `$1` placeholders.
    let sql = format!(
        "SELECT COUNT(*) AS c FROM information_schema.tables WHERE table_name = '{table}' AND {scope}"
    );
    let rows = db
        .query_all_raw(Statement::from_string(db.get_database_backend(), sql))
        .await
        .expect("information_schema query failed");

    rows.first()
        .and_then(|r| r.try_get::<i64>("", "c").ok())
        .unwrap_or(0)
        > 0
}

#[tokio::test]
#[ignore]
async fn migrations_create_ai_tables() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let db = connect_database(&url).await;

    Migrator::up(&db, None)
        .await
        .expect("migrations must apply cleanly");

    let backend = db.get_database_backend();
    println!("--- backend: {backend:?} ---");
    for table in AI_TABLES {
        assert!(table_exists(&db, table).await, "missing table: {table}");
        println!("ok: {table}");
    }
}
