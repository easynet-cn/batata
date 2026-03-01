// Config module gRPC handlers
// Implements handlers for configuration management requests

use std::collections::HashMap;
use std::sync::Arc;

use tonic::Status;
use tracing::{debug, info, warn};

use batata_core::{
    model::Connection,
    service::{
        cluster_client::{ClusterClientManager, ClusterRequestSender},
        remote::ConnectionManager,
    },
};
use batata_persistence::PersistenceService;

use crate::api::config_model::{
    ClientConfigMetricRequest, ClientConfigMetricResponse, ConfigBatchListenRequest,
    ConfigChangeBatchListenResponse, ConfigChangeClusterSyncRequest,
    ConfigChangeClusterSyncResponse, ConfigChangeNotifyRequest, ConfigChangeNotifyResponse,
    ConfigContext, ConfigFuzzyWatchChangeNotifyRequest, ConfigFuzzyWatchChangeNotifyResponse,
    ConfigFuzzyWatchRequest, ConfigFuzzyWatchResponse, ConfigFuzzyWatchSyncRequest,
    ConfigFuzzyWatchSyncResponse, ConfigPublishRequest, ConfigPublishResponse, ConfigQueryRequest,
    ConfigQueryResponse, ConfigRemoveRequest, ConfigRemoveResponse,
};
use crate::handler::config_fuzzy_watch::{ConfigFuzzyWatchManager, ConfigFuzzyWatchPattern};
use batata_api::grpc::Payload;
use batata_api::remote::model::{RequestTrait, ResponseCode, ResponseTrait};
use batata_common::error;
use batata_core::handler::rpc::{AuthRequirement, PayloadHandler};
use batata_server_common::model::AppState;

/// Build client labels from the connection metadata for gray rule matching.
fn build_client_labels(connection: &Connection) -> HashMap<String, String> {
    let mut labels = connection.meta_info.get_app_labels();
    // Add client IP as a label for beta (IP-based) gray rules
    let client_ip = if !connection.meta_info.client_ip.is_empty() {
        &connection.meta_info.client_ip
    } else {
        &connection.meta_info.remote_ip
    };
    labels.insert(
        crate::model::gray_rule::labels::CLIENT_IP.to_string(),
        client_ip.to_string(),
    );
    labels
}

/// Find a matching gray config for the given connection and config key.
/// Returns (content, md5, encrypted_data_key, last_modified) if a gray config matches.
async fn find_matching_gray_config(
    persistence: &dyn PersistenceService,
    data_id: &str,
    group: &str,
    tenant: &str,
    labels: &HashMap<String, String>,
) -> Option<(String, String, String, i64)> {
    let grays = match persistence
        .config_find_all_grays(data_id, group, tenant)
        .await
    {
        Ok(g) => g,
        Err(e) => {
            debug!(
                "Failed to find gray configs for {}/{}/{}: {}",
                data_id, group, tenant, e
            );
            return None;
        }
    };

    if grays.is_empty() {
        return None;
    }

    // Parse and sort by priority (higher priority first)
    let mut candidates: Vec<_> = grays
        .iter()
        .filter_map(|gray| {
            let rule = crate::model::gray_rule::parse_gray_rule(&gray.gray_rule)?;
            Some((gray, rule))
        })
        .collect();

    candidates.sort_by(|a, b| b.1.priority().cmp(&a.1.priority()));

    // Return the first matching gray config
    for (gray, rule) in candidates {
        if rule.matches(labels) {
            debug!(
                "Gray config matched: data_id={}, group={}, tenant={}, gray_name={}",
                data_id, group, tenant, gray.gray_name
            );
            return Some((
                gray.content.clone(),
                gray.md5.clone(),
                gray.encrypted_data_key.clone(),
                gray.modified_time,
            ));
        }
    }

    None
}

// Handler for ConfigQueryRequest - queries configuration by dataId, group, tenant
#[derive(Clone)]
pub struct ConfigQueryHandler {
    pub app_state: Arc<AppState>,
}

