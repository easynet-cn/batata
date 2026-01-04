// Configuration import service
// Provides functions for importing configurations from Nacos ZIP and Consul JSON formats

use std::io::{Cursor, Read};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use zip::ZipArchive;

use crate::{
    config::export_model::{
        ConfigImportItem, ConsulKVExportItem, ImportFailItem, ImportResult, NacosConfigMetadata,
        NacosExportItem, SameConfigPolicy,
    },
    entity::config_info,
    service::config as config_service,
};

/// Parse a Nacos-format ZIP file and extract configuration items
pub fn parse_nacos_import_zip(data: &[u8]) -> anyhow::Result<Vec<NacosExportItem>> {
    let cursor = Cursor::new(data);
    let mut archive = ZipArchive::new(cursor)?;
    let mut items: Vec<NacosExportItem> = Vec::new();
    let mut content_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut meta_map: std::collections::HashMap<String, NacosConfigMetadata> =
        std::collections::HashMap::new();

    // First pass: read all files
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();

        if file.is_dir() {
            continue;
        }

        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        if name.ends_with(".meta") {
            // Parse metadata file
            let base_name = name.strip_suffix(".meta").unwrap_or(&name);
            let metadata: NacosConfigMetadata = serde_yaml::from_str(&contents)?;
            meta_map.insert(base_name.to_string(), metadata);
        } else {
            // Content file
            content_map.insert(name, contents);
        }
    }

    // Second pass: match content with metadata
    for (path, content) in content_map {
        let metadata = if let Some(meta) = meta_map.get(&path) {
            meta.clone()
        } else {
            // Create default metadata from path
            // Path format: group/dataId
            let parts: Vec<&str> = path.splitn(2, '/').collect();
            if parts.len() == 2 {
                NacosConfigMetadata {
                    group: parts[0].to_string(),
                    data_id: parts[1].to_string(),
                    namespace_id: String::new(),
                    content_type: infer_config_type(parts[1]),
                    ..Default::default()
                }
            } else {
                NacosConfigMetadata {
                    data_id: path.clone(),
                    group: "DEFAULT_GROUP".to_string(),
                    content_type: infer_config_type(&path),
                    ..Default::default()
                }
            }
        };

        items.push(NacosExportItem { metadata, content });
    }

    Ok(items)
}

/// Parse Consul-format JSON and extract configuration items
pub fn parse_consul_import_json(
    data: &[u8],
    default_namespace: &str,
) -> anyhow::Result<Vec<ConfigImportItem>> {
    let kv_items: Vec<ConsulKVExportItem> = serde_json::from_slice(data)?;
    let mut items: Vec<ConfigImportItem> = Vec::new();

    for kv in kv_items {
        // Decode base64 value
        let content = match BASE64.decode(&kv.value) {
            Ok(bytes) => String::from_utf8(bytes).unwrap_or_default(),
            Err(_) => kv.value.clone(), // Assume plain text if not valid base64
        };

        // Parse key path: namespace/group/dataId or group/dataId or just dataId
        let parts: Vec<&str> = kv.key.split('/').collect();
        let (namespace_id, group, data_id) = match parts.len() {
            1 => (
                default_namespace.to_string(),
                "DEFAULT_GROUP".to_string(),
                parts[0].to_string(),
            ),
            2 => (
                default_namespace.to_string(),
                parts[0].to_string(),
                parts[1].to_string(),
            ),
            _ => (
                parts[0].to_string(),
                parts[1].to_string(),
                parts[2..].join("/"),
            ),
        };

        items.push(ConfigImportItem {
            namespace_id,
            group,
            data_id: data_id.clone(),
            content,
            config_type: infer_config_type(&data_id),
            app_name: String::new(),
            desc: String::new(),
            config_tags: String::new(),
            encrypted_data_key: String::new(),
        });
    }

    Ok(items)
}

/// Import configurations with the specified conflict policy
pub async fn import_configs(
    db: &DatabaseConnection,
    items: Vec<ConfigImportItem>,
    target_namespace_id: &str,
    policy: SameConfigPolicy,
    src_user: &str,
    src_ip: &str,
) -> anyhow::Result<ImportResult> {
    let mut result = ImportResult::default();

    for item in items {
        let namespace_id = if item.namespace_id.is_empty() {
            target_namespace_id.to_string()
        } else {
            item.namespace_id.clone()
        };

        // Check if config already exists
        let exists = config_info::Entity::find()
            .filter(config_info::Column::DataId.eq(&item.data_id))
            .filter(config_info::Column::GroupId.eq(&item.group))
            .filter(config_info::Column::TenantId.eq(&namespace_id))
            .one(db)
            .await?
            .is_some();

        if exists {
            match policy {
                SameConfigPolicy::Abort => {
                    result.fail_count += 1;
                    result.fail_data.push(ImportFailItem {
                        data_id: item.data_id.clone(),
                        group: item.group.clone(),
                        reason: "Configuration already exists".to_string(),
                    });
                    return Ok(result);
                }
                SameConfigPolicy::Skip => {
                    result.skip_count += 1;
                    continue;
                }
                SameConfigPolicy::Overwrite => {
                    // Continue to update
                }
            }
        }

        // Create or update the configuration
        match config_service::create_or_update(
            db,
            &item.data_id,
            &item.group,
            &namespace_id,
            &item.content,
            &item.app_name,
            src_user,
            src_ip,
            &item.config_tags,
            &item.desc,
            "",     // use
            "",     // effect
            &item.config_type,
            "",     // schema
            &item.encrypted_data_key,
        )
        .await
        {
            Ok(_) => {
                result.success_count += 1;
            }
            Err(e) => {
                result.fail_count += 1;
                result.fail_data.push(ImportFailItem {
                    data_id: item.data_id.clone(),
                    group: item.group.clone(),
                    reason: e.to_string(),
                });
            }
        }
    }

    Ok(result)
}

