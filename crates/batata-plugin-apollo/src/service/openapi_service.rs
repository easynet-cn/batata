//! Apollo Open API service
//!
//! Provides management operations for Apollo Open API compatibility.
//! Uses Apollo-native database tables for all operations.

use std::collections::HashMap;
use std::sync::Arc;

use batata_persistence::entity::{
    apollo_app, apollo_app_namespace, apollo_cluster, apollo_commit, apollo_item, apollo_namespace,
    apollo_release, apollo_release_history, apollo_release_message,
};
use sea_orm::{DatabaseConnection, Set};

use crate::model::{
    ApolloApp, ApolloCluster, ApolloEnvCluster, ApolloItem, ApolloNamespace, ApolloRelease,
    CreateAppRequest, CreateItemRequest, CreateNamespaceRequest, PublishReleaseRequest,
    UpdateItemRequest,
};
use crate::repository::{
    app_repository, cluster_repository, commit_repository, item_repository, namespace_repository,
    release_repository,
};

/// Release history operation types
const OP_NORMAL_RELEASE: i16 = 0;
const OP_ROLLBACK: i16 = 1;

/// Apollo Open API service
///
/// Provides management operations using Apollo-native database tables.
pub struct ApolloOpenApiService {
    db: Arc<DatabaseConnection>,
}

impl ApolloOpenApiService {
    /// Create a new ApolloOpenApiService
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    // ========================================================================
    // App Management
    // ========================================================================

    /// Get all apps
    pub async fn get_apps(&self) -> anyhow::Result<Vec<ApolloApp>> {
        let models = app_repository::find_all(&self.db).await?;
        Ok(models.into_iter().map(Self::model_to_app).collect())
    }

    /// Get a specific app
    pub async fn get_app(&self, app_id: &str) -> anyhow::Result<Option<ApolloApp>> {
        let model = app_repository::find_by_app_id(&self.db, app_id).await?;
        Ok(model.map(Self::model_to_app))
    }

    /// Create a new app
    pub async fn create_app(&self, req: CreateAppRequest) -> anyhow::Result<ApolloApp> {
        let model = apollo_app::ActiveModel {
            app_id: Set(req.app_id.clone()),
            name: Set(req.name.clone()),
            org_id: Set(req.org_id.clone()),
            org_name: Set(req.org_name.clone()),
            owner_name: Set(Some(req.owner_name.clone())),
            owner_email: Set(req.owner_email.clone()),
            created_by: Set(Some(req.owner_name.clone())),
            last_modified_by: Set(Some(req.owner_name.clone())),
            ..Default::default()
        };
        let result = app_repository::create(&self.db, model).await?;

        // Create default cluster
        let cluster_model = apollo_cluster::ActiveModel {
            name: Set("default".to_string()),
            app_id: Set(req.app_id.clone()),
            parent_cluster_id: Set(0),
            created_by: Set(Some(req.owner_name.clone())),
            last_modified_by: Set(Some(req.owner_name.clone())),
            ..Default::default()
        };
        cluster_repository::create(&self.db, cluster_model).await?;

        // Create default namespace (application)
        let ns_def_model = apollo_app_namespace::ActiveModel {
            name: Set("application".to_string()),
            app_id: Set(req.app_id.clone()),
            format: Set(Some("properties".to_string())),
            is_public: Set(false),
            created_by: Set(Some(req.owner_name.clone())),
            last_modified_by: Set(Some(req.owner_name.clone())),
            ..Default::default()
        };
        namespace_repository::create_definition(&self.db, ns_def_model).await?;

        // Create namespace instance
        let ns_inst_model = apollo_namespace::ActiveModel {
            app_id: Set(req.app_id.clone()),
            cluster_name: Set("default".to_string()),
            namespace_name: Set("application".to_string()),
            created_by: Set(Some(req.owner_name)),
            ..Default::default()
        };
        namespace_repository::create_instance(&self.db, ns_inst_model).await?;

        Ok(Self::model_to_app(result))
    }

    /// Update an app
    pub async fn update_app(
        &self,
        app_id: &str,
        req: CreateAppRequest,
    ) -> anyhow::Result<Option<ApolloApp>> {
        let existing = app_repository::find_by_app_id(&self.db, app_id).await?;
        match existing {
            Some(model) => {
                let mut active: apollo_app::ActiveModel = model.into();
                active.name = Set(req.name);
                active.org_id = Set(req.org_id);
                active.org_name = Set(req.org_name);
                active.owner_name = Set(Some(req.owner_name.clone()));
                active.owner_email = Set(req.owner_email);
                active.last_modified_by = Set(Some(req.owner_name));
                let result = app_repository::update(&self.db, active).await?;
                Ok(Some(Self::model_to_app(result)))
            }
            None => Ok(None),
        }
    }

