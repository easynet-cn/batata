//! Apollo configuration service
//!
//! Provides configuration retrieval with Apollo-compatible responses.

use std::collections::HashMap;
use std::sync::Arc;

use sea_orm::DatabaseConnection;

use crate::mapping::{
    ApolloMappingContext, generate_release_key, parse_properties, to_nacos_data_id, to_nacos_group,
    to_nacos_namespace,
};
use crate::model::{ApolloConfig, ConfigFormat};

/// Apollo configuration service
///
/// Wraps Nacos config operations with Apollo-compatible interface.
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
        env: Option<&str>,
        client_release_key: Option<&str>,
    ) -> anyhow::Result<Option<ApolloConfig>> {
        let ctx = ApolloMappingContext::new(app_id, cluster, namespace, env);

        // Fetch from Nacos config service
        let config_info = batata_config::service::config::find_one(
            &self.db,
            &ctx.nacos_data_id,
            &ctx.nacos_group,
            &ctx.nacos_namespace,
        )
        .await?;

        match config_info {
            Some(info) => {
                let md5 = info.config_info.config_info_base.md5.clone();
                let release_key = generate_release_key(&md5);

                // Check if client already has this version
                if let Some(client_key) = client_release_key {
                    if client_key != "-1"
                        && !md5.is_empty()
                        && client_key.ends_with(&md5[..8.min(md5.len())])
                    {
                        // Client has the same version, return None for 304
                        return Ok(None);
                    }
                }

                // Parse content based on format
                let content = info.config_info.config_info_base.content.clone();
                let format = ConfigFormat::from_namespace(&ctx.namespace);
                let configurations = match format {
                    ConfigFormat::Properties => parse_properties(&content),
                    _ => {
                        // For non-properties formats, return content as single "content" key
                        let mut map = HashMap::new();
                        map.insert("content".to_string(), content);
                        map
                    }
                };

                Ok(Some(ApolloConfig::new(
                    app_id.to_string(),
                    cluster.to_string(),
                    ctx.namespace.to_string(),
                    release_key,
                    configurations,
                )))
            }
            None => {
                // Return empty config (Apollo returns empty configurations, not 404)
                Ok(Some(ApolloConfig::empty(
                    app_id.to_string(),
                    cluster.to_string(),
                    ctx.namespace.to_string(),
                )))
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
        env: Option<&str>,
    ) -> anyhow::Result<Option<String>> {
        let ctx = ApolloMappingContext::new(app_id, cluster, namespace, env);

        let config_info = batata_config::service::config::find_one(
            &self.db,
            &ctx.nacos_data_id,
            &ctx.nacos_group,
            &ctx.nacos_namespace,
        )
        .await?;

        Ok(config_info.map(|info| info.config_info.config_info_base.content))
    }

    /// Get the current release key (MD5) for a config
    pub async fn get_release_key(
        &self,
        app_id: &str,
        cluster: &str,
        namespace: &str,
        env: Option<&str>,
    ) -> anyhow::Result<Option<String>> {
        let ctx = ApolloMappingContext::new(app_id, cluster, namespace, env);

        let config_info = batata_config::service::config::find_one(
            &self.db,
            &ctx.nacos_data_id,
            &ctx.nacos_group,
            &ctx.nacos_namespace,
        )
        .await?;

        Ok(config_info.map(|info| {
            let md5 = info.config_info.config_info_base.md5;
            generate_release_key(&md5)
        }))
    }
}

/// Convert Apollo parameters to Nacos query
pub fn build_nacos_query(
    app_id: &str,
    cluster: &str,
    namespace: &str,
    env: Option<&str>,
) -> (String, String, String) {
    let data_id = to_nacos_data_id(app_id, namespace);
    let group = to_nacos_group(cluster);
    let namespace_id = env.map(|e| to_nacos_namespace(e)).unwrap_or_default();
    (data_id, group, namespace_id)
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
