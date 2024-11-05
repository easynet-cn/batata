use std::collections::HashMap;

use chrono::NaiveDateTime;
use crypto::{digest::Digest, md5::Md5};
use sea_orm::*;

use crate::{
    common::model::{ConfigAllInfo, ConfigInfo, Page},
    entity::{config_info, config_tags_relation, his_config_info},
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

    let total_count = count_select.count(db).await.unwrap();
    let mut page_items = Vec::<ConfigInfo>::new();

    if total_count > 0 {
        let config_infos = query_select
            .paginate(db, page_size)
            .fetch_page(page_no - 1)
            .await
            .unwrap();

        for config_info in config_infos {
            page_items.push(ConfigInfo {
                id: config_info.id.clone(),
                data_id: config_info.data_id.clone(),
                group: config_info.group_id.clone().unwrap_or_default(),
                content: config_info.content.clone().unwrap_or_default(),
                md5: config_info.md5.clone().unwrap_or_default(),
                encrypted_data_key: config_info.encrypted_data_key.clone().unwrap_or_default(),
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

pub async fn find_config_all_info(
    db: &DatabaseConnection,
    data_id: &str,
    group: &str,
    tenant: &str,
) -> anyhow::Result<ConfigAllInfo> {
    let tags: Vec<String> = config_tags_relation::Entity::find()
        .select_only()
        .column(config_tags_relation::Column::TagName)
        .filter(config_tags_relation::Column::DataId.eq(data_id))
        .filter(config_tags_relation::Column::GroupId.eq(group))
        .filter(config_tags_relation::Column::TenantId.eq(tenant))
        .all(db)
        .await
        .unwrap()
        .iter()
        .map(|entity| entity.tag_name.clone())
        .collect();
    let config_all_info = config_info::Entity::find()
        .filter(config_info::Column::DataId.eq(data_id))
        .filter(config_info::Column::GroupId.eq(group))
        .filter(config_info::Column::TenantId.eq(tenant))
        .one(db)
        .await
        .unwrap()
        .map(|entity| ConfigAllInfo {
            id: entity.id,
            data_id: entity.data_id,
            group: entity.group_id.unwrap_or_default(),
            content: entity.content.unwrap_or_default(),
            md5: entity.md5.unwrap_or_default(),
            encrypted_data_key: entity.encrypted_data_key.unwrap_or_default(),
            app_name: entity.app_name.unwrap_or_default(),
            tenant: entity.tenant_id.unwrap_or_default(),
            _type: entity.r#type.unwrap_or_default(),
            create_time: entity.gmt_create.unwrap().and_utc().timestamp(),
            modify_time: entity.gmt_modified.unwrap().and_utc().timestamp(),
            create_user: entity.src_user.unwrap_or_default(),
            create_ip: entity.src_ip.unwrap_or_default(),
            desc: entity.c_desc.unwrap_or_default(),
            r#use: entity.c_use.unwrap_or_default(),
            effect: entity.effect.unwrap_or_default(),
            schema: entity.c_schema.unwrap_or_default(),
            config_tags: tags.join(","),
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