#[tonic::async_trait]
impl PayloadHandler for ConfigQueryHandler {
    async fn handle(
        &self,
        __connection: &Connection,
        payload: &Payload,
    ) -> Result<Payload, Status> {
        let request = ConfigQueryRequest::from(payload);
        let request_id = request.request_id();

        let data_id = &request.config_request.data_id;
        let group = &request.config_request.group;
        let tenant_raw = &request.config_request.tenant;
        let tenant = if tenant_raw.is_empty() {
            "public"
        } else {
            tenant_raw.as_str()
        };

        debug!(
            "ConfigQuery: data_id={}, group={}, tenant={}",
            data_id, group, tenant
        );

        let persistence = self.app_state.persistence();

        // Check for matching gray config first
        let labels = build_client_labels(__connection);
        if let Some((content, md5, encrypted_data_key, last_modified)) =
            find_matching_gray_config(persistence, data_id, group, tenant, &labels).await
        {
            let mut response = ConfigQueryResponse::new();
            response.response.request_id = request_id;
            response.content = content;
            response.md5 = md5;
            response.encrypted_data_key = encrypted_data_key;
            response.last_modified = last_modified;
            response.is_beta = true;
            return Ok(response.build_payload());
        }

        // Fall through to formal config query
        match persistence.config_find_one(data_id, group, tenant).await {
            Ok(Some(config)) => {
                let mut response = ConfigQueryResponse::new();
                response.response.request_id = request_id;
                response.content = config.content;
                response.md5 = config.md5;
                response.content_type = config.config_type;
                response.encrypted_data_key = config.encrypted_data_key;
                response.last_modified = config.modified_time;

                Ok(response.build_payload())
            }
            Ok(None) => {
                let mut response = ConfigQueryResponse::new();
                response.response.request_id = request_id;
                response.response.result_code = ResponseCode::Fail.code();
                response.response.error_code = ConfigQueryResponse::CONFIG_NOT_FOUND;
                response.response.message = "config data not exist".to_string();

                Ok(response.build_payload())
            }
            Err(e) => {
                let mut response = ConfigQueryResponse::new();
                response.response.request_id = request_id;
                response.response.result_code = ResponseCode::Fail.code();
                response.response.error_code = error::SERVER_ERROR.code;
                response.response.success = false;
                response.response.message = e.to_string();

                Ok(response.build_payload())
            }
        }
    }

    fn can_handle(&self) -> &'static str {
        "ConfigQueryRequest"
    }

    fn auth_requirement(&self) -> AuthRequirement {
        AuthRequirement::Read
    }

    fn sign_type(&self) -> &'static str {
        "config"
    }

    fn resource_type(&self) -> batata_core::ResourceType {
        batata_core::ResourceType::Config
    }
}

// Handler for ConfigPublishRequest - publishes/updates configuration
#[derive(Clone)]
pub struct ConfigPublishHandler {
    pub app_state: Arc<AppState>,
    pub fuzzy_watch_manager: Arc<ConfigFuzzyWatchManager>,
    pub connection_manager: Arc<ConnectionManager>,
    /// Cluster client manager for broadcasting config changes to other nodes
    pub cluster_client_manager: Option<Arc<ClusterClientManager>>,
}

#[tonic::async_trait]
impl PayloadHandler for ConfigPublishHandler {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = ConfigPublishRequest::from(payload);
        let request_id = request.request_id();

        let data_id = &request.config_request.data_id;
        let group = &request.config_request.group;
        let tenant_raw = &request.config_request.tenant;
        let tenant = if tenant_raw.is_empty() {
            "public"
        } else {
            tenant_raw.as_str()
        };
        let content = &request.content;

        // Validate required fields
        if data_id.is_empty() || group.is_empty() || content.is_empty() {
            let client_ip = payload
                .metadata
                .as_ref()
                .map(|m| m.client_ip.as_str())
                .unwrap_or("");
            warn!(
                "Invalid config publish request: data_id={}, group={}, content_length={}, client_ip={}",
                data_id,
                group,
                content.len(),
                client_ip
            );
            let mut response = ConfigPublishResponse::new();
            response.response.request_id = request_id;
            response.response.result_code = ResponseCode::Fail.code();
            response.response.error_code = error::PARAMETER_VALIDATE_ERROR.code;
            response.response.success = false;
            response.response.message = "data_id, group, and content are required".to_string();
            return Ok(response.build_payload());
        }

        // Extract additional params from addition_map - optimized single iteration
        let mut app_name = "";
        let mut config_tags = "";
        let mut desc = "";
        let mut r#use = "";
        let mut effect = "";
        let mut r#type = "";
        let mut schema = "";
        let mut encrypted_data_key = "";
        let mut src_user = "";
        let mut beta_ips = "";
        let mut tag = "";

