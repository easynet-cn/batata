//! Apollo configuration service
//!
//! Provides configuration retrieval with Apollo-compatible responses.
//! Uses Apollo release table for immutable config snapshots.

use std::collections::HashMap;
use std::sync::Arc;

use sea_orm::DatabaseConnection;

use crate::model::{ApolloConfig, ConfigFormat};
use crate::repository::{namespace_repository, release_repository};

/// Apollo configuration service
///
/// Retrieves configuration from Apollo release snapshots.
pub struct ApolloConfigService {
    db: Arc<DatabaseConnection>,
}

impl ApolloConfigService {
    /// Create a new ApolloConfigService
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    /// Get configuration in Apollo format
    ///
    /// Returns `None` if the release key matches (304 Not Modified scenario).
    pub async fn get_config(
        &self,
        app_id: &str,
        cluster: &str,
        namespace: &str,
        _env: Option<&str>,
        client_release_key: Option<&str>,
    ) -> anyhow::Result<Option<ApolloConfig>> {
        // Try to get latest release
        let release = release_repository::find_latest(&self.db, app_id, cluster, namespace).await?;

        match release {
            Some(rel) => {
                // Check if client already has this version
                if let Some(client_key) = client_release_key
                    && client_key != "-1"
                    && client_key == rel.release_key
                {
                    return Ok(None);
                }

                let configurations: HashMap<String, String> = rel
                    .configurations
                    .as_deref()
                    .and_then(|c| serde_json::from_str(c).ok())
                    .unwrap_or_default();

                Ok(Some(ApolloConfig::new(
                    app_id.to_string(),
                    cluster.to_string(),
                    namespace.to_string(),
                    rel.release_key,
                    configurations,
                )))
            }
            None => {
                // No release found - check if namespace exists with items
                let inst =
                    namespace_repository::find_instance(&self.db, app_id, cluster, namespace)
                        .await?;

                if inst.is_some() {
                    // Namespace exists but no release yet - return empty config
                    Ok(Some(ApolloConfig::empty(
                        app_id.to_string(),
                        cluster.to_string(),
                        namespace.to_string(),
                    )))
                } else {
                    // Return empty config (Apollo convention)
                    Ok(Some(ApolloConfig::empty(
                        app_id.to_string(),
                        cluster.to_string(),
                        namespace.to_string(),
                    )))
                }
            }
        }
    }

    /// Get configuration as raw content string
    ///
    /// Used for `/configfiles/*` endpoints.
    pub async fn get_config_content(
        &self,
        app_id: &str,
        cluster: &str,
        namespace: &str,
        _env: Option<&str>,
    ) -> anyhow::Result<Option<String>> {
        let release = release_repository::find_latest(&self.db, app_id, cluster, namespace).await?;

        match release {
            Some(rel) => {
                let configurations: HashMap<String, String> = rel
                    .configurations
                    .as_deref()
                    .and_then(|c| serde_json::from_str(c).ok())
                    .unwrap_or_default();

                let format = ConfigFormat::from_namespace(namespace);
                match format {
                    ConfigFormat::Properties => {
                        let content = crate::mapping::to_properties_string(&configurations);
                        Ok(Some(content))
                    }
                    _ => {
                        // For non-properties formats, return the "content" key
                        // or serialize the whole map
                        if let Some(content) = configurations.get("content") {
                            Ok(Some(content.clone()))
                        } else {
                            Ok(Some(serde_json::to_string_pretty(&configurations)?))
                        }
                    }
                }
            }
            None => Ok(None),
        }
    }

    /// Get the current release key for a config
    pub async fn get_release_key(
        &self,
        app_id: &str,
        cluster: &str,
        namespace: &str,
        _env: Option<&str>,
    ) -> anyhow::Result<Option<String>> {
        let release = release_repository::find_latest(&self.db, app_id, cluster, namespace).await?;

        Ok(release.map(|r| r.release_key))
    }
}

/// Convert Apollo parameters to Nacos query (kept for backward compatibility)
pub fn build_nacos_query(
    app_id: &str,
    cluster: &str,
    namespace: &str,
    env: Option<&str>,
) -> (String, String, String) {
    let data_id = crate::mapping::to_nacos_data_id(app_id, namespace);
    let group = crate::mapping::to_nacos_group(cluster);
    let namespace_id = env
        .map(crate::mapping::to_nacos_namespace)
        .unwrap_or_default();
    (data_id, group, namespace_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ConfigFormat;

    #[test]
    fn test_build_nacos_query() {
        let (data_id, group, namespace) = build_nacos_query("app1", "default", "application", None);
        assert_eq!(data_id, "app1+application");
        assert_eq!(group, "default");
        assert_eq!(namespace, "");

        let (data_id2, group2, namespace2) =
            build_nacos_query("app1", "cluster1", "config", Some("PRO"));
        assert_eq!(data_id2, "app1+config");
        assert_eq!(group2, "cluster1");
        assert_eq!(namespace2, "pro");
    }

    #[test]
    fn test_build_nacos_query_dev_env() {
        // DEV maps to public namespace (empty string)
        let (_, _, namespace) = build_nacos_query("app1", "default", "ns", Some("DEV"));
        assert_eq!(namespace, "");
    }

    #[test]
    fn test_build_nacos_query_fat_env() {
        let (_, _, namespace) = build_nacos_query("app1", "default", "ns", Some("FAT"));
        assert_eq!(namespace, "fat");
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
            ConfigFormat::from_namespace("readme.txt"),
            ConfigFormat::Txt
        );
    }

    #[test]
    fn test_apollo_config_creation() {
        let mut configs = HashMap::new();
        configs.insert("key1".to_string(), "val1".to_string());

        let config = ApolloConfig::new(
            "app1".to_string(),
            "default".to_string(),
            "application".to_string(),
            "release-key-1".to_string(),
            configs,
        );

        assert_eq!(config.app_id, "app1");
        assert_eq!(config.cluster, "default");
        assert_eq!(config.namespace_name, "application");
        assert_eq!(config.release_key, "release-key-1");
        assert_eq!(config.configurations.get("key1"), Some(&"val1".to_string()));
    }

    #[test]
    fn test_apollo_config_empty() {
        let config = ApolloConfig::empty(
            "app1".to_string(),
            "default".to_string(),
            "application".to_string(),
        );

        assert_eq!(config.app_id, "app1");
        assert!(config.configurations.is_empty());
        assert!(config.release_key.is_empty());
    }
}
