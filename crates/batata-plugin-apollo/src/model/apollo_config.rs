//! Apollo configuration response models
//!
//! These models match the Apollo Config Service API response format.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Apollo configuration response
///
/// This is the main response type for the `/configs/{appId}/{cluster}/{namespace}` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApolloConfig {
    /// Application ID
    pub app_id: String,

    /// Cluster name
    pub cluster: String,

    /// Namespace name
    pub namespace_name: String,

    /// Release key for version tracking
    pub release_key: String,

    /// Configuration key-value pairs
    pub configurations: HashMap<String, String>,
}

impl ApolloConfig {
    /// Create a new ApolloConfig
    pub fn new(
        app_id: String,
        cluster: String,
        namespace_name: String,
        release_key: String,
        configurations: HashMap<String, String>,
    ) -> Self {
        Self {
            app_id,
            cluster,
            namespace_name,
            release_key,
            configurations,
        }
    }

    /// Create an empty ApolloConfig
    pub fn empty(app_id: String, cluster: String, namespace_name: String) -> Self {
        Self {
            app_id,
            cluster,
            namespace_name,
            release_key: String::new(),
            configurations: HashMap::new(),
        }
    }
}

/// Apollo configuration with additional metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApolloConfigExt {
    /// Base configuration
    #[serde(flatten)]
    pub config: ApolloConfig,

    /// Data center
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_center: Option<String>,

    /// Client IP
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
}

/// Configuration format types supported by Apollo
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum ConfigFormat {
    #[default]
    Properties,
    Xml,
    Json,
    Yaml,
    Yml,
    Txt,
}

impl ConfigFormat {
    /// Parse format from namespace name
    ///
    /// Apollo convention: namespace ending with `.json`, `.yaml`, etc. indicates format
    pub fn from_namespace(namespace: &str) -> Self {
        if let Some(ext) = namespace.rsplit('.').next() {
            match ext.to_lowercase().as_str() {
                "json" => ConfigFormat::Json,
                "yaml" => ConfigFormat::Yaml,
                "yml" => ConfigFormat::Yml,
                "xml" => ConfigFormat::Xml,
                "txt" => ConfigFormat::Txt,
                _ => ConfigFormat::Properties,
            }
        } else {
            ConfigFormat::Properties
        }
    }

    /// Get content type for HTTP response
    pub fn content_type(&self) -> &'static str {
        match self {
            ConfigFormat::Properties => "text/plain;charset=UTF-8",
            ConfigFormat::Xml => "application/xml;charset=UTF-8",
            ConfigFormat::Json => "application/json;charset=UTF-8",
            ConfigFormat::Yaml | ConfigFormat::Yml => "text/yaml;charset=UTF-8",
            ConfigFormat::Txt => "text/plain;charset=UTF-8",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apollo_config_serialization() {
        let mut configs = HashMap::new();
        configs.insert("key1".to_string(), "value1".to_string());
        configs.insert("key2".to_string(), "value2".to_string());

        let config = ApolloConfig::new(
            "app1".to_string(),
            "default".to_string(),
            "application".to_string(),
            "20241015123456-abc123".to_string(),
            configs,
        );

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("appId"));
        assert!(json.contains("cluster"));
        assert!(json.contains("namespaceName"));
        assert!(json.contains("releaseKey"));
        assert!(json.contains("configurations"));
    }

    #[test]
    fn test_apollo_config_empty() {
        let config = ApolloConfig::empty(
            "app1".to_string(),
            "default".to_string(),
            "application".to_string(),
        );
        assert_eq!(config.app_id, "app1");
        assert_eq!(config.cluster, "default");
        assert_eq!(config.namespace_name, "application");
        assert!(config.release_key.is_empty());
        assert!(config.configurations.is_empty());
    }