        for (key, value) in &request.addition_map {
            match key.as_str() {
                "appName" => app_name = value.as_str(),
                "config_tags" | "configTags" => config_tags = value.as_str(),
                "desc" => desc = value.as_str(),
                "use" => r#use = value.as_str(),
                "effect" => effect = value.as_str(),
                "type" => r#type = value.as_str(),
                "schema" => schema = value.as_str(),
                "encryptedDataKey" => encrypted_data_key = value.as_str(),
                "src_user" => src_user = value.as_str(),
                "betaIps" => beta_ips = value.as_str(),
                "tag" => tag = value.as_str(),
                _ => {}
            }
        }

        let src_ip = payload
            .metadata
            .as_ref()
            .map(|m| m.client_ip.as_str())
            .unwrap_or("");

        debug!(
            "ConfigPublish: data_id={}, group={}, tenant={}, src_user={}, src_ip={}",
            data_id, group, tenant, src_user, src_ip
        );

        let persistence = self.app_state.persistence();

        // Handle gray (beta/tag) publish
        if !beta_ips.is_empty() {
            let gray_rule_info = crate::model::gray_rule::GrayRulePersistInfo::new_beta(
                beta_ips,
                crate::model::gray_rule::BetaGrayRule::PRIORITY,
            );
            let gray_rule = match gray_rule_info.to_json() {
                Ok(json) => json,
                Err(e) => {
                    let mut response = ConfigPublishResponse::new();
                    response.response.request_id = request_id;
                    response.response.result_code = ResponseCode::Fail.code();
                    response.response.error_code = error::SERVER_ERROR.code;
                    response.response.success = false;
                    response.response.message = format!("Failed to serialize gray rule: {}", e);
                    return Ok(response.build_payload());
                }
            };

            match persistence
                .config_create_or_update_gray(
                    data_id,
                    group,
                    tenant,
                    content,
                    "beta",
                    &gray_rule,
                    src_user,
                    src_ip,
                    app_name,
                    encrypted_data_key,
                )
                .await
            {
                Ok(_) => {
                    let mut response = ConfigPublishResponse::new();
                    response.response.request_id = request_id;
                    return Ok(response.build_payload());
                }
                Err(e) => {
                    let mut response = ConfigPublishResponse::new();
                    response.response.request_id = request_id;
                    response.response.result_code = ResponseCode::Fail.code();
                    response.response.error_code = error::SERVER_ERROR.code;
                    response.response.success = false;
                    response.response.message = e.to_string();
                    return Ok(response.build_payload());
                }
            }
        }

        if !tag.is_empty() {
            let gray_rule_info = crate::model::gray_rule::GrayRulePersistInfo::new_tag(
                tag,
                crate::model::gray_rule::TagGrayRule::PRIORITY,
            );
            let gray_rule = match gray_rule_info.to_json() {
                Ok(json) => json,
                Err(e) => {
                    let mut response = ConfigPublishResponse::new();
                    response.response.request_id = request_id;
                    response.response.result_code = ResponseCode::Fail.code();
                    response.response.error_code = error::SERVER_ERROR.code;
                    response.response.success = false;
                    response.response.message = format!("Failed to serialize gray rule: {}", e);
                    return Ok(response.build_payload());
                }
            };

            let gray_name = format!("tag_{}", tag);
            match persistence
                .config_create_or_update_gray(
                    data_id,
                    group,
                    tenant,
                    content,
                    &gray_name,
                    &gray_rule,
                    src_user,
                    src_ip,
                    app_name,
                    encrypted_data_key,
                )
                .await
            {
                Ok(_) => {
                    let mut response = ConfigPublishResponse::new();
                    response.response.request_id = request_id;
                    return Ok(response.build_payload());
                }
                Err(e) => {
                    let mut response = ConfigPublishResponse::new();
                    response.response.request_id = request_id;
                    response.response.result_code = ResponseCode::Fail.code();
                    response.response.error_code = error::SERVER_ERROR.code;
                    response.response.success = false;
                    response.response.message = e.to_string();
                    return Ok(response.build_payload());
                }
            }
        }