/// Import Nacos export items (convenience wrapper)
pub async fn import_nacos_items(
    db: &DatabaseConnection,
    items: Vec<NacosExportItem>,
    target_namespace_id: &str,
    policy: SameConfigPolicy,
    src_user: &str,
    src_ip: &str,
) -> anyhow::Result<ImportResult> {
    let config_items: Vec<ConfigImportItem> = items.into_iter().map(|i| i.into()).collect();
    import_configs(db, config_items, target_namespace_id, policy, src_user, src_ip).await
}

/// Infer configuration type from file extension
fn infer_config_type(filename: &str) -> String {
    let ext = filename
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "properties" => "properties".to_string(),
        "xml" => "xml".to_string(),
        "json" => "json".to_string(),
        "yaml" | "yml" => "yaml".to_string(),
        "toml" => "toml".to_string(),
        "html" | "htm" => "html".to_string(),
        _ => "text".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infer_config_type() {
        assert_eq!(infer_config_type("app.properties"), "properties");
        assert_eq!(infer_config_type("config.yaml"), "yaml");
        assert_eq!(infer_config_type("config.yml"), "yaml");
        assert_eq!(infer_config_type("settings.json"), "json");
        assert_eq!(infer_config_type("config.xml"), "xml");
        assert_eq!(infer_config_type("config.toml"), "toml");
        assert_eq!(infer_config_type("readme.txt"), "text");
        assert_eq!(infer_config_type("noextension"), "text");
    }

    #[test]
    fn test_parse_consul_import_json() {
        let json = r#"[
            {
                "Key": "public/DEFAULT_GROUP/app.yaml",
                "Flags": 0,
                "Value": "c2VydmVyOgogIHBvcnQ6IDgwODA="
            },
            {
                "Key": "my-group/config.json",
                "Flags": 0,
                "Value": "eyJrZXkiOiJ2YWx1ZSJ9"
            }
        ]"#;

        let items = parse_consul_import_json(json.as_bytes(), "default").unwrap();
        assert_eq!(items.len(), 2);

        assert_eq!(items[0].namespace_id, "public");
        assert_eq!(items[0].group, "DEFAULT_GROUP");
        assert_eq!(items[0].data_id, "app.yaml");
        assert_eq!(items[0].content, "server:\n  port: 8080");
        assert_eq!(items[0].config_type, "yaml");

        assert_eq!(items[1].namespace_id, "default");
        assert_eq!(items[1].group, "my-group");
        assert_eq!(items[1].data_id, "config.json");
        assert_eq!(items[1].content, r#"{"key":"value"}"#);
        assert_eq!(items[1].config_type, "json");
    }

    #[test]
    fn test_parse_consul_import_simple_key() {
        let json = r#"[{"Key": "simple-config", "Flags": 0, "Value": "dGVzdA=="}]"#;

        let items = parse_consul_import_json(json.as_bytes(), "public").unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].namespace_id, "public");
        assert_eq!(items[0].group, "DEFAULT_GROUP");
        assert_eq!(items[0].data_id, "simple-config");
        assert_eq!(items[0].content, "test");
    }

    #[test]
    fn test_parse_nacos_import_zip() {
        use std::io::Write;
        use zip::{write::SimpleFileOptions, ZipWriter};

        // Create a test ZIP in memory
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut zip = ZipWriter::new(&mut buffer);
            let options = SimpleFileOptions::default();

            // Add config content
            zip.start_file("DEFAULT_GROUP/app.yaml", options).unwrap();
            zip.write_all(b"server:\n  port: 8080").unwrap();

            // Add metadata
            let meta = r#"dataId: app.yaml
group: DEFAULT_GROUP
namespaceId: public
type: yaml
appName: test-app
desc: Test configuration
configTags: env:test
md5: abc123
encryptedDataKey: ""
createTime: 1704067200000
modifyTime: 1704067200000"#;
            zip.start_file("DEFAULT_GROUP/app.yaml.meta", options).unwrap();
            zip.write_all(meta.as_bytes()).unwrap();

            zip.finish().unwrap();
        }

        let zip_data = buffer.into_inner();
        let items = parse_nacos_import_zip(&zip_data).unwrap();

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].metadata.data_id, "app.yaml");
        assert_eq!(items[0].metadata.group, "DEFAULT_GROUP");
        assert_eq!(items[0].metadata.namespace_id, "public");
        assert_eq!(items[0].content, "server:\n  port: 8080");
    }

    #[test]
    fn test_parse_nacos_import_zip_without_meta() {
        use std::io::Write;
        use zip::{write::SimpleFileOptions, ZipWriter};

        let mut buffer = Cursor::new(Vec::new());
        {
            let mut zip = ZipWriter::new(&mut buffer);
            let options = SimpleFileOptions::default();

            // Only add config content, no metadata
            zip.start_file("MY_GROUP/config.properties", options).unwrap();
            zip.write_all(b"key=value").unwrap();

            zip.finish().unwrap();
        }

        let zip_data = buffer.into_inner();
        let items = parse_nacos_import_zip(&zip_data).unwrap();

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].metadata.data_id, "config.properties");
        assert_eq!(items[0].metadata.group, "MY_GROUP");
        assert_eq!(items[0].metadata.content_type, "properties");
        assert_eq!(items[0].content, "key=value");
    }
}