    /// Delete an app
    pub async fn delete_app(&self, app_id: &str) -> anyhow::Result<bool> {
        app_repository::soft_delete(&self.db, app_id).await
    }

    // ========================================================================
    // Cluster Management
    // ========================================================================

    /// Get a specific cluster
    pub async fn get_cluster(
        &self,
        app_id: &str,
        cluster_name: &str,
    ) -> anyhow::Result<Option<ApolloCluster>> {
        let model =
            cluster_repository::find_by_app_and_name(&self.db, app_id, cluster_name).await?;
        Ok(model.map(Self::model_to_cluster))
    }

    /// Create a cluster
    pub async fn create_cluster(
        &self,
        app_id: &str,
        cluster_name: &str,
        operator: &str,
    ) -> anyhow::Result<ApolloCluster> {
        let model = apollo_cluster::ActiveModel {
            name: Set(cluster_name.to_string()),
            app_id: Set(app_id.to_string()),
            parent_cluster_id: Set(0),
            created_by: Set(Some(operator.to_string())),
            last_modified_by: Set(Some(operator.to_string())),
            ..Default::default()
        };
        let result = cluster_repository::create(&self.db, model).await?;
        Ok(Self::model_to_cluster(result))
    }

    /// Delete a cluster
    pub async fn delete_cluster(&self, app_id: &str, cluster_name: &str) -> anyhow::Result<bool> {
        cluster_repository::soft_delete(&self.db, app_id, cluster_name).await
    }

    /// Get environment clusters for an app
    pub async fn get_env_clusters(&self, app_id: &str) -> anyhow::Result<Vec<ApolloEnvCluster>> {
        let clusters = cluster_repository::find_by_app(&self.db, app_id).await?;

        // Group clusters by parent (env concept)
        // For now, treat all clusters as being in "default" env
        let cluster_names: Vec<String> = clusters
            .into_iter()
            .filter(|c| c.parent_cluster_id == 0)
            .map(|c| c.name)
            .collect();

        if cluster_names.is_empty() {
            Ok(vec![ApolloEnvCluster::new(
                "default".to_string(),
                vec!["default".to_string()],
            )])
        } else {
            Ok(vec![ApolloEnvCluster::new(
                "default".to_string(),
                cluster_names,
            )])
        }
    }

    // ========================================================================
    // Namespace Management
    // ========================================================================

    /// Create a namespace definition
    pub async fn create_namespace_definition(
        &self,
        req: CreateNamespaceRequest,
    ) -> anyhow::Result<ApolloNamespace> {
        let ns_def_model = apollo_app_namespace::ActiveModel {
            name: Set(req.name.clone()),
            app_id: Set(req.app_id.clone()),
            format: Set(Some(req.format.clone())),
            is_public: Set(req.is_public),
            comment: Set(req.comment.clone()),
            created_by: Set(Some(req.data_change_created_by.clone())),
            last_modified_by: Set(Some(req.data_change_created_by.clone())),
            ..Default::default()
        };
        namespace_repository::create_definition(&self.db, ns_def_model).await?;

        // Create namespace instances for all clusters of the app
        let clusters = cluster_repository::find_by_app(&self.db, &req.app_id).await?;
        for cluster in clusters {
            if cluster.parent_cluster_id == 0 {
                let ns_inst_model = apollo_namespace::ActiveModel {
                    app_id: Set(req.app_id.clone()),
                    cluster_name: Set(cluster.name.clone()),
                    namespace_name: Set(req.name.clone()),
                    created_by: Set(Some(req.data_change_created_by.clone())),
                    ..Default::default()
                };
                let _ = namespace_repository::create_instance(&self.db, ns_inst_model).await;
            }
        }

        Ok(ApolloNamespace::new(
            req.app_id,
            "default".to_string(),
            req.name,
        ))
    }

