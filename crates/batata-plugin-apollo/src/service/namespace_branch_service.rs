//! Port of upstream `apollo-biz` NamespaceBranchService + related helpers.
//!
//! Branch model (upstream semantics):
//! - a gray branch is a child **Cluster** row (`ParentClusterId > 0`) whose
//!   name is a timestamp-based unique key, plus a same-app/same-namespace
//!   **Namespace** row whose `ClusterName` equals that generated name;
//! - the relationship lives on Cluster.ParentClusterId only;
//! - branch status is carried by GrayReleaseRule.BranchStatus
//!   (0 DELETED / 1 ACTIVE / 2 MERGED);
//! - deleting a branch inserts a tombstone rule and soft-deletes the child
//!   cluster/namespace/items while **child Releases are retained**.

use std::collections::HashMap;
use std::sync::Arc;


use crate::api::dto::NamespaceDTO;
use crate::persistence::shared::{StoredCluster, StoredNamespace};
use crate::persistence::traits::{
    ApolloPersistenceService, ClusterPersistence, ItemPersistence, NamespacePersistence,
    ReleasePersistence,
};

/// Upstream `ReleaseOperation` constants.
pub mod release_operation {
    /// The `NORMAL_RELEASE` constant.
    pub const NORMAL_RELEASE: i32 = 0;
    /// The `ROLLBACK` constant.
    pub const ROLLBACK: i32 = 1;
    /// The `GRAY_RELEASE` constant.
    pub const GRAY_RELEASE: i32 = 2;
    /// The `APPLY_GRAY_RULES` constant.
    pub const APPLY_GRAY_RULES: i32 = 3;
    #[allow(dead_code)]
    /// The `GRAY_RELEASE_MERGE_TO_MASTER` constant.
    pub const GRAY_RELEASE_MERGE_TO_MASTER: i32 = 4;
    /// The `MASTER_NORMAL_RELEASE_MERGE_TO_GRAY` constant.
    pub const MASTER_NORMAL_RELEASE_MERGE_TO_GRAY: i32 = 5;
    /// The `MASTER_ROLLBACK_MERGE_TO_GRAY` constant.
    pub const MASTER_ROLLBACK_MERGE_TO_GRAY: i32 = 6;
    /// The `ABANDON_GRAY_RELEASE` constant.
    pub const ABANDON_GRAY_RELEASE: i32 = 7;
    /// The `GRAY_RELEASE_DELETED_AFTER_MERGE` constant.
    pub const GRAY_RELEASE_DELETED_AFTER_MERGE: i32 = 8;
}

/// Upstream `NamespaceBranchStatus`.
pub const BRANCH_STATUS_ACTIVE: i32 = 1;
#[allow(dead_code)]
/// Namespace branch status: merged.
pub const BRANCH_STATUS_MERGED: i32 = 2;

/// Represents the `NamespaceBranchService` entity.
pub struct NamespaceBranchService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl NamespaceBranchService {
    /// Creates a new `NamespaceBranchService`.
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    /// Upstream `UniqueKeyGenerator.generate`: `{yyyyMMddHHmmss}-{16 hex}`.
    fn generate_branch_name() -> String {
        let ts = chrono::Utc::now().format("%Y%m%d%H%M%S");
        let mut seed = rand::random::<u64>();
        let hex: String = (0..16)
            .map(|_| {
                seed = seed.rotate_left(4) ^ (seed.wrapping_mul(0x9E3779B97F4A7C15) | 1);
                format!("{:x}", (seed >> 60) as u8 & 0x0f)
            })
            .collect();
        format!("{}-{}", ts, hex)
    }

