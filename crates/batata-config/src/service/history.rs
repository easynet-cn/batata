//! Config history service layer
//!
//! This module provides database operations for config history management:
//! - History search with pagination
//! - History retrieval by ID
//! - Config lookup by namespace
//! - Version comparison (diff)
//! - Rollback to historical version

use sea_orm::{prelude::Expr, sea_query::Asterisk, *};
use serde::{Deserialize, Serialize};

use batata_api::Page;
use batata_persistence::entity::{config_info, his_config_info};

use crate::model::{ConfigHistoryInfo, ConfigInfoWrapper};

/// Result of comparing two config history versions
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigDiff {
    /// Content of the older version
    pub old_content: String,
    /// Content of the newer version
    pub new_content: String,
    /// MD5 of the older version
    pub old_md5: String,
    /// MD5 of the newer version
    pub new_md5: String,
    /// Modified time of the older version (epoch millis)
    pub old_modified_time: i64,
    /// Modified time of the newer version (epoch millis)
    pub new_modified_time: i64,
}

/// Search config history with pagination
pub async fn search_page(
    db: &DatabaseConnection,
    data_id: &str,
    group_name: &str,
    namespace_id: &str,
    page_no: u64,
    page_size: u64,
) -> anyhow::Result<Page<ConfigHistoryInfo>> {
    let total_count = his_config_info::Entity::find()
        .select_only()
        .column_as(Expr::col(Asterisk).count(), "count")
        .filter(his_config_info::Column::TenantId.eq(namespace_id))
        .filter(his_config_info::Column::DataId.contains(data_id))
        .filter(his_config_info::Column::GroupId.contains(group_name))
        .into_tuple::<i64>()
        .one(db)
        .await?
        .unwrap_or_default() as u64;

    if total_count > 0 {
        let offset = (page_no - 1) * page_size;
        let page_item = his_config_info::Entity::find()
            .filter(his_config_info::Column::TenantId.eq(namespace_id))
            .filter(his_config_info::Column::DataId.contains(data_id))
            .filter(his_config_info::Column::GroupId.contains(group_name))
            .offset(offset)
            .limit(page_size)
            .order_by_desc(his_config_info::Column::Nid)
            .all(db)
            .await?
            .iter()
            .map(ConfigHistoryInfo::from)
            .collect();

        return Ok(Page::<ConfigHistoryInfo>::new(
            total_count,
            page_no,
            page_size,
            page_item,
        ));
    }

    Ok(Page::<ConfigHistoryInfo>::default())
}

/// Find history entry by ID
pub async fn find_by_id(
    db: &DatabaseConnection,
    id: u64,
) -> anyhow::Result<Option<ConfigHistoryInfo>> {
    his_config_info::Entity::find_by_id(id)
        .one(db)
        .await?
        .map_or(Ok(None), |e| Ok(Some(ConfigHistoryInfo::from(e))))
}

/// Find all configs by namespace ID
pub async fn find_configs_by_namespace_id(
    db: &DatabaseConnection,
    namespace_id: &str,
) -> anyhow::Result<Vec<ConfigInfoWrapper>> {
    let config_infos = config_info::Entity::find()
        .select_only()
        .columns([
            config_info::Column::Id,
            config_info::Column::DataId,
            config_info::Column::GroupId,
            config_info::Column::TenantId,
            config_info::Column::AppName,
            config_info::Column::Type,
        ])
        .filter(config_info::Column::TenantId.eq(namespace_id))
        .all(db)
        .await?
        .iter()
        .map(ConfigInfoWrapper::from)
        .collect();

    Ok(config_infos)
}

/// Get history count for a specific config
pub async fn get_history_count(
    db: &DatabaseConnection,
    data_id: &str,
    group: &str,
    namespace_id: &str,
) -> anyhow::Result<u64> {
    let count = his_config_info::Entity::find()
        .select_only()
        .column_as(Expr::col(Asterisk).count(), "count")
        .filter(his_config_info::Column::TenantId.eq(namespace_id))
        .filter(his_config_info::Column::DataId.eq(data_id))
        .filter(his_config_info::Column::GroupId.eq(group))
        .into_tuple::<i64>()
        .one(db)
        .await?
        .unwrap_or_default() as u64;

    Ok(count)
}