    /// Get namespaces for an app in a specific cluster
    pub async fn get_namespaces(
        &self,
        app_id: &str,
        cluster: &str,
        _env: &str,
    ) -> anyhow::Result<Vec<ApolloNamespace>> {
        let instances =
            namespace_repository::find_instances_by_cluster(&self.db, app_id, cluster).await?;

        let mut result = Vec::new();
        for inst in instances {
            let ns_id = inst.id;
            let mut ns = ApolloNamespace::new(
                app_id.to_string(),
                cluster.to_string(),
                inst.namespace_name.clone(),
            );

            // Load items for this namespace
            let items = item_repository::find_all_by_namespace(&self.db, ns_id).await?;
            if !items.is_empty() {
                let apollo_items: Vec<ApolloItem> =
                    items.into_iter().map(Self::model_to_item).collect();
                ns = ns.with_items(apollo_items);
            }

            // Load format from definition if available
            if let Ok(Some(def)) =
                namespace_repository::find_definition(&self.db, app_id, &inst.namespace_name).await
            {
                ns.format = def.format.unwrap_or_else(|| "properties".to_string());
                ns.is_public = def.is_public;
                ns.comment = def.comment;
            }

            result.push(ns);
        }

        Ok(result)
    }

    /// Get a specific namespace
    pub async fn get_namespace(
        &self,
        app_id: &str,
        cluster: &str,
        namespace: &str,
        _env: &str,
    ) -> anyhow::Result<Option<ApolloNamespace>> {
        let inst =
            namespace_repository::find_instance(&self.db, app_id, cluster, namespace).await?;

        match inst {
            Some(inst) => {
                let ns_id = inst.id;
                let mut ns = ApolloNamespace::new(
                    app_id.to_string(),
                    cluster.to_string(),
                    namespace.to_string(),
                );

                // Load items
                let items = item_repository::find_all_by_namespace(&self.db, ns_id).await?;
                if !items.is_empty() {
                    let apollo_items: Vec<ApolloItem> =
                        items.into_iter().map(Self::model_to_item).collect();
                    ns = ns.with_items(apollo_items);
                }

                // Load format from definition
                if let Ok(Some(def)) =
                    namespace_repository::find_definition(&self.db, app_id, namespace).await
                {
                    ns.format = def.format.unwrap_or_else(|| "properties".to_string());
                    ns.is_public = def.is_public;
                    ns.comment = def.comment;
                }

                Ok(Some(ns))
            }
            None => Ok(None),
        }
    }

    // ========================================================================
    // Item Management
    // ========================================================================

    /// Get all items in a namespace
    pub async fn get_items(
        &self,
        app_id: &str,
        cluster: &str,
        namespace: &str,
        _env: &str,
    ) -> anyhow::Result<Vec<ApolloItem>> {
        let inst =
            namespace_repository::find_instance(&self.db, app_id, cluster, namespace).await?;

        match inst {
            Some(inst) => {
                let items = item_repository::find_all_by_namespace(&self.db, inst.id).await?;
                Ok(items.into_iter().map(Self::model_to_item).collect())
            }
            None => Ok(Vec::new()),
        }
    }

    /// Get a specific item
    pub async fn get_item(
        &self,
        app_id: &str,
        cluster: &str,
        namespace: &str,
        _env: &str,
        key: &str,
    ) -> anyhow::Result<Option<ApolloItem>> {
        let inst =
            namespace_repository::find_instance(&self.db, app_id, cluster, namespace).await?;

        match inst {
            Some(inst) => {
                let item = item_repository::find_by_key(&self.db, inst.id, key).await?;
                Ok(item.map(Self::model_to_item))
            }
            None => Ok(None),
        }
    }

    /// Create a new item
    pub async fn create_item(
        &self,
        app_id: &str,
        cluster: &str,
        namespace: &str,
        _env: &str,
        req: CreateItemRequest,
    ) -> anyhow::Result<ApolloItem> {
        let inst =
            namespace_repository::find_instance(&self.db, app_id, cluster, namespace).await?;

        let ns = inst.ok_or_else(|| anyhow::anyhow!("Namespace not found"))?;

        // Get next line number
        let max_line = item_repository::find_max_line_num(&self.db, ns.id).await?;

        let model = apollo_item::ActiveModel {
            namespace_id: Set(ns.id),
            key: Set(req.key.clone()),
            value: Set(Some(req.value.clone())),
            comment: Set(req.comment.clone()),
            line_num: Set(Some(max_line + 1)),
            created_by: Set(Some(req.data_change_created_by.clone())),
            last_modified_by: Set(Some(req.data_change_created_by.clone())),
            ..Default::default()
        };
        let result = item_repository::create(&self.db, model).await?;

        // Create commit record
        let change_set = serde_json::json!({
            "createItems": [{"key": req.key, "value": req.value}]
        });
        let commit_model = apollo_commit::ActiveModel {
            change_sets: Set(Some(change_set.to_string())),
            app_id: Set(app_id.to_string()),
            cluster_name: Set(cluster.to_string()),
            namespace_name: Set(namespace.to_string()),
            comment: Set(req.comment),
            created_by: Set(Some(req.data_change_created_by)),
            ..Default::default()
        };
        let _ = commit_repository::create(&self.db, commit_model).await;

        Ok(Self::model_to_item(result))
    }

