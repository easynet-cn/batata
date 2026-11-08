//! Configuration import service
//!
//! Provides pure functions for parsing import files (Nacos ZIP, Consul JSON).
//! Database access is handled by PersistenceService; this module only handles deserialization.

use std::io::{Cursor, Read};

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use zip::ZipArchive;

use crate::model::{ConfigImportItem, ConsulKVExportItem, NacosConfigMetadata, NacosExportItem};

/// Maximum number of entries in an imported ZIP archive.
const MAX_ZIP_ENTRIES: usize = 10_000;

/// Maximum total uncompressed bytes read from an imported ZIP archive.
///
/// A small compressed upload can expand without bound; capping the *actual*
/// bytes read (not just the header-declared size) prevents a zip bomb from
/// exhausting memory.
const MAX_ZIP_UNCOMPRESSED_BYTES: u64 = 64 * 1024 * 1024;

/// Parse a Nacos-format ZIP file and extract configuration items
pub fn parse_nacos_import_zip(data: &[u8]) -> anyhow::Result<Vec<NacosExportItem>> {
    parse_nacos_import_zip_with_limits(data, MAX_ZIP_ENTRIES, MAX_ZIP_UNCOMPRESSED_BYTES)
}

fn parse_nacos_import_zip_with_limits(
    data: &[u8],
    max_entries: usize,
    max_uncompressed_bytes: u64,
) -> anyhow::Result<Vec<NacosExportItem>> {
    let cursor = Cursor::new(data);
    let mut archive = ZipArchive::new(cursor)?;

    if archive.len() > max_entries {
        anyhow::bail!(
            "import archive has {} entries, exceeding the limit of {}",
            archive.len(),
            max_entries
        );
    }

    // Pre-allocate with estimated capacity to avoid reallocations
    // Each config typically has a content file and a .meta file
    let estimated_configs = archive.len() / 2;
    let mut items: Vec<NacosExportItem> = Vec::with_capacity(estimated_configs);
    let mut content_map: std::collections::HashMap<String, String> =
        std::collections::HashMap::with_capacity(estimated_configs);
    let mut meta_map: std::collections::HashMap<String, NacosConfigMetadata> =
        std::collections::HashMap::with_capacity(estimated_configs);

    let mut total_uncompressed: u64 = 0;

    // First pass: read all files
    for i in 0..archive.len() {
        let file = archive.by_index(i)?;
        // zip 9 returns a `Result` for entry names that fail to decode.
        let name = file.name()?.to_string();

        if file.is_dir() {
            continue;
        }

        // Read at most one byte past the remaining budget, so an over-cap
        // entry is rejected instead of silently truncated. `take` bounds the
        // real bytes read even if the ZIP header lies about `size()`.
        let remaining = max_uncompressed_bytes.saturating_sub(total_uncompressed);
        let mut bytes = Vec::new();
        file.take(remaining.saturating_add(1))
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > remaining {
            anyhow::bail!(
                "import archive expands beyond {} uncompressed bytes",
                max_uncompressed_bytes
            );
        }
        total_uncompressed = total_uncompressed.saturating_add(bytes.len() as u64);
        let contents = String::from_utf8(bytes)
            .map_err(|e| anyhow::anyhow!("import entry is not valid UTF-8: {e}"))?;

        if name.ends_with(".meta") {
            // Parse metadata file
            let base_name = name.strip_suffix(".meta").unwrap_or(&name);
            let metadata: NacosConfigMetadata = yaml_serde::from_str(&contents)?;
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
    // Pre-allocate with exact capacity since we know the number of items
    let mut items: Vec<ConfigImportItem> = Vec::with_capacity(kv_items.len());

    for kv in kv_items {
        // Decode base64 value
        let content = match BASE64.decode(&kv.value) {
            // A decoded value that is not UTF-8 is a corrupt export, not an
            // empty config. Writing an empty string here silently loses data.
            Ok(bytes) => String::from_utf8(bytes).map_err(|e| {
                anyhow::anyhow!("Consul KV value for '{}' is not valid UTF-8: {}", kv.key, e)
            })?,
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

/// Infer configuration type from file extension
fn infer_config_type(filename: &str) -> String {
    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();

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

    /// Build a small in-memory ZIP for the parser guards.
    fn make_zip(entries: &[(&str, &str)]) -> Vec<u8> {
        use std::io::Write;
        use zip::{ZipWriter, write::SimpleFileOptions};

        let mut buffer = std::io::Cursor::new(Vec::new());
        {
            let mut zip = ZipWriter::new(&mut buffer);
            for (name, content) in entries {
                zip.start_file(*name, SimpleFileOptions::default()).unwrap();
                zip.write_all(content.as_bytes()).unwrap();
            }
            zip.finish().unwrap();
        }
        buffer.into_inner()
    }

    /// Mutant check: removing the entry-count guard makes this return `Ok`.
    #[test]
    fn nacos_import_zip_rejects_too_many_entries() {
        let zip = make_zip(&[("DEFAULT_GROUP/a.yaml", "a: 1")]);
        assert!(parse_nacos_import_zip_with_limits(&zip, 0, 1024).is_err());
    }

    /// Mutant check: removing the uncompressed-byte guard (or reading the
    /// entry without `take`) makes the over-cap call return `Ok`.
    #[test]
    fn nacos_import_zip_rejects_over_cap_uncompressed_bytes() {
        let zip = make_zip(&[("DEFAULT_GROUP/a.yaml", "0123456789")]);
        assert!(parse_nacos_import_zip_with_limits(&zip, 10, 4).is_err());
        // Success path: an adequate budget must still parse.
        assert!(parse_nacos_import_zip_with_limits(&zip, 10, 1024).is_ok());
    }

    /// Mutant check: restoring `String::from_utf8(...).unwrap_or_default()`
    /// silently writes an empty config and returns `Ok`.
    #[test]
    fn consul_import_rejects_non_utf8_base64_value() {
        // `/w==` decodes to the single byte 0xFF, which is not valid UTF-8.
        let json = r#"[{"Key":"public/DEFAULT_GROUP/bad.yaml","Flags":0,"Value":"/w=="}]"#;
        assert!(parse_consul_import_json(json.as_bytes(), "public").is_err());
    }

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
        use zip::{ZipWriter, write::SimpleFileOptions};

        // Create a test ZIP in memory
        let mut buffer = std::io::Cursor::new(Vec::new());
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
            zip.start_file("DEFAULT_GROUP/app.yaml.meta", options)
                .unwrap();
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
        use zip::{ZipWriter, write::SimpleFileOptions};

        let mut buffer = std::io::Cursor::new(Vec::new());
        {
            let mut zip = ZipWriter::new(&mut buffer);
            let options = SimpleFileOptions::default();

            // Only add config content, no metadata
            zip.start_file("MY_GROUP/config.properties", options)
                .unwrap();
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