/// Get the previous version of a config
pub async fn get_previous_version(
    db: &DatabaseConnection,
    data_id: &str,
    group: &str,
    namespace_id: &str,
    current_nid: i64,
) -> anyhow::Result<Option<ConfigHistoryInfo>> {
    his_config_info::Entity::find()
        .filter(his_config_info::Column::TenantId.eq(namespace_id))
        .filter(his_config_info::Column::DataId.eq(data_id))
        .filter(his_config_info::Column::GroupId.eq(group))
        .filter(his_config_info::Column::Nid.lt(current_nid))
        .order_by_desc(his_config_info::Column::Nid)
        .one(db)
        .await?
        .map_or(Ok(None), |e| Ok(Some(ConfigHistoryInfo::from(e))))
}

/// Compare two history versions and return a diff
pub async fn compare_versions(
    db: &DatabaseConnection,
    nid1: u64,
    nid2: u64,
) -> anyhow::Result<Option<(ConfigHistoryInfo, ConfigHistoryInfo)>> {
    let version1 = find_by_id(db, nid1).await?;
    let version2 = find_by_id(db, nid2).await?;

    match (version1, version2) {
        (Some(v1), Some(v2)) => Ok(Some((v1, v2))),
        _ => Ok(None),
    }
}

/// Get all history versions for a config (for rollback selection)
pub async fn get_all_versions(
    db: &DatabaseConnection,
    data_id: &str,
    group: &str,
    namespace_id: &str,
    limit: u64,
) -> anyhow::Result<Vec<ConfigHistoryInfo>> {
    let versions = his_config_info::Entity::find()
        .filter(his_config_info::Column::TenantId.eq(namespace_id))
        .filter(his_config_info::Column::DataId.eq(data_id))
        .filter(his_config_info::Column::GroupId.eq(group))
        .order_by_desc(his_config_info::Column::Nid)
        .limit(limit)
        .all(db)
        .await?
        .iter()
        .map(ConfigHistoryInfo::from)
        .collect();

    Ok(versions)
}

/// Search history with advanced filters
#[allow(clippy::too_many_arguments)]
pub async fn search_with_filters(
    db: &DatabaseConnection,
    data_id: &str,
    group_name: &str,
    namespace_id: &str,
    op_type: Option<&str>,
    src_user: Option<&str>,
    start_time: Option<i64>,
    end_time: Option<i64>,
    page_no: u64,
    page_size: u64,
) -> anyhow::Result<Page<ConfigHistoryInfo>> {
    let mut query =
        his_config_info::Entity::find().filter(his_config_info::Column::TenantId.eq(namespace_id));

    if !data_id.is_empty() {
        query = query.filter(his_config_info::Column::DataId.contains(data_id));
    }
    if !group_name.is_empty() {
        query = query.filter(his_config_info::Column::GroupId.contains(group_name));
    }
    if let Some(op) = op_type {
        query = query.filter(his_config_info::Column::OpType.eq(op));
    }
    if let Some(user) = src_user {
        query = query.filter(his_config_info::Column::SrcUser.contains(user));
    }
    if let Some(start) = start_time {
        query = query.filter(
            his_config_info::Column::GmtModified
                .gte(chrono::DateTime::from_timestamp_millis(start).unwrap_or_default()),
        );
    }
    if let Some(end) = end_time {
        query = query.filter(
            his_config_info::Column::GmtModified
                .lte(chrono::DateTime::from_timestamp_millis(end).unwrap_or_default()),
        );
    }

    // Get total count
    let count_query = query.clone();
    let total_count = count_query
        .select_only()
        .column_as(Expr::col(Asterisk).count(), "count")
        .into_tuple::<i64>()
        .one(db)
        .await?
        .unwrap_or_default() as u64;

    if total_count > 0 {
        let offset = (page_no - 1) * page_size;
        let page_items = query
            .offset(offset)
            .limit(page_size)
            .order_by_desc(his_config_info::Column::Nid)
            .all(db)
            .await?
            .iter()
            .map(ConfigHistoryInfo::from)
            .collect();

        return Ok(Page::<ConfigHistoryInfo>::new(
            total_count,
            page_no,
            page_size,
            page_items,
        ));
    }

    Ok(Page::<ConfigHistoryInfo>::default())
}

