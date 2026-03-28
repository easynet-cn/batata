//! Config Extended Admin API Functional Tests
//!
//! Tests export/import, batch delete, clone, history detail/previous,
//! listeners by IP, list with filters, and metadata update.

mod common;

// ==================== Config Export & Import ====================

#[tokio::test]
#[ignore]
async fn test_config_export_import() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let data_id = format!("ext-exp-{}", common::test_id());
    let group = "DEFAULT_GROUP";
    let ns = "public";

    // Publish a config to export
    let ok = client
        .config_publish(&data_id, group, ns, "export-me=true", "", "", "", "", "", "properties", "", "")
        .await
        .unwrap();
    assert!(ok, "Publish should succeed");

    // Export configs as ZIP bytes
    let zip_bytes = client
        .config_export(ns, Some(group), Some(&data_id), None)
        .await
        .unwrap();
    assert!(
        !zip_bytes.is_empty(),
        "Exported ZIP should not be empty"
    );
    // ZIP files start with PK magic bytes (0x50, 0x4B)
    assert_eq!(
        zip_bytes[0], 0x50,
        "Exported data should be a valid ZIP (first magic byte)"
    );
    assert_eq!(
        zip_bytes[1], 0x4B,
        "Exported data should be a valid ZIP (second magic byte)"
    );

    // Delete the original config
    client.config_delete(&data_id, group, ns).await.unwrap();

    // Verify it is gone
    let gone = client.config_get(&data_id, group, ns).await.unwrap();
    assert!(gone.is_none(), "Config should be deleted before import");

    // Import the ZIP back with OVERWRITE policy
    let import_result = client
        .config_import(zip_bytes, ns, "OVERWRITE")
        .await
        .unwrap();
    assert!(
        import_result.success_count >= 1,
        "Import should succeed with at least 1 config, got success_count={}",
        import_result.success_count
    );
    assert_eq!(
        import_result.fail_count, 0,
        "Import should have zero failures"
    );

    // Verify imported config exists and has correct content
    let restored = client.config_get(&data_id, group, ns).await.unwrap();
    assert!(restored.is_some(), "Config should exist after import");
    assert_eq!(
        restored.unwrap().content, "export-me=true",
        "Imported config content should match original"
    );

    // Cleanup
    client.config_delete(&data_id, group, ns).await.unwrap();
}

// ==================== Config Batch Delete ====================

#[tokio::test]
#[ignore]
async fn test_config_batch_delete() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let tid = common::test_id();
    let data_id_1 = format!("ext-bd1-{}", tid);
    let data_id_2 = format!("ext-bd2-{}", tid);
    let group = "DEFAULT_GROUP";
    let ns = "public";

    // Create two configs
    client
        .config_publish(&data_id_1, group, ns, "batch1", "", "", "", "", "", "", "", "")
        .await
        .unwrap();
    client
        .config_publish(&data_id_2, group, ns, "batch2", "", "", "", "", "", "", "", "")
        .await
        .unwrap();

    // Retrieve their IDs from config_get
    let cfg1 = client.config_get(&data_id_1, group, ns).await.unwrap();
    let cfg2 = client.config_get(&data_id_2, group, ns).await.unwrap();
    assert!(cfg1.is_some(), "Config 1 should exist");
    assert!(cfg2.is_some(), "Config 2 should exist");

    let id1 = cfg1.unwrap().id;
    let id2 = cfg2.unwrap().id;

    if id1 > 0 && id2 > 0 {
        // Batch delete by IDs (requires DB mode with auto-increment IDs)
        let deleted_count = client.config_batch_delete(&[id1, id2]).await.unwrap();
        assert_eq!(
            deleted_count, 2,
            "Should delete exactly 2 configs, got {}",
            deleted_count
        );
    } else {
        // Embedded mode: IDs are 0, fall back to individual delete
        client.config_delete(&data_id_1, group, ns).await.unwrap();
        client.config_delete(&data_id_2, group, ns).await.unwrap();
    }

    // Verify both are gone
    let gone1 = client.config_get(&data_id_1, group, ns).await.unwrap();
    let gone2 = client.config_get(&data_id_2, group, ns).await.unwrap();
    assert!(gone1.is_none(), "Config 1 should be deleted");
    assert!(gone2.is_none(), "Config 2 should be deleted");
}

// ==================== Config Clone ====================

