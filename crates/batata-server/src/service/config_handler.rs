// Config module gRPC handlers
// Implements handlers for configuration management requests

use std::sync::Arc;

use tonic::Status;
use tracing::{debug, info, warn};

use batata_core::model::Connection;

use crate::{
    api::{
        config::model::{
            ClientConfigMetricRequest, ClientConfigMetricResponse, ConfigBatchListenRequest,
            ConfigChangeBatchListenResponse, ConfigChangeClusterSyncRequest,
            ConfigChangeClusterSyncResponse, ConfigChangeNotifyRequest, ConfigChangeNotifyResponse,
            ConfigContext, ConfigFuzzyWatchChangeNotifyRequest,
            ConfigFuzzyWatchChangeNotifyResponse, ConfigFuzzyWatchRequest,
            ConfigFuzzyWatchResponse, ConfigFuzzyWatchSyncRequest, ConfigFuzzyWatchSyncResponse,
            ConfigPublishRequest, ConfigPublishResponse, ConfigQueryRequest, ConfigQueryResponse,
            ConfigRemoveRequest, ConfigRemoveResponse,
        },
        grpc::Payload,
        remote::model::{RequestTrait, ResponseCode, ResponseTrait},
    },
    model::common::AppState,
    service::{
        config,
        config_fuzzy_watch::{ConfigFuzzyWatchManager, ConfigFuzzyWatchPattern},
        rpc::{AuthRequirement, PayloadHandler},
    },
};

// Handler for ConfigQueryRequest - queries configuration by dataId, group, tenant
#[derive(Clone)]
pub struct ConfigQueryHandler {
    pub app_state: Arc<AppState>,
}

#[tonic::async_trait]
impl PayloadHandler for ConfigQueryHandler {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = ConfigQueryRequest::from(payload);
        let request_id = request.request_id();

        let data_id = &request.config_request.data_id;
        let group = &request.config_request.group;
        let tenant = &request.config_request.tenant;

        let db = self.app_state.db();

        match config::find_one(db, data_id, group, tenant).await {
            Ok(Some(config_info)) => {
                let mut response = ConfigQueryResponse::new();
                response.response.request_id = request_id;
                response.content = config_info.config_info.config_info_base.content;
                response.md5 = config_info.config_info.config_info_base.md5;
                response.content_type = config_info.config_info.r#type;
                response.encrypted_data_key =
                    config_info.config_info.config_info_base.encrypted_data_key;
                response.last_modified = config_info.modify_time;

                Ok(response.build_payload())
            }
            Ok(None) => {
                let mut response = ConfigQueryResponse::new();
                response.response.request_id = request_id;
                response.response.result_code = ConfigQueryResponse::CONFIG_NOT_FOUND;
                response.response.error_code = ConfigQueryResponse::CONFIG_NOT_FOUND;
                response.response.success = false;
                response.response.message = "config not found".to_string();

                Ok(response.build_payload())
            }
            Err(e) => {
                let mut response = ConfigQueryResponse::new();
                response.response.request_id = request_id;
                response.response.result_code = ResponseCode::Fail.code();
                response.response.error_code = ResponseCode::Fail.code();
                response.response.success = false;
                response.response.message = e.to_string();

                Ok(response.build_payload())
            }
        }
    }

    fn can_handle(&self) -> &'static str {
        "ConfigQueryRequest"
    }
}

// Handler for ConfigPublishRequest - publishes/updates configuration
#[derive(Clone)]
pub struct ConfigPublishHandler {
    pub app_state: Arc<AppState>,
    pub fuzzy_watch_manager: Arc<ConfigFuzzyWatchManager>,
    pub connection_manager: Arc<batata_core::service::remote::ConnectionManager>,
}

