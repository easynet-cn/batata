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
    let mut config_info_select =
        config_info::Entity::find().filter(config_info::Column::TenantId.eq(tenant));

    if !data_id.is_empty() {
        config_info_select =
            config_info_select.filter(config_info::Column::DataId.contains(data_id));
    }
    if !group.is_empty() {
        config_info_select =
            config_info_select.filter(config_info::Column::GroupId.contains(group));
    }
    if let Some(app_name) = config_advance_info.get("app_name") {
        config_info_select =
            config_info_select.filter(config_info::Column::AppName.contains(app_name));
    }
    if let Some(conent) = config_advance_info.get("conent") {
        config_info_select =
            config_info_select.filter(config_info::Column::Content.contains(conent));
    }

    let start = (page_no - 1) * page_size;

    let config_infos = config_info_select
        .offset(start)
        .limit(page_size)
        .all(db)
        .await
        .unwrap();
    let mut page_items = Vec::<ConfigInfo>::with_capacity(config_infos.len());

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

    let page_result = Page::<ConfigInfo> {
        total_count: 1,
        page_number: 1,
        pages_available: 1,
        page_items: page_items,
    };

    page_result
}
