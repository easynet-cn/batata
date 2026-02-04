//! Apollo Open API service
//!
//! Provides management operations for Apollo Open API compatibility.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use sea_orm::DatabaseConnection;

use crate::mapping::{ApolloMappingContext, parse_properties, to_properties_string};
use crate::model::{
    ApolloApp, ApolloEnvCluster, ApolloItem, ApolloNamespace, ApolloRelease, CreateItemRequest,
    PublishReleaseRequest, UpdateItemRequest,
};

/// Apollo Open API service
///
/// Provides management operations with Apollo-compatible interface.
pub struct ApolloOpenApiService {
    db: Arc<DatabaseConnection>,
}

impl ApolloOpenApiService {
    /// Create a new ApolloOpenApiService
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    /// Get all apps
    ///
    /// In Apollo, apps map to unique appIds in Nacos dataIds.
    /// We extract appId from dataId pattern: {appId}+{namespace}
    pub async fn get_apps(&self) -> anyhow::Result<Vec<ApolloApp>> {
        // Use search_page with empty filters to get all configs
        let page = batata_config::service::config::search_page(
            &self.db,
            1,
            10000, // Large page size to get all
            "",    // All namespaces
            "",    // All data_ids
            "",    // All groups
            "",    // All app_names
            vec![],
            vec![],
            "",
        )
        .await?;

        let mut apps: HashMap<String, ApolloApp> = HashMap::new();
        for config in page.page_items {
            let data_id = &config.data_id;
            // Extract appId from dataId pattern: {appId}+{namespace}
            if let Some(app_id) = data_id.split('+').next() {
                if !app_id.is_empty() && !apps.contains_key(app_id) {
                    apps.insert(
                        app_id.to_string(),
                        ApolloApp::new(app_id.to_string(), app_id.to_string()),
                    );
                }
            }
        }

        Ok(apps.into_values().collect())
    }

    /// Get environment clusters for an app
    ///
    /// Returns all environments (Nacos namespaces) and clusters (Nacos groups) for an app.
    pub async fn get_env_clusters(&self, app_id: &str) -> anyhow::Result<Vec<ApolloEnvCluster>> {
        // Get all namespaces
        let namespaces = batata_config::service::namespace::find_all(&self.db).await;

        let mut env_clusters: Vec<ApolloEnvCluster> = Vec::new();

        for ns in namespaces {
            let namespace_id = ns.namespace.clone();
            let env_name = if namespace_id.is_empty() {
                "default".to_string()
            } else {
                namespace_id.clone()
            };

            // Search for configs matching this app in this namespace
            let page = batata_config::service::config::search_page(
                &self.db,
                1,
                10000,
                &namespace_id,
                &format!("{}+*", app_id), // Pattern: appId+*
                "",
                "",
                vec![],
                vec![],
                "",
            )
            .await?;

            // Extract unique groups (clusters)
            let groups: HashSet<String> = page
                .page_items
                .iter()
                .map(|c| c.group_name.clone())
                .filter(|g: &String| !g.is_empty())
                .collect();

            if !groups.is_empty() {
                env_clusters.push(ApolloEnvCluster::new(
                    env_name,
                    groups.into_iter().collect(),
                ));
            }
        }

        // If no env_clusters found, return default
        if env_clusters.is_empty() {
            env_clusters.push(ApolloEnvCluster::new(
                "default".to_string(),
                vec!["default".to_string()],
            ));
        }

        Ok(env_clusters)
    }

    /// Get namespaces for an app in a specific environment and cluster
    pub async fn get_namespaces(
        &self,
        app_id: &str,
        cluster: &str,
        env: &str,
    ) -> anyhow::Result<Vec<ApolloNamespace>> {
        let namespace_id = if env == "default" || env.is_empty() {
            String::new()
        } else {
            env.to_lowercase()
        };

        // Search for configs matching this app and cluster in this namespace
        let page = batata_config::service::config::search_page(
            &self.db,
            1,
            10000,
            &namespace_id,
            &format!("{}+*", app_id),
            cluster,
            "",
            vec![],
            vec![],
            "",
        )
        .await?;

        let mut namespaces: Vec<ApolloNamespace> = Vec::new();

        for config in page.page_items {
            // Extract Apollo namespace from dataId
            if let Some((_app, apollo_namespace)) =
                crate::mapping::from_nacos_data_id(&config.data_id)
            {
                let mut ns =
                    ApolloNamespace::new(app_id.to_string(), cluster.to_string(), apollo_namespace);

                // Get full config to parse items if format is properties
                if ns.format == "properties" {
                    if let Ok(Some(full_config)) = batata_config::service::config::find_one(
                        &self.db,
                        &config.data_id,
                        &config.group_name,
                        &namespace_id,
                    )
                    .await
                    {
                        let content = &full_config.config_info.config_info_base.content;
                        let props = parse_properties(content);
                        let items: Vec<ApolloItem> = props
                            .into_iter()
                            .map(|(k, v)| ApolloItem::new(k, v))
                            .collect();
                        ns = ns.with_items(items);
                    }
                }

                namespaces.push(ns);
            }
        }

        Ok(namespaces)
    }

