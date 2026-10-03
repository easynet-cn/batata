use std::sync::Arc;
use std::collections::HashMap;

use serde_json::Value;

use crate::api::dto::{ReleaseDTO, ReleaseHistoryDTO};
use crate::persistence::shared::StoredRelease;
use crate::persistence::traits::{ApolloPersistenceService, ReleasePersistence, ReleaseHistoryPersistence, ItemPersistence, NamespacePersistence};
use crate::service::release_message_service::ReleaseMessageService;
use chrono::Utc;

/// Represents the `ReleaseService` entity.
pub struct ReleaseService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl ReleaseService {
    /// Creates a new `ReleaseService`.
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    /// Persist one ReleaseMessage row (upstream `DatabaseMessageSender`) and
    /// wake local long-pollers. The persisted row id IS the client-visible
    /// notificationId — monotonic and restart-safe (upstream semantics).
    async fn notify_publish(&self, app_id: &str, cluster_name: &str, namespace_name: &str) {
        let sender = ReleaseMessageService::new(self.persistence.clone());
        match sender
            .send_message(app_id, cluster_name, namespace_name)
            .await
        {
            Ok(stored) => {
                crate::service::notification_hub::hub().notify(&stored.message);
            }
            Err(e) => {
                tracing::error!("failed to persist release message: {}", e);
            }
        }
    }

    /// Performs the `publish` operation.
    pub async fn publish(&self, app_id: &str, cluster_name: &str, namespace_name: &str, 
        release_name: &str, release_comment: Option<String>, operator: &str, 
        _is_emergency_publish: bool) -> Result<ReleaseDTO, anyhow::Error> {

        let namespace = self.persistence.get_by_app_cluster(app_id, cluster_name, namespace_name).await?
            .ok_or_else(|| anyhow::anyhow!("Namespace not found: {}/{}/{}", app_id, cluster_name, namespace_name))?;

        let old_master = ReleasePersistence::get_latest(&self.persistence, app_id, cluster_name, namespace_name).await?;

        let items = <dyn ItemPersistence>::list_by_namespace(&self.persistence, namespace.id).await?;

        let mut configurations: HashMap<String, String> = HashMap::new();
        for item in items {
            configurations.insert(item.key, item.value);
        }

        let configurations_json = serde_json::to_string(&configurations)?;

        let now = Utc::now().timestamp_millis();
        let release_id = now;
        let release_key = format!("{}+{}+{}+{}", app_id, cluster_name, namespace_name, release_id);

        let stored = StoredRelease {
            id: 0,
            release_key: release_key.clone(),
            name: release_name.to_string(),
            comment: release_comment,
            app_id: app_id.to_string(),
            cluster_name: cluster_name.to_string(),
            namespace_name: namespace_name.to_string(),
            configurations: configurations_json,
            release_id: Some(release_id),
            is_abandoned: false,
            is_deleted: false,
            deleted_at: 0,
            data_change_created_by: operator.to_string(),
            data_change_created_time: now,
            data_change_last_modified_by: None,
            data_change_last_time: None,
        };

        let created = <dyn ReleasePersistence>::create(&self.persistence, stored).await?;
        Self::notify_publish(self, app_id, cluster_name, namespace_name).await;
        self.record_history(app_id, cluster_name, namespace_name, namespace_name, created.id, 0, 0, operator, "").await;

        // Upstream mergeFromMasterAndPublishBranch: when an ACTIVE branch
        // exists, keep its own modifications on top of the new master release
        // and republish the branch (op MASTER_NORMAL_RELEASE_MERGE_TO_GRAY).
        use crate::service::namespace_branch_service::NamespaceBranchService;
        let branch_svc = NamespaceBranchService::new(self.persistence.clone());
        if let Some(branch_cluster) = branch_svc
            .has_active_branch(app_id, cluster_name, namespace_name)
            .await?
        {
            let old_cfgs: HashMap<String, String> = old_master
                .as_ref()
                .and_then(|r| serde_json::from_str(&r.configurations).unwrap_or_default())
                .unwrap_or_default();
            let child_latest = branch_svc
                .branch_latest_configurations(app_id, &branch_cluster, namespace_name)
                .await?
                .unwrap_or_default();

            // Branch's own modifications = child keys whose value differs from
            // the OLD master release.
            let own_changes: HashMap<String, String> = child_latest
                .iter()
                .filter(|(k, v)| old_cfgs.get(*k).map(|old| old != *v).unwrap_or(true))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();

            let mut expected = configurations.clone();
            for (k, v) in own_changes {
                expected.insert(k, v);
            }
            if expected != child_latest {
                self.publish_branch_release(
                    app_id,
                    cluster_name,
                    &branch_cluster,
                    namespace_name,
                    expected,
                    format!("{}-master-normal-release-merge-to-gray", Utc::now().format("%Y%m%d%H%M%S")),
                    operator,
                    5,
                )
                .await?;
            }
        }
        Ok(created.into())
    }

