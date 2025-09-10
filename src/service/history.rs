use sea_orm::{prelude::Expr, sea_query::Asterisk, *};

use crate::{
    api::model::Page,
    config::model::ConfigHistoryInfo,
    entity::{config_info, his_config_info},
    model::config::ConfigInfoWrapper,
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

pub async fn get_by_id(
    db: &DatabaseConnection,
    id: u64,
) -> anyhow::Result<Option<ConfigHistoryInfo>> {
    let config_history_info = his_config_info::Entity::find_by_id(id)
        .select_only()
        .columns([
            his_config_info::Column::Id,
            his_config_info::Column::Nid,
            his_config_info::Column::DataId,
            his_config_info::Column::GroupId,
            his_config_info::Column::TenantId,
            his_config_info::Column::AppName,
            his_config_info::Column::Content,
            his_config_info::Column::Md5,
            his_config_info::Column::SrcUser,
            his_config_info::Column::SrcIp,
            his_config_info::Column::OpType,
            his_config_info::Column::GmtCreate,
            his_config_info::Column::GmtModified,
            his_config_info::Column::EncryptedDataKey,
        ])
        .one(db)
        .await?
        .map(|entity| ConfigHistoryInfo::from(entity));

    Ok(config_history_info)
}
pub async fn get_config_list_by_namespace(
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
            config_info::Column::GmtModified,
        ])
        .filter(config_info::Column::TenantId.eq(namespace_id))
        .all(db)
        .await
        .unwrap()
        .iter()
        .map(|entity| ConfigInfoWrapper::from(entity.clone()))
        .collect();

    Ok(config_infos)
}