    /// Get a specific namespace
    pub async fn get_namespace(
        &self,
        app_id: &str,
        cluster: &str,
        namespace: &str,
        env: &str,
    ) -> anyhow::Result<Option<ApolloNamespace>> {
        let ctx = ApolloMappingContext::new(app_id, cluster, namespace, Some(env));

        let config = batata_config::service::config::find_one(
            &self.db,
            &ctx.nacos_data_id,
            &ctx.nacos_group,
            &ctx.nacos_namespace,
        )
        .await?;

        match config {
            Some(info) => {
                let mut ns = ApolloNamespace::new(
                    app_id.to_string(),
                    cluster.to_string(),
                    namespace.to_string(),
                );

                // Parse content as items if format is properties
                if ns.format == "properties" {
                    let content = &info.config_info.config_info_base.content;
                    let props = parse_properties(content);
                    let items: Vec<ApolloItem> = props
                        .into_iter()
                        .map(|(k, v)| ApolloItem::new(k, v))
                        .collect();
                    ns = ns.with_items(items);
                }

                Ok(Some(ns))
            }
            None => Ok(None),
        }
    }

    /// Get a specific item from a namespace
    pub async fn get_item(
        &self,
        app_id: &str,
        cluster: &str,
        namespace: &str,
        env: &str,
        key: &str,
    ) -> anyhow::Result<Option<ApolloItem>> {
        let ctx = ApolloMappingContext::new(app_id, cluster, namespace, Some(env));

        let config = batata_config::service::config::find_one(
            &self.db,
            &ctx.nacos_data_id,
            &ctx.nacos_group,
            &ctx.nacos_namespace,
        )
        .await?;

        match config {
            Some(info) => {
                let content = &info.config_info.config_info_base.content;
                let props = parse_properties(content);

                Ok(props
                    .get(key)
                    .map(|v| ApolloItem::new(key.to_string(), v.clone())))
            }
            None => Ok(None),
        }
    }

    /// Get all items in a namespace
    pub async fn get_items(
        &self,
        app_id: &str,
        cluster: &str,
        namespace: &str,
        env: &str,
    ) -> anyhow::Result<Vec<ApolloItem>> {
        let ctx = ApolloMappingContext::new(app_id, cluster, namespace, Some(env));

        let config = batata_config::service::config::find_one(
            &self.db,
            &ctx.nacos_data_id,
            &ctx.nacos_group,
            &ctx.nacos_namespace,
        )
        .await?;

        match config {
            Some(info) => {
                let content = &info.config_info.config_info_base.content;
                let props = parse_properties(content);
                let items: Vec<ApolloItem> = props
                    .into_iter()
                    .map(|(k, v)| ApolloItem::new(k, v))
                    .collect();
                Ok(items)
            }
            None => Ok(Vec::new()),
        }
    }

    /// Create a new item
    pub async fn create_item(
        &self,
        app_id: &str,
        cluster: &str,
        namespace: &str,
        env: &str,
        req: CreateItemRequest,
    ) -> anyhow::Result<ApolloItem> {
        let ctx = ApolloMappingContext::new(app_id, cluster, namespace, Some(env));

        // Get existing config or start with empty
        let config = batata_config::service::config::find_one(
            &self.db,
            &ctx.nacos_data_id,
            &ctx.nacos_group,
            &ctx.nacos_namespace,
        )
        .await?;

        let mut props = match config {
            Some(info) => parse_properties(&info.config_info.config_info_base.content),
            None => HashMap::new(),
        };

        // Add new item
        props.insert(req.key.clone(), req.value.clone());

        // Convert back to properties string
        let new_content = to_properties_string(&props);

        // Publish the updated config
        batata_config::service::config::create_or_update(
            &self.db,
            &ctx.nacos_data_id,
            &ctx.nacos_group,
            &ctx.nacos_namespace,
            &new_content,
            app_id,
            &req.data_change_created_by,
            "", // src_ip
            "", // config_tags
            "", // desc
            "", // use
            "", // effect
            "", // type
            "", // schema
            "", // encrypted_data_key
        )
        .await?;

        let mut item = ApolloItem::new(req.key, req.value);
        if let Some(comment) = req.comment {
            item = item.with_comment(comment);
        }
        item.data_change_created_by = Some(req.data_change_created_by);

        Ok(item)
    }

