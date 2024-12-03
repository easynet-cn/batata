use std::collections::HashMap;

use chrono::{Local, NaiveDateTime};
use crypto::{digest::Digest, md5::Md5};
use sea_orm::*;

use crate::{
    common::model::{ConfigAllInfo, ConfigInfo, ConfigInfoStateWrapper, Page},
    entity::{config_info, config_tags_relation, his_config_info},
};

pub async fn search_page(
    db: &DatabaseConnection,
    page_no: u64,
    page_size: u64,
    tenant: &str,
    data_id: &str,
    group: &str,
    app_name: &str,
    config_tags: &str,
    types: &str,
    content: &str,
) -> anyhow::Result<Page<ConfigInfo>> {
    let mut count_select =
        config_info::Entity::find().filter(config_info::Column::TenantId.eq(tenant));
    let mut query_select =
        config_info::Entity::find().filter(config_info::Column::TenantId.eq(tenant));

    if !data_id.is_empty() {
        count_select = count_select.filter(config_info::Column::DataId.contains(data_id));
        query_select = query_select.filter(config_info::Column::DataId.contains(data_id));
    }
    if !group.is_empty() {
        count_select = count_select.filter(config_info::Column::GroupId.contains(group));
        query_select = query_select.filter(config_info::Column::GroupId.contains(group));
    }
    if !app_name.is_empty() {
        count_select = count_select.filter(config_info::Column::AppName.contains(app_name));
        query_select = query_select.filter(config_info::Column::AppName.contains(app_name));
    }
    if !content.is_empty() {
        count_select = count_select.filter(config_info::Column::Content.contains(content));
        query_select = query_select.filter(config_info::Column::Content.contains(content));
    }

    let total_count = count_select.count(db).await?;

    if total_count > 0 {
        let page_items = query_select
            .paginate(db, page_size)
            .fetch_page(page_no - 1)
            .await?
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

pub async fn find_state(
    db: &DatabaseConnection,
    data_id: &str,
    group: &str,
    tenant: &str,
) -> anyhow::Result<Option<ConfigInfoStateWrapper>> {
    let result = config_info::Entity::find()
        .select_only()
        .columns([
            config_info::Column::Id,
            config_info::Column::DataId,
            config_info::Column::GroupId,
            config_info::Column::TenantId,
            config_info::Column::GmtModified,
        ])
        .filter(config_info::Column::DataId.eq(data_id))
        .filter(config_info::Column::GroupId.eq(group))
        .filter(config_info::Column::TenantId.eq(tenant))
        .one(db)
        .await?
        .map(|entity| ConfigInfoStateWrapper::from(entity.clone()));

    anyhow::Ok(result)
}

pub async fn create_or_update(
    db: &DatabaseConnection,
    data_id: &str,
    group: &str,
    tenant: &str,
    content: &str,
    tag: &str,
    app_name: &str,
    src_user: &str,
    src_ip: &str,
    config_tags: &str,
    desc: &str,
    r#use: &str,
    efect: &str,
    r#type: &str,
    schema: &str,
    encrypted_data_key: &str,
) -> anyhow::Result<bool> {
    let entity_option = config_info::Entity::find()
        .filter(config_info::Column::DataId.eq(data_id))
        .filter(config_info::Column::GroupId.eq(group))
        .filter(config_info::Column::TenantId.eq(tenant))
        .one(db)
        .await?;

    let now = Local::now().naive_local();

    return match entity_option {
        Some(entity) => {
            let entity_c = entity.clone();
            let mut model: config_info::ActiveModel = entity.into();

            model.content = Set(Some(content.to_string()));
            model.md5 = Set(Some(md5_digest(content)));
            model.src_user = Set(Some(src_user.to_string()));
            model.src_ip = Set(Some((src_ip.to_string())));
            model.app_name = Set(Some(app_name.to_string()));
            model.c_desc = Set(Some(desc.to_string()));
            model.c_use = Set(Some(r#use.to_string()));
            model.effect = Set(Some(efect.to_string()));
            model.r#type = Set(Some(r#type.to_string()));
            model.c_schema = Set(Some(schema.to_string()));
            model.encrypted_data_key = Set(Some(encrypted_data_key.to_string()));

            if model.is_changed() {
                model.gmt_modified = Set(Some(now));

                model.update(db).await?;

                his_config_info::ActiveModel {
                    id: Set(entity_c.id as u64),
                    data_id: Set(entity_c.data_id),
                    group_id: Set(entity_c.group_id.unwrap_or_default()),
                    app_name: Set(entity_c.app_name),
                    content: Set(entity_c.content.unwrap_or_default()),
                    md5: Set(Some(entity_c.md5.unwrap_or_default())),
                    gmt_create: Set(entity_c.gmt_create.unwrap()),
                    gmt_modified: Set(entity_c.gmt_modified.unwrap()),
                    src_user: Set(Some(entity_c.src_user.unwrap_or_default())),
                    src_ip: Set(Some(entity_c.src_ip.unwrap_or_default())),
                    op_type: Set(Some(String::from("U"))),
                    tenant_id: Set(Some(entity_c.tenant_id.unwrap_or_default())),
                    encrypted_data_key: Set(entity_c.encrypted_data_key.unwrap_or_default()),
                    ..Default::default()
                }
                .insert(db)
                .await?;
            }

            anyhow::Ok(true)
        }
        None => {
            let model = config_info::ActiveModel {
                data_id: Set(data_id.to_string()),
                group_id: Set(Some(group.to_string())),
                content: Set(Some(content.to_string())),
                md5: Set(Some(md5_digest(content))),
                gmt_create: Set(Some(now)),
                gmt_modified: Set(Some(now)),
                src_user: Set(Some(src_user.to_string())),
                src_ip: Set(Some((src_ip.to_string()))),
                app_name: Set(Some(app_name.to_string())),
                tenant_id: Set(Some(tenant.to_string())),
                c_desc: Set(Some(desc.to_string())),
                c_use: Set(Some(r#use.to_string())),
                effect: Set(Some(efect.to_string())),
                r#type: Set(Some(r#type.to_string())),
                c_schema: Set(Some(schema.to_string())),
                encrypted_data_key: Set(Some(encrypted_data_key.to_string())),
                ..Default::default()
            };

            let entity_c = model.insert(db).await?;

            his_config_info::ActiveModel {
                id: Set(entity_c.id as u64),
                data_id: Set(entity_c.data_id),
                group_id: Set(entity_c.group_id.unwrap_or_default()),
                app_name: Set(entity_c.app_name),
                content: Set(entity_c.content.unwrap_or_default()),
                md5: Set(Some(entity_c.md5.unwrap_or_default())),
                gmt_create: Set(entity_c.gmt_create.unwrap()),
                gmt_modified: Set(entity_c.gmt_modified.unwrap()),
                src_user: Set(Some(entity_c.src_user.unwrap_or_default())),
                src_ip: Set(Some(entity_c.src_ip.unwrap_or_default())),
                op_type: Set(Some(String::from("I"))),
                tenant_id: Set(Some(entity_c.tenant_id.unwrap_or_default())),
                encrypted_data_key: Set(entity_c.encrypted_data_key.unwrap_or_default()),
                ..Default::default()
            }
            .insert(db)
            .await?;

            anyhow::Ok(true)
        }
    };
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