    #[test]
    fn test_apollo_config_deserialization() {
        let json = r#"{
            "appId": "app1",
            "cluster": "default",
            "namespaceName": "application",
            "releaseKey": "key-123",
            "configurations": {"db.url": "jdbc:mysql://localhost"}
        }"#;
        let config: ApolloConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.app_id, "app1");
        assert_eq!(config.release_key, "key-123");
        assert_eq!(
            config.configurations.get("db.url"),
            Some(&"jdbc:mysql://localhost".to_string())
        );
    }

    #[test]
    fn test_apollo_config_ext_serialization() {
        let config = ApolloConfig::new(
            "app1".to_string(),
            "default".to_string(),
            "app".to_string(),
            "key-1".to_string(),
            HashMap::new(),
        );
        let ext = ApolloConfigExt {
            config,
            data_center: Some("dc1".to_string()),
            ip: Some("10.0.0.1".to_string()),
        };
        let json = serde_json::to_string(&ext).unwrap();
        assert!(json.contains("\"dataCenter\":\"dc1\""));
        assert!(json.contains("\"ip\":\"10.0.0.1\""));
        // Flattened fields
        assert!(json.contains("\"appId\":\"app1\""));
    }

    #[test]
    fn test_apollo_config_ext_optional_fields_skipped() {
        let config = ApolloConfig::empty("a".to_string(), "b".to_string(), "c".to_string());
        let ext = ApolloConfigExt {
            config,
            data_center: None,
            ip: None,
        };
        let json = serde_json::to_string(&ext).unwrap();
        assert!(!json.contains("dataCenter"));
        assert!(!json.contains("\"ip\""));
    }

    #[test]
    fn test_config_format_from_namespace() {
        assert_eq!(
            ConfigFormat::from_namespace("application"),
            ConfigFormat::Properties
        );
        assert_eq!(
            ConfigFormat::from_namespace("config.json"),
            ConfigFormat::Json
        );
        assert_eq!(
            ConfigFormat::from_namespace("config.yaml"),
            ConfigFormat::Yaml
        );
        assert_eq!(
            ConfigFormat::from_namespace("config.yml"),
            ConfigFormat::Yml
        );
        assert_eq!(
            ConfigFormat::from_namespace("config.xml"),
            ConfigFormat::Xml
        );
        assert_eq!(
            ConfigFormat::from_namespace("config.txt"),
            ConfigFormat::Txt
        );
        assert_eq!(
            ConfigFormat::from_namespace("application.properties"),
            ConfigFormat::Properties
        );
    }

    #[test]
    fn test_config_format_case_insensitive() {
        assert_eq!(
            ConfigFormat::from_namespace("config.JSON"),
            ConfigFormat::Json
        );
        assert_eq!(
            ConfigFormat::from_namespace("config.YAML"),
            ConfigFormat::Yaml
        );
    }

    #[test]
    fn test_config_format_content_type() {
        assert_eq!(
            ConfigFormat::Properties.content_type(),
            "text/plain;charset=UTF-8"
        );
        assert_eq!(
            ConfigFormat::Json.content_type(),
            "application/json;charset=UTF-8"
        );
        assert_eq!(
            ConfigFormat::Xml.content_type(),
            "application/xml;charset=UTF-8"
        );
        assert_eq!(ConfigFormat::Yaml.content_type(), "text/yaml;charset=UTF-8");
        assert_eq!(ConfigFormat::Yml.content_type(), "text/yaml;charset=UTF-8");
        assert_eq!(ConfigFormat::Txt.content_type(), "text/plain;charset=UTF-8");
    }

    #[test]
    fn test_config_format_default() {
        assert_eq!(ConfigFormat::default(), ConfigFormat::Properties);
    }

    #[test]
    fn test_config_format_serialization() {
        assert_eq!(
            serde_json::to_string(&ConfigFormat::Properties).unwrap(),
            "\"properties\""
        );
        assert_eq!(
            serde_json::to_string(&ConfigFormat::Json).unwrap(),
            "\"json\""
        );
        assert_eq!(
            serde_json::to_string(&ConfigFormat::Yaml).unwrap(),
            "\"yaml\""
        );
    }
}
