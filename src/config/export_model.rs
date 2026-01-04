// Configuration export/import data models
// This file defines data structures for configuration export and import operations

use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::str::FromStr;

/// Nacos configuration metadata for export
/// Stored as .meta file alongside the config content in ZIP
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NacosConfigMetadata {
    pub data_id: String,
    pub group: String,
    pub namespace_id: String,
    #[serde(rename = "type")]
    pub content_type: String,
    pub app_name: String,
    pub desc: String,
    pub config_tags: String,
    pub md5: String,
    pub encrypted_data_key: String,
    pub create_time: i64,
    pub modify_time: i64,
}

/// Single configuration item for Nacos export/import
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NacosExportItem {
    pub metadata: NacosConfigMetadata,
    pub content: String,
}

/// Import operation result summary
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub success_count: u32,
    pub skip_count: u32,
    pub fail_count: u32,
    pub fail_data: Vec<ImportFailItem>,
}

/// Details of a failed import item
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportFailItem {
    pub data_id: String,
    pub group: String,
    pub reason: String,
}

/// Consul KV export format (matches Consul's native format)
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ConsulKVExportItem {
    #[serde(rename = "Key")]
    pub key: String, // Format: namespace/group/dataId
    #[serde(rename = "Flags")]
    pub flags: u64,
    #[serde(rename = "Value")]
    pub value: String, // Base64 encoded content
}

/// Import conflict resolution policy
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SameConfigPolicy {
    /// Stop import on first conflict
    #[default]
    Abort,
    /// Skip conflicting configs, continue with others
    Skip,
    /// Overwrite existing configs with imported data
    Overwrite,
}

impl Display for SameConfigPolicy {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SameConfigPolicy::Abort => write!(f, "ABORT"),
            SameConfigPolicy::Skip => write!(f, "SKIP"),
            SameConfigPolicy::Overwrite => write!(f, "OVERWRITE"),
        }
    }
}

impl FromStr for SameConfigPolicy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "ABORT" => Ok(SameConfigPolicy::Abort),
            "SKIP" => Ok(SameConfigPolicy::Skip),
            "OVERWRITE" => Ok(SameConfigPolicy::Overwrite),
            _ => Err(format!("Invalid policy: {}. Valid values: ABORT, SKIP, OVERWRITE", s)),
        }
    }
}

/// Nacos export request parameters
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ExportRequest {
    pub namespace_id: String,
    pub group: Option<String>,
    pub data_ids: Option<String>, // Comma-separated dataIds
    pub ids: Option<String>,      // Comma-separated config IDs
    pub app_name: Option<String>,
}

/// Nacos import request parameters
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ImportRequest {
    pub namespace_id: String,
    pub policy: Option<String>, // ABORT, SKIP, OVERWRITE
}

impl ImportRequest {
    pub fn get_policy(&self) -> SameConfigPolicy {
        self.policy
            .as_ref()
            .and_then(|p| p.parse().ok())
            .unwrap_or_default()
    }
}

/// Consul export request parameters
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ConsulExportRequest {
    pub namespace_id: Option<String>,
    pub prefix: Option<String>,
}

/// Consul import request parameters
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ConsulImportRequest {
    pub namespace_id: Option<String>,
}

/// Intermediate structure for config import
#[derive(Clone, Debug, Default)]
pub struct ConfigImportItem {
    pub namespace_id: String,
    pub group: String,
    pub data_id: String,
    pub content: String,
    pub config_type: String,
    pub app_name: String,
    pub desc: String,
    pub config_tags: String,
    pub encrypted_data_key: String,
}

impl From<NacosExportItem> for ConfigImportItem {
    fn from(item: NacosExportItem) -> Self {
        Self {
            namespace_id: item.metadata.namespace_id,
            group: item.metadata.group,
            data_id: item.metadata.data_id,
            content: item.content,
            config_type: item.metadata.content_type,
            app_name: item.metadata.app_name,
            desc: item.metadata.desc,
            config_tags: item.metadata.config_tags,
            encrypted_data_key: item.metadata.encrypted_data_key,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_same_config_policy_from_str() {
        assert_eq!(
            "ABORT".parse::<SameConfigPolicy>().unwrap(),
            SameConfigPolicy::Abort
        );
        assert_eq!(
            "skip".parse::<SameConfigPolicy>().unwrap(),
            SameConfigPolicy::Skip
        );
        assert_eq!(
            "Overwrite".parse::<SameConfigPolicy>().unwrap(),
            SameConfigPolicy::Overwrite
        );
        assert!("invalid".parse::<SameConfigPolicy>().is_err());
    }

    #[test]
    fn test_same_config_policy_display() {
        assert_eq!(format!("{}", SameConfigPolicy::Abort), "ABORT");
        assert_eq!(format!("{}", SameConfigPolicy::Skip), "SKIP");
        assert_eq!(format!("{}", SameConfigPolicy::Overwrite), "OVERWRITE");
    }

    #[test]
    fn test_import_request_get_policy() {
        let req = ImportRequest {
            namespace_id: "public".to_string(),
            policy: Some("SKIP".to_string()),
        };
        assert_eq!(req.get_policy(), SameConfigPolicy::Skip);

        let req_default = ImportRequest::default();
        assert_eq!(req_default.get_policy(), SameConfigPolicy::Abort);
    }

    #[test]
    fn test_nacos_config_metadata_serialization() {
        let meta = NacosConfigMetadata {
            data_id: "app.yaml".to_string(),
            group: "DEFAULT_GROUP".to_string(),
            namespace_id: "public".to_string(),
            content_type: "yaml".to_string(),
            app_name: "my-app".to_string(),
            desc: "Application config".to_string(),
            config_tags: "env:prod".to_string(),
            md5: "abc123".to_string(),
            encrypted_data_key: String::new(),
            create_time: 1704067200000,
            modify_time: 1704067200000,
        };

        let yaml = serde_yaml::to_string(&meta).unwrap();
        assert!(yaml.contains("dataId: app.yaml"));
        assert!(yaml.contains("group: DEFAULT_GROUP"));
    }

    #[test]
    fn test_consul_kv_export_item_serialization() {
        let item = ConsulKVExportItem {
            key: "public/DEFAULT_GROUP/app.yaml".to_string(),
            flags: 0,
            value: "c2VydmVyOgogIHBvcnQ6IDgwODA=".to_string(),
        };

        let json = serde_json::to_string(&item).unwrap();
        assert!(json.contains("\"Key\""));
        assert!(json.contains("\"Flags\""));
        assert!(json.contains("\"Value\""));
    }

    #[test]
    fn test_config_import_item_from_nacos_export() {
        let export_item = NacosExportItem {
            metadata: NacosConfigMetadata {
                data_id: "app.yaml".to_string(),
                group: "DEFAULT_GROUP".to_string(),
                namespace_id: "public".to_string(),
                content_type: "yaml".to_string(),
                app_name: "my-app".to_string(),
                desc: "Test".to_string(),
                config_tags: "tag1".to_string(),
                md5: "abc".to_string(),
                encrypted_data_key: String::new(),
                create_time: 0,
                modify_time: 0,
            },
            content: "server:\n  port: 8080".to_string(),
        };

        let import_item: ConfigImportItem = export_item.into();
        assert_eq!(import_item.data_id, "app.yaml");
        assert_eq!(import_item.group, "DEFAULT_GROUP");
        assert_eq!(import_item.content, "server:\n  port: 8080");
    }
}
