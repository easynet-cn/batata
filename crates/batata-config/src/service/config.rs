//! Configuration service layer
//!
//! This module provides database operations for configuration management:
//! - CRUD operations for configs
//! - Config history tracking
//! - Tag management
//! - Gray release (beta configs)

#![allow(clippy::too_many_arguments)]

use std::collections::HashMap;

use chrono::Local;
use sea_orm::{prelude::Expr, sea_query::Asterisk, *};

use batata_api::Page;
use batata_persistence::entity::{config_info, config_info_gray, config_tags_relation, his_config_info};

use crate::model::{ConfigAllInfo, ConfigBasicInfo, ConfigInfoGrayWrapper};

/// Normalize config tags by filtering empty entries.
/// Returns a Vec of non-empty tag references for direct iteration.
#[inline]
fn normalize_tags(config_tags: &str) -> Vec<&str> {
    config_tags
        .split(',')
        .map(|e| e.trim())
        .filter(|e| !e.is_empty())
        .collect()
}

/// Join normalized tags into a comma-separated string.
#[inline]
fn join_tags(tags: &[&str]) -> String {
    tags.join(",")
}

/// Find a single config by data_id, group, and namespace
pub async fn find_one(
    db: &DatabaseConnection,
    data_id: &str,
    group: &str,
    namespace_id: &str,
) -> anyhow::Result<Option<ConfigAllInfo>> {
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

/// Search configs with pagination
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
        if data_id.contains('*') {
            count_select = count_select.filter(config_info::Column::DataId.like(data_id));
            query_select = query_select.filter(config_info::Column::DataId.like(data_id));
        } else {
            count_select = count_select.filter(config_info::Column::DataId.contains(data_id));
            query_select = query_select.filter(config_info::Column::DataId.contains(data_id));
        }
    }
    if !group_id.is_empty() {
        if group_id.contains('*') {
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
            count_select.filter(config_tags_relation::Column::TagName.is_in(tags.clone()));
        query_select = query_select.join(
            JoinType::InnerJoin,
            config_info::Entity::belongs_to(config_tags_relation::Entity)
                .from(config_info::Column::Id)
                .to(config_tags_relation::Column::Id)
                .into(),
        );
        query_select = query_select.filter(config_tags_relation::Column::TagName.is_in(tags));
    }
    if !types.is_empty() {
        count_select = count_select.filter(config_info::Column::Type.is_in(types.clone()));
        query_select = query_select.filter(config_info::Column::Type.is_in(types));
    }
    if !content.is_empty() {
        count_select = count_select.filter(config_info::Column::Content.contains(content));
        query_select = query_select.filter(config_info::Column::Content.contains(content));
    }

    let offset = (page_no - 1) * page_size;

    // Execute count and data queries in parallel for better performance
    let (count_result, data_result) = tokio::join!(
        count_select
            .select_only()
            .column_as(Expr::col(Asterisk).count(), "count")
            .into_tuple::<i64>()
            .one(db),
        query_select.offset(offset).limit(page_size).all(db)
    );

    let total_count = count_result?.unwrap_or_default() as u64;

    if total_count > 0 {
        let page_items = data_result?
            .into_iter()
            .map(ConfigBasicInfo::from)
            .collect();

        return Ok(Page::<ConfigBasicInfo>::new(
            total_count,
            page_no,
            page_size,
            page_items,
        ));
    }

    // Consume the data result to avoid unused warning, even though count is 0
    let _ = data_result;

    Ok(Page::<ConfigBasicInfo>::default())
}

/// Find config with all details
pub async fn find_all(
    db: &DatabaseConnection,
    data_id: &str,
    group: &str,
    tenant: &str,
) -> anyhow::Result<ConfigAllInfo> {
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

/// Create or update a config
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
            update_existing_config(
                &tx,
                db,
                entity,
                content,
                app_name,
                src_user,
                src_ip,
                config_tags,
                desc,
                r#use,
                effect,
                r#type,
                schema,
                encrypted_data_key,
                now,
            )
            .await?;
        }
        None => {
            create_new_config(
                &tx,
                data_id,
                group_id,
                tenant_id,
                content,
                app_name,
                src_user,
                src_ip,
                config_tags,
                desc,
                r#use,
                effect,
                r#type,
                schema,
                encrypted_data_key,
                now,
            )
            .await?;
        }
    };

    tx.commit().await?;
    Ok(true)
}