    /// Returns the requested value.
    pub async fn get_latest_active(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> Result<Option<ReleaseDTO>, anyhow::Error> {
        let stored = <dyn ReleasePersistence>::get_latest(&self.persistence, app_id, cluster_name, namespace_name).await?;
        Ok(stored.map(|s| s.into()))
    }

    /// Returns the requested value.
    pub async fn get_configurations(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> Result<Option<HashMap<String, String>>, anyhow::Error> {
        let release = self.get_latest_active(app_id, cluster_name, namespace_name).await?;

        match release {
            Some(r) => {
                let configs: HashMap<String, String> = serde_json::from_str(&r.configurations.unwrap_or_default())?;
                Ok(Some(configs))
            }
            None => Ok(None),
        }
    }

    /// Returns the requested value.
    pub async fn find_active_releases(&self, app_id: &str, cluster_name: &str, namespace_name: &str, _page: u64, _size: u64) -> Result<(Vec<ReleaseDTO>, u64), anyhow::Error> {
        let stored_list = <dyn ReleasePersistence>::list_by_namespace(&self.persistence, app_id, cluster_name, namespace_name).await?;
        let active_releases: Vec<_> = stored_list.into_iter()
            .filter(|s| !s.is_deleted && !s.is_abandoned)
            .map(|s| s.into())
            .collect();
        
        let count = active_releases.len() as u64;
        Ok((active_releases, count))
    }

    /// All releases (including abandoned) for a namespace, newest first.
    /// Upstream `/releases/all` used by the release history page.
    pub async fn find_all_releases(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> Result<Vec<ReleaseDTO>, anyhow::Error> {
        let stored_list = <dyn ReleasePersistence>::list_all_by_namespace(&self.persistence, app_id, cluster_name, namespace_name).await?;
        Ok(stored_list.into_iter().map(|s| s.into()).collect())
    }

    /// Returns the requested value.
    pub async fn get_by_id(&self, release_id: i64) -> Result<Option<ReleaseDTO>, anyhow::Error> {
        let stored = <dyn ReleasePersistence>::get_by_id(&self.persistence, release_id).await?;
        Ok(stored.map(|s| s.into()))
    }

    /// Resolve a gray-rule target release.
    ///
    /// Upstream: `GrayReleaseRule.ReleaseId` references **Release.Id** (the
    /// row pk) and configservice calls findActiveOne(pk). batata's gray
    /// publish returns the row id too, so look up by pk first; fall back to
    /// the timestamp `release_id` column for legacy rows only.
    pub async fn get_gray_release(&self, release_id: i64) -> Result<Option<ReleaseDTO>, anyhow::Error> {
        let stored = <dyn ReleasePersistence>::get_by_id(&self.persistence, release_id).await?;
        if stored.is_some() {
            return Ok(stored.map(|s| s.into()));
        }
        let stored = <dyn ReleasePersistence>::get_by_release_id(&self.persistence, release_id).await?;
        Ok(stored.map(|s| s.into()))
    }

    /// Upstream `branchRelease` primitive: store a release under the BRANCH
    /// cluster, repoint the active gray rule's releaseId to it, record history
    /// under the PARENT cluster with the given operation code and notify via
    /// the parent watch key (upstream ReleaseMessageKeyGenerator semantics).
    pub(crate) async fn publish_branch_release(
        &self,
        app_id: &str,
        parent_cluster_name: &str,
        branch_cluster_name: &str,
        namespace_name: &str,
        configurations: HashMap<String, String>,
        release_name: String,
        operator: &str,
        operation: i32,
    ) -> Result<(), anyhow::Error> {
        let now = Utc::now().timestamp_millis();
        let stored = StoredRelease {
            id: 0,
            release_key: format!("{}+{}+{}+{}+gray", app_id, branch_cluster_name, namespace_name, now),
            name: release_name,
            comment: None,
            app_id: app_id.to_string(),
            cluster_name: branch_cluster_name.to_string(),
            namespace_name: namespace_name.to_string(),
            configurations: serde_json::to_string(&configurations)?,
            release_id: Some(now),
            is_abandoned: false,
            is_deleted: false,
            deleted_at: 0,
            data_change_created_by: operator.to_string(),
            data_change_created_time: now,
            data_change_last_modified_by: None,
            data_change_last_time: None,
        };
        let created = <dyn ReleasePersistence>::create(&self.persistence, stored).await?;

        // Repoint the active rule's releaseId (upstream inserts a new rule row
        // and deletes the old one; we keep a single mutable row).
        use crate::persistence::traits::GrayReleasePersistence;
        let rules = GrayReleasePersistence::list_by_app(&self.persistence, app_id).await?;
        if let Some(rule) = rules.into_iter().find(|r| {
            !r.is_deleted
                && r.branch_status == Some(1)
                && r.cluster_name == parent_cluster_name
                && r.namespace_name == namespace_name
        }) {
            GrayReleasePersistence::update_rules(
                &self.persistence,
                rule.id,
                rule.rules.clone(),
                created.release_id.unwrap_or(0),
            )
            .await?;
        }

        self.record_history(app_id, parent_cluster_name, namespace_name, branch_cluster_name, created.id, 0, operation, operator, "").await;
        Self::notify_publish(self, app_id, parent_cluster_name, namespace_name).await;
        Ok(())
    }

    /// Port of upstream `ReleaseService.rollback`: mark the current latest
    /// release `IsAbandoned` — no new Release row, no item/commit changes; the
    /// previously active release becomes effective because all queries filter
    /// abandoned rows. When a gray branch exists, an auto branch release
    /// (op MASTER_ROLLBACK_MERGE_TO_GRAY) keeps gray clients consistent.
    pub async fn rollback(&self, app_id: &str, cluster_name: &str, namespace_name: &str, release_id: i64, operator: &str) -> Result<ReleaseDTO, anyhow::Error> {
        let actives = ReleasePersistence::list_active(&self.persistence, app_id, cluster_name, namespace_name).await?;
        if actives.len() < 2 {
            return Err(anyhow::anyhow!("At least two active releases are required for rollback"));
        }
        // actives sorted newest-first; index 0 is the one being rolled back.
        if actives[0].id != release_id {
            return Err(anyhow::anyhow!("Release {} is not the latest active release", release_id));
        }
        let now_effective = self.abandon_release(&actives[0], operator).await?;

        Self::notify_publish(self, app_id, cluster_name, namespace_name).await;
        self.record_history(app_id, cluster_name, namespace_name, namespace_name, now_effective.id, actives[0].id, 1, operator, "").await;
        self.rollback_child_namespace(app_id, cluster_name, namespace_name, operator, 6).await?;
        Ok(now_effective.into())
    }

    /// Port of upstream `rollbackTo`: `release_id` is the target release to
    /// rollback TO (matching Apollo SDK `rollbackRelease(env, releaseId, op)`).
    /// When `to_release_id` is provided it is treated as the current release to
    /// rollback FROM; otherwise every active release newer than the target is
    /// abandoned.
    pub async fn rollback_by_id(&self, release_id: i64, to_release_id: Option<i64>, operator: &str) -> Result<ReleaseDTO, anyhow::Error> {
        let target = <dyn ReleasePersistence>::get_by_id(&self.persistence, release_id).await?
            .ok_or_else(|| anyhow::anyhow!("Release not found: {}", release_id))?;
        let actives = ReleasePersistence::list_active(&self.persistence, &target.app_id, &target.cluster_name, &target.namespace_name).await?;
        // If a specific "from" release was given, validate it exists and is
        // newer than the target; otherwise abandon everything above target.
        if let Some(from_id) = to_release_id {
            let from = <dyn ReleasePersistence>::get_by_id(&self.persistence, from_id).await?
                .ok_or_else(|| anyhow::anyhow!("Release not found: {}", from_id))?;
            if from.id <= target.id {
                return Err(anyhow::anyhow!("Cannot rollback from {} to {}", from_id, release_id));
            }
        }
        // Abandon every active release newer than the target (target stays).
        let mut abandoned_any = false;
        let mut last_abandoned: Option<StoredRelease> = None;
        for r in &actives {
            if r.id <= target.id {
                break;
            }
            self.abandon_release(r, operator).await?;
            abandoned_any = true;
            last_abandoned = Some(r.clone());
        }
        if !abandoned_any {
            return Err(anyhow::anyhow!("No releases newer than {} to abandon", release_id));
        }
        let _ = last_abandoned;

        Self::notify_publish(self, &target.app_id, &target.cluster_name, &target.namespace_name).await;
        self.record_history(&target.app_id, &target.cluster_name, &target.namespace_name, &target.namespace_name, target.id, last_abandoned.as_ref().map(|r| r.id).unwrap_or(0), 1, operator, "").await;
        self.rollback_child_namespace(&target.app_id, &target.cluster_name, &target.namespace_name, operator, 6).await?;
        Ok(target.into())
    }

    /// Upstream abandon step: set IsAbandoned=true on the row itself.
    async fn abandon_release(&self, release: &StoredRelease, operator: &str) -> Result<StoredRelease, anyhow::Error> {
        let mut abandoned = release.clone();
        abandoned.is_abandoned = true;
        abandoned.data_change_last_modified_by = Some(operator.to_string());
        abandoned.data_change_last_time = Some(Utc::now().timestamp_millis());
        ReleasePersistence::update(&self.persistence, abandoned).await
    }

    /// Port of upstream `rollbackChildNamespace`: when the master namespace
    /// has an ACTIVE gray branch whose latest release derives from configs that
    /// just changed (rollback), republish the branch so gray clients do not
    /// stay on stale derived configurations. Operation code passed by caller
    /// (6 MASTER_ROLLBACK_MERGE_TO_GRAY here).
    async fn rollback_child_namespace(
        &self,
        app_id: &str,
        parent_cluster_name: &str,
        namespace_name: &str,
        operator: &str,
        operation: i32,
    ) -> Result<(), anyhow::Error> {
        use crate::service::namespace_branch_service::NamespaceBranchService;
        let branch_svc = NamespaceBranchService::new(self.persistence.clone());
        let Some(branch_cluster) = branch_svc
            .has_active_branch(app_id, parent_cluster_name, namespace_name)
            .await?
        else {
            return Ok(());
        };
        let Some(master_cfgs) =
            ReleasePersistence::get_latest(&self.persistence, app_id, parent_cluster_name, namespace_name).await?
        else {
            return Ok(());
        };
        // Republish the branch from master base + existing branch item edits.
        let mut configurations: HashMap<String, String> =
            serde_json::from_str(&master_cfgs.configurations).unwrap_or_default();
        if let Some(branch_ns) = self
            .persistence
            .get_by_app_cluster(app_id, &branch_cluster, namespace_name)
            .await?
        {
            for item in ItemPersistence::list_by_namespace(&self.persistence, branch_ns.id).await? {
                configurations.insert(item.key, item.value);
            }
        }
        self.publish_branch_release(
            app_id,
            parent_cluster_name,
            &branch_cluster,
            namespace_name,
            configurations,
            format!("{}-master-rollback-merge-to-gray", Utc::now().format("%Y%m%d%H%M%S")),
            operator,
            operation,
        )
        .await
    }

    /// Performs the `compare` operation.
    pub async fn compare(&self, base_release_id: i64, to_compare_release_id: i64) -> Result<Value, anyhow::Error> {
        let base = <dyn ReleasePersistence>::get_by_id(&self.persistence, base_release_id).await?;
        let other = <dyn ReleasePersistence>::get_by_id(&self.persistence, to_compare_release_id).await?;

        let base_configs: Value = base.as_ref()
            .and_then(|b| serde_json::from_str(&b.configurations).ok())
            .unwrap_or_else(|| Value::Object(Default::default()));
        let other_configs: Value = other.as_ref()
            .and_then(|o| serde_json::from_str(&o.configurations).ok())
            .unwrap_or_else(|| Value::Object(Default::default()));

        Ok(serde_json::json!({
            "baseReleaseId": base_release_id,
            "toCompareReleaseId": to_compare_release_id,
            "baseConfigurations": base_configs,
            "toCompareConfigurations": other_configs,
        }))
    }

    /// Performs the `merge_branch_and_release` operation.
    pub async fn merge_branch_and_release(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
        _branch_name: &str,
        release_name: &str,
        release_comment: Option<String>,
        operator: &str,
        _is_emergency_publish: bool,
        change_sets: crate::api::dto::ItemChangeSets,
    ) -> Result<ReleaseDTO, anyhow::Error> {
        let item_set_service = crate::service::ItemSetService::new(self.persistence.clone());
        item_set_service.update_set(app_id, cluster_name, namespace_name, change_sets).await?;

        let namespace = self.persistence.get_by_app_cluster(app_id, cluster_name, namespace_name).await?
            .ok_or_else(|| anyhow::anyhow!("Namespace not found: {}/{}/{}", app_id, cluster_name, namespace_name))?;

        let items = <dyn ItemPersistence>::list_by_namespace(&self.persistence, namespace.id).await?;

        let mut configurations: HashMap<String, String> = HashMap::new();
        for item in items {
            configurations.insert(item.key, item.value);
        }
        let configurations_json = serde_json::to_string(&configurations)?;

        let now = Utc::now().timestamp_millis();
        let release_id = now;
        let release_key = format!("{}+{}+{}+{}+merge", app_id, cluster_name, namespace_name, release_id);

        let stored = StoredRelease {
            id: 0,
            release_key: release_key.clone(),
            name: release_name.to_string(),
            comment: release_comment,
            app_id: app_id.to_string(),
            cluster_name: cluster_name.to_string(),
            namespace_name: namespace_name.to_string(),
            configurations: configurations_json,
            release_id: Some(release_id),
            is_abandoned: false,
            is_deleted: false,
            deleted_at: 0,
            data_change_created_by: operator.to_string(),
            data_change_created_time: now,
            data_change_last_modified_by: None,
            data_change_last_time: None,
        };

        let created = <dyn ReleasePersistence>::create(&self.persistence, stored).await?;
        Self::notify_publish(self, app_id, cluster_name, namespace_name).await;
        // Upstream operation: GRAY_RELEASE_MERGE_TO_MASTER with context
        // {sourceBranch, baseReleaseId}.
        let context = serde_json::json!({
            "sourceBranch": _branch_name,
            "baseReleaseId": 0,
        })
        .to_string();
        self.record_history(app_id, cluster_name, namespace_name, namespace_name, created.id, 0,
            crate::service::namespace_branch_service::release_operation::GRAY_RELEASE_MERGE_TO_MASTER,
            operator, &context).await;

        Ok(created.into())
    }

    /// Merge + optional branch deletion wrapper used by the admin route.
    pub async fn merge_branch_and_release_full(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
        branch_name: &str,
        release_name: &str,
        release_comment: Option<String>,
        operator: &str,
        is_emergency_publish: bool,
        change_sets: crate::api::dto::ItemChangeSets,
        delete_branch: bool,
    ) -> Result<ReleaseDTO, anyhow::Error> {
        use crate::service::namespace_branch_service::{NamespaceBranchService, BRANCH_STATUS_MERGED};
        let merged = self
            .merge_branch_and_release(
                app_id, cluster_name, namespace_name, branch_name, release_name,
                release_comment, operator, is_emergency_publish, change_sets,
            )
            .await?;
        if delete_branch {
            let svc = NamespaceBranchService::new(self.persistence.clone());
            // Upstream portal passes the PARENT cluster; status MERGED(2).
            let _ = BRANCH_STATUS_MERGED;
            svc.delete_branch(app_id, cluster_name, namespace_name, operator, true)
                .await?;
        }
        Ok(merged)
    }

    /// Returns the requested value.
    pub async fn find_release_history(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
        page: u64,
        size: u64,
    ) -> Result<(Vec<ReleaseHistoryDTO>, u64), anyhow::Error> {
        let (models, total) = self
            .persistence
            .find_release_history(app_id, cluster_name, namespace_name, page, size)
            .await?;
        let dtos = models
            .into_iter()
            .map(|m| ReleaseHistoryDTO {
                id: Some(m.id),
                app_id: m.app_id,
                cluster_name: m.cluster_name,
                namespace_name: m.namespace_name,
                branch_name: m.branch_name,
                release_id: m.release_id,
                previous_release_id: m.previous_release_id,
                operation: m.operation,
                operation_context: m.operation_context,
                data_change_created_by: Some(m.data_change_created_by),
                data_change_created_time: Some(
                    m.data_change_created_time
                        .format("%Y-%m-%dT%H:%M:%S%.f+00:00")
                        .to_string(),
                ),
            })
            .collect();
        Ok((dtos, total))
    }

    /// Record a release operation into the release history table.
    pub(crate) async fn record_history(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
        branch_name: &str,
        release_id: i64,
        previous_release_id: i64,
        operation: i32,
        operator: &str,
        context: &str,
    ) {
        let _ = self
            .persistence
            .record_release_history(
                app_id,
                cluster_name,
                namespace_name,
                branch_name,
                release_id,
                previous_release_id,
                operation,
                context,
                operator,
            )
            .await;
    }
}

impl From<StoredRelease> for ReleaseDTO {
    fn from(stored: StoredRelease) -> Self {
        Self {
            id: Some(stored.id),
            release_key: stored.release_key,
            name: stored.name,
            comment: stored.comment,
            app_id: stored.app_id,
            cluster_name: stored.cluster_name,
            namespace_name: stored.namespace_name,
            configurations: Some(stored.configurations),
            release_id: stored.release_id,
            is_abandoned: Some(stored.is_abandoned),
            data_change_created_by: Some(stored.data_change_created_by),
            data_change_created_time: Some(format_timestamp(stored.data_change_created_time)),
        }
    }
}

fn format_timestamp(ts: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ts)
        .unwrap_or_default()
        .format("%Y-%m-%dT%H:%M:%S%.f+00:00")
        .to_string()
}