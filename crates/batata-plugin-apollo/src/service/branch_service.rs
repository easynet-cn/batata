//! Apollo branch service
//!
//! Provides branch management for gray releases.
//! A branch is a child cluster with `parent_cluster_id > 0`.

use std::collections::HashMap;
use std::sync::Arc;

use batata_persistence::entity::{
    apollo_cluster, apollo_gray_release_rule, apollo_item, apollo_namespace, apollo_release,
    apollo_release_history, apollo_release_message,
};
use sea_orm::{DatabaseConnection, Set};

use crate::model::{ApolloRelease, GrayReleaseRule, GrayReleaseStatus, GrayRule};
use crate::repository::{
    cluster_repository, gray_release_repository, item_repository, namespace_repository,
    release_repository,
};

/// Release history operation type for gray release
const OP_GRAY_RELEASE: i16 = 2;

/// Apollo branch service
///
/// Manages gray release branches (child clusters).
pub struct ApolloBranchService {
    db: Arc<DatabaseConnection>,
}

impl ApolloBranchService {
    /// Create a new ApolloBranchService
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    /// Create a branch (child cluster)
    pub async fn create_branch(
        &self,
        app_id: &str,
        parent_cluster: &str,
        branch_name: &str,
        operator: &str,
    ) -> anyhow::Result<String> {
        // Find parent cluster
        let parent = cluster_repository::find_by_app_and_name(&self.db, app_id, parent_cluster)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Parent cluster not found"))?;

        // Create child cluster
        let cluster_model = apollo_cluster::ActiveModel {
            name: Set(branch_name.to_string()),
            app_id: Set(app_id.to_string()),
            parent_cluster_id: Set(parent.id),
            comment: Set(Some(format!("Branch of {}", parent_cluster))),
            created_by: Set(Some(operator.to_string())),
            last_modified_by: Set(Some(operator.to_string())),
            ..Default::default()
        };
        cluster_repository::create(&self.db, cluster_model).await?;

        // Copy namespace instances from parent
        let parent_namespaces =
            namespace_repository::find_instances_by_cluster(&self.db, app_id, parent_cluster)
                .await?;

        for parent_ns in &parent_namespaces {
            // Create namespace instance for branch
            let ns_model = apollo_namespace::ActiveModel {
                app_id: Set(app_id.to_string()),
                cluster_name: Set(branch_name.to_string()),
                namespace_name: Set(parent_ns.namespace_name.clone()),
                created_by: Set(Some(operator.to_string())),
                ..Default::default()
            };
            let branch_ns = namespace_repository::create_instance(&self.db, ns_model).await?;

            // Copy items from parent namespace
            let parent_items =
                item_repository::find_all_by_namespace(&self.db, parent_ns.id).await?;
            for item in parent_items {
                let item_model = apollo_item::ActiveModel {
                    namespace_id: Set(branch_ns.id),
                    key: Set(item.key),
                    r#type: Set(item.r#type),
                    value: Set(item.value),
                    comment: Set(item.comment),
                    line_num: Set(item.line_num),
                    created_by: Set(Some(operator.to_string())),
                    last_modified_by: Set(Some(operator.to_string())),
                    ..Default::default()
                };
                let _ = item_repository::create(&self.db, item_model).await;
            }
        }

        Ok(branch_name.to_string())
    }

    /// Delete a branch
    pub async fn delete_branch(&self, app_id: &str, branch_name: &str) -> anyhow::Result<bool> {
        // Verify it's a branch (has parent)
        let cluster =
            cluster_repository::find_by_app_and_name(&self.db, app_id, branch_name).await?;
        match cluster {
            Some(c) if c.parent_cluster_id > 0 => {
                // Delete namespace instances and items for this branch
                let namespaces =
                    namespace_repository::find_instances_by_cluster(&self.db, app_id, branch_name)
                        .await?;
                for ns in namespaces {
                    let items = item_repository::find_all_by_namespace(&self.db, ns.id).await?;
                    for item in items {
                        item_repository::soft_delete(&self.db, item.id).await?;
                    }
                    namespace_repository::soft_delete_instance(
                        &self.db,
                        app_id,
                        branch_name,
                        &ns.namespace_name,
                    )
                    .await?;
                }

                // Delete the branch cluster
                cluster_repository::soft_delete(&self.db, app_id, branch_name).await
            }
            _ => Ok(false),
        }
    }

    /// Get a branch
    pub async fn get_branch(
        &self,
        app_id: &str,
        branch_name: &str,
    ) -> anyhow::Result<Option<apollo_cluster::Model>> {
        let cluster =
            cluster_repository::find_by_app_and_name(&self.db, app_id, branch_name).await?;
        match cluster {
            Some(c) if c.parent_cluster_id > 0 => Ok(Some(c)),
            _ => Ok(None),
        }
    }

