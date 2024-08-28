use serde::Deserialize;

use sea_orm::*;

use crate::{
    common::model::{ConfigHistoryInfo, Page},
    entity::{config_info, his_config_info, tenant_info},
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConfigHistorySearchPageParam {
    search: Option<String>,
    data_id: Option<String>,
    group: Option<String>,
    tenant: Option<String>,
    app_name: Option<String>,
    #[serde(rename = "config_tags")]
    config_tags: Option<String>,
    page_no: u64,
    page_size: u64,
}

pub async fn search_page(
    db: &DatabaseConnection,
    data_id: String,
    group: String,
    tenant: String,
    page_no: u64,
    page_size: u64,
) -> anyhow::Result<Page<ConfigHistoryInfo>> {
    let count_select = his_config_info::Entity::find()
        .filter(his_config_info::Column::TenantId.eq(tenant.clone()))
        .filter(his_config_info::Column::DataId.contains(data_id.clone()))
        .filter(his_config_info::Column::GroupId.contains(group.clone()));
    let query_select = his_config_info::Entity::find()
        .filter(his_config_info::Column::TenantId.eq(tenant.clone()))
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