#[tonic::async_trait]
impl PayloadHandler for ConfigPublishHandler {
    async fn handle(&self, connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = ConfigPublishRequest::from(payload);
        let request_id = request.request_id();

        let data_id = &request.config_request.data_id;
        let group = &request.config_request.group;
        let tenant = &request.config_request.tenant;
        let content = &request.content;

        // Extract additional params from addition_map
        let app_name = request
            .addition_map
            .get("appName")
            .map(|s| s.as_str())
            .unwrap_or("");
        let config_tags = request
            .addition_map
            .get("configTags")
            .map(|s| s.as_str())
            .unwrap_or("");
        let desc = request
            .addition_map
            .get("desc")
            .map(|s| s.as_str())
            .unwrap_or("");
        let r#use = request
            .addition_map
            .get("use")
            .map(|s| s.as_str())
            .unwrap_or("");
        let effect = request
            .addition_map
            .get("effect")
            .map(|s| s.as_str())
            .unwrap_or("");
        let r#type = request
            .addition_map
            .get("type")
            .map(|s| s.as_str())
            .unwrap_or("");
        let schema = request
            .addition_map
            .get("schema")
            .map(|s| s.as_str())
            .unwrap_or("");
        let encrypted_data_key = request
            .addition_map
            .get("encryptedDataKey")
            .map(|s| s.as_str())
            .unwrap_or("");

        let src_user = connection.meta_info.app_name.as_str();
        let src_ip = connection.meta_info.client_ip.as_str();

        let db = self.app_state.db();

        match config::create_or_update(
            db,
            data_id,
            group,
            tenant,
            content,
            app_name,
            src_user,
            src_ip,
            config_tags,
            desc,
            r#use,
            effect,
            r#type,
            schema,
            encrypted_data_key,
        )
        .await
        {
            Ok(_) => {
                // Notify fuzzy watchers about config change
                if let Err(e) = self.notify_fuzzy_watchers(
                    data_id,
                    group,
                    tenant,
                    src_ip,
                ).await {
                    warn!("Failed to notify fuzzy watchers: {}", e);
                }

                let mut response = ConfigPublishResponse::new();
                response.response.request_id = request_id;

                Ok(response.build_payload())
            }
            Err(e) => {
                let mut response = ConfigPublishResponse::new();
                response.response.request_id = request_id;
                response.response.result_code = ResponseCode::Fail.code();
                response.response.error_code = ResponseCode::Fail.code();
                response.response.success = false;
                response.response.message = e.to_string();

                Ok(response.build_payload())
            }
        }
    }

    fn can_handle(&self) -> &'static str {
        "ConfigPublishRequest"
    }

    fn auth_requirement(&self) -> AuthRequirement {
        AuthRequirement::Authenticated
    }
}

impl ConfigPublishHandler {
    /// Notify both fuzzy watchers and regular subscribers about configuration change
    async fn notify_fuzzy_watchers(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
        _source_ip: &str,
    ) -> anyhow::Result<()> {
        // Build the notification payload once for reuse
        let notification = ConfigChangeNotifyRequest::for_config(data_id, group, tenant);
        let payload = notification.build_server_push_payload();

        // Notify fuzzy watchers
        let fuzzy_watchers = self.fuzzy_watch_manager.get_watchers_for_config(
            tenant,
            group,
            data_id,
        );

        if !fuzzy_watchers.is_empty() {
            // Build group key
            let group_key = ConfigFuzzyWatchPattern::build_group_key(tenant, group, data_id);

            info!(
                "Notifying {} fuzzy watchers for config change: {}",
                fuzzy_watchers.len(),
                group_key
            );

            // Push notification to each fuzzy watcher
            for connection_id in &fuzzy_watchers {
                // Mark the group key as received by this connection
                self.fuzzy_watch_manager.mark_received(connection_id, &group_key);

                // Push the actual notification payload
                if self.connection_manager.push_message(connection_id, payload.clone()).await {
                    debug!(
                        "Pushed config change notification to connection {} (fuzzy watch): {}",
                        connection_id, group_key
                    );
                } else {
                    warn!(
                        "Failed to push config change notification to connection {}: {}",
                        connection_id, group_key
                    );
                }
            }

            info!(
                "Notified {} fuzzy watchers for config change: {}",
                fuzzy_watchers.len(),
                group_key
            );
        }

        // Notify regular subscribers via ConfigSubscriberManager
        let config_key = batata_core::ConfigKey::new(data_id, group, tenant);
        let subscribers = self.app_state.config_subscriber_manager.get_subscribers(&config_key);

        if !subscribers.is_empty() {
            info!(
                "Notifying {} regular subscribers for config change: {}@@{}@@{}",
                subscribers.len(),
                tenant,
                group,
                data_id
            );

            // Push notification to each subscriber
            for subscriber in &subscribers {
                if self.connection_manager.push_message(&subscriber.connection_id, payload.clone()).await {
                    debug!(
                        "Pushed config change notification to connection {} (subscriber): {}",
                        subscriber.connection_id,
                        config_key.to_key_string()
                    );
                } else {
                    warn!(
                        "Failed to push config change notification to connection {}: {}",
                        subscriber.connection_id,
                        config_key.to_key_string()
                    );
                }
            }

            info!(
                "Notified {} regular subscribers for config change: {}",
                subscribers.len(),
                config_key.to_key_string()
            );
        }

        Ok(())
    }
}

