use std::collections::HashMap;

use sea_orm::*;

use crate::{
    common::model::{ConfigInfo, Page},
    entity::config_info,
};

pub async fn find_config_info_like_4_page(
    db: &DatabaseConnection,
    page_no: u64,
    page_size: u64,
    data_id: String,
    group: String,
    tenant: String,
    config_advance_info: HashMap<String, String>,
) -> Page<ConfigInfo> {
    let mut config_info_count_select =
        config_info::Entity::find().filter(config_info::Column::TenantId.eq(tenant.clone()));
    let mut config_info_select =
        config_info::Entity::find().filter(config_info::Column::TenantId.eq(tenant));

    if !data_id.is_empty() {
        config_info_count_select =
            config_info_count_select.filter(config_info::Column::DataId.contains(data_id.clone()));
        config_info_select =
            config_info_select.filter(config_info::Column::DataId.contains(data_id));
    }
    if !group.is_empty() {
        config_info_count_select =
            config_info_count_select.filter(config_info::Column::GroupId.contains(group.clone()));
        config_info_select =
            config_info_select.filter(config_info::Column::GroupId.contains(group));
    }
    if let Some(app_name) = config_advance_info.get("app_name") {
        config_info_count_select = config_info_count_select
            .filter(config_info::Column::AppName.contains(app_name.clone()));
        config_info_select =
            config_info_select.filter(config_info::Column::AppName.contains(app_name));
    }
    if let Some(content) = config_advance_info.get("content") {
        config_info_count_select =
            config_info_count_select.filter(config_info::Column::Content.contains(content.clone()));
        config_info_select =
            config_info_select.filter(config_info::Column::Content.contains(content));
    }

    let total_count = config_info_count_select.count(db).await.unwrap();
    let mut page_items = Vec::<ConfigInfo>::new();

    if total_count > 0 {
        let config_infos = config_info_select
            .paginate(db, page_size)
            .fetch_page(page_no - 1)
            .await
            .unwrap();

        for config_info in config_infos {
            page_items.push(ConfigInfo {
                id: config_info.id.clone(),
                data_id: config_info.data_id.clone(),
                group: config_info.group_id.clone().unwrap_or_default(),
                content: config_info.content.clone(),
                md5: config_info.md5.clone().unwrap_or_default(),
                encrypted_data_key: config_info.encrypted_data_key.clone(),
                tenant: config_info.tenant_id.clone().unwrap_or_default(),
                app_name: config_info.app_name.clone().unwrap_or_default(),
                _type: config_info.r#type.clone().unwrap_or_default(),
            });
        }
    }

    let page_result = Page::<ConfigInfo> {
        total_count: total_count,
        page_number: page_no,
        pages_available: (total_count as f64 / page_size as f64).ceil() as u64,
        page_items: page_items,
    };

    page_result
}

fn check_cipher(data_id: String) -> bool {
    data_id.starts_with("cipher-") && !data_id.eq("cipher-")
}
