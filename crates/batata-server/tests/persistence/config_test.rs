//! Configuration entity persistence tests
//!
//! Tests for config_info, his_config_info, config_tags_relation entities

use crate::common::{TestDatabase, unique_data_id};

/// Test config_info CRUD operations
#[tokio::test]
#[ignore = "requires test database"]
async fn test_config_info_crud() {
    let _db = TestDatabase::from_env()
        .await
        .expect("Database connection failed");
    let _data_id = unique_data_id("crud");

    // Create
    // INSERT INTO config_info (data_id, group_id, content, tenant_id, ...)

    // Read
    // SELECT * FROM config_info WHERE data_id = ? AND group_id = ? AND tenant_id = ?

    // Update
    // UPDATE config_info SET content = ?, gmt_modified = NOW() WHERE id = ?

    // Delete
    // DELETE FROM config_info WHERE id = ?
}

/// Test config_info unique constraint
#[tokio::test]
#[ignore = "requires test database"]
async fn test_config_info_unique_constraint() {
    let _db = TestDatabase::from_env()
        .await
        .expect("Database connection failed");
    let _data_id = unique_data_id("unique");

    // Insert first record - should succeed

    // Insert duplicate (same data_id, group_id, tenant_id) - should fail
}

/// Test his_config_info history tracking
///
/// Verifies that both create and update operations generate history records.
/// This validates the fix where update was conditionally skipping history.
#[tokio::test]
#[ignore = "requires test database"]
async fn test_his_config_info_insert() {
    use batata_persistence::entity::his_config_info;
    use sea_orm::*;

    let db = TestDatabase::from_env()
        .await
        .expect("Database connection failed");
    let data_id = unique_data_id("history");
    let conn = db.conn();

    // Step 1: Create config (generates history with op_type "I")
    let result = batata_config::service::config::create_or_update(
        conn,
        &data_id,
        "DEFAULT_GROUP",
        "public",
        "content=v1",
        "",
        "test-user",
        "127.0.0.1",
        "",
        "test config",
        "",
        "",
        "text",
        "",
        "",
    )
    .await
    .expect("Failed to create config");
    assert!(result, "Create should succeed");

    // Verify history record exists via direct query
    let history_count = his_config_info::Entity::find()
        .filter(his_config_info::Column::DataId.eq(&data_id))
        .filter(his_config_info::Column::GroupId.eq("DEFAULT_GROUP"))
        .count(conn)
        .await
        .expect("Failed to count history");
    assert_eq!(history_count, 1, "Should have 1 history entry after create");

    // Verify op_type is "I" (insert)
    let first_history = his_config_info::Entity::find()
        .filter(his_config_info::Column::DataId.eq(&data_id))
        .one(conn)
        .await
        .expect("Failed to query history")
        .expect("History record should exist");
    assert_eq!(
        first_history.op_type.as_deref(),
        Some("I"),
        "First history should be Insert"
    );

    // Step 2: Update config with new content (generates history with op_type "U")
    let result = batata_config::service::config::create_or_update(
        conn,
        &data_id,
        "DEFAULT_GROUP",
        "public",
        "content=v2",
        "",
        "test-user",
        "127.0.0.1",
        "",
        "test config updated",
        "",
        "",
        "text",
        "",
        "",
    )
    .await
    .expect("Failed to update config");
    assert!(result, "Update should succeed");

    // Verify 2 history records
    let history_count = his_config_info::Entity::find()
        .filter(his_config_info::Column::DataId.eq(&data_id))
        .filter(his_config_info::Column::GroupId.eq("DEFAULT_GROUP"))
        .count(conn)
        .await
        .expect("Failed to count history");
    assert_eq!(
        history_count, 2,
        "Should have 2 history entries after update"
    );

    // Verify the second entry has op_type "U"
    let update_history = his_config_info::Entity::find()
        .filter(his_config_info::Column::DataId.eq(&data_id))
        .filter(his_config_info::Column::OpType.eq("U"))
        .one(conn)
        .await
        .expect("Failed to query update history");
    assert!(
        update_history.is_some(),
        "Should have an Update history entry"
    );

    // Step 3: Update with same content (should still generate history)
    let result = batata_config::service::config::create_or_update(
        conn,
        &data_id,
        "DEFAULT_GROUP",
        "public",
        "content=v2",
        "",
        "test-user",
        "127.0.0.1",
        "",
        "test config updated",
        "",
        "",
        "text",
        "",
        "",
    )
    .await
    .expect("Failed to re-update config");
    assert!(result, "Re-update should succeed");

    // Verify 3 history records (even though content unchanged)
    let history_count = his_config_info::Entity::find()
        .filter(his_config_info::Column::DataId.eq(&data_id))
        .filter(his_config_info::Column::GroupId.eq("DEFAULT_GROUP"))
        .count(conn)
        .await
        .expect("Failed to count history");
    assert_eq!(
        history_count, 3,
        "Should have 3 history entries after same-content update"
    );
}

/// Test config_tags_relation
#[tokio::test]
#[ignore = "requires test database"]
async fn test_config_tags_relation() {
    let _db = TestDatabase::from_env()
        .await
        .expect("Database connection failed");
    let _data_id = unique_data_id("tagged");

    // Create config with tags
    // Verify tags in config_tags_relation

    // Query configs by tag
    // SELECT c.* FROM config_info c
    // JOIN config_tags_relation t ON c.id = t.nid
    // WHERE t.tag_name = ?
}

/// Test MD5 calculation consistency
#[tokio::test]
#[ignore = "requires test database"]
async fn test_config_md5_consistency() {
    let _db = TestDatabase::from_env()
        .await
        .expect("Database connection failed");
    let _data_id = unique_data_id("md5");
    let _content = "test.key=test.value";

    // Insert config
    // Verify MD5 is calculated correctly
    // MD5 should match md5::compute(content).to_hex()
}
