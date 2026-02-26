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
#[tokio::test]
#[ignore = "requires test database"]
async fn test_his_config_info_insert() {
    let _db = TestDatabase::from_env()
        .await
        .expect("Database connection failed");
    let _data_id = unique_data_id("history");

    // Insert config
    // Verify history record created

    // Update config
    // Verify new history record created

    // Query history
    // SELECT * FROM his_config_info WHERE data_id = ? ORDER BY gmt_modified DESC
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