    /// Update an existing item
    pub async fn update_item(
        &self,
        app_id: &str,
        cluster: &str,
        namespace: &str,
        _env: &str,
        key: &str,
        req: UpdateItemRequest,
    ) -> anyhow::Result<Option<ApolloItem>> {
        let inst =
            namespace_repository::find_instance(&self.db, app_id, cluster, namespace).await?;

        let ns = match inst {
            Some(ns) => ns,
            None => {
                if req.create_if_not_exists {
                    return Ok(None);
                }
                return Ok(None);
            }
        };

        let existing = item_repository::find_by_key(&self.db, ns.id, key).await?;
        match existing {
            Some(item) => {
                let old_value = item.value.clone().unwrap_or_default();
                let mut active: apollo_item::ActiveModel = item.into();
                active.value = Set(Some(req.value.clone()));
                active.comment = Set(req.comment.clone());
                active.last_modified_by = Set(Some(req.data_change_last_modified_by.clone()));
                let result = item_repository::update(&self.db, active).await?;

                // Create commit record
                let change_set = serde_json::json!({
                    "updateItems": [{"key": key, "oldValue": old_value, "newValue": req.value}]
                });
                let commit_model = apollo_commit::ActiveModel {
                    change_sets: Set(Some(change_set.to_string())),
                    app_id: Set(app_id.to_string()),
                    cluster_name: Set(cluster.to_string()),
                    namespace_name: Set(namespace.to_string()),
                    comment: Set(req.comment),
                    created_by: Set(Some(req.data_change_last_modified_by)),
                    ..Default::default()
                };
                let _ = commit_repository::create(&self.db, commit_model).await;

                Ok(Some(Self::model_to_item(result)))
            }
            None => {
                if req.create_if_not_exists {
                    let max_line = item_repository::find_max_line_num(&self.db, ns.id).await?;
                    let model = apollo_item::ActiveModel {
                        namespace_id: Set(ns.id),
                        key: Set(key.to_string()),
                        value: Set(Some(req.value.clone())),
                        comment: Set(req.comment.clone()),
                        line_num: Set(Some(max_line + 1)),
                        created_by: Set(Some(req.data_change_last_modified_by.clone())),
                        last_modified_by: Set(Some(req.data_change_last_modified_by)),
                        ..Default::default()
                    };
                    let result = item_repository::create(&self.db, model).await?;
                    Ok(Some(Self::model_to_item(result)))
                } else {
                    Ok(None)
                }
            }
        }
    }

    /// Delete an item
    pub async fn delete_item(
        &self,
        app_id: &str,
        cluster: &str,
        namespace: &str,
        _env: &str,
        key: &str,
        operator: &str,
    ) -> anyhow::Result<bool> {
        let inst =
            namespace_repository::find_instance(&self.db, app_id, cluster, namespace).await?;

        match inst {
            Some(ns) => {
                let deleted = item_repository::soft_delete_by_key(&self.db, ns.id, key).await?;

                if deleted {
                    // Create commit record
                    let change_set = serde_json::json!({
                        "deleteItems": [{"key": key}]
                    });
                    let commit_model = apollo_commit::ActiveModel {
                        change_sets: Set(Some(change_set.to_string())),
                        app_id: Set(app_id.to_string()),
                        cluster_name: Set(cluster.to_string()),
                        namespace_name: Set(namespace.to_string()),
                        created_by: Set(Some(operator.to_string())),
                        ..Default::default()
                    };
                    let _ = commit_repository::create(&self.db, commit_model).await;
                }

                Ok(deleted)
            }
            None => Ok(false),
        }
    }

    // ========================================================================
    // Release Management
    // ========================================================================

