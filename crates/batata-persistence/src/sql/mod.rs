//! SQL-based persistence backend (MySQL/PostgreSQL via SeaORM)
//!
//! This module implements the `PersistenceService` trait by wrapping existing
//! SeaORM database operations. It is a thin adapter that delegates all operations
//! to the existing `batata_config::service::*` and `batata_auth::service::*` functions.

use std::collections::HashMap;

use async_trait::async_trait;
use sea_orm::{prelude::Expr, sea_query::Asterisk, *};

use crate::entity::{
    config_info, config_info_gray, config_tags_relation, group_capacity, his_config_info,
    permissions, roles, tenant_capacity, tenant_info, users,
};
use crate::model::*;
use crate::traits::*;

/// External database persistence service
///
/// Wraps a SeaORM `DatabaseConnection` and implements all persistence traits
/// by delegating to direct database queries.
pub struct ExternalDbPersistService {
    db: DatabaseConnection,
}

impl ExternalDbPersistService {
    /// Create a new ExternalDbPersistService with the given database connection
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Get a reference to the underlying database connection
    pub fn db(&self) -> &DatabaseConnection {
        &self.db
    }
}

// ============================================================================
// PersistenceService implementation
// ============================================================================

#[async_trait]
impl PersistenceService for ExternalDbPersistService {
    fn storage_mode(&self) -> StorageMode {
        StorageMode::ExternalDb
    }

    async fn health_check(&self) -> anyhow::Result<()> {
        // Execute a simple query to verify connectivity
        users::Entity::find()
            .select_only()
            .column_as(Expr::cust("1"), "health")
            .into_tuple::<i32>()
            .one(&self.db)
            .await?;
        Ok(())
    }
}

// ============================================================================
// ConfigPersistence implementation
// ============================================================================

/// Escape SQL wildcard characters and convert user wildcards to SQL LIKE pattern.
#[inline]
fn escape_sql_like_pattern(input: &str) -> String {
    input
        .replace('%', "\\%")
        .replace('_', "\\_")
        .replace('*', "%")
}

/// Normalize config tags by filtering empty entries.
#[inline]
fn normalize_tags(config_tags: &str) -> Vec<&str> {
    config_tags
        .split(',')
        .map(|e| e.trim())
        .filter(|e| !e.is_empty())
        .collect()
}

/// Build ext_info JSON from tags and config entity for history records
fn build_ext_info(tags: &[config_tags_relation::Model], entity: &config_info::Model) -> String {
    let mut ext = serde_json::Map::new();
    if !tags.is_empty() {
        let tag_str: String = tags
            .iter()
            .map(|t| t.tag_name.as_str())
            .collect::<Vec<_>>()
            .join(",");
        ext.insert(
            "config_tags".to_string(),
            serde_json::Value::String(tag_str),
        );
    }
    if let Some(ref desc) = entity.c_desc {
        ext.insert("desc".to_string(), serde_json::Value::String(desc.clone()));
    }
    if let Some(ref use_) = entity.c_use {
        ext.insert("use".to_string(), serde_json::Value::String(use_.clone()));
    }
    if let Some(ref effect) = entity.effect {
        ext.insert(
            "effect".to_string(),
            serde_json::Value::String(effect.clone()),
        );
    }
    if let Some(ref type_) = entity.r#type {
        ext.insert("type".to_string(), serde_json::Value::String(type_.clone()));
    }
    if let Some(ref schema) = entity.c_schema {
        ext.insert(
            "schema".to_string(),
            serde_json::Value::String(schema.clone()),
        );
    }
    serde_json::Value::Object(ext).to_string()
}

fn config_entity_to_storage(model: config_info::Model, tags: String) -> ConfigStorageData {
    ConfigStorageData {
        id: model.id,
        data_id: model.data_id.clone(),
        group: model.group_id.clone().unwrap_or_default(),
        tenant: model.tenant_id.clone().unwrap_or_default(),
        content: model.content.clone().unwrap_or_default(),
        md5: model.md5.clone().unwrap_or_default(),
        app_name: model.app_name.clone().unwrap_or_default(),
        config_type: model.r#type.clone().unwrap_or_default(),
        desc: model.c_desc.clone().unwrap_or_default(),
        r#use: model.c_use.clone().unwrap_or_default(),
        effect: model.effect.clone().unwrap_or_default(),
        schema: model.c_schema.clone().unwrap_or_default(),
        config_tags: tags,
        encrypted_data_key: model.encrypted_data_key.clone().unwrap_or_default(),
        src_user: model.src_user.clone().unwrap_or_default(),
        src_ip: model.src_ip.clone().unwrap_or_default(),
        created_time: model
            .gmt_create
            .unwrap_or_default()
            .and_utc()
            .timestamp_millis(),
        modified_time: model
            .gmt_modified
            .unwrap_or_default()
            .and_utc()
            .timestamp_millis(),
    }
}

fn history_entity_to_storage(model: his_config_info::Model) -> ConfigHistoryStorageData {
    ConfigHistoryStorageData {
        id: model.nid,
        data_id: model.data_id,
        group: model.group_id,
        tenant: model.tenant_id.unwrap_or_default(),
        content: model.content,
        md5: model.md5.unwrap_or_default(),
        app_name: model.app_name.unwrap_or_default(),
        src_user: model.src_user.unwrap_or_default(),
        src_ip: model.src_ip.unwrap_or_default(),
        op_type: model.op_type.unwrap_or_default(),
        publish_type: model.publish_type.unwrap_or_default(),
        gray_name: model.gray_name.unwrap_or_default(),
        ext_info: model.ext_info.unwrap_or_default(),
        encrypted_data_key: model.encrypted_data_key,
        created_time: model.gmt_create.and_utc().timestamp_millis(),
        modified_time: model.gmt_modified.and_utc().timestamp_millis(),
    }
}

#[async_trait]
impl ConfigPersistence for ExternalDbPersistService {
    async fn config_find_one(
        &self,
        data_id: &str,
        group: &str,
        namespace_id: &str,
    ) -> anyhow::Result<Option<ConfigStorageData>> {
        let config_result = config_info::Entity::find()
            .filter(config_info::Column::DataId.eq(data_id))
            .filter(config_info::Column::GroupId.eq(group))
            .filter(config_info::Column::TenantId.eq(namespace_id))
            .one(&self.db)
            .await?;

        if let Some(model) = config_result {
            let tag_names = config_tags_relation::Entity::find()
                .select_only()
                .column(config_tags_relation::Column::TagName)
                .filter(config_tags_relation::Column::DataId.eq(data_id))
                .filter(config_tags_relation::Column::GroupId.eq(group))
                .filter(config_tags_relation::Column::TenantId.eq(namespace_id))
                .order_by_asc(config_tags_relation::Column::Nid)
                .into_tuple::<String>()
                .all(&self.db)
                .await?;
            let tags = tag_names.join(",");
            Ok(Some(config_entity_to_storage(model, tags)))
        } else {
            Ok(None)
        }
    }