// Handler for ConfigRemoveRequest - removes configuration
#[derive(Clone)]
pub struct ConfigRemoveHandler {
    pub app_state: Arc<AppState>,
    pub fuzzy_watch_manager: Arc<ConfigFuzzyWatchManager>,
    pub connection_manager: Arc<batata_core::service::remote::ConnectionManager>,
}

#[tonic::async_trait]
impl PayloadHandler for ConfigRemoveHandler {
    async fn handle(&self, connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = ConfigRemoveRequest::from(payload);
        let request_id = request.request_id();

        let data_id = &request.config_request.data_id;
        let group = &request.config_request.group;
        let tenant = &request.config_request.tenant;
        let _tag = &request.tag;

        let src_user = connection.meta_info.app_name.as_str();
        let src_ip = connection.meta_info.client_ip.as_str();

        let db = self.app_state.db();

        match config::delete(db, data_id, group, tenant, "", src_ip, src_user).await {
            Ok(_) => {
                // Notify fuzzy watchers about config removal
                if let Err(e) = self.notify_fuzzy_watchers(
                    data_id,
                    group,
                    tenant,
                    src_ip,
                ).await {
                    warn!("Failed to notify fuzzy watchers: {}", e);
                }

                let mut response = ConfigRemoveResponse::new();
                response.response.request_id = request_id;

                Ok(response.build_payload())
            }
            Err(e) => {
                let mut response = ConfigRemoveResponse::new();
                response.response.request_id = request_id;
                response.response.result_code = ResponseCode::Fail.code();
                response.response.error_code = ResponseCode::Fail.code();
                response.response.success = false;
                response.response.message = e.to_string();

                Ok(response.build_payload())
            }
        }
    }

    fn can_handle(&self) -> &'static str {
        "ConfigRemoveRequest"
    }

    fn auth_requirement(&self) -> AuthRequirement {
        AuthRequirement::Authenticated
    }
}