    /// Publish a release
    pub async fn publish_release(
        &self,
        app_id: &str,
        cluster: &str,
        namespace: &str,
        _env: &str,
        req: PublishReleaseRequest,
    ) -> anyhow::Result<Option<ApolloRelease>> {
        let inst =
            namespace_repository::find_instance(&self.db, app_id, cluster, namespace).await?;

        let ns = match inst {
            Some(ns) => ns,
            None => return Ok(None),
        };

        // Collect all items into configurations map
        let items = item_repository::find_all_by_namespace(&self.db, ns.id).await?;
        let mut configurations: HashMap<String, String> = HashMap::new();
        for item in &items {
            configurations.insert(item.key.clone(), item.value.clone().unwrap_or_default());
        }

        let configurations_json = serde_json::to_string(&configurations)?;
        let release_key = format!(
            "{}-{}",
            chrono::Utc::now().timestamp_millis(),
            &uuid::Uuid::new_v4().to_string()[..8]
        );

        // Get previous release for history
        let prev_release =
            release_repository::find_latest(&self.db, app_id, cluster, namespace).await?;
        let prev_release_id = prev_release.as_ref().map(|r| r.id).unwrap_or(0);

        // Create release
        let release_model = apollo_release::ActiveModel {
            release_key: Set(release_key.clone()),
            name: Set(Some(req.release_title.clone())),
            app_id: Set(app_id.to_string()),
            cluster_name: Set(cluster.to_string()),
            namespace_name: Set(namespace.to_string()),
            configurations: Set(Some(configurations_json)),
            comment: Set(req.release_comment.clone()),
            is_abandoned: Set(false),
            created_by: Set(Some(req.released_by.clone())),
            last_modified_by: Set(Some(req.released_by.clone())),
            ..Default::default()
        };
        let release = release_repository::create(&self.db, release_model).await?;

        // Create release history
        let history_model = apollo_release_history::ActiveModel {
            app_id: Set(app_id.to_string()),
            cluster_name: Set(cluster.to_string()),
            namespace_name: Set(namespace.to_string()),
            branch_name: Set(Some("default".to_string())),
            release_id: Set(release.id),
            previous_release_id: Set(prev_release_id),
            operation: Set(OP_NORMAL_RELEASE),
            operation_context: Set(req.release_comment.clone()),
            created_by: Set(Some(req.released_by.clone())),
            ..Default::default()
        };
        let _ = release_repository::create_history(&self.db, history_model).await;

        // Send release message
        let message = format!("{}+{}+{}", app_id, cluster, namespace);
        let msg_model = apollo_release_message::ActiveModel {
            message: Set(message),
            created_by: Set(Some(req.released_by.clone())),
            ..Default::default()
        };
        let _ = release_repository::send_message(&self.db, msg_model).await;

        let mut result = ApolloRelease::new(
            app_id.to_string(),
            cluster.to_string(),
            namespace.to_string(),
            req.release_title,
            configurations,
        );
        result.comment = req.release_comment;
        result.data_change_created_by = Some(req.released_by);
        result.is_abandoned = false;

        Ok(Some(result))
    }

    /// Get the latest release
    pub async fn get_latest_release(
        &self,
        app_id: &str,
        cluster: &str,
        namespace: &str,
        _env: &str,
    ) -> anyhow::Result<Option<ApolloRelease>> {
        let release = release_repository::find_latest(&self.db, app_id, cluster, namespace).await?;

        match release {
            Some(r) => {
                let configurations: HashMap<String, String> = r
                    .configurations
                    .as_deref()
                    .and_then(|c| serde_json::from_str(c).ok())
                    .unwrap_or_default();

                let mut result = ApolloRelease::new(
                    r.app_id,
                    r.cluster_name,
                    r.namespace_name,
                    r.name.unwrap_or_default(),
                    configurations,
                );
                result.comment = r.comment;
                result.is_abandoned = r.is_abandoned;
                result.data_change_created_by = r.created_by;

                Ok(Some(result))
            }
            None => Ok(None),
        }
    }