/// Delete a config
pub async fn delete(
    db: &DatabaseConnection,
    data_id: &str,
    group: &str,
    namespace_id: &str,
    gray_name: &str,
    client_ip: &str,
    src_user: &str,
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

        // Record history
        let ext_info = build_ext_info(&tags, &entity);
        let now = Local::now().naive_local();

        let his_config = his_config_info::ActiveModel {
            id: Set(entity.id as u64),
            nid: Set(0),
            data_id: Set(entity.data_id),
            group_id: Set(entity.group_id.unwrap_or_default()),
            app_name: Set(entity.app_name),
            content: Set(entity.content.unwrap_or_default()),
            md5: Set(entity.md5),
            gmt_create: Set(now),
            gmt_modified: Set(now),
            src_user: Set(Some(src_user.to_string())),
            src_ip: Set(Some(client_ip.to_string())),
            op_type: Set(Some("D".to_string())),
            tenant_id: Set(Some(namespace_id.to_string())),
            encrypted_data_key: Set(entity.encrypted_data_key.unwrap_or_default()),
            publish_type: Set(Some("formal".to_string())),
            gray_name: Set(Some(gray_name.to_string())),
            ext_info: Set(Some(ext_info)),
        };

        his_config_info::Entity::insert(his_config)
            .exec(&tx)
            .await?;

        tx.commit().await?;
    }

    Ok(true)
}

/// Find gray config
pub async fn find_gray_one(
    db: &DatabaseConnection,
    data_id: &str,
    group: &str,
    namespace_id: &str,
) -> anyhow::Result<Option<ConfigInfoGrayWrapper>> {
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

        if let Ok(Some(Some(config_type))) = config_result {
            config_gray.config_info.r#type = config_type;
        }

        Ok(Some(config_gray))
    } else {
        Ok(None)
    }
}

// Helper functions

fn md5_digest(content: &str) -> String {
    format!("{:x}", md5::compute(content))
}

