//! Config history service layer
//!
//! This module provides database operations for config history management:
//! - History search with pagination
//! - History retrieval by ID
//! - Config lookup by namespace

use sea_orm::{prelude::Expr, sea_query::Asterisk, *};

use batata_api::Page;
use batata_persistence::entity::{config_info, his_config_info};

use crate::model::{ConfigHistoryInfo, ConfigInfoWrapper};

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

#[cfg(test)]
mod tests {
    #[test]
    fn test_page_calculation() {
        // Test page offset calculation
        let page_no = 2u64;
        let page_size = 10u64;
        let offset = (page_no - 1) * page_size;
        assert_eq!(offset, 10);
    }
}