    /// Rollback to a previous release
    pub async fn rollback_release(
        &self,
        release_id: i64,
        operator: &str,
    ) -> anyhow::Result<Option<ApolloRelease>> {
        let target_release = release_repository::find_by_id(&self.db, release_id).await?;

        let target = match target_release {
            Some(r) => r,
            None => return Ok(None),
        };

        let configurations: HashMap<String, String> = target
            .configurations
            .as_deref()
            .and_then(|c| serde_json::from_str(c).ok())
            .unwrap_or_default();

        let configurations_json = serde_json::to_string(&configurations)?;

        // Get current latest release
        let prev_release = release_repository::find_latest(
            &self.db,
            &target.app_id,
            &target.cluster_name,
            &target.namespace_name,
        )
        .await?;
        let prev_release_id = prev_release.as_ref().map(|r| r.id).unwrap_or(0);

        let release_key = format!(
            "{}-{}",
            chrono::Utc::now().timestamp_millis(),
            &uuid::Uuid::new_v4().to_string()[..8]
        );

        // Create new release with old configurations
        let release_model = apollo_release::ActiveModel {
            release_key: Set(release_key),
            name: Set(Some(format!("rollback-{}", release_id))),
            app_id: Set(target.app_id.clone()),
            cluster_name: Set(target.cluster_name.clone()),
            namespace_name: Set(target.namespace_name.clone()),
            configurations: Set(Some(configurations_json)),
            comment: Set(Some(format!("Rollback to release {}", release_id))),
            is_abandoned: Set(false),
            created_by: Set(Some(operator.to_string())),
            last_modified_by: Set(Some(operator.to_string())),
            ..Default::default()
        };
        let new_release = release_repository::create(&self.db, release_model).await?;

        // Create release history
        let history_model = apollo_release_history::ActiveModel {
            app_id: Set(target.app_id.clone()),
            cluster_name: Set(target.cluster_name.clone()),
            namespace_name: Set(target.namespace_name.clone()),
            branch_name: Set(Some("default".to_string())),
            release_id: Set(new_release.id),
            previous_release_id: Set(prev_release_id),
            operation: Set(OP_ROLLBACK),
            operation_context: Set(Some(format!("Rollback from release {}", release_id))),
            created_by: Set(Some(operator.to_string())),
            ..Default::default()
        };
        let _ = release_repository::create_history(&self.db, history_model).await;

        // Update items in namespace to match rollback configurations
        let inst = namespace_repository::find_instance(
            &self.db,
            &target.app_id,
            &target.cluster_name,
            &target.namespace_name,
        )
        .await?;

        if let Some(ns) = inst {
            // Delete existing items
            let existing_items = item_repository::find_all_by_namespace(&self.db, ns.id).await?;
            for item in existing_items {
                item_repository::soft_delete(&self.db, item.id).await?;
            }

            // Re-create items from rollback configurations
            let mut line_num = 1;
            for (key, value) in &configurations {
                let item_model = apollo_item::ActiveModel {
                    namespace_id: Set(ns.id),
                    key: Set(key.clone()),
                    value: Set(Some(value.clone())),
                    line_num: Set(Some(line_num)),
                    created_by: Set(Some(operator.to_string())),
                    last_modified_by: Set(Some(operator.to_string())),
                    ..Default::default()
                };
                let _ = item_repository::create(&self.db, item_model).await;
                line_num += 1;
            }
        }

        let mut result = ApolloRelease::new(
            target.app_id,
            target.cluster_name,
            target.namespace_name,
            format!("rollback-{}", release_id),
            configurations,
        );
        result.data_change_created_by = Some(operator.to_string());

        Ok(Some(result))
    }

    // ========================================================================
    // Model Conversion Helpers
    // ========================================================================

    fn model_to_app(model: apollo_app::Model) -> ApolloApp {
        ApolloApp {
            name: model.name,
            app_id: model.app_id,
            org_id: model.org_id,
            org_name: model.org_name,
            owner_name: model.owner_name,
            owner_email: model.owner_email,
            data_change_created_by: model.created_by,
            data_change_last_modified_by: model.last_modified_by,
        }
    }

    fn model_to_cluster(model: apollo_cluster::Model) -> ApolloCluster {
        ApolloCluster {
            name: model.name,
            app_id: model.app_id,
            data_change_created_by: model.created_by,
            data_change_last_modified_by: model.last_modified_by,
        }
    }

    fn model_to_item(model: apollo_item::Model) -> ApolloItem {
        let mut item = ApolloItem::new(model.key, model.value.unwrap_or_default());
        item.comment = model.comment;
        item.data_change_created_by = model.created_by;
        item.data_change_last_modified_by = model.last_modified_by;
        item
    }
}