    /// Get all branches for a parent cluster
    pub async fn get_branches(
        &self,
        app_id: &str,
        parent_cluster: &str,
    ) -> anyhow::Result<Vec<apollo_cluster::Model>> {
        let parent =
            cluster_repository::find_by_app_and_name(&self.db, app_id, parent_cluster).await?;
        match parent {
            Some(p) => cluster_repository::find_children(&self.db, p.id).await,
            None => Ok(Vec::new()),
        }
    }

    /// Get gray release rules for a branch
    pub async fn get_gray_rules(
        &self,
        app_id: &str,
        cluster: &str,
        namespace: &str,
    ) -> Option<GrayReleaseRule> {
        let rules =
            gray_release_repository::find_by_namespace(&self.db, app_id, cluster, namespace)
                .await
                .ok()?;

        rules.into_iter().next().map(|r| {
            let parsed_rules: Vec<GrayRule> = r
                .rules
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or_default();

            GrayReleaseRule {
                name: format!("{}-gray-{}", namespace, r.branch_name),
                app_id: r.app_id,
                cluster_name: r.cluster_name,
                namespace_name: r.namespace_name,
                branch_name: r.branch_name,
                rules: parsed_rules,
                release_status: match r.branch_status {
                    1 => GrayReleaseStatus::Merged,
                    2 => GrayReleaseStatus::Abandoned,
                    _ => GrayReleaseStatus::Active,
                },
            }
        })
    }

    /// Update gray release rules
    pub async fn update_gray_rules(
        &self,
        app_id: &str,
        cluster: &str,
        namespace: &str,
        branch_name: &str,
        rules: Vec<GrayRule>,
    ) -> anyhow::Result<GrayReleaseRule> {
        let rules_json = serde_json::to_string(&rules)?;

        // Try to find existing rule
        let existing =
            gray_release_repository::find_by_namespace(&self.db, app_id, cluster, namespace)
                .await?;

        if let Some(rule) = existing.into_iter().next() {
            // Update existing
            gray_release_repository::update_rules(&self.db, rule.id, &rules_json).await?;
        } else {
            // Create new
            let model = apollo_gray_release_rule::ActiveModel {
                app_id: Set(app_id.to_string()),
                cluster_name: Set(cluster.to_string()),
                namespace_name: Set(namespace.to_string()),
                branch_name: Set(branch_name.to_string()),
                rules: Set(Some(rules_json)),
                release_id: Set(0),
                branch_status: Set(0),
                ..Default::default()
            };
            gray_release_repository::create(&self.db, model).await?;
        }

        Ok(GrayReleaseRule {
            name: format!("{}-gray-{}", namespace, branch_name),
            app_id: app_id.to_string(),
            cluster_name: cluster.to_string(),
            namespace_name: namespace.to_string(),
            branch_name: branch_name.to_string(),
            rules,
            release_status: GrayReleaseStatus::Active,
        })
    }

    /// Publish a gray release on a branch
    pub async fn gray_release(
        &self,
        app_id: &str,
        cluster: &str,
        namespace: &str,
        branch_name: &str,
        release_title: &str,
        operator: &str,
    ) -> anyhow::Result<ApolloRelease> {
        // Get branch namespace instance
        let inst =
            namespace_repository::find_instance(&self.db, app_id, branch_name, namespace).await?;
        let ns = inst.ok_or_else(|| anyhow::anyhow!("Branch namespace not found"))?;

        // Collect items from branch
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

        // Create release
        let release_model = apollo_release::ActiveModel {
            release_key: Set(release_key),
            name: Set(Some(release_title.to_string())),
            app_id: Set(app_id.to_string()),
            cluster_name: Set(branch_name.to_string()),
            namespace_name: Set(namespace.to_string()),
            configurations: Set(Some(configurations_json)),
            comment: Set(Some(format!("Gray release on branch {}", branch_name))),
            is_abandoned: Set(false),
            created_by: Set(Some(operator.to_string())),
            last_modified_by: Set(Some(operator.to_string())),
            ..Default::default()
        };
        let release = release_repository::create(&self.db, release_model).await?;

        // Update gray release rule with release_id
        let rules =
            gray_release_repository::find_by_namespace(&self.db, app_id, cluster, namespace)
                .await?;
        if let Some(rule) = rules.into_iter().next() {
            let _ = gray_release_repository::update_release_id(&self.db, rule.id, release.id).await;
        }

        // Create release history
        let history_model = apollo_release_history::ActiveModel {
            app_id: Set(app_id.to_string()),
            cluster_name: Set(branch_name.to_string()),
            namespace_name: Set(namespace.to_string()),
            branch_name: Set(Some(branch_name.to_string())),
            release_id: Set(release.id),
            previous_release_id: Set(0),
            operation: Set(OP_GRAY_RELEASE),
            operation_context: Set(Some(format!("Gray release: {}", release_title))),
            created_by: Set(Some(operator.to_string())),
            ..Default::default()
        };
        let _ = release_repository::create_history(&self.db, history_model).await;

        Ok(ApolloRelease::new(
            app_id.to_string(),
            branch_name.to_string(),
            namespace.to_string(),
            release_title.to_string(),
            configurations,
        ))
    }