    /// Upstream `NamespaceBranchService.findBranch`.
    pub async fn find_branch(
        &self,
        app_id: &str,
        parent_cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<Option<(StoredCluster, StoredNamespace)>> {
        let parent = match ClusterPersistence::get(
            &self.persistence,
            app_id,
            parent_cluster_name,
        )
        .await?
        {
            Some(c) => c,
            None => return Ok(None),
        };
        for cluster in ClusterPersistence::list(&self.persistence, app_id).await? {
            if cluster.is_deleted || parent.id == 0 || cluster.parent_cluster_id != parent.id || parent_cluster_name == cluster.name {
                continue;
            }
            if let Some(ns) = self
                .persistence
                .get_by_app_cluster(app_id, &cluster.name, namespace_name)
                .await?
            {
                if !ns.is_deleted {
                    return Ok(Some((cluster, ns)));
                }
            }
        }
        Ok(None)
    }

    /// Port of upstream `NamespaceBranchService.createBranch`:
    /// idempotent; creates child Cluster (ParentClusterId set) + child
    /// Namespace. No rule row is created here (upstream creates it on first
    /// PUT .../rules).
    pub async fn create_branch(
        &self,
        app_id: &str,
        parent_cluster_name: &str,
        namespace_name: &str,
        operator: &str,
    ) -> anyhow::Result<NamespaceDTO> {
        if let Some(existing) = self
            .find_branch(app_id, parent_cluster_name, namespace_name)
            .await?
        {
            return Ok(self.namespace_to_dto(&existing.1));
        }

        let parent = ClusterPersistence::get(&self.persistence, app_id, parent_cluster_name)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!("Cluster not found: {}/{}", app_id, parent_cluster_name)
            })?;
        if parent.parent_cluster_id != 0 {
            return Err(anyhow::anyhow!(
                "Cannot create branch of a branch: {}",
                parent_cluster_name
            ));
        }
        // The namespace being branched must exist on the parent cluster.
        let parent_ns = self
            .persistence
            .get_by_app_cluster(app_id, parent_cluster_name, namespace_name)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Namespace not found: {}/{}/{}",
                    app_id,
                    parent_cluster_name,
                    namespace_name
                )
            })?;

        let now = chrono::Utc::now().timestamp_millis();

        // 1) child cluster
        let mut branch_cluster = StoredCluster {
            id: 0,
            name: Self::generate_branch_name(),
            app_id: app_id.to_string(),
            parent_cluster_id: parent.id,
            is_deleted: false,
            deleted_at: 0,
            data_change_created_by: operator.to_string(),
            data_change_created_time: now,
            data_change_last_modified_by: None,
            data_change_last_time: Some(now),
        };
        branch_cluster = ClusterPersistence::create(&self.persistence, branch_cluster).await?;

        // 2) child namespace mirroring the parent's definition
        let child = StoredNamespace {
            id: 0,
            app_id: parent_ns.app_id.clone(),
            cluster_name: branch_cluster.name.clone(),
            namespace_name: parent_ns.namespace_name.clone(),
            format: parent_ns.format.clone(),
            is_public: parent_ns.is_public,
            comment: None,
            is_deleted: false,
            deleted_at: 0,
            data_change_created_by: operator.to_string(),
            data_change_created_time: now,
            data_change_last_modified_by: None,
            data_change_last_time: Some(now),
        };
        let created = NamespacePersistence::create(&self.persistence, child).await?;
        Ok(self.namespace_to_dto(&created))
    }

    /// Port of upstream `NamespaceBranchService.deleteBranch`:
    /// tombstone rule + cascade soft-delete of child cluster/namespace/items.
    /// Child Releases are deliberately retained while the parent exists.
    pub async fn delete_branch(
        &self,
        app_id: &str,
        parent_cluster_name: &str,
        namespace_name: &str,
        operator: &str,
        merged: bool,
    ) -> anyhow::Result<()> {
        use crate::persistence::traits::{GrayReleasePersistence, ReleaseHistoryPersistence};

        let Some((cluster, _ns)) = self
            .find_branch(app_id, parent_cluster_name, namespace_name)
            .await?
        else {
            return Err(anyhow::anyhow!("Branch not found"));
        };
        let now = chrono::Utc::now().timestamp_millis();

        // Tombstone rule rows under the PARENT key (upstream inserts a rule
        // row with rules="[]" and the final status, deleting older rows).
        let rules = GrayReleasePersistence::list_by_app(&self.persistence, app_id).await?;
        for r in rules {
            if r.cluster_name != parent_cluster_name
                || r.namespace_name != namespace_name
                || r.is_deleted
            {
                continue;
            }
            GrayReleasePersistence::update_rules(
                &self.persistence,
                r.id,
                "[]".to_string(),
                0,
            )
            .await?;
        }
        // Mark all live rules for this namespace as terminal via soft delete.
        let rules = GrayReleasePersistence::list_by_app(&self.persistence, app_id).await?;
        for r in rules {
            if r.cluster_name == parent_cluster_name
                && r.namespace_name == namespace_name
                && !r.is_deleted
            {
                GrayReleasePersistence::delete(&self.persistence, r.id).await?;
            }
        }

        // Cascade: items, then namespace, then the child cluster.
        if let Some(child_ns) = self
            .persistence
            .get_by_app_cluster(app_id, &cluster.name, namespace_name)
            .await?
        {
            let items = ItemPersistence::list_by_namespace(&self.persistence, child_ns.id).await?;
            for item in items {
                ItemPersistence::delete(&self.persistence, item.id).await?;
            }
            NamespacePersistence::delete(&self.persistence, child_ns.id).await?;
        }
        ClusterPersistence::delete(&self.persistence, app_id, &cluster.name).await?;

        let op = if merged {
            release_operation::GRAY_RELEASE_DELETED_AFTER_MERGE
        } else {
            release_operation::ABANDON_GRAY_RELEASE
        };
        ReleaseHistoryPersistence::record_release_history(
            &self.persistence,
            app_id,
            parent_cluster_name,
            namespace_name,
            &cluster.name,
            0,
            0,
            op,
            "",
            operator,
        )
        .await?;

        tracing::info!(
            "branch {} of {}/{}/{} deleted at {}",
            cluster.name,
            app_id,
            parent_cluster_name,
            namespace_name,
            now
        );
        Ok(())
    }

    /// Latest active release configurations of the BRANCH namespace.
    pub async fn branch_latest_configurations(
        &self,
        app_id: &str,
        branch_cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<Option<HashMap<String, String>>> {
        match ReleasePersistence::get_latest(
            &self.persistence,
            app_id,
            branch_cluster_name,
            namespace_name,
        )
        .await?
        {
            Some(r) => Ok(serde_json::from_str(&r.configurations).unwrap_or_default()),
            None => Ok(None),
        }
    }

    /// Active-branch check for auto-compensations: upstream triggers
    /// mergeFromMasterAndPublishBranch whenever the branch NAMESPACE exists
    /// (rule state is irrelevant to the trigger).
    pub async fn has_active_branch(
        &self,
        app_id: &str,
        parent_cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<Option<String>> {
        Ok(self
            .find_branch(app_id, parent_cluster_name, namespace_name)
            .await?
            .map(|(c, _)| c.name))
    }

    fn namespace_to_dto(&self, stored: &StoredNamespace) -> NamespaceDTO {
        NamespaceDTO {
            app_id: stored.app_id.clone(),
            cluster_name: stored.cluster_name.clone(),
            namespace_name: stored.namespace_name.clone(),
            format: Some(stored.format.clone()),
            is_public: Some(stored.is_public),
            comment: stored.comment.clone(),
            data_change_created_by: Some(stored.data_change_created_by.clone()),
            data_change_last_modified_by: stored.data_change_last_modified_by.clone(),
            data_change_last_time: stored
                .data_change_last_time
                .map(|t| t.to_string()),
            data_change_created_time: Some(stored.data_change_created_time.to_string()),
        }
    }
}