    async fn config_search_page(
        &self,
        page_no: u64,
        page_size: u64,
        namespace_id: &str,
        data_id: &str,
        group_id: &str,
        app_name: &str,
        tags: Vec<String>,
        types: Vec<String>,
        content: &str,
    ) -> anyhow::Result<Page<ConfigStorageData>> {
        let mut base_select =
            config_info::Entity::find().filter(config_info::Column::TenantId.eq(namespace_id));

        if !data_id.is_empty() {
            if data_id.contains('*') {
                let pattern = escape_sql_like_pattern(data_id);
                base_select = base_select.filter(config_info::Column::DataId.like(&pattern));
            } else {
                base_select = base_select.filter(config_info::Column::DataId.contains(data_id));
            }
        }
        if !group_id.is_empty() {
            if group_id.contains('*') {
                let pattern = escape_sql_like_pattern(group_id);
                base_select = base_select.filter(config_info::Column::GroupId.like(&pattern));
            } else {
                base_select = base_select.filter(config_info::Column::GroupId.contains(group_id));
            }
        }
        if !app_name.is_empty() {
            base_select = base_select.filter(config_info::Column::AppName.eq(app_name));
        }
        if !content.is_empty() {
            base_select = base_select.filter(config_info::Column::Content.contains(content));
        }
        if !types.is_empty() {
            base_select = base_select.filter(config_info::Column::Type.is_in(&types));
        }

        // Tag filtering via subquery
        if !tags.is_empty() {
            for tag in &tags {
                base_select = base_select.filter(
                    config_info::Column::Id.in_subquery(
                        config_tags_relation::Entity::find()
                            .select_only()
                            .column(config_tags_relation::Column::Id)
                            .filter(config_tags_relation::Column::TagName.eq(tag.as_str()))
                            .into_query(),
                    ),
                );
            }
        }

        // Count query
        let count_result = base_select
            .clone()
            .select_only()
            .column_as(Expr::col(Asterisk).count(), "count")
            .into_tuple::<i64>()
            .one(&self.db)
            .await?
            .unwrap_or_default() as u64;

        if count_result == 0 {
            return Ok(Page::empty());
        }

        let offset = (page_no - 1) * page_size;
        let items = base_select
            .offset(offset)
            .limit(page_size)
            .all(&self.db)
            .await?
            .into_iter()
            .map(|m| config_entity_to_storage(m, String::new()))
            .collect();

        Ok(Page::new(count_result, page_no, page_size, items))
    }

