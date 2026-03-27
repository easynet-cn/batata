//! Configuration entity persistence tests
//!
//! Tests for config persistence via PersistenceService trait

use crate::common::{TestDatabase, unique_data_id};
use batata_persistence::{ConfigPersistence, ExternalDbPersistService};

/// Test config history tracking via PersistenceService
///
/// Verifies that both create and update operations generate history records.
#[tokio::test]
#[ignore = "requires test database"]
async fn test_config_history_tracking() {
    let db = TestDatabase::from_env()
        .await
        .expect("Database connection failed");
    let data_id = unique_data_id("history");

    let persistence = ExternalDbPersistService::new(db.conn().clone());

    // Step 1: Create config (generates history with op_type "I")
    let result = persistence
        .config_create_or_update(
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
            None,
        )
        .await
        .expect("Failed to create config");
    assert!(result, "Create should succeed");

    // Verify config was created
    let config = persistence
        .config_find_one(&data_id, "DEFAULT_GROUP", "public")
        .await
        .expect("Failed to find config");
    assert!(config.is_some(), "Config should exist after create");
    assert_eq!(config.unwrap().content, "content=v1");

    // Verify history record exists
    let history = persistence
        .config_history_search_page(&data_id, "DEFAULT_GROUP", "public", 1, 10)
        .await
        .expect("Failed to search history");
    assert_eq!(
        history.total_count, 1,
        "Should have 1 history entry after create"
    );

    // Step 2: Update config with new content
    let result = persistence
        .config_create_or_update(
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
            None,
        )
        .await
        .expect("Failed to update config");
    assert!(result, "Update should succeed");

    // Verify 2 history records
    let history = persistence
        .config_history_search_page(&data_id, "DEFAULT_GROUP", "public", 1, 10)
        .await
        .expect("Failed to search history");
    assert_eq!(
        history.total_count, 2,
        "Should have 2 history entries after update"
    );

    // Step 3: Update with same content (should still generate history)
    let result = persistence
        .config_create_or_update(
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
            None,
        )
        .await
        .expect("Failed to re-update config");
    assert!(result, "Re-update should succeed");

    // Verify 3 history records (even though content unchanged)
    let history = persistence
        .config_history_search_page(&data_id, "DEFAULT_GROUP", "public", 1, 10)
        .await
        .expect("Failed to search history");
    assert_eq!(
        history.total_count, 3,
        "Should have 3 history entries after same-content update"
    );
}

/// Test config CRUD operations via PersistenceService
#[tokio::test]
#[ignore = "requires test database"]
async fn test_config_crud() {
    let db = TestDatabase::from_env()
        .await
        .expect("Database connection failed");
    let data_id = unique_data_id("crud");

    let persistence = ExternalDbPersistService::new(db.conn().clone());

    // Create
    let created = persistence
        .config_create_or_update(
            &data_id,
            "DEFAULT_GROUP",
            "public",
            "key=value",
            "",
            "test-user",
            "127.0.0.1",
            "",
            "",
            "",
            "",
            "properties",
            "",
            "",
            None,
        )
        .await
        .expect("Failed to create config");
    assert!(created);

    // Read
    let found = persistence
        .config_find_one(&data_id, "DEFAULT_GROUP", "public")
        .await
        .expect("Failed to find config");
    assert!(found.is_some());
    assert_eq!(found.unwrap().content, "key=value");

    // Delete
    let deleted = persistence
        .config_delete(&data_id, "DEFAULT_GROUP", "public", "", "127.0.0.1", "test-user")
        .await
        .expect("Failed to delete config");
    assert!(deleted);

    // Verify deleted
    let found = persistence
        .config_find_one(&data_id, "DEFAULT_GROUP", "public")
        .await
        .expect("Failed to find config");
    assert!(found.is_none(), "Config should be gone after delete");
}