        // Normal (formal) config publish
        match persistence
            .config_create_or_update(
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
                if let Err(e) = self
                    .notify_fuzzy_watchers(data_id, group, tenant, src_ip)
                    .await
                {
                    warn!("Failed to notify fuzzy watchers: {}", e);
                }

                // Broadcast config change to other cluster nodes
                if let Some(ref ccm) = self.cluster_client_manager
                    && let Some(smm) = self.app_state.try_member_manager()
                    && !smm.is_standalone()
                {
                    let members = smm.all_members();
                    let sender = ClusterRequestSender::new(ccm.clone());
                    let data_id = data_id.to_string();
                    let group = group.to_string();
                    let tenant = tenant.to_string();
                    let last_modified = chrono::Utc::now().timestamp_millis();
                    tokio::spawn(async move {
                        sender
                            .broadcast_config_change(
                                &members,
                                &data_id,
                                &group,
                                &tenant,
                                last_modified,
                            )
                            .await;
                    });
                }

                let mut response = ConfigPublishResponse::new();
                response.response.request_id = request_id;

                Ok(response.build_payload())
            }
            Err(e) => {
                let mut response = ConfigPublishResponse::new();
                response.response.request_id = request_id;
                response.response.result_code = ResponseCode::Fail.code();
                response.response.error_code = error::SERVER_ERROR.code;
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
        AuthRequirement::Write
    }