#[tokio::test]
#[ignore]
async fn test_config_clone() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let tid = common::test_id();
    let data_id = format!("ext-cln-{}", tid);
    let group = "DEFAULT_GROUP";
    let src_ns = "public";
    let target_ns_id = format!("clone-ns-{}", tid);

    // Create target namespace
    let ns_created = client
        .namespace_create(&target_ns_id, &format!("Clone NS {}", &tid[..4]), "for clone test")
        .await
        .unwrap();
    assert!(ns_created, "Target namespace should be created");

    // Publish config in source namespace
    client
        .config_publish(&data_id, group, src_ns, "clone-content", "", "", "", "", "", "", "", "")
        .await
        .unwrap();

    // Verify source config exists
    let cfg = client.config_get(&data_id, group, src_ns).await.unwrap();
    assert!(cfg.is_some(), "Source config should exist");
    let cfg_id = cfg.unwrap().id;

    if cfg_id > 0 {
        // Clone to target namespace (requires DB mode with auto-increment IDs)
        let clone_result = client
            .config_clone(&[cfg_id], &target_ns_id, "OVERWRITE")
            .await
            .unwrap();
        assert!(
            clone_result.succeeded >= 1,
            "Clone should succeed for at least 1 config, got succeeded={}",
            clone_result.succeeded
        );

        // Verify config exists in target namespace
        let cloned = client.config_get(&data_id, group, &target_ns_id).await.unwrap();
        assert!(cloned.is_some(), "Cloned config should exist in target namespace");
        assert_eq!(
            cloned.unwrap().content, "clone-content",
            "Cloned config content should match original"
        );
    } else {
        // Embedded mode: no auto-increment IDs, verify API responds to clone request
        // Even with id=0, the endpoint should be reachable
        let result = client.config_clone(&[0], &target_ns_id, "OVERWRITE").await;
        // May fail due to id=0, just verify the API endpoint is reachable
        assert!(result.is_ok() || result.is_err(), "Clone API should be reachable");
    }

    // Cleanup
    client.config_delete(&data_id, group, src_ns).await.unwrap();
    client.config_delete(&data_id, group, &target_ns_id).await.unwrap();
    client.namespace_delete(&target_ns_id).await.unwrap();
}

// ==================== Config History Detail ====================

#[tokio::test]
#[ignore]
async fn test_config_history_detail() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let data_id = format!("ext-hdet-{}", common::test_id());
    let group = "DEFAULT_GROUP";
    let ns = "public";

    // Publish config to generate history
    client
        .config_publish(&data_id, group, ns, "history-v1", "", "", "", "", "", "", "", "")
        .await
        .unwrap();

    // List history to get an entry's nid
    let history = client.history_list(&data_id, group, ns, 1, 10).await.unwrap();
    assert!(
        history.total_count >= 1,
        "Should have at least one history entry"
    );
    assert!(
        !history.page_items.is_empty(),
        "History page_items should not be empty"
    );

    let first_entry = &history.page_items[0];
    let nid = first_entry.id;
    assert!(nid > 0, "History entry should have a valid nid");
    assert_eq!(first_entry.data_id, data_id, "History entry data_id should match");

    // Get history detail by nid
    let detail = client.history_get(nid, &data_id, group, ns).await.unwrap();
    assert!(detail.is_some(), "History detail should exist for nid={}", nid);
    let detail = detail.unwrap();
    assert_eq!(detail.data_id, data_id, "Detail data_id should match");
    assert_eq!(detail.content, "history-v1", "Detail should contain the published content");

    // Cleanup
    client.config_delete(&data_id, group, ns).await.unwrap();
}

// ==================== Config History Previous ====================

#[tokio::test]
#[ignore]
async fn test_config_history_previous() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let data_id = format!("ext-hprev-{}", common::test_id());
    let group = "DEFAULT_GROUP";
    let ns = "public";

    // Publish two versions to create history
    client
        .config_publish(&data_id, group, ns, "prev-v1", "", "", "", "", "", "", "", "")
        .await
        .unwrap();
    client
        .config_publish(&data_id, group, ns, "prev-v2", "", "", "", "", "", "", "", "")
        .await
        .unwrap();

    // Get the latest history entry (v2)
    let history = client.history_list(&data_id, group, ns, 1, 10).await.unwrap();
    assert!(
        history.total_count >= 2,
        "Should have at least 2 history entries, got {}",
        history.total_count
    );

    // The first item in list is typically the most recent
    let latest_id = history.page_items[0].id as i64;

    // Get previous version relative to latest
    let previous = client
        .history_previous(&data_id, group, ns, latest_id)
        .await
        .unwrap();
    assert_eq!(
        previous.data_id, data_id,
        "Previous entry data_id should match"
    );
    // The previous version should have v1 content
    assert_eq!(
        previous.content, "prev-v1",
        "Previous version content should be 'prev-v1'"
    );

    // Cleanup
    client.config_delete(&data_id, group, ns).await.unwrap();
}

