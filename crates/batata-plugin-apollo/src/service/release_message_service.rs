//! Port of upstream `DatabaseMessageSender` + `ReleaseMessageServiceWithCache`.
//!
//! A publish inserts one `ReleaseMessage` row whose content is the plain
//! `"appId+cluster+namespace"` watch key; the row's auto-increment id IS the
//! client-visible `notificationId` (globally monotonic, survives restarts).
//! The in-memory [`crate::service::notification_hub`] remains only as a
//! process-local wake-up accelerator on top of this persisted truth.

use std::collections::HashMap;
use std::sync::Arc;

use crate::persistence::shared::StoredReleaseMessage;
use crate::persistence::traits::{ApolloPersistenceService, ReleaseMessagePersistence};

/// Upstream `ConfigConsts.CLUSTER_NAMESPACE_SEPARATOR`.
pub const CLUSTER_NAMESPACE_SEPARATOR: char = '+';

/// Represents the `ReleaseMessageService` entity.
pub struct ReleaseMessageService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl ReleaseMessageService {
    /// Creates a new `ReleaseMessageService`.
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    /// Build the plain watch key: "appId+cluster+namespace".
    pub fn generate_message(app_id: &str, cluster: &str, namespace_name: &str) -> String {
        format!(
            "{}{}{}{}{}",
            app_id, CLUSTER_NAMESPACE_SEPARATOR, cluster, CLUSTER_NAMESPACE_SEPARATOR, namespace_name
        )
    }

    /// Upstream `DatabaseMessageSender.sendMessage`: persist one row and
    /// return it — `stored.id` is the notification id handed to clients.
    pub async fn send_message(
        &self,
        app_id: &str,
        cluster: &str,
        namespace_name: &str,
    ) -> anyhow::Result<StoredReleaseMessage> {
        let stored = StoredReleaseMessage {
            id: 0,
            message: Self::generate_message(app_id, cluster, namespace_name),
            data_change_created_time: chrono::Utc::now().timestamp_millis(),
        };
        self.persistence.create(stored).await
    }

    /// Latest persisted row for a single watch key.
    pub async fn find_latest_by_key(
        &self,
        key: &str,
    ) -> anyhow::Result<Option<StoredReleaseMessage>> {
        self.persistence.find_latest_by_message(key).await
    }

    /// Latest row per watch key (upstream
    /// `releaseMessageService.findLatestReleaseMessagesGroupByMessages`).
    pub async fn find_latest_by_keys(
        &self,
        keys: &[String],
    ) -> anyhow::Result<HashMap<String, StoredReleaseMessage>> {
        let mut out = HashMap::with_capacity(keys.len());
        for key in keys {
            if let Some(latest) = self.persistence.find_latest_by_message(key).await? {
                out.insert(key.clone(), latest);
            }
        }
        Ok(out)
    }

    /// Retain only the newest row per watch key globally (upstream cleanup
    /// thread keeps the last row per message). The embedded backend already
    /// prunes per-key on write; SQL accumulates history until invoked.
    pub async fn prune(&self) -> anyhow::Result<usize> {
        let all = self.persistence.list_all().await?;
        let mut newest_per_key: HashMap<&str, i64> = HashMap::new();
        for m in &all {
            let e = newest_per_key.entry(m.message.as_str()).or_insert(m.id);
            *e = (*e).max(m.id);
        }
        let mut deleted = 0;
        for m in &all {
            if newest_per_key.get(m.message.as_str()).copied() != Some(m.id) {
                self.persistence.delete_by_id(m.id).await?;
                deleted += 1;
            }
        }
        Ok(deleted)
    }
}