    /// Update an existing item
    pub async fn update_item(
        &self,
        app_id: &str,
        cluster: &str,
        namespace: &str,
        env: &str,
        key: &str,
        req: UpdateItemRequest,
    ) -> anyhow::Result<Option<ApolloItem>> {
        let ctx = ApolloMappingContext::new(app_id, cluster, namespace, Some(env));

        let config = batata_config::service::config::find_one(
            &self.db,
            &ctx.nacos_data_id,
            &ctx.nacos_group,
            &ctx.nacos_namespace,
        )
        .await?;

        let mut props = match config {
            Some(info) => parse_properties(&info.config_info.config_info_base.content),
            None => {
                if req.create_if_not_exists {
                    HashMap::new()
                } else {
                    return Ok(None);
                }
            }
        };

        // Check if key exists (unless create_if_not_exists is true)
        if !req.create_if_not_exists && !props.contains_key(key) {
            return Ok(None);
        }

        // Update item
        props.insert(key.to_string(), req.value.clone());

        // Convert back to properties string
        let new_content = to_properties_string(&props);

        // Publish the updated config
        batata_config::service::config::create_or_update(
            &self.db,
            &ctx.nacos_data_id,
            &ctx.nacos_group,
            &ctx.nacos_namespace,
            &new_content,
            app_id,
            &req.data_change_last_modified_by,
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
        )
        .await?;

        let mut item = ApolloItem::new(key.to_string(), req.value);
        if let Some(comment) = req.comment {
            item = item.with_comment(comment);
        }
        item.data_change_last_modified_by = Some(req.data_change_last_modified_by);

        Ok(Some(item))
    }

    /// Delete an item
    pub async fn delete_item(
        &self,
        app_id: &str,
        cluster: &str,
        namespace: &str,
        env: &str,
        key: &str,
        operator: &str,
    ) -> anyhow::Result<bool> {
        let ctx = ApolloMappingContext::new(app_id, cluster, namespace, Some(env));

        let config = batata_config::service::config::find_one(
            &self.db,
            &ctx.nacos_data_id,
            &ctx.nacos_group,
            &ctx.nacos_namespace,
        )
        .await?;

        match config {
            Some(info) => {
                let mut props = parse_properties(&info.config_info.config_info_base.content);

                if props.remove(key).is_none() {
                    return Ok(false);
                }

                // Convert back to properties string
                let new_content = to_properties_string(&props);

                // Publish the updated config
                batata_config::service::config::create_or_update(
                    &self.db,
                    &ctx.nacos_data_id,
                    &ctx.nacos_group,
                    &ctx.nacos_namespace,
                    &new_content,
                    app_id,
                    operator,
                    "",
                    "",
                    "",
                    "",
                    "",
                    "",
                    "",
                    "",
                )
                .await?;

                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Publish a release
    pub async fn publish_release(
        &self,
        app_id: &str,
        cluster: &str,
        namespace: &str,
        env: &str,
        req: PublishReleaseRequest,
    ) -> anyhow::Result<Option<ApolloRelease>> {
        let ctx = ApolloMappingContext::new(app_id, cluster, namespace, Some(env));

        let config = batata_config::service::config::find_one(
            &self.db,
            &ctx.nacos_data_id,
            &ctx.nacos_group,
            &ctx.nacos_namespace,
        )
        .await?;

        match config {
            Some(info) => {
                let content = &info.config_info.config_info_base.content;

                // For Apollo, publishing just returns the current config as a release
                // The actual publish was done when items were created/updated
                let configurations = parse_properties(content);

                let mut release = ApolloRelease::new(
                    app_id.to_string(),
                    cluster.to_string(),
                    namespace.to_string(),
                    req.release_title,
                    configurations,
                );
                release.comment = req.release_comment;
                release.data_change_created_by = Some(req.released_by);

                Ok(Some(release))
            }
            None => Ok(None),
        }
    }

    /// Get the latest release
    pub async fn get_latest_release(
        &self,
        app_id: &str,
        cluster: &str,
        namespace: &str,
        env: &str,
    ) -> anyhow::Result<Option<ApolloRelease>> {
        let ctx = ApolloMappingContext::new(app_id, cluster, namespace, Some(env));

        let config = batata_config::service::config::find_one(
            &self.db,
            &ctx.nacos_data_id,
            &ctx.nacos_group,
            &ctx.nacos_namespace,
        )
        .await?;

        match config {
            Some(info) => {
                let content = &info.config_info.config_info_base.content;
                let md5 = &info.config_info.config_info_base.md5;
                let configurations = parse_properties(content);

                let release = ApolloRelease::new(
                    app_id.to_string(),
                    cluster.to_string(),
                    namespace.to_string(),
                    format!("release-{}", &md5[..8.min(md5.len())]),
                    configurations,
                );

                Ok(Some(release))
            }
            None => Ok(None),
        }
    }
}