    fn sign_type(&self) -> &'static str {
        "config"
    }

    fn resource_type(&self) -> batata_core::ResourceType {
        batata_core::ResourceType::Config
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
        let fuzzy_watchers = self
            .fuzzy_watch_manager
            .get_watchers_for_config(tenant, group, data_id);

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
                self.fuzzy_watch_manager
                    .mark_received(connection_id, &group_key);

                // Push the actual notification payload
                if self
                    .connection_manager
                    .push_message(connection_id, payload.clone())
                    .await
                {
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
        let subscribers = self
            .app_state
            .config_subscriber_manager
            .get_subscribers(&config_key);

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
                if self
                    .connection_manager
                    .push_message(&subscriber.connection_id, payload.clone())
                    .await
                {
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
    pub connection_manager: Arc<ConnectionManager>,
    /// Cluster client manager for broadcasting config removals to other nodes
    pub cluster_client_manager: Option<Arc<ClusterClientManager>>,
}

#[tonic::async_trait]
impl PayloadHandler for ConfigRemoveHandler {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = ConfigRemoveRequest::from(payload);
        let request_id = request.request_id();

        let data_id = &request.config_request.data_id;
        let group = &request.config_request.group;
        let tenant_raw = &request.config_request.tenant;
        let tenant = if tenant_raw.is_empty() {
            "public"
        } else {
            tenant_raw.as_str()
        };
        let tag = &request.tag;

        let src_user = "";
        let src_ip = payload
            .metadata
            .as_ref()
            .map(|m| m.client_ip.as_str())
            .unwrap_or("");

        let persistence = self.app_state.persistence();

        match persistence
            .config_delete(data_id, group, tenant, tag, src_ip, src_user)
            .await
        {
            Ok(_) => {
                // Notify fuzzy watchers about config removal
                if let Err(e) = self
                    .notify_fuzzy_watchers(data_id, group, tenant, src_ip)
                    .await
                {
                    warn!("Failed to notify fuzzy watchers: {}", e);
                }

                // Broadcast config removal to other cluster nodes
                if let Some(ref ccm) = self.cluster_client_manager
                    && let Some(smm) = self.app_state.try_member_manager()
                    && !smm.is_standalone()
                {
                    let members = smm.all_members();
                    let sender = ClusterRequestSender::new(ccm.clone());
                    let data_id = data_id.to_string();
                    let group = group.to_string();
                    let tenant = tenant.to_string();
                    let last_modified = chrono::Utc::now().timestamp_millis();
                    tokio::spawn(async move {
                        sender
                            .broadcast_config_change(
                                &members,
                                &data_id,
                                &group,
                                &tenant,
                                last_modified,
                            )
                            .await;
                    });
                }

                let mut response = ConfigRemoveResponse::new();
                response.response.request_id = request_id;

                Ok(response.build_payload())
            }
            Err(e) => {
                let mut response = ConfigRemoveResponse::new();
                response.response.request_id = request_id;
                response.response.result_code = ResponseCode::Fail.code();
                response.response.error_code = error::SERVER_ERROR.code;
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
        AuthRequirement::Write
    }

    fn sign_type(&self) -> &'static str {
        "config"
    }

    fn resource_type(&self) -> batata_core::ResourceType {
        batata_core::ResourceType::Config
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
        let fuzzy_watchers = self
            .fuzzy_watch_manager
            .get_watchers_for_config(tenant, group, data_id);

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
                self.fuzzy_watch_manager
                    .mark_received(connection_id, &group_key);

                // Push the actual notification payload
                if self
                    .connection_manager
                    .push_message(connection_id, payload.clone())
                    .await
                {
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
        let subscribers = self
            .app_state
            .config_subscriber_manager
            .get_subscribers(&config_key);

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
                if self
                    .connection_manager
                    .push_message(&subscriber.connection_id, payload.clone())
                    .await
                {
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
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = ConfigBatchListenRequest::from(payload);
        let request_id = request.request_id();

        let persistence = self.app_state.persistence();
        let subscriber_manager = &self.app_state.config_subscriber_manager;
        let connection_id = &_connection.meta_info.connection_id;
        let client_ip = &_connection.meta_info.remote_ip;

        // Build client labels once for gray matching
        let labels = build_client_labels(_connection);

        let mut changed_configs = Vec::new();

        // Check each config in the listen list for changes
        for ctx in &request.config_listen_contexts {
            let data_id = &ctx.data_id;
            let group = &ctx.group;
            let tenant = if ctx.tenant.is_empty() {
                "public"
            } else {
                &ctx.tenant
            };
            let client_md5 = &ctx.md5;

            let config_key = batata_core::ConfigKey::new(data_id, group, tenant);

            if request.listen {
                // Register subscription
                subscriber_manager.subscribe(connection_id, client_ip, &config_key, client_md5);
            } else {
                // Unregister subscription
                subscriber_manager.unsubscribe(connection_id, &config_key);
            }

            // First check for matching gray config
            if let Some((_, gray_md5, _, _)) =
                find_matching_gray_config(persistence, data_id, group, tenant, &labels).await
            {
                if client_md5 != &gray_md5 {
                    changed_configs.push(ConfigContext {
                        data_id: data_id.clone(),
                        group: group.clone(),
                        tenant: tenant.to_string(),
                    });
                }
                continue;
            }

            // Fall through to formal config comparison
            if let Ok(Some(config)) = persistence.config_find_one(data_id, group, tenant).await {
                let server_md5 = &config.md5;

                // If MD5 differs, config has changed
                if client_md5 != server_md5 {
                    changed_configs.push(ConfigContext {
                        data_id: data_id.clone(),
                        group: group.clone(),
                        tenant: tenant.to_string(),
                    });
                }
            } else if request.listen {
                // Config doesn't exist but client is listening - report as changed
                changed_configs.push(ConfigContext {
                    data_id: data_id.clone(),
                    group: group.clone(),
                    tenant: tenant.to_string(),
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

    fn auth_requirement(&self) -> AuthRequirement {
        AuthRequirement::Read
    }

    fn sign_type(&self) -> &'static str {
        "config"
    }

    fn resource_type(&self) -> batata_core::ResourceType {
        batata_core::ResourceType::Config
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
    async fn handle(
        &self,
        __connection: &Connection,
        payload: &Payload,
    ) -> Result<Payload, Status> {
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
    /// Fuzzy watch manager for notifying local fuzzy watchers
    pub fuzzy_watch_manager: Arc<ConfigFuzzyWatchManager>,
    /// Connection manager for pushing notifications to local clients
    pub connection_manager: Arc<ConnectionManager>,
}

#[tonic::async_trait]
impl PayloadHandler for ConfigChangeClusterSyncHandler {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
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
            _connection.meta_info.remote_ip, data_id, group, tenant, last_modified, gray_name
        );

        // In a distributed cluster, this notification indicates another node has
        // changed a config. Since we use a shared database, the change is already
        // persisted. This handler implements:
        // 1. Local cache invalidation (if caching is implemented)
        // 2. Notification to local listeners via config subscriber manager
        // 3. Metrics/audit logging

        // Notify config subscribers about change
        if let Err(e) = self
            .notify_config_change(
                data_id,
                group,
                tenant,
                gray_name.as_str(),
                _connection.meta_info.remote_ip.as_str(),
            )
            .await
        {
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

    fn sign_type(&self) -> &'static str {
        "internal"
    }

    fn resource_type(&self) -> batata_core::ResourceType {
        batata_core::ResourceType::Internal
    }
}

impl ConfigChangeClusterSyncHandler {
    /// Notify local config subscribers and fuzzy watchers about configuration changes
    /// received from another cluster node. Does NOT re-broadcast to avoid infinite loops.
    async fn notify_config_change(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
        _gray_name: &str,
        source_ip: &str,
    ) -> anyhow::Result<()> {
        // Build the notification payload once for reuse
        let notification = ConfigChangeNotifyRequest::for_config(data_id, group, tenant);
        let payload = notification.build_server_push_payload();

        // Notify fuzzy watchers
        let fuzzy_watchers = self
            .fuzzy_watch_manager
            .get_watchers_for_config(tenant, group, data_id);

        if !fuzzy_watchers.is_empty() {
            let group_key = ConfigFuzzyWatchPattern::build_group_key(tenant, group, data_id);

            info!(
                "Cluster sync: notifying {} fuzzy watchers for config change: {} (source: {})",
                fuzzy_watchers.len(),
                group_key,
                source_ip
            );

            for connection_id in &fuzzy_watchers {
                self.fuzzy_watch_manager
                    .mark_received(connection_id, &group_key);

                if self
                    .connection_manager
                    .push_message(connection_id, payload.clone())
                    .await
                {
                    debug!(
                        "Pushed cluster sync config notification to connection {} (fuzzy watch): {}",
                        connection_id, group_key
                    );
                } else {
                    warn!(
                        "Failed to push cluster sync config notification to connection {}: {}",
                        connection_id, group_key
                    );
                }
            }
        }

        // Notify regular subscribers via ConfigSubscriberManager
        let config_key = batata_core::ConfigKey::new(data_id, group, tenant);
        let subscribers = self
            .app_state
            .config_subscriber_manager
            .get_subscribers(&config_key);

        if !subscribers.is_empty() {
            info!(
                "Cluster sync: notifying {} regular subscribers for config change: {} (source: {})",
                subscribers.len(),
                config_key.to_key_string(),
                source_ip
            );

            for subscriber in &subscribers {
                if self
                    .connection_manager
                    .push_message(&subscriber.connection_id, payload.clone())
                    .await
                {
                    debug!(
                        "Pushed cluster sync config notification to connection {} (subscriber): {}",
                        subscriber.connection_id,
                        config_key.to_key_string()
                    );
                } else {
                    warn!(
                        "Failed to push cluster sync config notification to connection {}: {}",
                        subscriber.connection_id,
                        config_key.to_key_string()
                    );
                }
            }
        }

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
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = ConfigFuzzyWatchRequest::from(payload);
        let request_id = request.request_id();

        let connection_id = &_connection.meta_info.connection_id;
        let group_key_pattern = &request.group_key_pattern;
        let watch_type = &request.watch_type;

        // Register the fuzzy watch pattern for this connection
        let registered =
            self.fuzzy_watch_manager
                .register_watch(connection_id, group_key_pattern, watch_type);

        if registered {
            debug!(
                "Registered config fuzzy watch for connection {}: pattern={}, type={}",
                connection_id, group_key_pattern, watch_type
            );
        }

        // Mark received group keys as already sent to client
        self.fuzzy_watch_manager
            .mark_received_batch(connection_id, &request.received_group_keys);

        let mut response = ConfigFuzzyWatchResponse::new();
        response.response.request_id = request_id;

        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "ConfigFuzzyWatchRequest"
    }

    fn auth_requirement(&self) -> AuthRequirement {
        AuthRequirement::Read
    }

    fn sign_type(&self) -> &'static str {
        "config"
    }

    fn resource_type(&self) -> batata_core::ResourceType {
        batata_core::ResourceType::Config
    }
}

// Handler for ConfigFuzzyWatchChangeNotifyRequest - notifies fuzzy watch changes
#[derive(Clone)]
pub struct ConfigFuzzyWatchChangeNotifyHandler {
    pub app_state: Arc<AppState>,
}

#[tonic::async_trait]
impl PayloadHandler for ConfigFuzzyWatchChangeNotifyHandler {
    async fn handle(
        &self,
        __connection: &Connection,
        payload: &Payload,
    ) -> Result<Payload, Status> {
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
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = ConfigFuzzyWatchSyncRequest::from(payload);
        let request_id = request.request_id();

        let connection_id = &_connection.meta_info.connection_id;
        let group_key_pattern = &request.group_key_pattern;
        let sync_type = &request.sync_type;

        // Register the pattern if not already registered
        self.fuzzy_watch_manager
            .register_watch(connection_id, group_key_pattern, sync_type);

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

    fn sign_type(&self) -> &'static str {
        "internal"
    }

    fn resource_type(&self) -> batata_core::ResourceType {
        batata_core::ResourceType::Internal
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
    async fn handle(
        &self,
        __connection: &Connection,
        payload: &Payload,
    ) -> Result<Payload, Status> {
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
