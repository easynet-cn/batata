// Allow many arguments for config service operations - parameters match Nacos API
#![allow(clippy::too_many_arguments)]

use std::collections::HashMap;

use anyhow::Ok;
use chrono::Local;
use sea_orm::{prelude::Expr, sea_query::Asterisk, *};

use crate::{
    api::{config::model::ConfigBasicInfo, model::Page},
    config::model::{ConfigAllInfo, ConfigInfoGrayWrapper},
    entity::{config_info, config_info_gray, config_tags_relation, his_config_info},
};

pub async fn find_one(
    db: &DatabaseConnection,
    data_id: &str,
    group: &str,
    namespace_id: &str,
) -> anyhow::Result<Option<ConfigAllInfo>> {
    // Execute both queries concurrently to reduce latency
    // config_tags_relation has data_id, group_id, tenant_id columns allowing parallel query
    let (config_result, tags_result) = tokio::join!(
        config_info::Entity::find()
            .filter(config_info::Column::DataId.eq(data_id))
            .filter(config_info::Column::GroupId.eq(group))
            .filter(config_info::Column::TenantId.eq(namespace_id))
            .one(db),
        config_tags_relation::Entity::find()
            .select_only()
            .column(config_tags_relation::Column::TagName)
            .filter(config_tags_relation::Column::DataId.eq(data_id))
            .filter(config_tags_relation::Column::GroupId.eq(group))
            .filter(config_tags_relation::Column::TenantId.eq(namespace_id))
            .order_by_asc(config_tags_relation::Column::Nid)
            .into_tuple::<String>()
            .all(db)
    );

    if let Some(mut config_all_info) = config_result?.map(ConfigAllInfo::from) {
        config_all_info.config_tags = tags_result?.join(",");
        Ok(Some(config_all_info))
    } else {
        Ok(None)
    }
}

pub async fn search_page(
    db: &DatabaseConnection,
    page_no: u64,
    page_size: u64,
    tenant_id: &str,
    data_id: &str,
    group_id: &str,
    app_name: &str,
    tags: Vec<String>,
    types: Vec<String>,
    content: &str,
) -> anyhow::Result<Page<ConfigBasicInfo>> {
    let mut count_select =
        config_info::Entity::find().filter(config_info::Column::TenantId.eq(tenant_id));
    let mut query_select =
        config_info::Entity::find().filter(config_info::Column::TenantId.eq(tenant_id));

    if !data_id.is_empty() {
        if data_id.contains("*") {
            count_select = count_select.filter(config_info::Column::DataId.like(data_id));
            query_select = query_select.filter(config_info::Column::DataId.like(data_id));
        } else {
            count_select = count_select.filter(config_info::Column::DataId.contains(data_id));
            query_select = query_select.filter(config_info::Column::DataId.contains(data_id));
        }
    }
    if !group_id.is_empty() {
        if group_id.contains("*") {
            count_select = count_select.filter(config_info::Column::GroupId.like(group_id));
            query_select = query_select.filter(config_info::Column::GroupId.like(group_id));
        } else {
            count_select = count_select.filter(config_info::Column::GroupId.eq(group_id));
            query_select = query_select.filter(config_info::Column::GroupId.eq(group_id));
        }
    }
    if !app_name.is_empty() {
        count_select = count_select.filter(config_info::Column::AppName.contains(app_name));
        query_select = query_select.filter(config_info::Column::AppName.contains(app_name));
    }
    if !tags.is_empty() {
        count_select = count_select.join(
            JoinType::InnerJoin,
            config_info::Entity::belongs_to(config_tags_relation::Entity)
                .from(config_info::Column::Id)
                .to(config_tags_relation::Column::Id)
                .into(),
        );
        count_select =
            count_select.filter(config_tags_relation::Column::TagName.is_in(tags.to_vec()));
        query_select = query_select.join(
            JoinType::InnerJoin,
            config_info::Entity::belongs_to(config_tags_relation::Entity)
                .from(config_info::Column::Id)
                .to(config_tags_relation::Column::Id)
                .into(),
        );
        query_select =
            query_select.filter(config_tags_relation::Column::TagName.is_in(tags.to_vec()));
    }
    if !types.is_empty() {
        count_select = count_select.filter(config_info::Column::Type.is_in(types.clone()));
        query_select = query_select.filter(config_info::Column::Type.is_in(types));
    }
    if !content.is_empty() {
        count_select = count_select.filter(config_info::Column::Content.contains(content));
        query_select = query_select.filter(config_info::Column::Content.contains(content));
    }

    let total_count = count_select
        .select_only()
        .column_as(Expr::col(Asterisk).count(), "count")
        .into_tuple::<i64>()
        .one(db)
        .await?
        .unwrap_or_default() as u64;

    if total_count > 0 {
        let offset = (page_no - 1) * page_size;

        let page_items = query_select
            .offset(offset)
            .limit(page_size)
            .all(db)
            .await?
            .iter()
            .map(|entity| ConfigBasicInfo {
                id: entity.id,
                namespace_id: entity.tenant_id.clone().unwrap_or_default(),
                group_name: entity.group_id.clone().unwrap_or_default(),
                data_id: entity.data_id.clone(),
                md5: entity.md5.clone().unwrap_or_default(),
                r#type: entity.r#type.clone().unwrap_or_default(),
                app_name: entity.app_name.clone().unwrap_or_default(),
                create_time: entity
                    .gmt_create
                    .unwrap_or_default()
                    .and_utc()
                    .timestamp_millis(),
                modify_time: entity
                    .gmt_modified
                    .unwrap_or_default()
                    .and_utc()
                    .timestamp_millis(),
            })
            .collect();

        return Ok(Page::<ConfigBasicInfo>::new(
            total_count,
            page_no,
            page_size,
            page_items,
        ));
    }

    Ok(Page::<ConfigBasicInfo>::default())
}

