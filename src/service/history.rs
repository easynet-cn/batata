use sea_orm::{prelude::Expr, sea_query::Asterisk, *};

use crate::{
    api::model::Page,
    config::model::{ConfigHistoryInfo, ConfigInfoWrapper},
    entity::{config_info, his_config_info},
};

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

pub async fn find_by_id(
    db: &DatabaseConnection,
    id: u64,
) -> anyhow::Result<Option<ConfigHistoryInfo>> {
    his_config_info::Entity::find_by_id(id)
        .one(db)
        .await?
        .map_or(Ok(None), |e| Ok(Some(ConfigHistoryInfo::from(e))))
}

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
