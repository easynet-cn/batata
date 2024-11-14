use std::collections::HashMap;

use chrono::NaiveDateTime;
use crypto::{digest::Digest, md5::Md5};
use sea_orm::*;

use crate::{
    common::model::{ConfigAllInfo, ConfigInfo, Page},
    entity::{config_info, config_tags_relation, his_config_info},
};

pub async fn search_page(
    db: &DatabaseConnection,
    page_no: u64,
    page_size: u64,
    data_id: String,
    group: String,
    tenant: String,
    config_advance_info: HashMap<String, String>,
) -> anyhow::Result<Page<ConfigInfo>> {
    let mut count_select =
        config_info::Entity::find().filter(config_info::Column::TenantId.eq(tenant.clone()));
    let mut query_select =
        config_info::Entity::find().filter(config_info::Column::TenantId.eq(tenant));

    if !data_id.is_empty() {
        count_select = count_select.filter(config_info::Column::DataId.contains(data_id.clone()));
        query_select = query_select.filter(config_info::Column::DataId.contains(data_id));
    }
    if !group.is_empty() {
        count_select = count_select.filter(config_info::Column::GroupId.contains(group.clone()));
        query_select = query_select.filter(config_info::Column::GroupId.contains(group));
    }
    if let Some(app_name) = config_advance_info.get("app_name") {
        count_select = count_select.filter(config_info::Column::AppName.contains(app_name.clone()));
        query_select = query_select.filter(config_info::Column::AppName.contains(app_name));
    }
    if let Some(content) = config_advance_info.get("content") {
        count_select = count_select.filter(config_info::Column::Content.contains(content.clone()));
        query_select = query_select.filter(config_info::Column::Content.contains(content));
    }

    let total_count = count_select.count(db).await?;

    if total_count > 0 {
        let query_result = query_select
            .paginate(db, page_size)
            .fetch_page(page_no - 1)
            .await?;
        let page_items = query_result
            .iter()
            .map(|entity| ConfigInfo::from(entity.clone()))
            .collect();

        return anyhow::Ok(Page::<ConfigInfo>::new(
            total_count,
            page_no,
            page_size,
            page_items,
        ));
    }

    return anyhow::Ok(Page::<ConfigInfo>::default());
}

pub async fn find_all(
    db: &DatabaseConnection,
    data_id: &str,
    group: &str,
    tenant: &str,
) -> anyhow::Result<ConfigAllInfo> {
    let tags_result = config_tags_relation::Entity::find()
        .select_only()
        .column(config_tags_relation::Column::TagName)
        .filter(config_tags_relation::Column::DataId.eq(data_id))
        .filter(config_tags_relation::Column::GroupId.eq(group))
        .filter(config_tags_relation::Column::TenantId.eq(tenant))
        .all(db)
        .await?;
    let tags: Vec<String> = tags_result
        .iter()
        .map(|entity| entity.tag_name.clone())
        .collect();
    let config_all_info_result = config_info::Entity::find()
        .filter(config_info::Column::DataId.eq(data_id))
        .filter(config_info::Column::GroupId.eq(group))
        .filter(config_info::Column::TenantId.eq(tenant))
        .one(db)
        .await?;

    let config_all_info = config_all_info_result
        .map(|entity| {
            let mut m = ConfigAllInfo::from(entity.clone());

            m.config_tags = tags.join(",");

            m
        })
        .unwrap();

    Ok(config_all_info)
}

fn check_cipher(data_id: String) -> bool {
    data_id.starts_with("cipher-") && !data_id.eq("cipher-")
}

async fn insert_config_history_atomic(
    db: &DatabaseConnection,
    id: u64,
    config_info: ConfigInfo,
    src_ip: String,
    src_user: String,
    time: NaiveDateTime,
    ops: String,
) -> anyhow::Result<()> {
    let content = md5_digest(config_info.content.as_str());

    let his_config_info = his_config_info::ActiveModel {
        id: Set(id),
        data_id: Set(config_info.data_id),
        group_id: Set(config_info.group),
        app_name: Set(Some(config_info.app_name)),
        content: Set(content),
        md5: Set(Some(config_info.md5)),
        gmt_create: Set(time),
        gmt_modified: Set(time),
        src_user: Set(Some(src_user)),
        src_ip: Set(Some(src_ip)),
        op_type: Set(Some(ops)),
        tenant_id: Set(Some(config_info.tenant)),
        encrypted_data_key: Set(config_info.encrypted_data_key),
        ..Default::default()
    };

    his_config_info::Entity::insert(his_config_info)
        .exec(db)
        .await
        .map_err(|e| anyhow::anyhow!("insert config history failed: {:?}", e));

    Ok(())
}

fn md5_digest(content: &str) -> String {
    let mut md5 = Md5::new();

    md5.input_str(content);

    md5.result_str()
}