fn build_ext_info(
    tags: &[config_tags_relation::Model],
    entity: &config_info::Model,
) -> String {
    let mut map = HashMap::<String, String>::with_capacity(6);

    let tags_str = tags
        .iter()
        .map(|e| e.tag_name.to_string())
        .collect::<Vec<String>>()
        .join(",");

    if !tags_str.is_empty() {
        map.insert("config_tags".to_string(), tags_str);
    }
    if let Some(ref desc) = entity.c_desc
        && !desc.is_empty()
    {
        map.insert("desc".to_string(), desc.clone());
    }
    if let Some(ref r#use) = entity.c_use
        && !r#use.is_empty()
    {
        map.insert("use".to_string(), r#use.clone());
    }
    if let Some(ref effect) = entity.effect
        && !effect.is_empty()
    {
        map.insert("effect".to_string(), effect.clone());
    }
    if let Some(ref r#type) = entity.r#type
        && !r#type.is_empty()
    {
        map.insert("type".to_string(), r#type.clone());
    }
    if let Some(ref schema) = entity.c_schema
        && !schema.is_empty()
    {
        map.insert("schema".to_string(), schema.clone());
    }

    serde_json::to_string(&map).unwrap_or_default()
}

#[allow(clippy::too_many_arguments)]
async fn update_existing_config(
    tx: &DatabaseTransaction,
    db: &DatabaseConnection,
    entity: config_info::Model,
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
    now: chrono::NaiveDateTime,
) -> anyhow::Result<()> {
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
        config_info::Entity::update(model).exec(tx).await?;
    }

    let new_tags_vec = normalize_tags(config_tags);
    let new_tags = join_tags(&new_tags_vec);

    let old_tags = config_tags_relation::Entity::find()
        .select_only()
        .column(config_tags_relation::Column::TagName)
        .filter(config_tags_relation::Column::Id.eq(entity_c.id))
        .order_by_asc(config_tags_relation::Column::Nid)
        .into_tuple::<String>()
        .all(db)
        .await?
        .join(",");

    if new_tags != old_tags {
        tags_changed = true;

        config_tags_relation::Entity::delete_many()
            .filter(config_tags_relation::Column::Id.eq(entity_c.id))
            .exec(tx)
            .await?;

        if !new_tags_vec.is_empty() {
            let tags = new_tags_vec
                .iter()
                .map(|e| config_tags_relation::ActiveModel {
                    id: Set(entity_c.id),
                    tag_name: Set((*e).to_string()),
                    data_id: Set(entity_c.data_id.clone()),
                    tag_type: Set(Some(String::default())),
                    group_id: Set(entity_c.group_id.clone().unwrap_or_default()),
                    tenant_id: Set(entity_c.tenant_id.clone()),
                    nid: Set(0),
                })
                .collect::<Vec<config_tags_relation::ActiveModel>>();

            config_tags_relation::Entity::insert_many(tags)
                .on_empty_do_nothing()
                .exec(tx)
                .await?;
        }
    }

    if model_changed || tags_changed {
        let mut map = HashMap::<String, String>::with_capacity(6);

        if !old_tags.is_empty() {
            map.insert("config_tags".to_string(), config_tags.to_string());
        }
        if entity_c.c_desc.as_ref().is_some_and(|e| !e.is_empty()) {
            map.insert("desc".to_string(), entity_c.c_desc.clone().unwrap_or_default());
        }
        if entity_c.c_use.as_ref().is_some_and(|e| !e.is_empty()) {
            map.insert("use".to_string(), entity_c.c_use.clone().unwrap_or_default());
        }
        if entity_c.effect.as_ref().is_some_and(|e| !e.is_empty()) {
            map.insert("effect".to_string(), entity_c.effect.clone().unwrap_or_default());
        }
        if entity_c.r#type.as_ref().is_some_and(|e| !e.is_empty()) {
            map.insert("type".to_string(), entity_c.r#type.clone().unwrap_or_default());
        }
        if entity_c.c_schema.as_ref().is_some_and(|e| !e.is_empty()) {
            map.insert("schema".to_string(), entity_c.c_schema.clone().unwrap_or_default());
        }

        let ext_info = serde_json::to_string(&map).unwrap_or_default();

        let his_config = his_config_info::ActiveModel {
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
            op_type: Set(Some("U".to_string())),
            tenant_id: Set(Some(entity_c.tenant_id.clone().unwrap_or_default())),
            encrypted_data_key: Set(entity_c.encrypted_data_key.unwrap_or_default()),
            publish_type: Set(Some("formal".to_string())),
            gray_name: Set(Some(String::new())),
            ext_info: Set(Some(ext_info)),
        };

        his_config_info::Entity::insert(his_config).exec(tx).await?;
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn create_new_config(
    tx: &DatabaseTransaction,
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
    now: chrono::NaiveDateTime,
) -> anyhow::Result<()> {
    let md5 = md5_digest(content);

    let model = config_info::ActiveModel {
        data_id: Set(data_id.to_string()),
        group_id: Set(Some(group_id.to_string())),
        content: Set(Some(content.to_string())),
        md5: Set(Some(md5.clone())),
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
        .exec(tx)
        .await?
        .last_insert_id;

    let tags_vec = normalize_tags(config_tags);
    let tags_str = join_tags(&tags_vec);

    if !tags_vec.is_empty() {
        let tags = tags_vec
            .iter()
            .map(|e| config_tags_relation::ActiveModel {
                id: Set(last_insert_id),
                tag_name: Set((*e).to_string()),
                data_id: Set(data_id.to_string()),
                tag_type: Set(Some(String::default())),
                group_id: Set(group_id.to_string()),
                tenant_id: Set(Some(tenant_id.to_string())),
                nid: Set(0),
            })
            .collect::<Vec<config_tags_relation::ActiveModel>>();

        config_tags_relation::Entity::insert_many(tags)
            .on_empty_do_nothing()
            .exec(tx)
            .await?;
    }

    // Record history
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

    let his_config = his_config_info::ActiveModel {
        id: Set(last_insert_id as u64),
        nid: Set(0),
        data_id: Set(data_id.to_string()),
        group_id: Set(group_id.to_string()),
        app_name: Set(Some(app_name.to_string())),
        content: Set(content.to_string()),
        md5: Set(Some(md5)),
        gmt_create: Set(now),
        gmt_modified: Set(now),
        src_user: Set(Some(src_user.to_string())),
        src_ip: Set(Some(src_ip.to_string())),
        op_type: Set(Some("I".to_string())),
        tenant_id: Set(Some(tenant_id.to_string())),
        encrypted_data_key: Set(encrypted_data_key.to_string()),
        publish_type: Set(Some("formal".to_string())),
        gray_name: Set(Some(String::new())),
        ext_info: Set(Some(ext_info)),
    };

    his_config_info::Entity::insert(his_config).exec(tx).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // === MD5 digest tests ===

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
        assert_eq!(digest, "d41d8cd98f00b204e9800998ecf8427e");
    }

    #[test]
    fn test_md5_digest_deterministic() {
        let content = "hello world";
        let digest1 = md5_digest(content);
        let digest2 = md5_digest(content);
        assert_eq!(digest1, digest2);
    }

    #[test]
    fn test_md5_digest_different_content() {
        let digest1 = md5_digest("content1");
        let digest2 = md5_digest("content2");
        assert_ne!(digest1, digest2);
    }

    #[test]
    fn test_md5_digest_unicode() {
        let content = "你好世界";
        let digest = md5_digest(content);
        assert_eq!(digest.len(), 32);
    }

    #[test]
    fn test_md5_digest_whitespace() {
        let digest1 = md5_digest("  hello  ");
        let digest2 = md5_digest("hello");
        assert_ne!(digest1, digest2); // Whitespace matters
    }

    // === Tag normalization tests ===

    #[test]
    fn test_normalize_tags_basic() {
        let tags = normalize_tags("tag1,tag2,tag3");
        assert_eq!(tags, vec!["tag1", "tag2", "tag3"]);
    }

    #[test]
    fn test_normalize_tags_empty() {
        let tags = normalize_tags("");
        assert!(tags.is_empty());
    }

    #[test]
    fn test_normalize_tags_with_empty_entries() {
        let tags = normalize_tags("tag1,,tag2,,,tag3");
        assert_eq!(tags, vec!["tag1", "tag2", "tag3"]);
    }

    #[test]
    fn test_normalize_tags_single() {
        let tags = normalize_tags("single");
        assert_eq!(tags, vec!["single"]);
    }

    #[test]
    fn test_normalize_tags_with_whitespace() {
        let tags = normalize_tags("  tag1  ,  tag2  ,  tag3  ");
        assert_eq!(tags, vec!["tag1", "tag2", "tag3"]);
    }

    #[test]
    fn test_normalize_tags_only_commas() {
        let tags = normalize_tags(",,,");
        assert!(tags.is_empty());
    }

    #[test]
    fn test_normalize_tags_trailing_comma() {
        let tags = normalize_tags("tag1,tag2,");
        assert_eq!(tags, vec!["tag1", "tag2"]);
    }

    #[test]
    fn test_normalize_tags_leading_comma() {
        let tags = normalize_tags(",tag1,tag2");
        assert_eq!(tags, vec!["tag1", "tag2"]);
    }

    // === Join tags tests ===

    #[test]
    fn test_join_tags_basic() {
        let tags = vec!["tag1", "tag2", "tag3"];
        assert_eq!(join_tags(&tags), "tag1,tag2,tag3");
    }

    #[test]
    fn test_join_tags_empty() {
        let tags: Vec<&str> = vec![];
        assert_eq!(join_tags(&tags), "");
    }

    #[test]
    fn test_join_tags_single() {
        let tags = vec!["single"];
        assert_eq!(join_tags(&tags), "single");
    }

    // === Round-trip tests ===

    #[test]
    fn test_normalize_and_join_round_trip() {
        let original = "tag1,tag2,tag3";
        let normalized = normalize_tags(original);
        let joined = join_tags(&normalized);
        assert_eq!(joined, original);
    }

    #[test]
    fn test_normalize_and_join_cleans_empty() {
        let dirty = "tag1,,tag2,,,tag3,";
        let normalized = normalize_tags(dirty);
        let joined = join_tags(&normalized);
        assert_eq!(joined, "tag1,tag2,tag3");
    }

    #[test]
    fn test_normalize_and_join_trims_whitespace() {
        let dirty = "  tag1  ,  tag2  ";
        let normalized = normalize_tags(dirty);
        let joined = join_tags(&normalized);
        assert_eq!(joined, "tag1,tag2");
    }
}
