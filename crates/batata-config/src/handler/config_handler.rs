// Config module gRPC handlers
// Implements handlers for configuration management requests

use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

/// Build config group key: dataId+group+tenant (pre-allocated capacity)
#[inline]
fn build_group_key(data_id: &str, group: &str, tenant: &str) -> String {
    let mut key = String::with_capacity(data_id.len() + group.len() + tenant.len() + 2);
    key.push_str(data_id);
    key.push('+');
    key.push_str(group);
    key.push('+');
    key.push_str(tenant);
    key
}

use futures::future::join_all;
use tonic::Status;
use tracing::{debug, info, warn};

use batata_core::{
    ClientConnectionManager,
    model::Connection,
    service::cluster_client::{ClusterClientManager, ClusterRequestSender},
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
use batata_core::{GrpcResource, PermissionAction};
use batata_server_common::model::AppState;

/// Default max gray versions per config (overridden by batata.config.gray.version.max_count)
const DEFAULT_MAX_GRAY_VERSION_COUNT: usize = 10;

use crate::model::gray_rule::GrayRule;

/// Cached parsed gray rules to avoid re-parsing on every config query.
/// Key: gray_rule JSON string, Value: parsed gray rule (Arc-wrapped for shared ownership).
/// TTL of 30 seconds ensures stale rules are evicted quickly after updates.
static GRAY_RULE_CACHE: LazyLock<moka::sync::Cache<String, Arc<dyn GrayRule>>> =
    LazyLock::new(|| {
        moka::sync::Cache::builder()
            .max_capacity(1000)
            .time_to_live(Duration::from_secs(30))
            .build()
    });

/// Parse a gray rule from JSON, using a cache to avoid redundant parsing.
fn cached_parse_gray_rule(gray_rule_json: &str) -> Option<Arc<dyn GrayRule>> {
    if let Some(cached) = GRAY_RULE_CACHE.get(gray_rule_json) {
        return Some(cached);
    }
    let rule = crate::model::gray_rule::parse_gray_rule(gray_rule_json)?;
    let arc_rule: Arc<dyn GrayRule> = Arc::from(rule);
    GRAY_RULE_CACHE.insert(gray_rule_json.to_string(), arc_rule.clone());
    Some(arc_rule)
}

/// Check if adding a new gray version would exceed the max count.
/// Returns true if over the limit (and the gray_name doesn't already exist).
async fn is_gray_version_over_max_count(
    persistence: &dyn PersistenceService,
    data_id: &str,
    group: &str,
    tenant: &str,
    gray_name: &str,
    max_count: usize,
) -> bool {
    match persistence
        .config_find_all_grays(data_id, group, tenant)
        .await
    {
        Ok(grays) => {
            // If this gray_name already exists, it's an update, not a new version
            if grays.iter().any(|g| g.gray_name == gray_name) {
                return false;
            }
            grays.len() >= max_count
        }
        Err(_) => false,
    }
}

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
/// Returns (content, md5, encrypted_data_key, last_modified, gray_name) if a gray config matches.
async fn find_matching_gray_config(
    persistence: &dyn PersistenceService,
    data_id: &str,
    group: &str,
    tenant: &str,
    labels: &HashMap<String, String>,
) -> Option<(String, String, String, i64, String)> {
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

    // Parse (with caching) and sort by priority (higher priority first)
    let mut candidates: Vec<_> = grays
        .iter()
        .filter_map(|gray| {
            let rule = cached_parse_gray_rule(&gray.gray_rule)?;
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
                gray.gray_name.clone(),
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

        // Get encryption service for decryption
        let enc_svc = crate::service::encryption::get_encryption_provider(&self.app_state);

        // Check for matching gray config first
        let labels = build_client_labels(__connection);
        if let Some((content, md5, encrypted_data_key, last_modified, gray_name)) =
            find_matching_gray_config(persistence, data_id, group, tenant, &labels).await
        {
            let decrypted = enc_svc
                .decrypt_if_needed(data_id, &content, &encrypted_data_key)
                .await;
            let mut response = ConfigQueryResponse::new();
            response.response.request_id = request_id;
            response.content = decrypted;
            response.md5 = md5;
            response.encrypted_data_key = encrypted_data_key;
            response.last_modified = last_modified;
            // Distinguish beta vs tag gray types in response (consistent with Nacos)
            if gray_name == "beta" {
                response.is_beta = true;
            } else {
                response.tag = Some(gray_name);
            }
            return Ok(response.build_payload());
        }

        // Fall through to formal config query
        match persistence.config_find_one(data_id, group, tenant).await {
            Ok(Some(config)) => {
                let decrypted = enc_svc
                    .decrypt_if_needed(data_id, &config.content, &config.encrypted_data_key)
                    .await;
                let mut response = ConfigQueryResponse::new();
                response.response.request_id = request_id;
                response.content = decrypted;
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

    fn resource_from_payload(
        &self,
        payload: &batata_api::grpc::Payload,
    ) -> Option<(GrpcResource, PermissionAction)> {
        let request = ConfigQueryRequest::from(payload);
        let tenant = if request.config_request.tenant.is_empty() {
            "public"
        } else {
            &request.config_request.tenant
        };
        Some((
            GrpcResource::config(
                tenant,
                &request.config_request.group,
                &request.config_request.data_id,
            ),
            PermissionAction::Read,
        ))
    }
}

// Handler for ConfigPublishRequest - publishes/updates configuration
#[derive(Clone)]
pub struct ConfigPublishHandler {
    pub app_state: Arc<AppState>,
    pub fuzzy_watch_manager: Arc<ConfigFuzzyWatchManager>,
    pub connection_manager: Arc<dyn ClientConnectionManager>,
    /// Cluster client manager for broadcasting config changes to other nodes
    pub cluster_client_manager: Option<Arc<ClusterClientManager>>,
    /// Notifier for waking up long-polling HTTP listeners on config changes
    pub config_change_notifier: Arc<crate::service::notifier::ConfigChangeNotifier>,
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
        let mut cas_md5: Option<&str> = None;

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
                "casMd5" => cas_md5 = Some(value.as_str()),
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

        // Encrypt content if needed (based on data_id pattern)
        let enc_svc = crate::service::encryption::get_encryption_provider(&self.app_state);
        let (content, encrypted_data_key) = if encrypted_data_key.is_empty() {
            let (enc_content, enc_key) = enc_svc.encrypt_if_needed(data_id, content).await;
            (enc_content, enc_key)
        } else {
            (content.to_string(), encrypted_data_key.to_string())
        };
        let content = content.as_str();
        let encrypted_data_key = encrypted_data_key.as_str();

        let persistence = self.app_state.persistence();
        let max_gray_count = self.app_state.configuration.config_gray_max_version_count();

        // Handle gray (beta/tag) publish
        if !beta_ips.is_empty() {
            // Check max gray version count
            if is_gray_version_over_max_count(
                persistence,
                data_id,
                group,
                tenant,
                "beta",
                max_gray_count,
            )
            .await
            {
                let mut response = ConfigPublishResponse::new();
                response.response.request_id = request_id;
                response.response.result_code = ResponseCode::Fail.code();
                response.response.error_code = 20010; // CONFIG_GRAY_OVER_MAX_VERSION_COUNT
                response.response.success = false;
                response.response.message =
                    format!("gray config version is over max count: {}", max_gray_count);
                return Ok(response.build_payload());
            }
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
                    cas_md5,
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
            let gray_name = format!("tag_{}", tag);
            // Check max gray version count
            if is_gray_version_over_max_count(
                persistence,
                data_id,
                group,
                tenant,
                &gray_name,
                max_gray_count,
            )
            .await
            {
                let mut response = ConfigPublishResponse::new();
                response.response.request_id = request_id;
                response.response.result_code = ResponseCode::Fail.code();
                response.response.error_code = 20010; // CONFIG_GRAY_OVER_MAX_VERSION_COUNT
                response.response.success = false;
                response.response.message =
                    format!("gray config version is over max count: {}", max_gray_count);
                return Ok(response.build_payload());
            }

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
                    cas_md5,
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
                cas_md5,
            )
            .await
        {
            Ok(_) => {
                // Notify long-polling HTTP listeners about config change
                self.config_change_notifier
                    .notify_change(tenant, group, data_id);

                // Notify fuzzy watchers about config change
                if let Err(e) = self
                    .notify_fuzzy_watchers(data_id, group, tenant, src_ip)
                    .await
                {
                    warn!("Failed to notify fuzzy watchers: {}", e);
                }

                // Broadcast config change to other cluster nodes
                if let Some(ref ccm) = self.cluster_client_manager
                    && let Some(cm) = self.app_state.try_cluster_manager()
                    && !cm.is_standalone()
                {
                    let members: Vec<batata_api::model::Member> = cm
                        .all_members_extended()
                        .into_iter()
                        .map(|m| batata_api::model::MemberBuilder::new(m.ip, m.port).build())
                        .collect();
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

    fn resource_from_payload(
        &self,
        payload: &batata_api::grpc::Payload,
    ) -> Option<(GrpcResource, PermissionAction)> {
        let request = ConfigPublishRequest::from(payload);
        let tenant = if request.config_request.tenant.is_empty() {
            "public"
        } else {
            &request.config_request.tenant
        };
        Some((
            GrpcResource::config(
                tenant,
                &request.config_request.group,
                &request.config_request.data_id,
            ),
            PermissionAction::Write,
        ))
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
        // Notify fuzzy watchers
        let fuzzy_watchers = self
            .fuzzy_watch_manager
            .get_watchers_for_config(tenant, group, data_id);

        info!(
            "[FUZZY-DIAG] get_watchers_for_config(tenant={}, group={}, dataId={}) → {} matches (total={})",
            tenant,
            group,
            data_id,
            fuzzy_watchers.len(),
            self.fuzzy_watch_manager.watcher_count()
        );

        if !fuzzy_watchers.is_empty() {
            // Build group key in Nacos GroupKey format: dataId+group+tenant
            let group_key = build_group_key(data_id, group, tenant);

            // Use ConfigFuzzyWatchChangeNotifyRequest (not ConfigChangeNotifyRequest)
            // — the SDK expects this specific type for fuzzy watch notifications
            let mut notification = ConfigFuzzyWatchChangeNotifyRequest::new();
            notification.group_key = group_key.clone();
            notification.change_type = "CONFIG_CHANGED".to_string();
            let payload = notification.build_server_push_payload();

            info!(
                "Notifying {} fuzzy watchers for config change: {}",
                fuzzy_watchers.len(),
                group_key
            );

            // Mark all watchers as received before sending
            for connection_id in &fuzzy_watchers {
                self.fuzzy_watch_manager
                    .mark_received(connection_id, &group_key);
            }

            // Push notification to all fuzzy watchers using batch API
            let sent = self
                .connection_manager
                .push_message_to_many(&fuzzy_watchers, payload)
                .await;

            info!(
                "Notified {}/{} fuzzy watchers for config change: {}",
                sent,
                fuzzy_watchers.len(),
                group_key
            );
        }

        // Notify regular subscribers via ConfigSubscriptionService
        let config_key = batata_common::ConfigSubscriptionKey::new(data_id, group, tenant);
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

            // Push notification to each subscriber in parallel.
            // Use subscriber's client_tenant (original value from SDK) so the SDK
            // can match the notification against its local cache key.
            let futs: Vec<_> = subscribers
                .iter()
                .map(|subscriber| {
                    let cm = &self.connection_manager;
                    let sub_tenant = &subscriber.client_tenant;
                    let notification =
                        ConfigChangeNotifyRequest::for_config(data_id, group, sub_tenant);
                    let p = notification.build_server_push_payload();
                    let cid = subscriber.connection_id.clone();
                    async move {
                        if !cm.push_message(&cid, p).await {
                            warn!(
                                "Failed to push config change notification to connection {}",
                                cid
                            );
                        }
                    }
                })
                .collect();
            join_all(futs).await;

            info!(
                "Notified {} regular subscribers for config change: {}@@{}@@{}",
                subscribers.len(),
                tenant,
                group,
                data_id
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
    pub connection_manager: Arc<dyn ClientConnectionManager>,
    /// Cluster client manager for broadcasting config removals to other nodes
    pub cluster_client_manager: Option<Arc<ClusterClientManager>>,
    /// Notifier for waking up long-polling HTTP listeners on config changes
    pub config_change_notifier: Arc<crate::service::notifier::ConfigChangeNotifier>,
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
                // Notify long-polling HTTP listeners about config removal
                self.config_change_notifier
                    .notify_change(tenant, group, data_id);

                // Notify fuzzy watchers about config removal
                if let Err(e) = self
                    .notify_fuzzy_watchers(data_id, group, tenant, src_ip)
                    .await
                {
                    warn!("Failed to notify fuzzy watchers: {}", e);
                }

                // Broadcast config removal to other cluster nodes
                if let Some(ref ccm) = self.cluster_client_manager
                    && let Some(cm) = self.app_state.try_cluster_manager()
                    && !cm.is_standalone()
                {
                    let members: Vec<batata_api::model::Member> = cm
                        .all_members_extended()
                        .into_iter()
                        .map(|m| batata_api::model::MemberBuilder::new(m.ip, m.port).build())
                        .collect();
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

    fn resource_from_payload(
        &self,
        payload: &batata_api::grpc::Payload,
    ) -> Option<(GrpcResource, PermissionAction)> {
        let request = ConfigRemoveRequest::from(payload);
        let tenant = if request.config_request.tenant.is_empty() {
            "public"
        } else {
            &request.config_request.tenant
        };
        Some((
            GrpcResource::config(
                tenant,
                &request.config_request.group,
                &request.config_request.data_id,
            ),
            PermissionAction::Write,
        ))
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
        // Notify fuzzy watchers
        let fuzzy_watchers = self
            .fuzzy_watch_manager
            .get_watchers_for_config(tenant, group, data_id);

        info!(
            "[FUZZY-DIAG] get_watchers_for_config(tenant={}, group={}, dataId={}) → {} matches (total={})",
            tenant,
            group,
            data_id,
            fuzzy_watchers.len(),
            self.fuzzy_watch_manager.watcher_count()
        );

        if !fuzzy_watchers.is_empty() {
            // Build group key in Nacos GroupKey format: dataId+group+tenant
            let group_key = build_group_key(data_id, group, tenant);

            // Use DELETE_CONFIG for removal notifications
            let mut notification = ConfigFuzzyWatchChangeNotifyRequest::new();
            notification.group_key = group_key.clone();
            notification.change_type = "DELETE_CONFIG".to_string();
            let payload = notification.build_server_push_payload();

            info!(
                "Notifying {} fuzzy watchers for config removal: {}",
                fuzzy_watchers.len(),
                group_key
            );

            // Mark all watchers as received before sending
            for connection_id in &fuzzy_watchers {
                self.fuzzy_watch_manager
                    .mark_received(connection_id, &group_key);
            }

            // Push notification to all fuzzy watchers using batch API
            let sent = self
                .connection_manager
                .push_message_to_many(&fuzzy_watchers, payload)
                .await;

            info!(
                "Notified {}/{} fuzzy watchers for config removal: {}",
                sent,
                fuzzy_watchers.len(),
                group_key
            );
        }

        // Notify regular subscribers via ConfigSubscriptionService
        let config_key = batata_common::ConfigSubscriptionKey::new(data_id, group, tenant);
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

            // Push notification to each subscriber in parallel.
            // Use subscriber's client_tenant so the SDK can match against its local cache.
            let futs: Vec<_> = subscribers
                .iter()
                .map(|subscriber| {
                    let cm = &self.connection_manager;
                    let sub_tenant = &subscriber.client_tenant;
                    let notification =
                        ConfigChangeNotifyRequest::for_config(data_id, group, sub_tenant);
                    let p = notification.build_server_push_payload();
                    let cid = subscriber.connection_id.clone();
                    async move {
                        if !cm.push_message(&cid, p).await {
                            warn!(
                                "Failed to push config removal notification to connection {}",
                                cid
                            );
                        }
                    }
                })
                .collect();
            join_all(futs).await;

            info!(
                "Notified {} regular subscribers for config removal: {}@@{}@@{}",
                subscribers.len(),
                tenant,
                group,
                data_id
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

        info!(
            "ConfigBatchListen: listen={}, contexts={}, connection_id={}, client_ip={}",
            request.listen,
            request.config_listen_contexts.len(),
            connection_id,
            client_ip
        );

        // Build client labels once for gray matching
        let labels = build_client_labels(_connection);

        let mut changed_configs = Vec::new();

        // Check each config in the listen list for changes
        for ctx in &request.config_listen_contexts {
            let data_id = &ctx.data_id;
            let group = &ctx.group;
            // Normalize empty tenant to "public" for server-side operations (DB lookup, subscription)
            let tenant = if ctx.tenant.is_empty() {
                "public"
            } else {
                &ctx.tenant
            };
            // Keep the original tenant from the client for the response.
            // The SDK uses the original namespace (e.g. "") to build cache keys,
            // so returning "public" would cause a cache key mismatch.
            let response_tenant = &ctx.tenant;
            let client_md5 = &ctx.md5;

            let config_key = batata_common::ConfigSubscriptionKey::new(data_id, group, tenant);

            if request.listen {
                // Register subscription (pass original tenant for response matching)
                subscriber_manager.subscribe(
                    connection_id,
                    client_ip,
                    &config_key,
                    client_md5,
                    response_tenant,
                );
                debug!(
                    "ConfigBatchListen: subscribed data_id={}, group={}, tenant={}, client_md5={}",
                    data_id, group, tenant, client_md5
                );
            } else {
                // Unregister subscription
                subscriber_manager.unsubscribe(connection_id, &config_key);
                debug!(
                    "ConfigBatchListen: unsubscribed data_id={}, group={}, tenant={}",
                    data_id, group, tenant
                );
            }

            // First check for matching gray config
            if let Some((_, gray_md5, _, _, _)) =
                find_matching_gray_config(persistence, data_id, group, tenant, &labels).await
            {
                if client_md5 != &gray_md5 {
                    changed_configs.push(ConfigContext {
                        data_id: data_id.clone(),
                        group: group.clone(),
                        tenant: response_tenant.clone(),
                    });
                }
                continue;
            }

            // Fall through to formal config comparison
            if let Ok(Some(config)) = persistence.config_find_one(data_id, group, tenant).await {
                let server_md5 = &config.md5;

                // If MD5 differs, config has changed
                if client_md5 != server_md5 {
                    info!(
                        "ConfigBatchListen: MD5 changed for data_id={}, group={}, tenant={}, client_md5={}, server_md5={}",
                        data_id, group, tenant, client_md5, server_md5
                    );
                    changed_configs.push(ConfigContext {
                        data_id: data_id.clone(),
                        group: group.clone(),
                        tenant: response_tenant.clone(),
                    });
                }
            } else if request.listen {
                // Config doesn't exist but client is listening - report as changed
                info!(
                    "ConfigBatchListen: config not found (new listen) data_id={}, group={}, tenant={}",
                    data_id, group, tenant
                );
                changed_configs.push(ConfigContext {
                    data_id: data_id.clone(),
                    group: group.clone(),
                    tenant: response_tenant.clone(),
                });
            }
        }

        info!(
            "ConfigBatchListen: returning {} changed configs",
            changed_configs.len()
        );

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
    pub connection_manager: Arc<dyn ClientConnectionManager>,
    /// Notifier for waking up long-polling HTTP listeners on config changes
    pub config_change_notifier: Arc<crate::service::notifier::ConfigChangeNotifier>,
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

        // Notify long-polling HTTP listeners about config change from cluster
        self.config_change_notifier
            .notify_change(tenant, group, data_id);

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
    ///
    /// Note: `gray_name` is intentionally not used for subscriber filtering.
    /// Consistent with Nacos, all subscribers are notified regardless of gray type.
    /// Gray matching happens at query time when subscribers re-fetch the config.
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

        info!(
            "[FUZZY-DIAG] get_watchers_for_config(tenant={}, group={}, dataId={}) → {} matches (total={})",
            tenant,
            group,
            data_id,
            fuzzy_watchers.len(),
            self.fuzzy_watch_manager.watcher_count()
        );

        if !fuzzy_watchers.is_empty() {
            let group_key = ConfigFuzzyWatchPattern::build_group_key(tenant, group, data_id);

            info!(
                "Cluster sync: notifying {} fuzzy watchers for config change: {} (source: {})",
                fuzzy_watchers.len(),
                group_key,
                source_ip
            );

            let futs: Vec<_> = fuzzy_watchers
                .iter()
                .map(|connection_id| {
                    self.fuzzy_watch_manager
                        .mark_received(connection_id, &group_key);
                    let cm = &self.connection_manager;
                    let p = payload.clone();
                    let cid = connection_id.clone();
                    let gk = group_key.clone();
                    async move {
                        if !cm.push_message(&cid, p).await {
                            warn!(
                                "Failed to push cluster sync config notification to connection {}: {}",
                                cid, gk
                            );
                        }
                    }
                })
                .collect();
            join_all(futs).await;
        }

        // Notify regular subscribers via ConfigSubscriptionService
        let config_key = batata_common::ConfigSubscriptionKey::new(data_id, group, tenant);
        let subscribers = self
            .app_state
            .config_subscriber_manager
            .get_subscribers(&config_key);

        if !subscribers.is_empty() {
            info!(
                "Cluster sync: notifying {} regular subscribers for config change: {}@@{}@@{} (source: {})",
                subscribers.len(),
                tenant,
                group,
                data_id,
                source_ip
            );

            let futs: Vec<_> = subscribers
                .iter()
                .map(|subscriber| {
                    let cm = &self.connection_manager;
                    let p = payload.clone();
                    let cid = subscriber.connection_id.clone();
                    async move {
                        if !cm.push_message(&cid, p).await {
                            warn!(
                                "Failed to push cluster sync config notification to connection {}",
                                cid
                            );
                        }
                    }
                })
                .collect();
            join_all(futs).await;
        }

        Ok(())
    }
}

// Handler for ConfigFuzzyWatchRequest - handles fuzzy pattern watch for configs
#[derive(Clone)]
pub struct ConfigFuzzyWatchHandler {
    pub app_state: Arc<AppState>,
    pub fuzzy_watch_manager: Arc<ConfigFuzzyWatchManager>,
    pub connection_manager: Arc<dyn ClientConnectionManager>,
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
        match self
            .fuzzy_watch_manager
            .register_watch(connection_id, group_key_pattern, watch_type)
        {
            Ok(true) => {
                debug!(
                    "Registered config fuzzy watch for connection {}: pattern={}, type={}",
                    connection_id, group_key_pattern, watch_type
                );
            }
            Ok(false) => {
                // Invalid pattern, ignored
            }
            Err(e) => {
                warn!(
                    "Config fuzzy watch registration rejected for connection {}: {}",
                    connection_id, e
                );
                return Err(Status::resource_exhausted(e.to_string()));
            }
        }

        // Mark received group keys as already sent to client
        self.fuzzy_watch_manager
            .mark_received_batch(connection_id, &request.received_group_keys);

        // Initial sync: send existing matching configs to client via push
        if request.initializing && request.watch_type == "WATCH" {
            let pattern = ConfigFuzzyWatchPattern::from_group_key_pattern(group_key_pattern);
            if let Some(pat) = pattern {
                let persistence = self.app_state.persistence();
                // Find all configs matching the pattern's namespace
                if let Ok(configs) = persistence.config_find_by_namespace(&pat.namespace).await {
                    let conn_id = connection_id.to_string();
                    let cm = self.connection_manager.clone();
                    // Send matching configs as ADD_CONFIG notifications
                    for config in &configs {
                        if pat.matches(&pat.namespace, &config.group, &config.data_id) {
                            let group_key =
                                build_group_key(&config.data_id, &config.group, &pat.namespace);
                            let mut notification = ConfigFuzzyWatchChangeNotifyRequest::new();
                            notification.group_key = group_key;
                            notification.change_type = "ADD_CONFIG".to_string();
                            let payload = notification.build_server_push_payload();
                            cm.push_message(&conn_id, payload).await;
                        }
                    }
                }
            }

            // Send init finish notification so SDK's Future completes
            let mut finish_req = ConfigFuzzyWatchSyncRequest::new();
            finish_req.group_key_pattern = group_key_pattern.to_string();
            finish_req.sync_type = "FINISH_FUZZY_WATCH_INIT_NOTIFY".to_string();
            let finish_payload = finish_req.build_server_push_payload();
            self.connection_manager
                .push_message(connection_id, finish_payload)
                .await;
        }

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
        match self
            .fuzzy_watch_manager
            .register_watch(connection_id, group_key_pattern, sync_type)
        {
            Ok(_) => {}
            Err(e) => {
                warn!(
                    "Config fuzzy watch sync registration rejected for connection {}: {}",
                    connection_id, e
                );
                return Err(Status::resource_exhausted(e.to_string()));
            }
        }

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