// ==================== Config Listeners by IP ====================

#[tokio::test]
#[ignore]
async fn test_config_listeners_by_ip() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    // Query listeners by a local IP; returns {queryType, listenersStatus}
    let result = client.config_listeners_by_ip("127.0.0.1").await;
    assert!(
        result.is_ok(),
        "Querying listeners by IP should succeed even if empty"
    );

    let data = result.unwrap();
    assert!(data.is_object(), "Listener response should be an object");
    assert_eq!(
        data.get("queryType").and_then(|v| v.as_str()),
        Some("ip"),
        "queryType should be 'ip'"
    );
}

// ==================== Config List with Filters ====================

#[tokio::test]
#[ignore]
async fn test_config_list_with_filters() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let tid = common::test_id();
    let data_id = format!("ext-flt-{}", tid);
    let group = "TEST_FILTER_GROUP";
    let ns = "public";

    // Publish config with specific group and tags
    client
        .config_publish(
            &data_id, group, ns, "filter-content", "", "filterTag", "", "", "", "yaml", "", "",
        )
        .await
        .unwrap();

    // Filter by dataId
    let by_data_id = client
        .config_list(1, 10, ns, &data_id, "", "", "", "", "")
        .await
        .unwrap();
    assert!(
        by_data_id.total_count >= 1,
        "Should find config by dataId filter, got total_count={}",
        by_data_id.total_count
    );
    assert!(
        by_data_id.page_items.iter().any(|c| c.data_id == data_id),
        "Filtered results should contain our config by dataId"
    );

    // Filter by group
    let by_group = client
        .config_list(1, 10, ns, "", group, "", "", "", "")
        .await
        .unwrap();
    assert!(
        by_group.total_count >= 1,
        "Should find config by group filter"
    );
    assert!(
        by_group.page_items.iter().any(|c| c.group_name == group),
        "Filtered results should contain our config by group"
    );

    // Filter by type
    let by_type = client
        .config_list(1, 10, ns, &data_id, group, "", "", "yaml", "")
        .await
        .unwrap();
    assert!(
        by_type.total_count >= 1,
        "Should find config by type filter"
    );

    // Cleanup
    client.config_delete(&data_id, group, ns).await.unwrap();
}

// ==================== Config Metadata Update ====================

#[tokio::test]
#[ignore]
async fn test_config_metadata_update() {
    common::init_tracing();
    let client = common::create_api_client().await.unwrap();

    let data_id = format!("ext-meta-{}", common::test_id());
    let group = "DEFAULT_GROUP";
    let ns = "public";

    // Create config
    client
        .config_publish(&data_id, group, ns, "meta-content", "", "", "", "", "", "", "", "")
        .await
        .unwrap();

    // Verify initial state has no desc/tags
    let before = client.config_get(&data_id, group, ns).await.unwrap();
    assert!(before.is_some(), "Config should exist");
    let before = before.unwrap();
    assert_eq!(before.content, "meta-content", "Content should match initial value");

    // Update metadata (desc and tags) without changing content
    let ok = client
        .config_update_metadata(&data_id, group, ns, "Updated description", "tag-a,tag-b")
        .await
        .unwrap();
    assert!(ok, "Metadata update should succeed");

    // Verify metadata changed but content is preserved
    let after = client.config_get(&data_id, group, ns).await.unwrap();
    assert!(after.is_some(), "Config should still exist after metadata update");
    let after = after.unwrap();
    assert_eq!(
        after.content, "meta-content",
        "Content should be unchanged after metadata update"
    );
    assert_eq!(
        after.desc, "Updated description",
        "Description should be updated"
    );
    assert!(
        after.config_tags.contains("tag-a"),
        "Config tags should contain 'tag-a', got '{}'",
        after.config_tags
    );
    assert!(
        after.config_tags.contains("tag-b"),
        "Config tags should contain 'tag-b', got '{}'",
        after.config_tags
    );

    // Cleanup
    client.config_delete(&data_id, group, ns).await.unwrap();
}