/// Compare two history versions and return a structured diff.
///
/// Both versions must belong to the same config (data_id, group, tenant).
/// Returns an error if either version is not found.
pub async fn diff_versions(
    db: &DatabaseConnection,
    nid1: u64,
    nid2: u64,
) -> anyhow::Result<ConfigDiff> {
    let v1 = find_by_id(db, nid1)
        .await?
        .ok_or_else(|| anyhow::anyhow!("History version {} not found", nid1))?;

    let v2 = find_by_id(db, nid2)
        .await?
        .ok_or_else(|| anyhow::anyhow!("History version {} not found", nid2))?;

    Ok(ConfigDiff {
        old_content: v1.content,
        new_content: v2.content,
        old_md5: v1.md5,
        new_md5: v2.md5,
        old_modified_time: v1.last_modified_time,
        new_modified_time: v2.last_modified_time,
    })
}

/// Rollback a config to a historical version by re-publishing the old content.
///
/// Retrieves the content from the specified history entry and publishes it
/// as the current version using `config::create_or_update`.
pub async fn rollback_to_version(
    db: &DatabaseConnection,
    nid: u64,
    data_id: &str,
    group: &str,
    tenant: &str,
    operator: &str,
    client_ip: &str,
) -> anyhow::Result<bool> {
    let history = find_by_id(db, nid)
        .await?
        .ok_or_else(|| anyhow::anyhow!("History version {} not found", nid))?;

    // Re-publish the historical content as current config
    super::config::create_or_update(
        db,
        data_id,
        group,
        tenant,
        &history.content,
        "",       // app_name
        operator, // src_user
        client_ip,
        "", // config_tags
        "", // desc
        "", // use
        "", // effect
        "", // type
        "", // schema
        "", // encrypted_data_key
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_diff_serialization() {
        let diff = ConfigDiff {
            old_content: "key=old_value".to_string(),
            new_content: "key=new_value".to_string(),
            old_md5: "abc123".to_string(),
            new_md5: "def456".to_string(),
            old_modified_time: 1700000000000,
            new_modified_time: 1700000060000,
        };

        let json = serde_json::to_string(&diff).unwrap();
        assert!(json.contains("oldContent"));
        assert!(json.contains("newContent"));
        assert!(json.contains("oldMd5"));
        assert!(json.contains("newMd5"));
        assert!(json.contains("oldModifiedTime"));
        assert!(json.contains("newModifiedTime"));

        let deserialized: ConfigDiff = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.old_content, "key=old_value");
        assert_eq!(deserialized.new_content, "key=new_value");
        assert_eq!(deserialized.old_md5, "abc123");
        assert_eq!(deserialized.new_md5, "def456");
        assert_eq!(deserialized.old_modified_time, 1700000000000);
        assert_eq!(deserialized.new_modified_time, 1700000060000);
    }

    #[test]
    fn test_config_diff_identical_content() {
        let diff = ConfigDiff {
            old_content: "same_content".to_string(),
            new_content: "same_content".to_string(),
            old_md5: "same_md5".to_string(),
            new_md5: "same_md5".to_string(),
            old_modified_time: 1700000000000,
            new_modified_time: 1700000060000,
        };

        assert_eq!(diff.old_content, diff.new_content);
        assert_eq!(diff.old_md5, diff.new_md5);
        // Times can still differ even if content is the same (re-publish)
        assert_ne!(diff.old_modified_time, diff.new_modified_time);
    }

    #[test]
    fn test_config_diff_empty_content() {
        let diff = ConfigDiff {
            old_content: String::new(),
            new_content: "new_content".to_string(),
            old_md5: String::new(),
            new_md5: "md5hash".to_string(),
            old_modified_time: 0,
            new_modified_time: 1700000000000,
        };

        assert!(diff.old_content.is_empty());
        assert!(!diff.new_content.is_empty());
    }

    #[test]
    fn test_page_calculation() {
        // Test page offset calculation
        let page_no = 2u64;
        let page_size = 10u64;
        let offset = (page_no - 1) * page_size;
        assert_eq!(offset, 10);
    }

    #[test]
    fn test_page_calculation_first_page() {
        let page_no = 1u64;
        let page_size = 10u64;
        let offset = (page_no - 1) * page_size;
        assert_eq!(offset, 0);
    }

    #[test]
    fn test_page_calculation_large_page() {
        let page_no = 1000u64;
        let page_size = 50u64;
        let offset = (page_no - 1) * page_size;
        assert_eq!(offset, 49950);
    }

    #[test]
    fn test_page_calculation_single_item_page() {
        let page_no = 5u64;
        let page_size = 1u64;
        let offset = (page_no - 1) * page_size;
        assert_eq!(offset, 4);
    }
}
