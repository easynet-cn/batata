//! ConfigPersistence implementation for ExternalDbPersistService

use md5::Digest;
use std::collections::HashMap;

use async_trait::async_trait;
use sea_orm::{prelude::Expr, sea_query::Asterisk, *};

use crate::entity::{config_info, config_info_gray, config_tags_relation, his_config_info};
use crate::model::*;
use crate::traits::*;

use super::ExternalDbPersistService;

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

        // Tag filtering: single subquery with GROUP BY + HAVING COUNT
        // replaces N separate subqueries (one per tag) with one query
        if !tags.is_empty() {
            let tag_count = tags.len() as i64;
            base_select = base_select.filter(
                config_info::Column::Id.in_subquery(
                    config_tags_relation::Entity::find()
                        .select_only()
                        .column(config_tags_relation::Column::Id)
                        .filter(
                            config_tags_relation::Column::TagName
                                .is_in(tags.iter().map(|t| t.as_str())),
                        )
                        .group_by(config_tags_relation::Column::Id)
                        .having(
                            Expr::col(config_tags_relation::Column::TagName)
                                .count()
                                .eq(tag_count),
                        )
                        .into_query(),
                ),
            );
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
        cas_md5: Option<&str>,
    ) -> anyhow::Result<bool> {
        let md5_hash = const_hex::encode(md5::Md5::digest(content));
        let tags = normalize_tags(config_tags);
        let now = chrono::Local::now().naive_local();

        // Start transaction BEFORE reading so the CAS check and UPDATE are
        // atomic — prevents TOCTOU race where two concurrent requests both
        // read the same MD5 and both pass the check. Matches Nacos's approach
        // of putting the MD5 in the UPDATE WHERE clause.
        let tx = self.db.begin().await?;

        let mut query = config_info::Entity::find()
            .filter(config_info::Column::DataId.eq(data_id))
            .filter(config_info::Column::GroupId.eq(group_id))
            .filter(config_info::Column::TenantId.eq(tenant_id));

        // CAS: add MD5 filter to the SELECT so we only get the row if MD5 matches.
        // If cas_md5 is provided but the row doesn't match, `existing` will be None
        // and we know the CAS failed (either config doesn't exist or MD5 mismatch).
        if let Some(expected_md5) = cas_md5 {
            if !expected_md5.is_empty() {
                query = query.filter(config_info::Column::Md5.eq(expected_md5));
            }
        }

        let existing = query.one(&tx).await?;

        // CAS conflict check
        if cas_md5.is_some() && existing.is_none() {
            // Re-check without MD5 filter to distinguish "not exists" from "MD5 mismatch"
            let exists = config_info::Entity::find()
                .filter(config_info::Column::DataId.eq(data_id))
                .filter(config_info::Column::GroupId.eq(group_id))
                .filter(config_info::Column::TenantId.eq(tenant_id))
                .one(&tx)
                .await?;
            if exists.is_some() {
                anyhow::bail!("CAS conflict: MD5 mismatch");
            } else if cas_md5.map(|m| !m.is_empty()).unwrap_or(false) {
                anyhow::bail!("CAS conflict: config does not exist");
            }
        }

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
                    id: Set(entity.id),
                    nid: NotSet,
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

                // Update config — use set_if_not_equals to skip unchanged columns,
                // reducing the SQL UPDATE SET clause to only changed fields.
                // If nothing changed, skip the UPDATE entirely.
                let mut active: config_info::ActiveModel = entity.into();
                active.content.set_if_not_equals(Some(content.to_string()));
                active.md5.set_if_not_equals(Some(md5_hash));
                active
                    .src_user
                    .set_if_not_equals(Some(src_user.to_string()));
                active.src_ip.set_if_not_equals(Some(src_ip.to_string()));
                active
                    .app_name
                    .set_if_not_equals(Some(app_name.to_string()));
                active.c_desc.set_if_not_equals(Some(desc.to_string()));
                active.c_use.set_if_not_equals(Some(r#use.to_string()));
                active.effect.set_if_not_equals(Some(effect.to_string()));
                active.r#type.set_if_not_equals(Some(r#type.to_string()));
                active.c_schema.set_if_not_equals(Some(schema.to_string()));
                active
                    .encrypted_data_key
                    .set_if_not_equals(Some(encrypted_data_key.to_string()));
                if active.is_changed() {
                    // Only set gmt_modified when there are actual changes
                    active.gmt_modified = Set(Some(now));
                    active.update(&tx).await?;
                }

                // Update tags only if changed — skip delete+insert when tags are identical.
                // Build sorted old tag names for comparison.
                let mut old_tag_names: Vec<&str> =
                    old_tags.iter().map(|t| t.tag_name.as_str()).collect();
                old_tag_names.sort_unstable();
                let mut new_tag_names: Vec<&str> = tags.clone();
                new_tag_names.sort_unstable();

                if old_tag_names != new_tag_names {
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
            }
            None => {
                // Create new config
                let entity = config_info::ActiveModel {
                    data_id: Set(data_id.to_string()),
                    group_id: Set(Some(group_id.to_string())),
                    content: Set(Some(content.to_string())),
                    md5: Set(Some(md5_hash.clone())),
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

                // Record creation in history — reuse md5_hash instead of recomputing
                let his = his_config_info::ActiveModel {
                    id: Set(new_id),
                    nid: NotSet,
                    data_id: Set(data_id.to_string()),
                    group_id: Set(group_id.to_string()),
                    app_name: Set(Some(app_name.to_string())),
                    content: Set(content.to_string()),
                    md5: Set(Some(md5_hash)),
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
                id: Set(entity.id),
                nid: NotSet,
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
        cas_md5: Option<&str>,
    ) -> anyhow::Result<bool> {
        let md5_hash = const_hex::encode(md5::Md5::digest(content));
        let now = chrono::Local::now().naive_local();

        let existing = config_info_gray::Entity::find()
            .filter(config_info_gray::Column::DataId.eq(data_id))
            .filter(config_info_gray::Column::GroupId.eq(group_id))
            .filter(config_info_gray::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?;

        // CAS check: if cas_md5 provided, verify current gray MD5 matches before update
        if let Some(expected_md5) = cas_md5 {
            if let Some(ref entity) = existing {
                let current_md5 = entity.md5.as_deref().unwrap_or("");
                if current_md5 != expected_md5 {
                    anyhow::bail!(
                        "CAS conflict: expected md5={}, actual md5={}",
                        expected_md5,
                        current_md5
                    );
                }
            } else if !expected_md5.is_empty() {
                anyhow::bail!("CAS conflict: gray config does not exist");
            }
        }

        match existing {
            Some(entity) => {
                let mut active: config_info_gray::ActiveModel = entity.into();
                active.content.set_if_not_equals(content.to_string());
                active.md5.set_if_not_equals(Some(md5_hash));
                active.gray_name.set_if_not_equals(gray_name.to_string());
                active.gray_rule.set_if_not_equals(gray_rule.to_string());
                active
                    .src_user
                    .set_if_not_equals(Some(src_user.to_string()));
                active.src_ip.set_if_not_equals(Some(src_ip.to_string()));
                active
                    .app_name
                    .set_if_not_equals(Some(app_name.to_string()));
                active
                    .encrypted_data_key
                    .set_if_not_equals(encrypted_data_key.to_string());
                if active.is_changed() {
                    active.gmt_modified = Set(now);
                    active.update(&self.db).await?;
                }
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

        // Bulk fetch all entities in a single query instead of N individual queries
        let entities: Vec<config_info::Model> = config_info::Entity::find()
            .filter(config_info::Column::Id.is_in(ids.iter().copied()))
            .all(&tx)
            .await?;

        if entities.is_empty() {
            tx.commit().await?;
            return Ok(0);
        }

        // Bulk insert history records
        let history_records: Vec<his_config_info::ActiveModel> = entities
            .iter()
            .map(|entity| his_config_info::ActiveModel {
                id: Set(entity.id),
                nid: NotSet,
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
            })
            .collect();

        let deleted = entities.len();
        his_config_info::Entity::insert_many(history_records)
            .exec(&tx)
            .await?;

        // Bulk delete all configs in a single query
        let found_ids: Vec<i64> = entities.iter().map(|e| e.id).collect();
        config_info::Entity::delete_many()
            .filter(config_info::Column::Id.is_in(found_ids))
            .exec(&tx)
            .await?;

        tx.commit().await?;
        Ok(deleted)
    }

    async fn config_history_find_by_id(
        &self,
        nid: i64,
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

    async fn config_count_by_namespace(&self, namespace_id: &str) -> anyhow::Result<i64> {
        let count = config_info::Entity::find()
            .select_only()
            .column_as(Expr::col(Asterisk).count(), "count")
            .filter(config_info::Column::TenantId.eq(namespace_id))
            .into_tuple::<i64>()
            .one(&self.db)
            .await?
            .unwrap_or_default();

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
        current_nid: i64,
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

        // Batch fetch tags for all configs (avoids N+1 queries)
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

        // Batch fetch tags for all configs (avoids N+1 queries)
        let config_ids: Vec<i64> = models.iter().map(|c| c.id).collect();

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
        let mut tags_map: HashMap<i64, Vec<String>> = HashMap::with_capacity(models.len());
        for tag in tags {
            tags_map.entry(tag.id).or_default().push(tag.tag_name);
        }

        // Convert to ConfigStorageData with tags
        let result: Vec<ConfigStorageData> = models
            .into_iter()
            .map(|c| {
                let config_tags = tags_map.get(&c.id).map(|t| t.join(",")).unwrap_or_default();
                config_entity_to_storage(c, config_tags)
            })
            .collect();

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
