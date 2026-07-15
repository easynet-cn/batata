use std::sync::{Arc, Mutex};
use std::collections::HashMap;

use crate::persistence::shared::StoredReleaseMessage;
use crate::persistence::traits::{ApolloPersistenceService, ReleaseMessagePersistence};
use chrono::Utc;
use serde_json::json;

pub struct ReleaseMessageService {
    persistence: Arc<dyn ApolloPersistenceService>,
    pending_messages: Arc<Mutex<HashMap<String, Vec<ReleaseMessage>>>>,
}

#[derive(Debug, Clone)]
pub struct ReleaseMessage {
    pub app_id: String,
    pub cluster: String,
    pub namespace_name: String,
    pub notification_id: i64,
    pub message: String,
}

impl ReleaseMessageService {
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self {
            persistence,
            pending_messages: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn publish(&self, app_id: &str, cluster: &str, namespace_name: &str, notification_id: i64) -> Result<(), anyhow::Error> {
        let message_json = json!({
            "appId": app_id,
            "cluster": cluster,
            "namespaceName": namespace_name,
            "notificationId": notification_id,
            "dataChangeCreatedTime": Utc::now().to_rfc3339(),
        }).to_string();

        let stored = StoredReleaseMessage {
            id: 0,
            message: message_json.clone(),
            data_change_created_time: Utc::now().timestamp_millis(),
        };

        self.persistence.create(stored).await?;

        let cache_key = format!("{}_{}_{}", app_id, cluster, namespace_name);
        let message = ReleaseMessage {
            app_id: app_id.to_string(),
            cluster: cluster.to_string(),
            namespace_name: namespace_name.to_string(),
            notification_id,
            message: message_json,
        };

        let mut pending = self.pending_messages.lock().unwrap();
        pending.entry(cache_key).or_insert_with(Vec::new).push(message);

        Ok(())
    }

    pub async fn poll(&self, app_id: &str, cluster: &str, namespace_name: &str, last_notification_id: i64) -> Result<Option<ReleaseMessage>, anyhow::Error> {
        let cache_key = format!("{}_{}_{}", app_id, cluster, namespace_name);
        let mut pending = self.pending_messages.lock().unwrap();

        if let Some(messages) = pending.get_mut(&cache_key) {
            if let Some(index) = messages.iter().position(|m| m.notification_id > last_notification_id) {
                let message = messages.remove(index);
                return Ok(Some(message));
            }
        }

        let latest = self.persistence.get_latest().await?;

        if let Some(model) = latest {
            let message: serde_json::Value = serde_json::from_str(&model.message).unwrap_or_default();
            let msg_app_id = message.get("appId").and_then(|v| v.as_str());
            let msg_cluster = message.get("cluster").and_then(|v| v.as_str());
            let msg_namespace = message.get("namespaceName").and_then(|v| v.as_str());
            let notification_id = message.get("notificationId").and_then(|v| v.as_i64()).unwrap_or(0);

            if msg_app_id == Some(app_id) && msg_cluster == Some(cluster) && msg_namespace == Some(namespace_name) {
                if notification_id > last_notification_id {
                    return Ok(Some(ReleaseMessage {
                        app_id: app_id.to_string(),
                        cluster: cluster.to_string(),
                        namespace_name: namespace_name.to_string(),
                        notification_id,
                        message: model.message,
                    }));
                }
            }
        }

        Ok(None)
    }

    pub async fn get_all_messages(&self, _limit: u64) -> Result<Vec<ReleaseMessage>, anyhow::Error> {
        let stored_list = self.persistence.list_all().await?;

        Ok(stored_list.into_iter().filter_map(|model| {
            let message: serde_json::Value = serde_json::from_str(&model.message).unwrap_or_default();
            Some(ReleaseMessage {
                app_id: message.get("appId").and_then(|v| v.as_str())?.to_string(),
                cluster: message.get("cluster").and_then(|v| v.as_str())?.to_string(),
                namespace_name: message.get("namespaceName").and_then(|v| v.as_str())?.to_string(),
                notification_id: message.get("notificationId").and_then(|v| v.as_i64()).unwrap_or(0),
                message: model.message,
            })
        }).collect())
    }

    pub async fn cleanup_old_messages(&self, hours: i64) -> Result<u64, anyhow::Error> {
        let cutoff_timestamp = Utc::now().timestamp_millis() - (hours * 3600 * 1000);
        let stored_list = self.persistence.list_all().await?;
        
        let mut deleted_count = 0;
        for stored in stored_list {
            if stored.data_change_created_time < cutoff_timestamp {
                self.persistence.delete_old(stored.id).await?;
                deleted_count += 1;
            }
        }

        Ok(deleted_count)
    }
}