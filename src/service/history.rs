use sea_orm::*;

use crate::{
    common::model::{ConfigHistoryInfo, ConfigInfoWrapper, Page},
    entity::{config_info, his_config_info},
};

pub async fn search_page(
    db: &DatabaseConnection,
    data_id: &str,
    group: &str,
    tenant: &str,
    page_no: u64,
    page_size: u64,
) -> anyhow::Result<Page<ConfigHistoryInfo>> {
    let count_select = his_config_info::Entity::find()
        .filter(his_config_info::Column::TenantId.eq(tenant))
        .filter(his_config_info::Column::DataId.contains(data_id))
        .filter(his_config_info::Column::GroupId.contains(group));
    let query_select = his_config_info::Entity::find()
        .filter(his_config_info::Column::TenantId.eq(tenant))
        .filter(his_config_info::Column::DataId.contains(data_id))
        .filter(his_config_info::Column::GroupId.contains(group));

    let total_count = count_select.count(db).await?;
    let mut page_items = Vec::<ConfigHistoryInfo>::new();

    if total_count > 0 {
        let his_config_infos = query_select
            .paginate(db, page_size)
            .fetch_page(page_no - 1)
            .await?;

        for his_config_info in his_config_infos {
            page_items.push(ConfigHistoryInfo {
                id: his_config_info.nid.clone() as i64,
                last_id: -1,
                data_id: his_config_info.data_id.clone(),
                group: his_config_info.group_id.clone(),
                tenant: his_config_info.tenant_id.clone().unwrap_or_default(),
                app_name: his_config_info.app_name.clone().unwrap_or_default(),
                md5: his_config_info.md5.clone().unwrap_or_default(),
                content: his_config_info.content.clone(),
                src_ip: his_config_info.src_ip.clone().unwrap_or_default(),
                src_user: his_config_info.src_user.clone().unwrap_or_default(),
                op_type: his_config_info.op_type.clone().unwrap_or_default(),
                created_time: his_config_info.gmt_create.clone(),
                last_modified_time: his_config_info.gmt_modified.clone(),
                encrypted_data_key: his_config_info.encrypted_data_key.clone(),
            });
        }
    }

    let page_result = Page::<ConfigHistoryInfo> {
        total_count: total_count,
        page_number: page_no,
        pages_available: (total_count as f64 / page_size as f64).ceil() as u64,
        page_items: page_items,
    };

    Ok(page_result)
}

pub async fn get_config_list_by_namespace(
    db: &DatabaseConnection,
    namespace_id: &str,
) -> anyhow::Result<Vec<ConfigInfoWrapper>> {
    let config_infos = config_info::Entity::find()
        .filter(config_info::Column::TenantId.eq(namespace_id))
        .all(db)
        .await
        .unwrap()
        .iter()
        .map(|config_info| ConfigInfoWrapper {
            id: 0,
            data_id: config_info.data_id.clone(),
            group: config_info.group_id.clone().unwrap_or_default(),
            content: String::from(""),
            md5: String::from(""),
            encrypted_data_key: String::from(""),
            tenant: config_info.tenant_id.clone().unwrap_or_default(),
            app_name: config_info.app_name.clone().unwrap_or_default(),
            _type: config_info.r#type.clone().unwrap_or_default(),
            last_modified: config_info.gmt_modified.and_utc().timestamp(),
        })
        .collect();

    Ok(config_infos)
}
