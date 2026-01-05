// Configuration export service
// Provides functions for exporting configurations in Nacos ZIP and Consul JSON formats

use std::io::{Cursor, Write};

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use zip::{ZipWriter, write::SimpleFileOptions};

use crate::{
    config::{
        export_model::{ConsulKVExportItem, NacosConfigMetadata, NacosExportItem},
        model::ConfigAllInfo,
    },
    entity::{config_info, config_tags_relation},
};

/// Find all configurations for export with optional filters
pub async fn find_configs_for_export(
    db: &DatabaseConnection,
    namespace_id: &str,
    group: Option<&str>,
    data_ids: Option<Vec<String>>,
    app_name: Option<&str>,
) -> anyhow::Result<Vec<ConfigAllInfo>> {
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

    let configs = query.all(db).await?;

    // Fetch tags for all configs
    let config_ids: Vec<i64> = configs.iter().map(|c| c.id).collect();

    let tags = if !config_ids.is_empty() {
        config_tags_relation::Entity::find()
            .filter(config_tags_relation::Column::Id.is_in(config_ids))
            .order_by_asc(config_tags_relation::Column::Id)
            .order_by_asc(config_tags_relation::Column::Nid)
            .all(db)
            .await?
    } else {
        vec![]
    };

    // Group tags by config id
    let mut tags_map: std::collections::HashMap<i64, Vec<String>> =
        std::collections::HashMap::new();
    for tag in tags {
        tags_map.entry(tag.id).or_default().push(tag.tag_name);
    }

    // Build ConfigAllInfo with tags
    let result: Vec<ConfigAllInfo> = configs
        .into_iter()
        .map(|c| {
            let config_tags = tags_map.get(&c.id).map(|t| t.join(",")).unwrap_or_default();
            let mut info = ConfigAllInfo::from(c);
            info.config_tags = config_tags;
            info
        })
        .collect();

    Ok(result)
}

/// Create a Nacos-format ZIP file from configurations
pub fn create_nacos_export_zip(configs: Vec<ConfigAllInfo>) -> anyhow::Result<Vec<u8>> {
    let mut buffer = Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(&mut buffer);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);

    for config in configs {
        let group = &config.config_info.config_info_base.group;
        let data_id = &config.config_info.config_info_base.data_id;
        let content = &config.config_info.config_info_base.content;

        // Write config content file: group/dataId
        let content_path = format!("{}/{}", group, data_id);
        zip.start_file(&content_path, options)?;
        zip.write_all(content.as_bytes())?;

        // Write metadata file: group/dataId.meta
        let meta_path = format!("{}/{}.meta", group, data_id);
        let metadata = NacosConfigMetadata {
            data_id: data_id.clone(),
            group: group.clone(),
            namespace_id: config.config_info.tenant.clone(),
            content_type: config.config_info.r#type.clone(),
            app_name: config.config_info.app_name.clone(),
            desc: config.desc.clone(),
            config_tags: config.config_tags.clone(),
            md5: config.config_info.config_info_base.md5.clone(),
            encrypted_data_key: config
                .config_info
                .config_info_base
                .encrypted_data_key
                .clone(),
            create_time: config.create_time,
            modify_time: config.modify_time,
        };

        let meta_yaml = serde_yaml::to_string(&metadata)?;
        zip.start_file(&meta_path, options)?;
        zip.write_all(meta_yaml.as_bytes())?;
    }

    zip.finish()?;

    Ok(buffer.into_inner())
}

/// Create Consul-format JSON from configurations
pub fn create_consul_export_json(
    configs: Vec<ConfigAllInfo>,
    default_namespace: &str,
) -> anyhow::Result<Vec<u8>> {
    let items: Vec<ConsulKVExportItem> = configs
        .into_iter()
        .map(|c| {
            let namespace = if c.config_info.tenant.is_empty() {
                default_namespace.to_string()
            } else {
                c.config_info.tenant
            };
            let group = c.config_info.config_info_base.group;
            let data_id = c.config_info.config_info_base.data_id;
            let content = c.config_info.config_info_base.content;

            ConsulKVExportItem {
                key: format!("{}/{}/{}", namespace, group, data_id),
                flags: 0,
                value: BASE64.encode(content.as_bytes()),
            }
        })
        .collect();

    let json = serde_json::to_vec_pretty(&items)?;
    Ok(json)
}