    /// Merge a branch into the parent cluster
    pub async fn merge_branch(
        &self,
        app_id: &str,
        parent_cluster: &str,
        branch_name: &str,
        namespace: &str,
        operator: &str,
    ) -> anyhow::Result<ApolloRelease> {
        // Get branch namespace
        let branch_inst =
            namespace_repository::find_instance(&self.db, app_id, branch_name, namespace).await?;
        let branch_ns = branch_inst.ok_or_else(|| anyhow::anyhow!("Branch namespace not found"))?;

        // Get parent namespace
        let parent_inst =
            namespace_repository::find_instance(&self.db, app_id, parent_cluster, namespace)
                .await?;
        let parent_ns = parent_inst.ok_or_else(|| anyhow::anyhow!("Parent namespace not found"))?;

        // Get branch items
        let branch_items = item_repository::find_all_by_namespace(&self.db, branch_ns.id).await?;

        // Delete existing parent items
        let parent_items = item_repository::find_all_by_namespace(&self.db, parent_ns.id).await?;
        for item in parent_items {
            item_repository::soft_delete(&self.db, item.id).await?;
        }

        // Copy branch items to parent
        let mut configurations: HashMap<String, String> = HashMap::new();
        for item in &branch_items {
            configurations.insert(item.key.clone(), item.value.clone().unwrap_or_default());

            let item_model = apollo_item::ActiveModel {
                namespace_id: Set(parent_ns.id),
                key: Set(item.key.clone()),
                r#type: Set(item.r#type.clone()),
                value: Set(item.value.clone()),
                comment: Set(item.comment.clone()),
                line_num: Set(item.line_num),
                created_by: Set(Some(operator.to_string())),
                last_modified_by: Set(Some(operator.to_string())),
                ..Default::default()
            };
            let _ = item_repository::create(&self.db, item_model).await;
        }

        // Publish main release
        let configurations_json = serde_json::to_string(&configurations)?;
        let release_key = format!(
            "{}-{}",
            chrono::Utc::now().timestamp_millis(),
            &uuid::Uuid::new_v4().to_string()[..8]
        );

        let release_model = apollo_release::ActiveModel {
            release_key: Set(release_key),
            name: Set(Some(format!("Merge from branch {}", branch_name))),
            app_id: Set(app_id.to_string()),
            cluster_name: Set(parent_cluster.to_string()),
            namespace_name: Set(namespace.to_string()),
            configurations: Set(Some(configurations_json)),
            comment: Set(Some(format!("Merged from branch {}", branch_name))),
            is_abandoned: Set(false),
            created_by: Set(Some(operator.to_string())),
            last_modified_by: Set(Some(operator.to_string())),
            ..Default::default()
        };
        let release = release_repository::create(&self.db, release_model).await?;

        // Send release message
        let message = format!("{}+{}+{}", app_id, parent_cluster, namespace);
        let msg_model = apollo_release_message::ActiveModel {
            message: Set(message),
            created_by: Set(Some(operator.to_string())),
            ..Default::default()
        };
        let _ = release_repository::send_message(&self.db, msg_model).await;

        // Mark gray release rule as merged
        let rules =
            gray_release_repository::find_by_namespace(&self.db, app_id, parent_cluster, namespace)
                .await?;
        if let Some(rule) = rules.into_iter().next() {
            let _ = gray_release_repository::update_status(&self.db, rule.id, 1).await; // Merged
        }

        // Create release history
        let history_model = apollo_release_history::ActiveModel {
            app_id: Set(app_id.to_string()),
            cluster_name: Set(parent_cluster.to_string()),
            namespace_name: Set(namespace.to_string()),
            branch_name: Set(Some(branch_name.to_string())),
            release_id: Set(release.id),
            previous_release_id: Set(0),
            operation: Set(0), // Normal release (merge)
            operation_context: Set(Some(format!("Merge branch {} to main", branch_name))),
            created_by: Set(Some(operator.to_string())),
            ..Default::default()
        };
        let _ = release_repository::create_history(&self.db, history_model).await;

        Ok(ApolloRelease::new(
            app_id.to_string(),
            parent_cluster.to_string(),
            namespace.to_string(),
            format!("Merge from branch {}", branch_name),
            configurations,
        ))
    }
}