pub async fn find_all(
    db: &DatabaseConnection,
    data_id: &str,
    group: &str,
    tenant: &str,
) -> anyhow::Result<ConfigAllInfo> {
    // Execute both queries concurrently to reduce latency
    let (config_result, tags_result) = tokio::join!(
        config_info::Entity::find()
            .filter(config_info::Column::DataId.eq(data_id))
            .filter(config_info::Column::GroupId.eq(group))
            .filter(config_info::Column::TenantId.eq(tenant))
            .one(db),
        config_tags_relation::Entity::find()
            .select_only()
            .column(config_tags_relation::Column::TagName)
            .filter(config_tags_relation::Column::DataId.eq(data_id))
            .filter(config_tags_relation::Column::GroupId.eq(group))
            .filter(config_tags_relation::Column::TenantId.eq(tenant))
            .order_by_asc(config_tags_relation::Column::Nid)
            .into_tuple::<String>()
            .all(db)
    );

    match config_result? {
        Some(entity) => {
            let mut m = ConfigAllInfo::from(entity);
            m.config_tags = tags_result?.join(",");
            Ok(m)
        }
        None => Err(anyhow::anyhow!(
            "Config not found for data_id: {}, group: {}, tenant: {}",
            data_id,
            group,
            tenant
        )),
    }
}