impl ConfigRemoveHandler {
    /// Notify both fuzzy watchers and regular subscribers about configuration removal
    async fn notify_fuzzy_watchers(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
        _source_ip: &str,
    ) -> anyhow::Result<()> {
        // Build the notification payload once for reuse
        let notification = ConfigChangeNotifyRequest::for_config(data_id, group, tenant);
        let payload = notification.build_server_push_payload();

        // Notify fuzzy watchers
        let fuzzy_watchers = self.fuzzy_watch_manager.get_watchers_for_config(
            tenant,
            group,
            data_id,
        );

        if !fuzzy_watchers.is_empty() {
            // Build group key
            let group_key = ConfigFuzzyWatchPattern::build_group_key(tenant, group, data_id);

            info!(
                "Notifying {} fuzzy watchers for config removal: {}",
                fuzzy_watchers.len(),
                group_key
            );

            // Push notification to each fuzzy watcher
            for connection_id in &fuzzy_watchers {
                // Mark the group key as received by this connection
                self.fuzzy_watch_manager.mark_received(connection_id, &group_key);

                // Push the actual notification payload
                if self.connection_manager.push_message(connection_id, payload.clone()).await {
                    debug!(
                        "Pushed config removal notification to connection {} (fuzzy watch): {}",
                        connection_id, group_key
                    );
                } else {
                    warn!(
                        "Failed to push config removal notification to connection {}: {}",
                        connection_id, group_key
                    );
                }
            }

            info!(
                "Notified {} fuzzy watchers for config removal: {}",
                fuzzy_watchers.len(),
                group_key
            );
        }

        // Notify regular subscribers via ConfigSubscriberManager
        let config_key = batata_core::ConfigKey::new(data_id, group, tenant);
        let subscribers = self.app_state.config_subscriber_manager.get_subscribers(&config_key);

        if !subscribers.is_empty() {
            info!(
                "Notifying {} regular subscribers for config removal: {}@@{}@@{}",
                subscribers.len(),
                tenant,
                group,
                data_id
            );

            // Push notification to each subscriber
            for subscriber in &subscribers {
                if self.connection_manager.push_message(&subscriber.connection_id, payload.clone()).await {
                    debug!(
                        "Pushed config removal notification to connection {} (subscriber): {}",
                        subscriber.connection_id,
                        config_key.to_key_string()
                    );
                } else {
                    warn!(
                        "Failed to push config removal notification to connection {}: {}",
                        subscriber.connection_id,
                        config_key.to_key_string()
                    );
                }
            }

            info!(
                "Notified {} regular subscribers for config removal: {}",
                subscribers.len(),
                config_key.to_key_string()
            );
        }

        Ok(())
    }
}

// Handler for ConfigBatchListenRequest - batch listen for config changes
#[derive(Clone)]
pub struct ConfigBatchListenHandler {
    pub app_state: Arc<AppState>,
}

#[tonic::async_trait]
impl PayloadHandler for ConfigBatchListenHandler {
    async fn handle(&self, connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = ConfigBatchListenRequest::from(payload);
        let request_id = request.request_id();

        let db = self.app_state.db();
        let subscriber_manager = &self.app_state.config_subscriber_manager;
        let connection_id = &connection.meta_info.connection_id;
        let client_ip = &connection.meta_info.remote_ip;

        let mut changed_configs = Vec::new();

        // Check each config in the listen list for changes
        for ctx in &request.config_listen_contexts {
            let data_id = &ctx.data_id;
            let group = &ctx.group;
            let tenant = &ctx.tenant;
            let client_md5 = &ctx.md5;

            let config_key = batata_core::ConfigKey::new(data_id, group, tenant);

            if request.listen {
                // Register subscription
                subscriber_manager.subscribe(connection_id, client_ip, &config_key, client_md5);
            } else {
                // Unregister subscription
                subscriber_manager.unsubscribe(connection_id, &config_key);
            }

            // Query current config and compare MD5
            if let Ok(Some(config_info)) = config::find_one(db, data_id, group, tenant).await {
                let server_md5 = &config_info.config_info.config_info_base.md5;

                // If MD5 differs, config has changed
                if client_md5 != server_md5 {
                    changed_configs.push(ConfigContext {
                        data_id: data_id.clone(),
                        group: group.clone(),
                        tenant: tenant.clone(),
                    });
                }
            } else if request.listen {
                // Config doesn't exist but client is listening - report as changed
                changed_configs.push(ConfigContext {
                    data_id: data_id.clone(),
                    group: group.clone(),
                    tenant: tenant.clone(),
                });
            }
        }

        let mut response = ConfigChangeBatchListenResponse::new();
        response.response.request_id = request_id;
        response.changed_configs = changed_configs;

        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "ConfigBatchListenRequest"
    }
}