/// Convert ConfigAllInfo to NacosExportItem for import operations
pub fn config_to_export_item(config: ConfigAllInfo) -> NacosExportItem {
    NacosExportItem {
        metadata: NacosConfigMetadata {
            data_id: config.config_info.config_info_base.data_id,
            group: config.config_info.config_info_base.group,
            namespace_id: config.config_info.tenant,
            content_type: config.config_info.r#type,
            app_name: config.config_info.app_name,
            desc: config.desc,
            config_tags: config.config_tags,
            md5: config.config_info.config_info_base.md5,
            encrypted_data_key: config.config_info.config_info_base.encrypted_data_key,
            create_time: config.create_time,
            modify_time: config.modify_time,
        },
        content: config.config_info.config_info_base.content,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_consul_export_json() {
        let configs = vec![ConfigAllInfo {
            config_info: crate::config::model::ConfigInfo {
                config_info_base: crate::config::model::ConfigInfoBase {
                    id: 1,
                    data_id: "app.yaml".to_string(),
                    group: "DEFAULT_GROUP".to_string(),
                    content: "server:\n  port: 8080".to_string(),
                    md5: "abc123".to_string(),
                    encrypted_data_key: String::new(),
                },
                tenant: "public".to_string(),
                app_name: "my-app".to_string(),
                r#type: "yaml".to_string(),
            },
            create_time: 0,
            modify_time: 0,
            create_user: String::new(),
            create_ip: String::new(),
            desc: String::new(),
            r#use: String::new(),
            effect: String::new(),
            schema: String::new(),
            config_tags: String::new(),
        }];

        let json = create_consul_export_json(configs, "public").unwrap();
        let parsed: Vec<ConsulKVExportItem> = serde_json::from_slice(&json).unwrap();

        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].key, "public/DEFAULT_GROUP/app.yaml");
        assert_eq!(parsed[0].flags, 0);

        // Verify base64 decode
        let decoded = BASE64.decode(&parsed[0].value).unwrap();
        assert_eq!(String::from_utf8(decoded).unwrap(), "server:\n  port: 8080");
    }

    #[test]
    fn test_create_nacos_export_zip() {
        let configs = vec![ConfigAllInfo {
            config_info: crate::config::model::ConfigInfo {
                config_info_base: crate::config::model::ConfigInfoBase {
                    id: 1,
                    data_id: "app.properties".to_string(),
                    group: "DEFAULT_GROUP".to_string(),
                    content: "key=value".to_string(),
                    md5: "abc123".to_string(),
                    encrypted_data_key: String::new(),
                },
                tenant: "public".to_string(),
                app_name: "my-app".to_string(),
                r#type: "properties".to_string(),
            },
            create_time: 1704067200000,
            modify_time: 1704067200000,
            create_user: "admin".to_string(),
            create_ip: "127.0.0.1".to_string(),
            desc: "Test config".to_string(),
            r#use: String::new(),
            effect: String::new(),
            schema: String::new(),
            config_tags: "env:test".to_string(),
        }];

        let zip_data = create_nacos_export_zip(configs).unwrap();
        assert!(!zip_data.is_empty());

        // Verify ZIP structure
        let cursor = Cursor::new(zip_data);
        let mut archive = zip::ZipArchive::new(cursor).unwrap();
        assert_eq!(archive.len(), 2); // content + meta

        // Check content file
        {
            let content_file = archive.by_name("DEFAULT_GROUP/app.properties").unwrap();
            assert!(content_file.size() > 0);
        }

        // Check meta file
        {
            let meta_file = archive
                .by_name("DEFAULT_GROUP/app.properties.meta")
                .unwrap();
            assert!(meta_file.size() > 0);
        }
    }

    #[test]
    fn test_config_to_export_item() {
        let config = ConfigAllInfo {
            config_info: crate::config::model::ConfigInfo {
                config_info_base: crate::config::model::ConfigInfoBase {
                    id: 1,
                    data_id: "test.yaml".to_string(),
                    group: "TEST_GROUP".to_string(),
                    content: "test: true".to_string(),
                    md5: "xyz789".to_string(),
                    encrypted_data_key: String::new(),
                },
                tenant: "dev".to_string(),
                app_name: "test-app".to_string(),
                r#type: "yaml".to_string(),
            },
            create_time: 1000,
            modify_time: 2000,
            create_user: "user".to_string(),
            create_ip: "10.0.0.1".to_string(),
            desc: "Description".to_string(),
            r#use: "Testing".to_string(),
            effect: "All".to_string(),
            schema: String::new(),
            config_tags: "tag1,tag2".to_string(),
        };

        let item = config_to_export_item(config);
        assert_eq!(item.metadata.data_id, "test.yaml");
        assert_eq!(item.metadata.group, "TEST_GROUP");
        assert_eq!(item.metadata.namespace_id, "dev");
        assert_eq!(item.content, "test: true");
        assert_eq!(item.metadata.config_tags, "tag1,tag2");
    }
}