pub async fn create_or_update(
    db: &DatabaseConnection,
    data_id: &str,
    group_id: &str,
    tenant_id: &str,
    content: &str,
    app_name: &str,
    src_user: &str,
    src_ip: &str,
    config_tags: &str,
    desc: &str,
    r#use: &str,
    effect: &str,
    r#type: &str,
    schema: &str,
    encrypted_data_key: &str,
) -> anyhow::Result<bool> {
    let entity_option = config_info::Entity::find()
        .filter(config_info::Column::DataId.eq(data_id))
        .filter(config_info::Column::GroupId.eq(group_id))
        .filter(config_info::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?;

    let tx = db.begin().await?;

    let now = Local::now().naive_local();

    match entity_option {
        Some(entity) => {
            let entity_c = entity.clone();
            let mut model: config_info::ActiveModel = entity.into();

            model.content = Set(Some(content.to_string()));
            model.md5 = Set(Some(md5_digest(content)));
            model.src_user = Set(Some(src_user.to_string()));
            model.src_ip = Set(Some(src_ip.to_string()));
            model.app_name = Set(Some(app_name.to_string()));
            model.c_desc = Set(Some(desc.to_string()));
            model.c_use = Set(Some(r#use.to_string()));
            model.effect = Set(Some(effect.to_string()));
            model.r#type = Set(Some(r#type.to_string()));
            model.c_schema = Set(Some(schema.to_string()));
            model.encrypted_data_key = Set(Some(encrypted_data_key.to_string()));

            let mut model_changed = false;
            let mut tags_changed = false;

            if model.is_changed() {
                model_changed = true;

                model.gmt_modified = Set(Some(now));

                config_info::Entity::update(model).exec(&tx).await?;
            }

            let new_tags = config_tags
                .split(",")
                .filter(|e| !e.is_empty())
                .map(|e| e.to_string())
                .collect::<Vec<String>>()
                .join(",");

            let old_tags = config_tags_relation::Entity::find()
                .select_only()
                .column(config_tags_relation::Column::TagName)
                .filter(config_tags_relation::Column::Id.eq(entity_c.id))
                .order_by_asc(config_tags_relation::Column::Nid)
                .into_tuple::<String>()
                .all(db)
                .await?
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<String>>()
                .join(",");

            if new_tags != old_tags {
                tags_changed = true;

                config_tags_relation::Entity::delete_many()
                    .filter(config_tags_relation::Column::Id.eq(entity_c.id))
                    .exec(&tx)
                    .await?;

                let tags = new_tags
                    .split(",")
                    .map(|e| config_tags_relation::ActiveModel {
                        id: Set(entity_c.id),
                        tag_name: Set(e.to_string()),
                        data_id: Set(entity_c.data_id.clone().to_string()),
                        tag_type: Set(Some(String::default())),
                        group_id: Set(entity_c.group_id.clone().unwrap_or_default()),
                        tenant_id: Set(entity_c.tenant_id.clone()),
                        nid: Set(0),
                    })
                    .collect::<Vec<config_tags_relation::ActiveModel>>();

                config_tags_relation::Entity::insert_many(tags)
                    .on_empty_do_nothing()
                    .exec(&tx)
                    .await?;
            }

            if model_changed || tags_changed {
                let mut map = HashMap::<String, String>::with_capacity(6);

                if !old_tags.is_empty() {
                    map.insert("config_tags".to_string(), config_tags.to_string());
                }
                if entity_c.c_desc.clone().is_some_and(|e| !e.is_empty()) {
                    map.insert(
                        "desc".to_string(),
                        entity_c.c_desc.clone().unwrap_or_default(),
                    );
                }
                if entity_c.c_use.clone().is_some_and(|e| !e.is_empty()) {
                    map.insert(
                        "use".to_string(),
                        entity_c.c_use.clone().unwrap_or_default(),
                    );
                }
                if entity_c.effect.clone().is_some_and(|e| !e.is_empty()) {
                    map.insert(
                        "effect".to_string(),
                        entity_c.effect.clone().unwrap_or_default(),
                    );
                }
                if entity_c.r#type.clone().is_some_and(|e| !e.is_empty()) {
                    map.insert(
                        "type".to_string(),
                        entity_c.r#type.clone().unwrap_or_default(),
                    );
                }
                if entity_c.c_schema.clone().is_some_and(|e| !e.is_empty()) {
                    map.insert(
                        "schema".to_string(),
                        entity_c.c_schema.clone().unwrap_or_default(),
                    );
                }

                let ext_info = serde_json::to_string(&map).unwrap_or_default();

                let his_config_info = his_config_info::ActiveModel {
                    id: Set(entity_c.id as u64),
                    nid: Set(0),
                    data_id: Set(entity_c.data_id.clone()),
                    group_id: Set(entity_c.group_id.clone().unwrap_or_default()),
                    app_name: Set(entity_c.app_name),
                    content: Set(entity_c.content.unwrap_or_default()),
                    md5: Set(Some(entity_c.md5.unwrap_or_default())),
                    gmt_create: Set(entity_c.gmt_create.unwrap_or(now)),
                    gmt_modified: Set(entity_c.gmt_modified.unwrap_or(now)),
                    src_user: Set(Some(entity_c.src_user.unwrap_or_default())),
                    src_ip: Set(Some(entity_c.src_ip.unwrap_or_default())),
                    op_type: Set(Some(String::from("U"))),
                    tenant_id: Set(Some(entity_c.tenant_id.clone().unwrap_or_default())),
                    encrypted_data_key: Set(entity_c.encrypted_data_key.unwrap_or_default()),
                    publish_type: Set(Some("formal".to_string())),
                    gray_name: Set(Some("".to_string())),
                    ext_info: Set(Some(ext_info)),
                };

                his_config_info::Entity::insert(his_config_info)
                    .exec(&tx)
                    .await?;
            }
        }
        None => {
            let md5 = md5_digest(content);

            let model = config_info::ActiveModel {
                data_id: Set(data_id.to_string()),
                group_id: Set(Some(group_id.to_string())),
                content: Set(Some(content.to_string())),
                md5: Set(Some(md5.to_string())),
                gmt_create: Set(Some(now)),
                gmt_modified: Set(Some(now)),
                src_user: Set(Some(src_user.to_string())),
                src_ip: Set(Some(src_ip.to_string())),
                app_name: Set(Some(app_name.to_string())),
                tenant_id: Set(Some(tenant_id.to_string())),
                c_desc: Set(Some(desc.to_string())),
                c_use: Set(Some(r#use.to_string())),
                effect: Set(Some(effect.to_string())),
                r#type: Set(Some(r#type.to_string())),
                c_schema: Set(Some(schema.to_string())),
                encrypted_data_key: Set(Some(encrypted_data_key.to_string())),
                ..Default::default()
            };

            let last_insert_id = config_info::Entity::insert(model)
                .exec(&tx)
                .await?
                .last_insert_id;

            let tags_str = config_tags
                .split(",")
                .filter(|e| !e.is_empty())
                .map(|e| e.to_string())
                .collect::<Vec<String>>()
                .join(",");

            if !tags_str.is_empty() {
                let tags = tags_str
                    .split(",")
                    .map(|e| config_tags_relation::ActiveModel {
                        id: Set(last_insert_id),
                        tag_name: Set(e.to_string()),
                        data_id: Set(data_id.to_string()),
                        tag_type: Set(Some(String::default())),
                        group_id: Set(group_id.to_string()),
                        tenant_id: Set(Some(tenant_id.to_string())),
                        nid: Set(0),
                    })
                    .collect::<Vec<config_tags_relation::ActiveModel>>();

                config_tags_relation::Entity::insert_many(tags)
                    .on_empty_do_nothing()
                    .exec(&tx)
                    .await?;
            }

            let mut map = HashMap::<String, String>::with_capacity(6);

            if !tags_str.is_empty() {
                map.insert("config_tags".to_string(), tags_str);
            }
            if !desc.is_empty() {
                map.insert("desc".to_string(), desc.to_string());
            }
            if !r#use.is_empty() {
                map.insert("use".to_string(), r#use.to_string());
            }
            if !effect.is_empty() {
                map.insert("effect".to_string(), effect.to_string());
            }
            if !r#type.is_empty() {
                map.insert("type".to_string(), r#type.to_string());
            }
            if !schema.is_empty() {
                map.insert("schema".to_string(), schema.to_string());
            }

            let ext_info = serde_json::to_string(&map).unwrap_or_default();

            let his_config_info = his_config_info::ActiveModel {
                id: Set(last_insert_id as u64),
                nid: Set(0),
                data_id: Set(data_id.to_string()),
                group_id: Set(group_id.to_string()),
                app_name: Set(Some(app_name.to_string())),
                content: Set(content.to_string()),
                md5: Set(Some(md5.to_string())),
                gmt_create: Set(now),
                gmt_modified: Set(now),
                src_user: Set(Some(src_user.to_string())),
                src_ip: Set(Some(src_ip.to_string())),
                op_type: Set(Some(String::from("I"))),
                tenant_id: Set(Some(tenant_id.to_string())),
                encrypted_data_key: Set(encrypted_data_key.to_string()),
                publish_type: Set(Some("formal".to_string())),
                gray_name: Set(Some("".to_string())),
                ext_info: Set(Some(ext_info)),
            };

            his_config_info::Entity::insert(his_config_info)
                .exec(&tx)
                .await?;
        }
    };

    tx.commit().await?;

    Ok(true)
}

pub async fn delete(
    db: &DatabaseConnection,
    data_id: &str,
    group: &str,
    namespace_id: &str,
    gray_name: &str,
    client_ip: &str,
    src_user: &str,
    _src_type: &str,
) -> anyhow::Result<bool> {
    if let Some(entity) = config_info::Entity::find()
        .filter(config_info::Column::DataId.eq(data_id))
        .filter(config_info::Column::GroupId.eq(group))
        .filter(config_info::Column::TenantId.eq(namespace_id))
        .one(db)
        .await?
    {
        let tags = config_tags_relation::Entity::find()
            .filter(config_tags_relation::Column::Id.eq(entity.id))
            .all(db)
            .await?;

        let tx = db.begin().await?;

        config_info::Entity::delete_by_id(entity.id)
            .exec(&tx)
            .await?;

        if !tags.is_empty() {
            config_tags_relation::Entity::delete_many()
                .filter(config_tags_relation::Column::Id.eq(entity.id))
                .exec(&tx)
                .await?;
        }

        let mut map = HashMap::<String, String>::with_capacity(6);

        let tags_str = tags
            .iter()
            .map(|e| e.tag_name.to_string())
            .collect::<Vec<String>>()
            .join(",");
        let desc = entity.c_desc.unwrap_or_default();
        let r#use = entity.c_use.unwrap_or_default();
        let effect = entity.effect.unwrap_or_default();
        let r#type = entity.r#type.unwrap_or_default();
        let schema = entity.c_schema.unwrap_or_default();

        if !tags_str.is_empty() {
            map.insert("config_tags".to_string(), tags_str);
        }
        if !desc.is_empty() {
            map.insert("desc".to_string(), desc.to_string());
        }
        if !r#use.is_empty() {
            map.insert("use".to_string(), r#use.to_string());
        }
        if !effect.is_empty() {
            map.insert("effect".to_string(), effect.to_string());
        }
        if !r#type.is_empty() {
            map.insert("type".to_string(), r#type.to_string());
        }
        if !schema.is_empty() {
            map.insert("schema".to_string(), schema.to_string());
        }

        let ext_info = serde_json::to_string(&map).unwrap_or_default();
        let now = Local::now().naive_local();

        let his_config_info = his_config_info::ActiveModel {
            id: Set(entity.id as u64),
            nid: Set(0),
            data_id: Set(entity.data_id.to_string()),
            group_id: Set(entity.group_id.unwrap_or_default()),
            app_name: Set(entity.app_name),
            content: Set(entity.content.unwrap_or_default()),
            md5: Set(entity.md5),
            gmt_create: Set(now),
            gmt_modified: Set(now),
            src_user: Set(Some(src_user.to_string())),
            src_ip: Set(Some(client_ip.to_string())),
            op_type: Set(Some(String::from("D"))),
            tenant_id: Set(Some(namespace_id.to_string())),
            encrypted_data_key: Set(entity.encrypted_data_key.unwrap_or_default()),
            publish_type: Set(Some("formal".to_string())),
            gray_name: Set(Some(gray_name.to_string())),
            ext_info: Set(Some(ext_info)),
        };

        his_config_info::Entity::insert(his_config_info)
            .exec(&tx)
            .await?;

        tx.commit().await?;
    }

    Ok(true)
}

pub async fn find_gray_one(
    db: &DatabaseConnection,
    data_id: &str,
    group: &str,
    namespace_id: &str,
) -> anyhow::Result<Option<ConfigInfoGrayWrapper>> {
    // Execute both queries concurrently to reduce latency
    let (gray_result, config_result) = tokio::join!(
        config_info_gray::Entity::find()
            .filter(config_info_gray::Column::DataId.eq(data_id))
            .filter(config_info_gray::Column::GroupId.eq(group))
            .filter(config_info_gray::Column::TenantId.eq(namespace_id))
            .one(db),
        config_info::Entity::find()
            .select_only()
            .column(config_info::Column::Type)
            .filter(config_info::Column::DataId.eq(data_id))
            .filter(config_info::Column::GroupId.eq(group))
            .filter(config_info::Column::TenantId.eq(namespace_id))
            .into_tuple::<Option<String>>()
            .one(db)
    );

    if let Some(gray_entity) = gray_result? {
        let mut config_gray = ConfigInfoGrayWrapper::from(gray_entity);

        // Apply type from config_info if available
        if let std::result::Result::Ok(Some(Some(config_type))) = config_result {
            config_gray.config_info.r#type = config_type;
        }

        Ok(Some(config_gray))
    } else {
        Ok(None)
    }
}

fn md5_digest(content: &str) -> String {
    format!("{:x}", md5::compute(content))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_md5_digest() {
        let content = "test content";
        let digest = md5_digest(content);
        assert_eq!(digest.len(), 32);
        assert!(digest.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_md5_digest_empty() {
        let content = "";
        let digest = md5_digest(content);
        // MD5 of empty string is d41d8cd98f00b204e9800998ecf8427e
        assert_eq!(digest, "d41d8cd98f00b204e9800998ecf8427e");
    }

    #[test]
    fn test_md5_digest_consistency() {
        let content = "consistent";
        let digest1 = md5_digest(content);
        let digest2 = md5_digest(content);
        assert_eq!(digest1, digest2);
    }
}