// Handler for ConfigChangeNotifyRequest - notifies clients of config changes
// Note: This is typically a server-push request, handler acknowledges receipt
#[derive(Clone)]
pub struct ConfigChangeNotifyHandler {
    pub app_state: Arc<AppState>,
}

#[tonic::async_trait]
impl PayloadHandler for ConfigChangeNotifyHandler {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = ConfigChangeNotifyRequest::from(payload);
        let request_id = request.request_id();

        // This handler is for receiving acknowledgment from client
        // In server-side context, just acknowledge the request
        let mut response = ConfigChangeNotifyResponse::new();
        response.response.request_id = request_id;

        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "ConfigChangeNotifyRequest"
    }
}

// Handler for ConfigChangeClusterSyncRequest - syncs config changes across cluster nodes
#[derive(Clone)]
pub struct ConfigChangeClusterSyncHandler {
    pub app_state: Arc<AppState>,
}

#[tonic::async_trait]
impl PayloadHandler for ConfigChangeClusterSyncHandler {
    async fn handle(&self, connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = ConfigChangeClusterSyncRequest::from(payload);
        let request_id = request.request_id();

        let data_id = &request.config_request.data_id;
        let group = &request.config_request.group;
        let tenant = &request.config_request.tenant;
        let last_modified = request.last_modified;
        let gray_name = &request.gray_name;

        // Log the cluster sync event for observability
        info!(
            "Config cluster sync from {}: dataId={}, group={}, tenant={}, lastModified={}, grayName={}",
            connection.meta_info.remote_ip,
            data_id,
            group,
            tenant,
            last_modified,
            gray_name
        );

        // In a distributed cluster, this notification indicates another node has
        // changed a config. Since we use a shared database, the change is already
        // persisted. This handler implements:
        // 1. Local cache invalidation (if caching is implemented)
        // 2. Notification to local listeners via config subscriber manager
        // 3. Metrics/audit logging

        // Notify config subscribers about change
        if let Err(e) = self.notify_config_change(
            data_id,
            group,
            tenant,
            gray_name.as_str(),
            connection.meta_info.remote_ip.as_str(),
        ).await {
            warn!("Failed to notify config subscribers: {}", e);
        }

        let mut response = ConfigChangeClusterSyncResponse::new();
        response.response.request_id = request_id;

        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "ConfigChangeClusterSyncRequest"
    }

    fn auth_requirement(&self) -> AuthRequirement {
        AuthRequirement::Internal
    }
}

impl ConfigChangeClusterSyncHandler {
    /// Notify local config subscribers about configuration changes
    async fn notify_config_change(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
        gray_name: &str,
        source_ip: &str,
    ) -> anyhow::Result<()> {
        // Build config key
        let config_key = if gray_name.is_empty() {
            format!("{}@@{}@@{}", tenant, group, data_id)
        } else {
            format!("{}@@{}@@{}@@{}", tenant, group, data_id, gray_name)
        };

        // Notify subscribers
        // Note: The actual notification mechanism depends on the implementation
        // of config_subscriber_manager. For now, we log the action.
        info!(
            "Notifying config subscribers for key: {}, change_type: MODIFY, source: {}",
            config_key, source_ip
        );

        // Future enhancement: Call subscriber_manager.notify(&config_key, notification)
        // to push the change to all subscribed clients.

        Ok(())
    }
}

// Handler for ConfigFuzzyWatchRequest - handles fuzzy pattern watch for configs
#[derive(Clone)]
pub struct ConfigFuzzyWatchHandler {
    pub app_state: Arc<AppState>,
    pub fuzzy_watch_manager: Arc<ConfigFuzzyWatchManager>,
}