    async fn config_create_or_update(
        &self,
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
        let md5_hash = format!("{:x}", md5::compute(content));
        let tags = normalize_tags(config_tags);
        let now = chrono::Local::now().naive_local();

        let existing = config_info::Entity::find()
            .filter(config_info::Column::DataId.eq(data_id))
            .filter(config_info::Column::GroupId.eq(group_id))
            .filter(config_info::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?;

        let tx = self.db.begin().await?;

        match existing {
            Some(entity) => {
                // Update existing config
                let old_tags = config_tags_relation::Entity::find()
                    .filter(config_tags_relation::Column::Id.eq(entity.id))
                    .all(&tx)
                    .await?;

                // Record history
                let ext_info = build_ext_info(&old_tags, &entity);
                let his = his_config_info::ActiveModel {
                    id: Set(entity.id as u64),
                    nid: Set(0),
                    data_id: Set(entity.data_id.clone()),
                    group_id: Set(entity.group_id.clone().unwrap_or_default()),
                    app_name: Set(entity.app_name.clone()),
                    content: Set(entity.content.clone().unwrap_or_default()),
                    md5: Set(entity.md5.clone()),
                    gmt_create: Set(now),
                    gmt_modified: Set(now),
                    src_user: Set(Some(src_user.to_string())),
                    src_ip: Set(Some(src_ip.to_string())),
                    op_type: Set(Some("U".to_string())),
                    tenant_id: Set(Some(tenant_id.to_string())),
                    encrypted_data_key: Set(entity.encrypted_data_key.clone().unwrap_or_default()),
                    publish_type: Set(Some("formal".to_string())),
                    gray_name: Set(None),
                    ext_info: Set(Some(ext_info)),
                };
                his_config_info::Entity::insert(his).exec(&tx).await?;

                // Capture entity ID before moving into ActiveModel
                let entity_id = entity.id;

                // Update config
                let mut active: config_info::ActiveModel = entity.into();
                active.content = Set(Some(content.to_string()));
                active.md5 = Set(Some(md5_hash));
                active.src_user = Set(Some(src_user.to_string()));
                active.src_ip = Set(Some(src_ip.to_string()));
                active.app_name = Set(Some(app_name.to_string()));
                active.c_desc = Set(Some(desc.to_string()));
                active.c_use = Set(Some(r#use.to_string()));
                active.effect = Set(Some(effect.to_string()));
                active.r#type = Set(Some(r#type.to_string()));
                active.c_schema = Set(Some(schema.to_string()));
                active.encrypted_data_key = Set(Some(encrypted_data_key.to_string()));
                active.gmt_modified = Set(Some(now));
                active.update(&tx).await?;

                // Update tags
                if !old_tags.is_empty() {
                    config_tags_relation::Entity::delete_many()
                        .filter(config_tags_relation::Column::Id.eq(entity_id))
                        .exec(&tx)
                        .await?;
                }
                if !tags.is_empty() {
                    let tag_entities: Vec<config_tags_relation::ActiveModel> = tags
                        .iter()
                        .map(|tag| config_tags_relation::ActiveModel {
                            tag_name: Set((*tag).to_owned()),
                            tag_type: Set(None),
                            data_id: Set(data_id.to_string()),
                            group_id: Set(group_id.to_string()),
                            tenant_id: Set(Some(tenant_id.to_string())),
                            id: Set(entity_id),
                            ..Default::default()
                        })
                        .collect();
                    config_tags_relation::Entity::insert_many(tag_entities)
                        .exec(&tx)
                        .await?;
                }
            }
            None => {
                // Create new config
                let entity = config_info::ActiveModel {
                    data_id: Set(data_id.to_string()),
                    group_id: Set(Some(group_id.to_string())),
                    content: Set(Some(content.to_string())),
                    md5: Set(Some(md5_hash)),
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
                    gmt_create: Set(Some(now)),
                    gmt_modified: Set(Some(now)),
                    ..Default::default()
                };

                let result = config_info::Entity::insert(entity).exec(&tx).await?;
                let new_id = result.last_insert_id;

                // Record creation in history
                let his = his_config_info::ActiveModel {
                    id: Set(new_id as u64),
                    nid: Set(0),
                    data_id: Set(data_id.to_string()),
                    group_id: Set(group_id.to_string()),
                    app_name: Set(Some(app_name.to_string())),
                    content: Set(content.to_string()),
                    md5: Set(Some(format!("{:x}", md5::compute(content)))),
                    gmt_create: Set(now),
                    gmt_modified: Set(now),
                    src_user: Set(Some(src_user.to_string())),
                    src_ip: Set(Some(src_ip.to_string())),
                    op_type: Set(Some("I".to_string())),
                    tenant_id: Set(Some(tenant_id.to_string())),
                    encrypted_data_key: Set(encrypted_data_key.to_string()),
                    publish_type: Set(Some("formal".to_string())),
                    gray_name: Set(None),
                    ext_info: Set(None),
                };
                his_config_info::Entity::insert(his).exec(&tx).await?;

                // Insert tags
                if !tags.is_empty() {
                    let tag_entities: Vec<config_tags_relation::ActiveModel> = tags
                        .iter()
                        .map(|tag| config_tags_relation::ActiveModel {
                            tag_name: Set((*tag).to_owned()),
                            tag_type: Set(None),
                            data_id: Set(data_id.to_string()),
                            group_id: Set(group_id.to_string()),
                            tenant_id: Set(Some(tenant_id.to_string())),
                            id: Set(new_id),
                            ..Default::default()
                        })
                        .collect();
                    config_tags_relation::Entity::insert_many(tag_entities)
                        .exec(&tx)
                        .await?;
                }
            }
        }

        tx.commit().await?;
        Ok(true)
    }

    async fn config_delete(
        &self,
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
            .one(&self.db)
            .await?
        {
            let tags = config_tags_relation::Entity::find()
                .filter(config_tags_relation::Column::Id.eq(entity.id))
                .all(&self.db)
                .await?;

            let tx = self.db.begin().await?;

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
            let now = chrono::Local::now().naive_local();
            let his = his_config_info::ActiveModel {
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
            his_config_info::Entity::insert(his).exec(&tx).await?;

            tx.commit().await?;
        }

        Ok(true)
    }

    async fn config_find_gray_one(
        &self,
        data_id: &str,
        group: &str,
        namespace_id: &str,
    ) -> anyhow::Result<Option<ConfigGrayStorageData>> {
        let result = config_info_gray::Entity::find()
            .filter(config_info_gray::Column::DataId.eq(data_id))
            .filter(config_info_gray::Column::GroupId.eq(group))
            .filter(config_info_gray::Column::TenantId.eq(namespace_id))
            .one(&self.db)
            .await?;

        Ok(result.map(|m| ConfigGrayStorageData {
            data_id: m.data_id,
            group: m.group_id,
            tenant: m.tenant_id.unwrap_or_default(),
            content: m.content,
            md5: m.md5.unwrap_or_default(),
            app_name: m.app_name.unwrap_or_default(),
            gray_name: m.gray_name,
            gray_rule: m.gray_rule,
            encrypted_data_key: m.encrypted_data_key,
            src_user: m.src_user.unwrap_or_default(),
            src_ip: m.src_ip.unwrap_or_default(),
            created_time: m.gmt_create.and_utc().timestamp_millis(),
            modified_time: m.gmt_modified.and_utc().timestamp_millis(),
        }))
    }

    async fn config_create_or_update_gray(
        &self,
        data_id: &str,
        group_id: &str,
        tenant_id: &str,
        content: &str,
        gray_name: &str,
        gray_rule: &str,
        src_user: &str,
        src_ip: &str,
        app_name: &str,
        encrypted_data_key: &str,
    ) -> anyhow::Result<bool> {
        let md5_hash = format!("{:x}", md5::compute(content));
        let now = chrono::Local::now().naive_local();

        let existing = config_info_gray::Entity::find()
            .filter(config_info_gray::Column::DataId.eq(data_id))
            .filter(config_info_gray::Column::GroupId.eq(group_id))
            .filter(config_info_gray::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?;

        match existing {
            Some(entity) => {
                let mut active: config_info_gray::ActiveModel = entity.into();
                active.content = Set(content.to_string());
                active.md5 = Set(Some(md5_hash));
                active.gray_name = Set(gray_name.to_string());
                active.gray_rule = Set(gray_rule.to_string());
                active.src_user = Set(Some(src_user.to_string()));
                active.src_ip = Set(Some(src_ip.to_string()));
                active.app_name = Set(Some(app_name.to_string()));
                active.encrypted_data_key = Set(encrypted_data_key.to_string());
                active.gmt_modified = Set(now);
                active.update(&self.db).await?;
            }
            None => {
                let entity = config_info_gray::ActiveModel {
                    data_id: Set(data_id.to_string()),
                    group_id: Set(group_id.to_string()),
                    content: Set(content.to_string()),
                    md5: Set(Some(md5_hash)),
                    src_user: Set(Some(src_user.to_string())),
                    src_ip: Set(Some(src_ip.to_string())),
                    gmt_create: Set(now),
                    gmt_modified: Set(now),
                    app_name: Set(Some(app_name.to_string())),
                    tenant_id: Set(Some(tenant_id.to_string())),
                    gray_name: Set(gray_name.to_string()),
                    gray_rule: Set(gray_rule.to_string()),
                    encrypted_data_key: Set(encrypted_data_key.to_string()),
                    ..Default::default()
                };
                config_info_gray::Entity::insert(entity)
                    .exec(&self.db)
                    .await?;
            }
        }

        Ok(true)
    }

    async fn config_find_all_grays(
        &self,
        data_id: &str,
        group: &str,
        namespace_id: &str,
    ) -> anyhow::Result<Vec<ConfigGrayStorageData>> {
        let results = config_info_gray::Entity::find()
            .filter(config_info_gray::Column::DataId.eq(data_id))
            .filter(config_info_gray::Column::GroupId.eq(group))
            .filter(config_info_gray::Column::TenantId.eq(namespace_id))
            .all(&self.db)
            .await?;

        Ok(results
            .into_iter()
            .map(|m| ConfigGrayStorageData {
                data_id: m.data_id,
                group: m.group_id,
                tenant: m.tenant_id.unwrap_or_default(),
                content: m.content,
                md5: m.md5.unwrap_or_default(),
                app_name: m.app_name.unwrap_or_default(),
                gray_name: m.gray_name,
                gray_rule: m.gray_rule,
                encrypted_data_key: m.encrypted_data_key,
                src_user: m.src_user.unwrap_or_default(),
                src_ip: m.src_ip.unwrap_or_default(),
                created_time: m.gmt_create.and_utc().timestamp_millis(),
                modified_time: m.gmt_modified.and_utc().timestamp_millis(),
            })
            .collect())
    }

    async fn config_delete_gray(
        &self,
        data_id: &str,
        group: &str,
        namespace_id: &str,
        gray_name: &str,
        client_ip: &str,
        src_user: &str,
    ) -> anyhow::Result<bool> {
        let mut query = config_info_gray::Entity::delete_many()
            .filter(config_info_gray::Column::DataId.eq(data_id))
            .filter(config_info_gray::Column::GroupId.eq(group))
            .filter(config_info_gray::Column::TenantId.eq(namespace_id));

        if !gray_name.is_empty() {
            query = query.filter(config_info_gray::Column::GrayName.eq(gray_name));
        }

        let result = query.exec(&self.db).await?;

        let _ = (client_ip, src_user); // Used in history if needed

        Ok(result.rows_affected > 0)
    }

    async fn config_batch_delete(
        &self,
        ids: &[i64],
        client_ip: &str,
        src_user: &str,
    ) -> anyhow::Result<usize> {
        if ids.is_empty() {
            return Ok(0);
        }

        let tx = self.db.begin().await?;
        let now = chrono::Local::now().naive_local();
        let mut deleted = 0;

        for &id in ids {
            if let Some(entity) = config_info::Entity::find_by_id(id).one(&tx).await? {
                // Record history
                let his = his_config_info::ActiveModel {
                    id: Set(entity.id as u64),
                    nid: Set(0),
                    data_id: Set(entity.data_id.clone()),
                    group_id: Set(entity.group_id.clone().unwrap_or_default()),
                    app_name: Set(entity.app_name.clone()),
                    content: Set(entity.content.clone().unwrap_or_default()),
                    md5: Set(entity.md5.clone()),
                    gmt_create: Set(now),
                    gmt_modified: Set(now),
                    src_user: Set(Some(src_user.to_string())),
                    src_ip: Set(Some(client_ip.to_string())),
                    op_type: Set(Some("D".to_string())),
                    tenant_id: Set(entity.tenant_id.clone()),
                    encrypted_data_key: Set(entity.encrypted_data_key.clone().unwrap_or_default()),
                    publish_type: Set(Some("formal".to_string())),
                    gray_name: Set(None),
                    ext_info: Set(None),
                };
                his_config_info::Entity::insert(his).exec(&tx).await?;

                config_info::Entity::delete_by_id(id).exec(&tx).await?;
                deleted += 1;
            }
        }

        tx.commit().await?;
        Ok(deleted)
    }

    async fn config_history_find_by_id(
        &self,
        nid: u64,
    ) -> anyhow::Result<Option<ConfigHistoryStorageData>> {
        let result = his_config_info::Entity::find()
            .filter(his_config_info::Column::Nid.eq(nid))
            .one(&self.db)
            .await?;

        Ok(result.map(history_entity_to_storage))
    }

    async fn config_history_search_page(
        &self,
        data_id: &str,
        group: &str,
        namespace_id: &str,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<ConfigHistoryStorageData>> {
        let mut select = his_config_info::Entity::find()
            .filter(his_config_info::Column::TenantId.eq(namespace_id));

        if !data_id.is_empty() {
            select = select.filter(his_config_info::Column::DataId.contains(data_id));
        }
        if !group.is_empty() {
            select = select.filter(his_config_info::Column::GroupId.contains(group));
        }

        let count = select
            .clone()
            .select_only()
            .column_as(Expr::col(Asterisk).count(), "count")
            .into_tuple::<i64>()
            .one(&self.db)
            .await?
            .unwrap_or_default() as u64;

        if count == 0 {
            return Ok(Page::empty());
        }

        let offset = (page_no - 1) * page_size;
        let items = select
            .order_by_desc(his_config_info::Column::Nid)
            .offset(offset)
            .limit(page_size)
            .all(&self.db)
            .await?
            .into_iter()
            .map(history_entity_to_storage)
            .collect();

        Ok(Page::new(count, page_no, page_size, items))
    }

    async fn config_count_by_namespace(&self, namespace_id: &str) -> anyhow::Result<i32> {
        let count = config_info::Entity::find()
            .select_only()
            .column_as(Expr::col(Asterisk).count(), "count")
            .filter(config_info::Column::TenantId.eq(namespace_id))
            .into_tuple::<i64>()
            .one(&self.db)
            .await?
            .unwrap_or_default() as i32;

        Ok(count)
    }

    async fn config_find_all_group_names(&self, namespace_id: &str) -> anyhow::Result<Vec<String>> {
        let groups = config_info::Entity::find()
            .select_only()
            .column(config_info::Column::GroupId)
            .filter(config_info::Column::TenantId.eq(namespace_id))
            .group_by(config_info::Column::GroupId)
            .into_tuple::<Option<String>>()
            .all(&self.db)
            .await?
            .into_iter()
            .flatten()
            .collect();

        Ok(groups)
    }

    async fn config_history_get_previous(
        &self,
        data_id: &str,
        group: &str,
        namespace_id: &str,
        current_nid: u64,
    ) -> anyhow::Result<Option<ConfigHistoryStorageData>> {
        let result = his_config_info::Entity::find()
            .filter(his_config_info::Column::TenantId.eq(namespace_id))
            .filter(his_config_info::Column::DataId.eq(data_id))
            .filter(his_config_info::Column::GroupId.eq(group))
            .filter(his_config_info::Column::Nid.lt(current_nid))
            .order_by_desc(his_config_info::Column::Nid)
            .one(&self.db)
            .await?;

        Ok(result.map(history_entity_to_storage))
    }

    async fn config_history_search_with_filters(
        &self,
        data_id: &str,
        group: &str,
        namespace_id: &str,
        op_type: Option<&str>,
        src_user: Option<&str>,
        start_time: Option<i64>,
        end_time: Option<i64>,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<ConfigHistoryStorageData>> {
        let mut select = his_config_info::Entity::find()
            .filter(his_config_info::Column::TenantId.eq(namespace_id));

        if !data_id.is_empty() {
            select = select.filter(his_config_info::Column::DataId.contains(data_id));
        }
        if !group.is_empty() {
            select = select.filter(his_config_info::Column::GroupId.contains(group));
        }
        if let Some(op) = op_type {
            select = select.filter(his_config_info::Column::OpType.eq(op));
        }
        if let Some(user) = src_user {
            select = select.filter(his_config_info::Column::SrcUser.contains(user));
        }
        if let Some(start) = start_time
            && let Some(dt) = chrono::DateTime::from_timestamp_millis(start)
        {
            select = select.filter(his_config_info::Column::GmtModified.gte(dt.naive_utc()));
        }
        if let Some(end) = end_time
            && let Some(dt) = chrono::DateTime::from_timestamp_millis(end)
        {
            select = select.filter(his_config_info::Column::GmtModified.lte(dt.naive_utc()));
        }

        let count = select
            .clone()
            .select_only()
            .column_as(Expr::col(Asterisk).count(), "count")
            .into_tuple::<i64>()
            .one(&self.db)
            .await?
            .unwrap_or_default() as u64;

        if count == 0 {
            return Ok(Page::empty());
        }

        let offset = (page_no - 1) * page_size;
        let items = select
            .order_by_desc(his_config_info::Column::Nid)
            .offset(offset)
            .limit(page_size)
            .all(&self.db)
            .await?
            .into_iter()
            .map(history_entity_to_storage)
            .collect();

        Ok(Page::new(count, page_no, page_size, items))
    }

    async fn config_find_by_namespace(
        &self,
        namespace_id: &str,
    ) -> anyhow::Result<Vec<ConfigStorageData>> {
        let configs = config_info::Entity::find()
            .filter(config_info::Column::TenantId.eq(namespace_id))
            .all(&self.db)
            .await?;

        let mut result = Vec::with_capacity(configs.len());
        for model in configs {
            let tag_names = config_tags_relation::Entity::find()
                .select_only()
                .column(config_tags_relation::Column::TagName)
                .filter(config_tags_relation::Column::DataId.eq(&model.data_id))
                .filter(
                    config_tags_relation::Column::GroupId
                        .eq(model.group_id.as_deref().unwrap_or_default()),
                )
                .filter(
                    config_tags_relation::Column::TenantId
                        .eq(model.tenant_id.as_deref().unwrap_or_default()),
                )
                .order_by_asc(config_tags_relation::Column::Nid)
                .into_tuple::<String>()
                .all(&self.db)
                .await?;
            let tags = tag_names.join(",");
            result.push(config_entity_to_storage(model, tags));
        }

        Ok(result)
    }

    async fn config_find_for_export(
        &self,
        namespace_id: &str,
        group: Option<&str>,
        data_ids: Option<Vec<String>>,
        app_name: Option<&str>,
    ) -> anyhow::Result<Vec<ConfigStorageData>> {
        let mut query = config_info::Entity::find()
            .filter(config_info::Column::TenantId.eq(namespace_id))
            .order_by_asc(config_info::Column::GroupId)
            .order_by_asc(config_info::Column::DataId);

        if let Some(g) = group
            && !g.is_empty()
        {
            query = query.filter(config_info::Column::GroupId.eq(g));
        }

        if let Some(ref ids) = data_ids
            && !ids.is_empty()
        {
            query = query.filter(config_info::Column::DataId.is_in(ids.clone()));
        }

        if let Some(app) = app_name
            && !app.is_empty()
        {
            query = query.filter(config_info::Column::AppName.eq(app));
        }

        let configs = query.all(&self.db).await?;

        // Fetch tags for all matching configs
        let config_ids: Vec<i64> = configs.iter().map(|c| c.id).collect();

        let tags = if !config_ids.is_empty() {
            config_tags_relation::Entity::find()
                .filter(config_tags_relation::Column::Id.is_in(config_ids))
                .order_by_asc(config_tags_relation::Column::Id)
                .order_by_asc(config_tags_relation::Column::Nid)
                .all(&self.db)
                .await?
        } else {
            vec![]
        };

        // Group tags by config id
        let mut tags_map: HashMap<i64, Vec<String>> = HashMap::with_capacity(configs.len());
        for tag in tags {
            tags_map.entry(tag.id).or_default().push(tag.tag_name);
        }

        // Convert to ConfigStorageData with tags
        let result: Vec<ConfigStorageData> = configs
            .into_iter()
            .map(|c| {
                let config_tags = tags_map.get(&c.id).map(|t| t.join(",")).unwrap_or_default();
                config_entity_to_storage(c, config_tags)
            })
            .collect();

        Ok(result)
    }

    async fn config_find_by_ids(&self, ids: &[i64]) -> anyhow::Result<Vec<ConfigStorageData>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let models = config_info::Entity::find()
            .filter(config_info::Column::Id.is_in(ids.iter().copied()))
            .all(&self.db)
            .await?;

        let mut result = Vec::with_capacity(models.len());
        for model in models {
            let data_id = model.data_id.clone();
            let group_id = model.group_id.clone().unwrap_or_default();
            let tenant_id = model.tenant_id.clone().unwrap_or_default();
            let tag_names = config_tags_relation::Entity::find()
                .select_only()
                .column(config_tags_relation::Column::TagName)
                .filter(config_tags_relation::Column::DataId.eq(&data_id))
                .filter(config_tags_relation::Column::GroupId.eq(&group_id))
                .filter(config_tags_relation::Column::TenantId.eq(&tenant_id))
                .order_by_asc(config_tags_relation::Column::Nid)
                .into_tuple::<String>()
                .all(&self.db)
                .await?;
            let tags = tag_names.join(",");
            result.push(config_entity_to_storage(model, tags));
        }
        Ok(result)
    }

    async fn config_update_metadata(
        &self,
        data_id: &str,
        group: &str,
        namespace_id: &str,
        config_tags: &str,
        desc: &str,
    ) -> anyhow::Result<bool> {
        // Update the config_info record
        let result = config_info::Entity::update_many()
            .filter(config_info::Column::DataId.eq(data_id))
            .filter(config_info::Column::GroupId.eq(group))
            .filter(config_info::Column::TenantId.eq(namespace_id))
            .col_expr(config_info::Column::CDesc, Expr::value(desc))
            .exec(&self.db)
            .await?;

        // Update tags: first delete existing tags
        config_tags_relation::Entity::delete_many()
            .filter(config_tags_relation::Column::DataId.eq(data_id))
            .filter(config_tags_relation::Column::GroupId.eq(group))
            .filter(config_tags_relation::Column::TenantId.eq(namespace_id))
            .exec(&self.db)
            .await?;

        // Then insert new tags
        if !config_tags.is_empty() {
            let tags: Vec<&str> = config_tags
                .split(',')
                .filter(|t| !t.trim().is_empty())
                .collect();
            for tag in tags {
                let tag_model = config_tags_relation::ActiveModel {
                    tag_name: Set(tag.trim().to_string()),
                    tag_type: Set(None),
                    data_id: Set(data_id.to_string()),
                    group_id: Set(group.to_string()),
                    tenant_id: Set(Some(namespace_id.to_string())),
                    ..Default::default()
                };
                config_tags_relation::Entity::insert(tag_model)
                    .exec(&self.db)
                    .await?;
            }
        }

        Ok(result.rows_affected > 0)
    }
}

// ============================================================================
// NamespacePersistence implementation
// ============================================================================

const DEFAULT_KP: &str = "1";
const DEFAULT_CREATE_SOURCE: &str = "nacos";

#[async_trait]
impl NamespacePersistence for ExternalDbPersistService {
    async fn namespace_find_all(&self) -> anyhow::Result<Vec<NamespaceInfo>> {
        let tenants: Vec<tenant_info::Model> = tenant_info::Entity::find()
            .filter(tenant_info::Column::Kp.eq(DEFAULT_KP))
            .all(&self.db)
            .await?;

        let config_count_rows: Vec<(String, i32)> = config_info::Entity::find()
            .select_only()
            .column(config_info::Column::TenantId)
            .column_as(config_info::Column::Id.count(), "count")
            .filter(config_info::Column::TenantId.is_not_null())
            .group_by(config_info::Column::TenantId)
            .into_tuple::<(String, i32)>()
            .all(&self.db)
            .await?;

        let config_counts: HashMap<String, i32> = config_count_rows.into_iter().collect();

        let mut namespaces: Vec<NamespaceInfo> = tenants
            .into_iter()
            .map(|t| {
                let tenant_id = t.tenant_id.clone().unwrap_or_default();
                let count = config_counts.get(&tenant_id).copied().unwrap_or(0);
                NamespaceInfo {
                    namespace_id: tenant_id,
                    namespace_name: t.tenant_name.unwrap_or_default(),
                    namespace_desc: t.tenant_desc.unwrap_or_default(),
                    config_count: count,
                    quota: 200,
                }
            })
            .collect();

        // Insert default namespace at beginning
        let default_count = config_counts.get("").copied().unwrap_or(0);
        namespaces.insert(
            0,
            NamespaceInfo {
                namespace_id: "public".to_string(),
                namespace_name: "Public".to_string(),
                namespace_desc: "Public Namespace".to_string(),
                config_count: default_count,
                quota: 200,
            },
        );

        Ok(namespaces)
    }

    async fn namespace_get_by_id(
        &self,
        namespace_id: &str,
    ) -> anyhow::Result<Option<NamespaceInfo>> {
        if namespace_id.is_empty() || namespace_id == "public" {
            let count = self.config_count_by_namespace("").await.unwrap_or(0);
            return Ok(Some(NamespaceInfo {
                namespace_id: "public".to_string(),
                namespace_name: "Public".to_string(),
                namespace_desc: "Public Namespace".to_string(),
                config_count: count,
                quota: 200,
            }));
        }

        let tenant_opt = tenant_info::Entity::find()
            .filter(tenant_info::Column::TenantId.eq(namespace_id))
            .filter(tenant_info::Column::Kp.eq(DEFAULT_KP))
            .one(&self.db)
            .await?;

        if let Some(t) = tenant_opt {
            let count: i32 = self
                .config_count_by_namespace(namespace_id)
                .await
                .unwrap_or(0);
            Ok(Some(NamespaceInfo {
                namespace_id: t.tenant_id.unwrap_or_default(),
                namespace_name: t.tenant_name.unwrap_or_default(),
                namespace_desc: t.tenant_desc.unwrap_or_default(),
                config_count: count,
                quota: 200,
            }))
        } else {
            Ok(None)
        }
    }

    async fn namespace_create(
        &self,
        namespace_id: &str,
        name: &str,
        desc: &str,
    ) -> anyhow::Result<()> {
        let entity = tenant_info::ActiveModel {
            tenant_id: Set(Some(namespace_id.to_string())),
            tenant_name: Set(Some(name.to_string())),
            tenant_desc: Set(Some(desc.to_string())),
            kp: Set(DEFAULT_KP.to_string()),
            create_source: Set(Some(DEFAULT_CREATE_SOURCE.to_string())),
            gmt_create: Set(chrono::Utc::now().timestamp_millis()),
            gmt_modified: Set(chrono::Utc::now().timestamp_millis()),
            ..Default::default()
        };

        tenant_info::Entity::insert(entity).exec(&self.db).await?;
        Ok(())
    }

    async fn namespace_update(
        &self,
        namespace_id: &str,
        name: &str,
        desc: &str,
    ) -> anyhow::Result<bool> {
        if let Some(entity) = tenant_info::Entity::find()
            .filter(tenant_info::Column::TenantId.eq(namespace_id))
            .one(&self.db)
            .await?
        {
            let mut active: tenant_info::ActiveModel = entity.into();
            active.tenant_name = Set(Some(name.to_string()));
            active.tenant_desc = Set(Some(desc.to_string()));
            active.gmt_modified = Set(chrono::Utc::now().timestamp_millis());
            active.update(&self.db).await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn namespace_delete(&self, namespace_id: &str) -> anyhow::Result<bool> {
        let result = tenant_info::Entity::delete_many()
            .filter(tenant_info::Column::TenantId.eq(namespace_id))
            .exec(&self.db)
            .await?;

        Ok(result.rows_affected > 0)
    }

    async fn namespace_check(&self, namespace_id: &str) -> anyhow::Result<bool> {
        let count = tenant_info::Entity::find()
            .select_only()
            .column_as(Expr::col(Asterisk).count(), "count")
            .filter(tenant_info::Column::TenantId.eq(namespace_id))
            .into_tuple::<i64>()
            .one(&self.db)
            .await?
            .unwrap_or_default();

        Ok(count > 0)
    }
}

// ============================================================================
// AuthPersistence implementation
// ============================================================================

#[async_trait]
impl AuthPersistence for ExternalDbPersistService {
    async fn user_find_by_username(&self, username: &str) -> anyhow::Result<Option<UserInfo>> {
        let user = users::Entity::find()
            .filter(users::Column::Username.eq(username))
            .one(&self.db)
            .await?
            .map(|m| UserInfo {
                username: m.username,
                password: m.password,
                enabled: m.enabled != 0,
            });

        Ok(user)
    }

    async fn user_find_page(
        &self,
        username: &str,
        page_no: u64,
        page_size: u64,
        accurate: bool,
    ) -> anyhow::Result<Page<UserInfo>> {
        let mut count_select = users::Entity::find();
        let mut query_select = users::Entity::find();

        if !username.is_empty() {
            if accurate {
                count_select = count_select.filter(users::Column::Username.eq(username));
                query_select = query_select.filter(users::Column::Username.eq(username));
            } else {
                count_select = count_select.filter(users::Column::Username.contains(username));
                query_select = query_select.filter(users::Column::Username.contains(username));
            }
        }

        let total_count = count_select
            .select_only()
            .column_as(Expr::col(Asterisk).count(), "count")
            .into_tuple::<i64>()
            .one(&self.db)
            .await?
            .unwrap_or_default() as u64;

        if total_count == 0 {
            return Ok(Page::empty());
        }

        let offset = (page_no - 1) * page_size;
        let items = query_select
            .offset(offset)
            .limit(page_size)
            .all(&self.db)
            .await?
            .into_iter()
            .map(|m| UserInfo {
                username: m.username,
                password: m.password,
                enabled: m.enabled != 0,
            })
            .collect();

        Ok(Page::new(total_count, page_no, page_size, items))
    }

    async fn user_create(
        &self,
        username: &str,
        password_hash: &str,
        _enabled: bool,
    ) -> anyhow::Result<()> {
        let entity = users::ActiveModel {
            username: Set(username.to_string()),
            password: Set(password_hash.to_string()),
            enabled: Set(1),
        };

        users::Entity::insert(entity).exec(&self.db).await?;
        Ok(())
    }

    async fn user_update_password(
        &self,
        username: &str,
        password_hash: &str,
    ) -> anyhow::Result<()> {
        match users::Entity::find_by_id(username).one(&self.db).await? {
            Some(entity) => {
                let mut user: users::ActiveModel = entity.into();
                user.password = Set(password_hash.to_string());
                user.update(&self.db).await?;
                Ok(())
            }
            None => Err(anyhow::anyhow!("User not found: {}", username)),
        }
    }

    async fn user_delete(&self, username: &str) -> anyhow::Result<()> {
        match users::Entity::find_by_id(username).one(&self.db).await? {
            Some(entity) => {
                entity.delete(&self.db).await?;
                Ok(())
            }
            None => Err(anyhow::anyhow!("User not found: {}", username)),
        }
    }

    async fn user_search(&self, username: &str) -> anyhow::Result<Vec<String>> {
        let usernames = users::Entity::find()
            .select_only()
            .column(users::Column::Username)
            .filter(users::Column::Username.like(format!("%{}%", username)))
            .into_tuple::<String>()
            .all(&self.db)
            .await?;

        Ok(usernames)
    }

    async fn role_find_by_username(&self, username: &str) -> anyhow::Result<Vec<RoleInfo>> {
        let role_list = roles::Entity::find()
            .filter(roles::Column::Username.eq(username))
            .all(&self.db)
            .await?
            .into_iter()
            .map(|m| RoleInfo {
                role: m.role,
                username: m.username,
            })
            .collect();

        Ok(role_list)
    }

    async fn role_find_page(
        &self,
        username: &str,
        role: &str,
        page_no: u64,
        page_size: u64,
        accurate: bool,
    ) -> anyhow::Result<Page<RoleInfo>> {
        let mut count_select = roles::Entity::find();
        let mut query_select = roles::Entity::find();

        if !username.is_empty() {
            if accurate {
                count_select = count_select.filter(roles::Column::Username.eq(username));
                query_select = query_select.filter(roles::Column::Username.eq(username));
            } else {
                count_select = count_select.filter(roles::Column::Username.contains(username));
                query_select = query_select.filter(roles::Column::Username.contains(username));
            }
        }
        if !role.is_empty() {
            if accurate {
                count_select = count_select.filter(roles::Column::Role.eq(role));
                query_select = query_select.filter(roles::Column::Role.eq(role));
            } else {
                count_select = count_select.filter(roles::Column::Role.contains(role));
                query_select = query_select.filter(roles::Column::Role.contains(role));
            }
        }

        let total_count = count_select
            .select_only()
            .column_as(Expr::col(Asterisk).count(), "count")
            .into_tuple::<i64>()
            .one(&self.db)
            .await?
            .unwrap_or_default() as u64;

        if total_count == 0 {
            return Ok(Page::empty());
        }

        let offset = (page_no - 1) * page_size;
        let items = query_select
            .offset(offset)
            .limit(page_size)
            .all(&self.db)
            .await?
            .into_iter()
            .map(|m| RoleInfo {
                role: m.role,
                username: m.username,
            })
            .collect();

        Ok(Page::new(total_count, page_no, page_size, items))
    }

    async fn role_create(&self, role: &str, username: &str) -> anyhow::Result<()> {
        let entity = roles::ActiveModel {
            role: Set(role.to_string()),
            username: Set(username.to_string()),
        };

        roles::Entity::insert(entity).exec(&self.db).await?;
        Ok(())
    }

    async fn role_delete(&self, role: &str, username: &str) -> anyhow::Result<()> {
        if username.is_empty() {
            roles::Entity::delete_many()
                .filter(roles::Column::Role.eq(role))
                .exec(&self.db)
                .await?;
        } else {
            roles::Entity::delete_by_id((role.to_string(), username.to_string()))
                .exec(&self.db)
                .await?;
        }
        Ok(())
    }

    async fn role_has_global_admin(&self) -> anyhow::Result<bool> {
        let result = roles::Entity::find()
            .select_only()
            .column_as(Expr::cust("1"), "exists_flag")
            .filter(roles::Column::Role.eq("ROLE_ADMIN"))
            .into_tuple::<i32>()
            .one(&self.db)
            .await?;

        Ok(result.is_some())
    }

    async fn role_has_global_admin_by_username(&self, username: &str) -> anyhow::Result<bool> {
        let result = roles::Entity::find()
            .select_only()
            .column_as(Expr::cust("1"), "exists_flag")
            .filter(roles::Column::Username.eq(username))
            .filter(roles::Column::Role.eq("ROLE_ADMIN"))
            .into_tuple::<i32>()
            .one(&self.db)
            .await?;

        Ok(result.is_some())
    }

    async fn role_search(&self, role: &str) -> anyhow::Result<Vec<String>> {
        let role_names = roles::Entity::find()
            .select_only()
            .column(roles::Column::Role)
            .filter(roles::Column::Role.contains(role))
            .into_tuple::<String>()
            .all(&self.db)
            .await?;

        Ok(role_names)
    }

    async fn permission_find_by_role(&self, role: &str) -> anyhow::Result<Vec<PermissionInfo>> {
        let perms = permissions::Entity::find()
            .filter(permissions::Column::Role.eq(role))
            .all(&self.db)
            .await?
            .into_iter()
            .map(|m| PermissionInfo {
                role: m.role,
                resource: m.resource,
                action: m.action,
            })
            .collect();

        Ok(perms)
    }

    async fn permission_find_by_roles(
        &self,
        role_list: Vec<String>,
    ) -> anyhow::Result<Vec<PermissionInfo>> {
        if role_list.is_empty() {
            return Ok(vec![]);
        }

        let perms = permissions::Entity::find()
            .filter(permissions::Column::Role.is_in(&role_list))
            .all(&self.db)
            .await?
            .into_iter()
            .map(|m| PermissionInfo {
                role: m.role,
                resource: m.resource,
                action: m.action,
            })
            .collect();

        Ok(perms)
    }

    async fn permission_find_page(
        &self,
        role: &str,
        page_no: u64,
        page_size: u64,
        accurate: bool,
    ) -> anyhow::Result<Page<PermissionInfo>> {
        let mut count_select = permissions::Entity::find();
        let mut query_select = permissions::Entity::find();

        if !role.is_empty() {
            if accurate {
                count_select = count_select.filter(permissions::Column::Role.eq(role));
                query_select = query_select.filter(permissions::Column::Role.eq(role));
            } else {
                count_select = count_select.filter(permissions::Column::Role.contains(role));
                query_select = query_select.filter(permissions::Column::Role.contains(role));
            }
        }

        let total_count = count_select.count(&self.db).await?;

        if total_count == 0 {
            return Ok(Page::empty());
        }

        let offset = (page_no - 1) * page_size;
        let items = query_select
            .offset(offset)
            .limit(page_size)
            .all(&self.db)
            .await?
            .into_iter()
            .map(|m| PermissionInfo {
                role: m.role,
                resource: m.resource,
                action: m.action,
            })
            .collect();

        Ok(Page::new(total_count, page_no, page_size, items))
    }

    async fn permission_find_by_id(
        &self,
        role: &str,
        resource: &str,
        action: &str,
    ) -> anyhow::Result<Option<PermissionInfo>> {
        let perm = permissions::Entity::find_by_id((
            role.to_string(),
            resource.to_string(),
            action.to_string(),
        ))
        .one(&self.db)
        .await?
        .map(|m| PermissionInfo {
            role: m.role,
            resource: m.resource,
            action: m.action,
        });

        Ok(perm)
    }

    async fn permission_grant(
        &self,
        role: &str,
        resource: &str,
        action: &str,
    ) -> anyhow::Result<()> {
        let entity = permissions::ActiveModel {
            role: Set(role.to_string()),
            resource: Set(resource.to_string()),
            action: Set(action.to_string()),
        };

        permissions::Entity::insert(entity).exec(&self.db).await?;
        Ok(())
    }

    async fn permission_revoke(
        &self,
        role: &str,
        resource: &str,
        action: &str,
    ) -> anyhow::Result<()> {
        permissions::Entity::delete_by_id((
            role.to_string(),
            resource.to_string(),
            action.to_string(),
        ))
        .exec(&self.db)
        .await?;
        Ok(())
    }
}

// ============================================================================
// CapacityPersistence implementation
// ============================================================================

/// Default capacity values
const DEFAULT_QUOTA: u32 = 200;
const DEFAULT_MAX_SIZE: u32 = 102400; // 100KB
const DEFAULT_MAX_AGGR_COUNT: u32 = 10000;
const DEFAULT_MAX_AGGR_SIZE: u32 = 2097152; // 2MB
const DEFAULT_MAX_HISTORY_COUNT: u32 = 24;

#[async_trait]
impl CapacityPersistence for ExternalDbPersistService {
    async fn capacity_get_tenant(&self, tenant_id: &str) -> anyhow::Result<Option<CapacityInfo>> {
        let result = tenant_capacity::Entity::find()
            .filter(tenant_capacity::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?;

        Ok(result.map(|m| CapacityInfo {
            id: Some(m.id),
            identifier: m.tenant_id,
            quota: m.quota,
            usage: m.usage,
            max_size: m.max_size,
            max_aggr_count: m.max_aggr_count,
            max_aggr_size: m.max_aggr_size,
            max_history_count: m.max_history_count,
        }))
    }

    #[allow(clippy::too_many_arguments)]
    async fn capacity_upsert_tenant(
        &self,
        tenant_id: &str,
        quota: Option<u32>,
        max_size: Option<u32>,
        max_aggr_count: Option<u32>,
        max_aggr_size: Option<u32>,
        max_history_count: Option<u32>,
    ) -> anyhow::Result<CapacityInfo> {
        let existing = tenant_capacity::Entity::find()
            .filter(tenant_capacity::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?;

        let now = chrono::Utc::now().naive_utc();

        if let Some(model) = existing {
            // Update existing record
            let mut active: tenant_capacity::ActiveModel = model.into();
            if let Some(q) = quota {
                active.quota = Set(q);
            }
            if let Some(s) = max_size {
                active.max_size = Set(s);
            }
            if let Some(c) = max_aggr_count {
                active.max_aggr_count = Set(c);
            }
            if let Some(s) = max_aggr_size {
                active.max_aggr_size = Set(s);
            }
            if let Some(h) = max_history_count {
                active.max_history_count = Set(h);
            }
            active.gmt_modified = Set(now);

            let updated = active.update(&self.db).await?;

            Ok(CapacityInfo {
                id: Some(updated.id),
                identifier: updated.tenant_id,
                quota: updated.quota,
                usage: updated.usage,
                max_size: updated.max_size,
                max_aggr_count: updated.max_aggr_count,
                max_aggr_size: updated.max_aggr_size,
                max_history_count: updated.max_history_count,
            })
        } else {
            // Create new record with defaults overridden by provided values
            let active = tenant_capacity::ActiveModel {
                tenant_id: Set(tenant_id.to_string()),
                quota: Set(quota.unwrap_or(DEFAULT_QUOTA)),
                usage: Set(0),
                max_size: Set(max_size.unwrap_or(DEFAULT_MAX_SIZE)),
                max_aggr_count: Set(max_aggr_count.unwrap_or(DEFAULT_MAX_AGGR_COUNT)),
                max_aggr_size: Set(max_aggr_size.unwrap_or(DEFAULT_MAX_AGGR_SIZE)),
                max_history_count: Set(max_history_count.unwrap_or(DEFAULT_MAX_HISTORY_COUNT)),
                gmt_create: Set(now),
                gmt_modified: Set(now),
                ..Default::default()
            };

            let inserted = active.insert(&self.db).await?;

            Ok(CapacityInfo {
                id: Some(inserted.id),
                identifier: inserted.tenant_id,
                quota: inserted.quota,
                usage: inserted.usage,
                max_size: inserted.max_size,
                max_aggr_count: inserted.max_aggr_count,
                max_aggr_size: inserted.max_aggr_size,
                max_history_count: inserted.max_history_count,
            })
        }
    }

    async fn capacity_delete_tenant(&self, tenant_id: &str) -> anyhow::Result<bool> {
        let result = tenant_capacity::Entity::delete_many()
            .filter(tenant_capacity::Column::TenantId.eq(tenant_id))
            .exec(&self.db)
            .await?;

        Ok(result.rows_affected > 0)
    }

    async fn capacity_get_group(&self, group_id: &str) -> anyhow::Result<Option<CapacityInfo>> {
        let result = group_capacity::Entity::find()
            .filter(group_capacity::Column::GroupId.eq(group_id))
            .one(&self.db)
            .await?;

        Ok(result.map(|m| CapacityInfo {
            id: Some(m.id),
            identifier: m.group_id,
            quota: m.quota,
            usage: m.usage,
            max_size: m.max_size,
            max_aggr_count: m.max_aggr_count,
            max_aggr_size: m.max_aggr_size,
            max_history_count: m.max_history_count,
        }))
    }

    #[allow(clippy::too_many_arguments)]
    async fn capacity_upsert_group(
        &self,
        group_id: &str,
        quota: Option<u32>,
        max_size: Option<u32>,
        max_aggr_count: Option<u32>,
        max_aggr_size: Option<u32>,
        max_history_count: Option<u32>,
    ) -> anyhow::Result<CapacityInfo> {
        let existing = group_capacity::Entity::find()
            .filter(group_capacity::Column::GroupId.eq(group_id))
            .one(&self.db)
            .await?;

        let now = chrono::Utc::now().naive_utc();

        if let Some(model) = existing {
            // Update existing record
            let mut active: group_capacity::ActiveModel = model.into();
            if let Some(q) = quota {
                active.quota = Set(q);
            }
            if let Some(s) = max_size {
                active.max_size = Set(s);
            }
            if let Some(c) = max_aggr_count {
                active.max_aggr_count = Set(c);
            }
            if let Some(s) = max_aggr_size {
                active.max_aggr_size = Set(s);
            }
            if let Some(h) = max_history_count {
                active.max_history_count = Set(h);
            }
            active.gmt_modified = Set(now);

            let updated = active.update(&self.db).await?;

            Ok(CapacityInfo {
                id: Some(updated.id),
                identifier: updated.group_id,
                quota: updated.quota,
                usage: updated.usage,
                max_size: updated.max_size,
                max_aggr_count: updated.max_aggr_count,
                max_aggr_size: updated.max_aggr_size,
                max_history_count: updated.max_history_count,
            })
        } else {
            // Create new record with defaults overridden by provided values
            let active = group_capacity::ActiveModel {
                group_id: Set(group_id.to_string()),
                quota: Set(quota.unwrap_or(DEFAULT_QUOTA)),
                usage: Set(0),
                max_size: Set(max_size.unwrap_or(DEFAULT_MAX_SIZE)),
                max_aggr_count: Set(max_aggr_count.unwrap_or(DEFAULT_MAX_AGGR_COUNT)),
                max_aggr_size: Set(max_aggr_size.unwrap_or(DEFAULT_MAX_AGGR_SIZE)),
                max_history_count: Set(max_history_count.unwrap_or(DEFAULT_MAX_HISTORY_COUNT)),
                gmt_create: Set(now),
                gmt_modified: Set(now),
                ..Default::default()
            };

            let inserted = active.insert(&self.db).await?;

            Ok(CapacityInfo {
                id: Some(inserted.id),
                identifier: inserted.group_id,
                quota: inserted.quota,
                usage: inserted.usage,
                max_size: inserted.max_size,
                max_aggr_count: inserted.max_aggr_count,
                max_aggr_size: inserted.max_aggr_size,
                max_history_count: inserted.max_history_count,
            })
        }
    }

    async fn capacity_delete_group(&self, group_id: &str) -> anyhow::Result<bool> {
        let result = group_capacity::Entity::delete_many()
            .filter(group_capacity::Column::GroupId.eq(group_id))
            .exec(&self.db)
            .await?;

        Ok(result.rows_affected > 0)
    }
}