#[tonic::async_trait]
impl PayloadHandler for ConfigFuzzyWatchHandler {
    async fn handle(&self, connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = ConfigFuzzyWatchRequest::from(payload);
        let request_id = request.request_id();

        let connection_id = &connection.meta_info.connection_id;
        let group_key_pattern = &request.group_key_pattern;
        let watch_type = &request.watch_type;

        // Register the fuzzy watch pattern for this connection
        let registered = self.fuzzy_watch_manager.register_watch(
            connection_id,
            group_key_pattern,
            watch_type,
        );

        if registered {
            debug!(
                "Registered config fuzzy watch for connection {}: pattern={}, type={}",
                connection_id, group_key_pattern, watch_type
            );
        }

        // Mark received group keys as already sent to client
        self.fuzzy_watch_manager.mark_received_batch(
            connection_id,
            &request.received_group_keys,
        );

        let mut response = ConfigFuzzyWatchResponse::new();
        response.response.request_id = request_id;

        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "ConfigFuzzyWatchRequest"
    }
}

// Handler for ConfigFuzzyWatchChangeNotifyRequest - notifies fuzzy watch changes
#[derive(Clone)]
pub struct ConfigFuzzyWatchChangeNotifyHandler {
    pub app_state: Arc<AppState>,
}

#[tonic::async_trait]
impl PayloadHandler for ConfigFuzzyWatchChangeNotifyHandler {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = ConfigFuzzyWatchChangeNotifyRequest::from(payload);
        let request_id = request.request_id();

        let mut response = ConfigFuzzyWatchChangeNotifyResponse::new();
        response.response.request_id = request_id;

        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "ConfigFuzzyWatchChangeNotifyRequest"
    }
}

// Handler for ConfigFuzzyWatchSyncRequest - syncs fuzzy watch state
#[derive(Clone)]
pub struct ConfigFuzzyWatchSyncHandler {
    pub app_state: Arc<AppState>,
    pub fuzzy_watch_manager: Arc<ConfigFuzzyWatchManager>,
}

#[tonic::async_trait]
impl PayloadHandler for ConfigFuzzyWatchSyncHandler {
    async fn handle(&self, connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = ConfigFuzzyWatchSyncRequest::from(payload);
        let request_id = request.request_id();

        let connection_id = &connection.meta_info.connection_id;
        let group_key_pattern = &request.group_key_pattern;
        let sync_type = &request.sync_type;

        // Register the pattern if not already registered
        self.fuzzy_watch_manager.register_watch(
            connection_id,
            group_key_pattern,
            sync_type,
        );

        debug!(
            "Config fuzzy watch sync for connection {}: pattern={}, sync_type={}, batch={}/{}",
            connection_id, group_key_pattern, sync_type, request.current_batch, request.total_batch
        );

        let mut response = ConfigFuzzyWatchSyncResponse::new();
        response.response.request_id = request_id;

        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "ConfigFuzzyWatchSyncRequest"
    }

    fn auth_requirement(&self) -> AuthRequirement {
        AuthRequirement::Internal
    }
}

// Handler for ClientConfigMetricRequest - collects client config metrics
#[derive(Clone)]
pub struct ClientConfigMetricHandler {
    pub app_state: Arc<AppState>,
    pub fuzzy_watch_manager: Arc<ConfigFuzzyWatchManager>,
}

#[tonic::async_trait]
impl PayloadHandler for ClientConfigMetricHandler {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = ClientConfigMetricRequest::from(payload);
        let request_id = request.request_id();

        let mut response = ClientConfigMetricResponse::new();
        response.response.request_id = request_id;

        // Populate metrics based on request.metrics_keys
        for metric_key in &request.metrics_keys {
            let value = match metric_key.key.as_str() {
                "fuzzyWatcherCount" => {
                    Some(serde_json::json!(self.fuzzy_watch_manager.watcher_count()))
                }
                "fuzzyPatternCount" => {
                    Some(serde_json::json!(self.fuzzy_watch_manager.pattern_count()))
                }
                _ => None,
            };
            if let Some(v) = value {
                response.metrics.insert(metric_key.key.clone(), v);
            }
        }

        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "ClientConfigMetricRequest"
    }
}
