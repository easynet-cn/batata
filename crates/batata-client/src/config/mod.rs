//! Configuration management service
//!
//! Provides `BatataConfigService` for config CRUD operations,
//! config listening, and server push handling.
//!
//! Matches Nacos Java `ClientWorker` with:
//! - Background listen loop coordinated by a bell signal (`tokio::sync::Notify`)
//! - MD5-based change detection with per-listener tracking
//! - `receiveNotifyChanged` + `consistentWithServer` state flags
//! - Batch listen protocol with deduplication

pub mod cache;
pub mod change_parser;
pub mod encryption;
pub mod filter;
pub mod fuzzy_watch;
pub mod listener;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use batata_api::{
    config::model::{
        ConfigBatchListenRequest, ConfigChangeBatchListenResponse, ConfigChangeNotifyRequest,
        ConfigChangeNotifyResponse, ConfigListenContext, ConfigPublishRequest,
        ConfigPublishResponse, ConfigQueryRequest, ConfigQueryResponse, ConfigRemoveRequest,
        ConfigRemoveResponse, ConfigRequest,
    },
    grpc::Payload,
    remote::model::ResponseTrait,
};
use dashmap::DashMap;
use tokio::sync::Notify;
use tracing::{debug, error, info, warn};

use crate::error::Result;
use crate::grpc::{GrpcClient, ServerPushHandler};
use crate::local_config::LocalConfigInfoProcessor;

/// Helper to create a ConfigRequest with data_id, group, and tenant.
fn make_config_request(data_id: &str, group: &str, tenant: &str) -> ConfigRequest {
    let mut req = ConfigRequest::new();
    req.data_id = data_id.to_string();
    req.group = group.to_string();
    req.tenant = tenant.to_string();
    req
}

use self::cache::{CacheData, build_cache_key};
use self::filter::{
    ConfigFilterChainManager, ConfigRequest as FilterConfigRequest,
    ConfigResponse as FilterConfigResponse,
};
use self::listener::ConfigChangeListener;

/// Default listen loop poll timeout in seconds.
const DEFAULT_LISTEN_POLL_TIMEOUT_SECS: u64 = 5;

/// Nacos-compatible config service backed by gRPC.
///
/// Implements the ClientWorker pattern with a background listen loop
/// that coordinates between server push notifications and batch listen requests.
pub struct BatataConfigService {
    grpc_client: Arc<GrpcClient>,
    cache_map: DashMap<String, CacheData>,
    filter_chain: Arc<tokio::sync::RwLock<ConfigFilterChainManager>>,
    local_processor: Option<Arc<LocalConfigInfoProcessor>>,
    /// Bell signal to wake the listen loop (matches Nacos `listenExecutebell`)
    listen_bell: Arc<Notify>,
    /// Shutdown flag for the listen loop
    shutdown: Arc<AtomicBool>,
}

impl BatataConfigService {
    /// Create a new config service with the given gRPC client.
    pub fn new(grpc_client: Arc<GrpcClient>) -> Self {
        Self {
            grpc_client,
            cache_map: DashMap::new(),
            filter_chain: Arc::new(tokio::sync::RwLock::new(ConfigFilterChainManager::new())),
            local_processor: None,
            listen_bell: Arc::new(Notify::new()),
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create with local processor for snapshot/failover
    pub fn with_local_processor(
        grpc_client: Arc<GrpcClient>,
        local_processor: Arc<LocalConfigInfoProcessor>,
    ) -> Self {
        Self {
            grpc_client,
            cache_map: DashMap::new(),
            filter_chain: Arc::new(tokio::sync::RwLock::new(ConfigFilterChainManager::new())),
            local_processor: Some(local_processor),
            listen_bell: Arc::new(Notify::new()),
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create with custom filter chain
    pub fn with_filter_chain(
        grpc_client: Arc<GrpcClient>,
        filter_chain: ConfigFilterChainManager,
    ) -> Self {
        Self {
            grpc_client,
            cache_map: DashMap::new(),
            filter_chain: Arc::new(tokio::sync::RwLock::new(filter_chain)),
            local_processor: None,
            listen_bell: Arc::new(Notify::new()),
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create with all options
    pub fn with_all(
        grpc_client: Arc<GrpcClient>,
        filter_chain: ConfigFilterChainManager,
        local_processor: Option<Arc<LocalConfigInfoProcessor>>,
    ) -> Self {
        Self {
            grpc_client,
            cache_map: DashMap::new(),
            filter_chain: Arc::new(tokio::sync::RwLock::new(filter_chain)),
            local_processor,
            listen_bell: Arc::new(Notify::new()),
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Register server push handlers on the gRPC client and start the listen loop.
    ///
    /// Must be called after wrapping in `Arc` to enable the push handler
    /// to call back into the config service when config change notifications arrive.
    pub fn register_push_handlers(self: &Arc<Self>) {
        self.grpc_client.register_push_handler(
            "ConfigChangeNotifyRequest",
            ConfigChangeNotifyHandler::new(self.clone()),
        );

        // Start the background listen loop
        self.start_listen_loop();
    }

    /// Add a filter to the chain
    pub async fn add_filter(&self, filter: Arc<dyn filter::IConfigFilter>) {
        let mut chain = self.filter_chain.write().await;
        chain.add_filter(filter);
    }

    /// Get filter chain manager
    pub fn filter_chain(&self) -> Arc<tokio::sync::RwLock<ConfigFilterChainManager>> {
        self.filter_chain.clone()
    }

    /// Signal the listen loop to run a check cycle.
    ///
    /// Equivalent to Nacos `notifyListenConfig()`.
    pub fn notify_listen_config(&self) {
        self.listen_bell.notify_one();
    }

    /// Get a config value from the server.
    pub async fn get_config(&self, data_id: &str, group: &str, tenant: &str) -> Result<String> {
        match self.get_config_full(data_id, group, tenant).await {
            Ok(result) => Ok(result.content),
            Err(e) => {
                // Try failover if available
                if let Some(processor) = &self.local_processor
                    && let Ok(Some(failover_content)) =
                        processor.get_failover(data_id, group, tenant)
                {
                    warn!(
                        "Using failover config for dataId={}, group={}, tenant={}",
                        data_id, group, tenant
                    );
                    return Ok(failover_content);
                }
                Err(e)
            }
        }
    }

    /// Get a config value with full metadata (MD5, type, encrypted_data_key).
    ///
    /// Equivalent to Nacos `getConfigWithResult()`.
    pub async fn get_config_with_result(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
    ) -> Result<crate::traits::ConfigQueryResult> {
        match self.get_config_full(data_id, group, tenant).await {
            Ok(result) => Ok(result),
            Err(e) => {
                // Try failover if available
                if let Some(processor) = &self.local_processor
                    && let Ok(Some(failover_content)) =
                        processor.get_failover(data_id, group, tenant)
                {
                    warn!(
                        "Using failover config for dataId={}, group={}, tenant={}",
                        data_id, group, tenant
                    );
                    return Ok(crate::traits::ConfigQueryResult {
                        content: failover_content,
                        ..Default::default()
                    });
                }
                Err(e)
            }
        }
    }

    /// Internal: get config with full metadata from server
    async fn get_config_full(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
    ) -> Result<crate::traits::ConfigQueryResult> {
        let mut req = ConfigQueryRequest {
            config_request: make_config_request(data_id, group, tenant),
            tag: String::new(),
        };
        req.config_request.request.request_id = uuid::Uuid::new_v4().to_string();

        let resp: ConfigQueryResponse = self.grpc_client.request_typed(&req).await?;

        // Apply filter chain
        let mut filter_resp = FilterConfigResponse::new();
        filter_resp.data_id = data_id.to_string();
        filter_resp.group = group.to_string();
        filter_resp.tenant = tenant.to_string();
        filter_resp.content = resp.content.clone();
        filter_resp.encrypted_data_key = resp.encrypted_data_key.clone();

        {
            let chain = self.filter_chain.read().await;
            if let Err(e) = chain.do_filter_query(&mut filter_resp).await {
                warn!("Filter query failed: {}", e);
            }
        }

        let content = filter_resp.content;
        let md5 = crate::config::cache::compute_md5(&content);

        // Cache the content
        let key = build_cache_key(data_id, group, tenant);
        self.cache_map
            .entry(key)
            .and_modify(|cache| {
                cache.update_content(&content);
                cache.encrypted_data_key = resp.encrypted_data_key.clone();
                cache.config_type = resp.content_type.clone();
            })
            .or_insert_with(|| {
                let mut cache = CacheData::new(data_id, group, tenant);
                cache.update_content(&content);
                cache.encrypted_data_key = resp.encrypted_data_key.clone();
                cache.config_type = resp.content_type.clone();
                cache
            });

        // Save snapshot if available
        if let Some(processor) = &self.local_processor
            && let Err(e) = processor.save_snapshot(data_id, group, tenant, Some(&content))
        {
            warn!("Failed to save snapshot: {}", e);
        }

        Ok(crate::traits::ConfigQueryResult {
            content,
            md5,
            config_type: resp.content_type,
            encrypted_data_key: resp.encrypted_data_key,
        })
    }

    /// Publish (create or update) a config on the server.
    pub async fn publish_config(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
        content: &str,
    ) -> Result<bool> {
        self.publish_config_inner(data_id, group, tenant, content, "", None)
            .await
    }

    /// Publish config with type
    pub async fn publish_config_with_type(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
        content: &str,
        config_type: &str,
    ) -> Result<bool> {
        self.publish_config_inner(data_id, group, tenant, content, config_type, None)
            .await
    }

    /// Publish config with CAS (Compare-And-Swap) to avoid concurrent overwrites
    pub async fn publish_config_cas(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
        content: &str,
        cas_md5: &str,
    ) -> Result<bool> {
        self.publish_config_inner(data_id, group, tenant, content, "", Some(cas_md5))
            .await
    }

    /// Publish config with CAS and type
    pub async fn publish_config_cas_with_type(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
        content: &str,
        cas_md5: &str,
        config_type: &str,
    ) -> Result<bool> {
        self.publish_config_inner(data_id, group, tenant, content, config_type, Some(cas_md5))
            .await
    }

    /// Internal publish method
    async fn publish_config_inner(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
        content: &str,
        config_type: &str,
        cas_md5: Option<&str>,
    ) -> Result<bool> {
        // Apply filter chain
        let mut filter_req =
            FilterConfigRequest::new(data_id.to_string(), group.to_string(), tenant.to_string());
        filter_req.content = content.to_string();
        filter_req.r#type = if config_type.is_empty() {
            "text".to_string()
        } else {
            config_type.to_string()
        };

        {
            let chain = self.filter_chain.read().await;
            if let Err(e) = chain.do_filter_publish(&mut filter_req).await {
                warn!("Filter publish failed: {}", e);
            }
        }

        let mut req = ConfigPublishRequest {
            config_request: make_config_request(data_id, group, tenant),
            content: filter_req.content.clone(),
            cas_md5: cas_md5.unwrap_or("").to_string(),
            addition_map: Default::default(),
        };
        req.config_request.request.request_id = uuid::Uuid::new_v4().to_string();

        let resp: ConfigPublishResponse = self.grpc_client.request_typed(&req).await?;

        // Update cache and snapshot on success
        if resp.response.success {
            let key = build_cache_key(data_id, group, tenant);
            if let Some(mut entry) = self.cache_map.get_mut(&key) {
                entry.update_content(&filter_req.content);
            }

            if let Some(processor) = &self.local_processor
                && let Err(e) =
                    processor.save_snapshot(data_id, group, tenant, Some(&filter_req.content))
            {
                warn!("Failed to save snapshot: {}", e);
            }
        }

        Ok(resp.response.success)
    }

    /// Remove a config from the server.
    pub async fn remove_config(&self, data_id: &str, group: &str, tenant: &str) -> Result<bool> {
        let mut req = ConfigRemoveRequest {
            config_request: make_config_request(data_id, group, tenant),
            tag: String::new(),
        };
        req.config_request.request.request_id = uuid::Uuid::new_v4().to_string();

        let resp: ConfigRemoveResponse = self.grpc_client.request_typed(&req).await?;

        if resp.response.success {
            // Remove from cache
            let key = build_cache_key(data_id, group, tenant);
            self.cache_map.remove(&key);

            // Remove snapshot
            if let Some(processor) = &self.local_processor
                && let Err(e) = processor.save_snapshot(data_id, group, tenant, None)
            {
                warn!("Failed to remove snapshot: {}", e);
            }
        }

        Ok(resp.response.success)
    }

    /// Add a listener for config changes.
    ///
    /// If this is the first listener for the config, a `ConfigBatchListenRequest`
    /// is sent to the server. The background listen loop will handle subsequent
    /// change detection.
    pub async fn add_listener(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
        listener: Arc<dyn ConfigChangeListener>,
    ) -> Result<()> {
        let key = build_cache_key(data_id, group, tenant);
        let should_subscribe;

        // Pre-populate cache with current content so the MD5 in the listen
        // request matches the server and no spurious notification fires.
        self.ensure_cache_populated(data_id, group, tenant).await;

        {
            let mut entry = self
                .cache_map
                .entry(key.clone())
                .or_insert_with(|| CacheData::new(data_id, group, tenant));
            entry.add_listener(listener);
            entry.is_discard = false;
            should_subscribe = !entry.is_consistent_with_server.load(Ordering::Relaxed);
        }

        if should_subscribe {
            self.send_listen_request(data_id, group, tenant, true)
                .await?;
            debug!(
                "Started listening for config: data_id={}, group={}, tenant={}",
                data_id, group, tenant
            );
        }

        // Signal listen loop to pick up the new listener
        self.notify_listen_config();

        Ok(())
    }

    /// Add a detailed change event listener that receives field-level diffs.
    pub async fn add_change_event_listener(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
        config_type: &str,
        listener: Arc<dyn listener::ConfigChangeEventListener>,
    ) -> Result<()> {
        let key = build_cache_key(data_id, group, tenant);
        let should_subscribe;

        self.ensure_cache_populated(data_id, group, tenant).await;

        {
            let mut entry = self
                .cache_map
                .entry(key.clone())
                .or_insert_with(|| CacheData::new(data_id, group, tenant));
            entry.add_change_event_listener(listener);
            if !config_type.is_empty() {
                entry.config_type = config_type.to_string();
            }
            entry.is_discard = false;
            should_subscribe = !entry.is_consistent_with_server.load(Ordering::Relaxed);
        }

        if should_subscribe {
            self.send_listen_request(data_id, group, tenant, true)
                .await?;
        }

        self.notify_listen_config();

        Ok(())
    }

    /// Remove all listeners for a config.
    ///
    /// Marks the cache entry for discard. The listen loop will send
    /// the unsubscribe request.
    pub async fn remove_listener(&self, data_id: &str, group: &str, tenant: &str) -> Result<()> {
        let key = build_cache_key(data_id, group, tenant);

        {
            if let Some(mut entry) = self.cache_map.get_mut(&key) {
                entry.remove_all_listeners();
                // is_discard is set by remove_all_listeners
                entry
                    .is_consistent_with_server
                    .store(false, Ordering::Relaxed);
            } else {
                return Ok(());
            }
        }

        // Signal listen loop to process the discard
        self.notify_listen_config();

        debug!(
            "Stopped listening for config: data_id={}, group={}, tenant={}",
            data_id, group, tenant
        );

        Ok(())
    }

    /// Remove a specific listener for a config.
    ///
    /// Unlike `remove_listener` which removes ALL listeners, this removes only
    /// the specified listener instance (matched by pointer equality).
    /// Matches Nacos Java `removeListener(dataId, group, listener)`.
    pub async fn remove_specific_listener(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
        listener: &Arc<dyn ConfigChangeListener>,
    ) -> Result<()> {
        let key = build_cache_key(data_id, group, tenant);

        let should_unsubscribe = {
            if let Some(mut entry) = self.cache_map.get_mut(&key) {
                entry.remove_listener(listener);
                if !entry.has_listeners() {
                    entry
                        .is_consistent_with_server
                        .store(false, Ordering::Relaxed);
                    true
                } else {
                    false
                }
            } else {
                return Ok(());
            }
        };

        if should_unsubscribe {
            self.notify_listen_config();
        }

        Ok(())
    }

    /// Re-establish all config listen subscriptions (called after reconnect).
    pub async fn redo_listeners(&self) -> Result<()> {
        // Mark all listening entries as inconsistent
        for entry in self.cache_map.iter_mut() {
            if entry.has_listeners() {
                entry
                    .is_consistent_with_server
                    .store(false, Ordering::Relaxed);
            }
        }

        // Signal the listen loop to re-sync
        self.notify_listen_config();

        info!("Marked config listeners for re-sync after reconnect");
        Ok(())
    }

    /// Get config and atomically register a listener in one call.
    pub async fn get_config_and_sign_listener(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
        listener: Arc<dyn ConfigChangeListener>,
    ) -> Result<String> {
        // Register listener FIRST so we don't miss any changes
        self.add_listener(data_id, group, tenant, listener).await?;
        // Then fetch the current config
        self.get_config(data_id, group, tenant).await
    }

    /// Check if the config server is healthy.
    pub async fn get_server_status(&self) -> String {
        if self.grpc_client.is_connected().await {
            "UP".to_string()
        } else {
            "DOWN".to_string()
        }
    }

    /// Gracefully shutdown the config service.
    pub async fn shutdown(&self) {
        info!("Shutting down config service...");
        self.shutdown.store(true, Ordering::Relaxed);
        self.listen_bell.notify_one(); // Wake loop to exit
        self.cache_map.clear();
        info!("Config service shutdown complete");
    }

    // --- Background Listen Loop (ClientWorker pattern) ---

    /// Start the background listen loop.
    ///
    /// Equivalent to Nacos Java `ClientWorker.startInternal()`.
    /// Spawns a tokio task that:
    /// 1. Waits on the bell signal or a 5-second timeout
    /// 2. Collects inconsistent cache entries
    /// 3. Sends batch listen requests and processes responses
    /// 4. Notifies listeners via `check_listener_md5()`
    fn start_listen_loop(self: &Arc<Self>) {
        let service = Arc::downgrade(self);
        let bell = self.listen_bell.clone();
        let shutdown = self.shutdown.clone();

        tokio::spawn(async move {
            loop {
                if shutdown.load(Ordering::Relaxed) {
                    debug!("Config listen loop: shutdown");
                    break;
                }

                // Wait for bell signal or timeout (matches Nacos 5s poll)
                tokio::select! {
                    _ = bell.notified() => {},
                    _ = tokio::time::sleep(std::time::Duration::from_secs(DEFAULT_LISTEN_POLL_TIMEOUT_SECS)) => {},
                }

                if shutdown.load(Ordering::Relaxed) {
                    break;
                }

                let Some(service) = service.upgrade() else {
                    debug!("Config listen loop: service dropped");
                    break;
                };

                if let Err(e) = service.execute_config_listen().await {
                    error!("Config listen loop error: {}", e);
                    // Signal to retry (matches Nacos: notifyListenConfig on error)
                    service.notify_listen_config();
                }
            }
        });
    }

    /// Execute one config listen cycle.
    ///
    /// Matches Nacos Java `ClientWorker.executeConfigListen()` and `checkListenCache()`.
    async fn execute_config_listen(&self) -> Result<()> {
        // Step 1: Collect entries that need checking
        let mut listen_contexts = Vec::new();
        let mut discard_contexts = Vec::new();

        for entry in self.cache_map.iter() {
            // Handle discarded entries (no listeners left)
            if entry.is_discard {
                discard_contexts.push((
                    entry.data_id.clone(),
                    entry.group.clone(),
                    entry.tenant.clone(),
                ));
                continue;
            }

            // Handle entries with receiveNotifyChanged flag (set by server push)
            if entry.receive_notify_changed.load(Ordering::Relaxed) {
                // Reset the flag
                entry.receive_notify_changed.store(false, Ordering::Relaxed);

                // Re-fetch config from server
                let data_id = entry.data_id.clone();
                let group = entry.group.clone();
                let tenant = entry.tenant.clone();
                let is_initializing = entry.is_initializing;

                drop(entry); // Release the entry guard before async call

                if let Err(e) = self.refresh_content_and_check(&data_id, &group, &tenant, !is_initializing).await {
                    warn!("Failed to refresh config on notify: data_id={}, error={}", data_id, e);
                }
                continue;
            }

            // Collect inconsistent entries for batch listen
            if !entry.is_consistent_with_server.load(Ordering::Relaxed) && entry.has_listeners() {
                listen_contexts.push(ConfigListenContext {
                    data_id: entry.data_id.clone(),
                    group: entry.group.clone(),
                    tenant: entry.tenant.clone(),
                    md5: entry.md5.clone(),
                });
            }
        }

        // Step 2: Send unsubscribe for discarded entries
        for (data_id, group, tenant) in &discard_contexts {
            if let Err(e) = self.send_listen_request(data_id, group, tenant, false).await {
                warn!("Failed to unsubscribe discarded config: {}", e);
            }
            let key = build_cache_key(data_id, group, tenant);
            self.cache_map.remove(&key);
        }

        // Step 3: Send batch listen for inconsistent entries
        if listen_contexts.is_empty() {
            return Ok(());
        }

        let mut req = ConfigBatchListenRequest {
            config_request: ConfigRequest::default(),
            listen: true,
            config_listen_contexts: listen_contexts.clone(),
        };
        req.config_request.request.request_id = uuid::Uuid::new_v4().to_string();

        let resp: ConfigChangeBatchListenResponse = self.grpc_client.request_typed(&req).await?;

        // Step 4: Process changed configs from response
        let mut changed_keys = std::collections::HashSet::new();

        for changed in &resp.changed_configs {
            let key = build_cache_key(&changed.data_id, &changed.group, &changed.tenant);
            changed_keys.insert(key);

            let is_initializing = self
                .cache_map
                .get(&build_cache_key(&changed.data_id, &changed.group, &changed.tenant))
                .map(|e| e.is_initializing)
                .unwrap_or(false);

            if let Err(e) = self
                .refresh_content_and_check(
                    &changed.data_id,
                    &changed.group,
                    &changed.tenant,
                    !is_initializing,
                )
                .await
            {
                warn!("Failed to refresh changed config: {}", e);
            }
        }

        // Step 5: Also check entries that were pushed during our listen request
        for ctx in &listen_contexts {
            let key = build_cache_key(&ctx.data_id, &ctx.group, &ctx.tenant);
            if changed_keys.contains(&key) {
                continue; // Already handled
            }

            let was_pushed = self
                .cache_map
                .get(&key)
                .map(|e| e.receive_notify_changed.load(Ordering::Relaxed))
                .unwrap_or(false);

            if was_pushed {
                if let Some(entry) = self.cache_map.get(&key) {
                    entry.receive_notify_changed.store(false, Ordering::Relaxed);
                    let is_initializing = entry.is_initializing;
                    let data_id = entry.data_id.clone();
                    let group = entry.group.clone();
                    let tenant = entry.tenant.clone();
                    drop(entry);

                    if let Err(e) = self
                        .refresh_content_and_check(&data_id, &group, &tenant, !is_initializing)
                        .await
                    {
                        warn!("Failed to refresh pushed config: {}", e);
                    }
                    changed_keys.insert(key);
                }
            }
        }

        // Step 6: Mark unchanged configs as consistent with server
        for ctx in &listen_contexts {
            let key = build_cache_key(&ctx.data_id, &ctx.group, &ctx.tenant);
            if !changed_keys.contains(&key) {
                if let Some(mut entry) = self.cache_map.get_mut(&key) {
                    if !entry.receive_notify_changed.load(Ordering::Relaxed) {
                        entry
                            .is_consistent_with_server
                            .store(true, Ordering::Relaxed);
                        entry.is_initializing = false;
                    }
                }
            }
        }

        // If there were changes, signal another cycle
        if !changed_keys.is_empty() {
            self.notify_listen_config();
        }

        Ok(())
    }

    /// Re-fetch config content from server and notify listeners if changed.
    ///
    /// Matches Nacos Java `ClientWorker.refreshContentAndCheck()`.
    async fn refresh_content_and_check(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
        notify: bool,
    ) -> Result<()> {
        let content = self.get_config_full(data_id, group, tenant).await?.content;

        let key = build_cache_key(data_id, group, tenant);
        if let Some(mut entry) = self.cache_map.get_mut(&key) {
            let changed = entry.update_content(&content);
            entry.is_initializing = false;

            if changed && notify {
                // Notify listeners via MD5 check (only those not yet notified)
                entry.check_listener_md5();

                // Notify change event listeners with field-level diffs
                if !entry.change_event_listeners.is_empty() {
                    // Get old content from the first change event listener's last_content
                    let old_content = entry
                        .change_event_listeners
                        .first()
                        .map(|w| w.last_content.clone())
                        .unwrap_or_default();

                    let items = change_parser::parse_change_items(
                        &entry.config_type,
                        &old_content,
                        &content,
                    );
                    if !items.is_empty() {
                        let event = change_parser::ConfigChangeEvent {
                            data_id: data_id.to_string(),
                            group: group.to_string(),
                            tenant: tenant.to_string(),
                            items,
                        };
                        for wrap in &entry.change_event_listeners {
                            wrap.listener.receive_config_change(event.clone());
                            // Update last state (same unsafe pattern as check_listener_md5)
                            unsafe {
                                let wrap_ptr = wrap
                                    as *const cache::ManagerChangeEventListenerWrap
                                    as *mut cache::ManagerChangeEventListenerWrap;
                                (*wrap_ptr).last_call_md5 = entry.md5.clone();
                                (*wrap_ptr).last_content = content.clone();
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Pre-fetch config into the cache if not already present.
    async fn ensure_cache_populated(&self, data_id: &str, group: &str, tenant: &str) {
        let key = build_cache_key(data_id, group, tenant);
        let already_has_content = self
            .cache_map
            .get(&key)
            .map(|e| !e.content.is_empty())
            .unwrap_or(false);

        if !already_has_content
            && let Ok(content) = self.get_config(data_id, group, tenant).await
        {
            debug!(
                "Pre-populated cache for: data_id={}, group={}, tenant={}, len={}",
                data_id,
                group,
                tenant,
                content.len()
            );
        }
    }

    /// Send a ConfigBatchListenRequest.
    async fn send_listen_request(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
        listen: bool,
    ) -> Result<()> {
        let key = build_cache_key(data_id, group, tenant);
        let md5 = self
            .cache_map
            .get(&key)
            .map(|e| e.md5.clone())
            .unwrap_or_default();

        let context = ConfigListenContext {
            data_id: data_id.to_string(),
            group: group.to_string(),
            tenant: tenant.to_string(),
            md5,
        };

        let mut req = ConfigBatchListenRequest {
            config_request: ConfigRequest::default(),
            listen,
            config_listen_contexts: vec![context],
        };
        req.config_request.request.request_id = uuid::Uuid::new_v4().to_string();

        let _resp: ConfigChangeBatchListenResponse = self.grpc_client.request_typed(&req).await?;

        Ok(())
    }
}

/// Server push handler for `ConfigChangeNotifyRequest`.
///
/// Instead of immediately re-fetching (old behavior), this handler sets
/// the `receive_notify_changed` flag and signals the listen loop, matching
/// the Nacos Java `ClientWorker.handleConfigChangeNotifyRequest()` pattern.
pub struct ConfigChangeNotifyHandler {
    config_service: Arc<BatataConfigService>,
}

impl ConfigChangeNotifyHandler {
    pub fn new(config_service: Arc<BatataConfigService>) -> Self {
        Self { config_service }
    }
}

impl ServerPushHandler for ConfigChangeNotifyHandler {
    fn handle(&self, payload: &Payload) -> Option<Payload> {
        let req: ConfigChangeNotifyRequest = crate::grpc::deserialize_payload(payload);

        info!(
            "[server-push] config changed: data_id={}, group={}, tenant={}",
            req.data_id, req.group, req.tenant
        );

        let key = build_cache_key(&req.data_id, &req.group, &req.tenant);

        // Set flags on the cache entry (matches Nacos Java pattern)
        if let Some(entry) = self.config_service.cache_map.get(&key) {
            entry.receive_notify_changed.store(true, Ordering::Relaxed);
            entry
                .is_consistent_with_server
                .store(false, Ordering::Relaxed);
        }

        // Signal the listen loop to process the change
        self.config_service.notify_listen_config();

        // Send acknowledgment
        let resp = ConfigChangeNotifyResponse::new();
        Some(resp.build_payload())
    }
}

// ============================================================================
// ConfigService trait implementation
// ============================================================================

#[async_trait::async_trait]
impl crate::traits::ConfigService for BatataConfigService {
    async fn get_config(
        &self,
        data_id: &str,
        group: &str,
        timeout: std::time::Duration,
    ) -> Result<String> {
        tokio::time::timeout(timeout, self.get_config(data_id, group, ""))
            .await
            .map_err(|_| crate::error::ClientError::Timeout)?
    }

    async fn get_config_with_result(
        &self,
        data_id: &str,
        group: &str,
        timeout: std::time::Duration,
    ) -> Result<crate::traits::ConfigQueryResult> {
        tokio::time::timeout(timeout, self.get_config_with_result(data_id, group, ""))
            .await
            .map_err(|_| crate::error::ClientError::Timeout)?
    }

    async fn get_config_and_sign_listener(
        &self,
        data_id: &str,
        group: &str,
        timeout: std::time::Duration,
        listener: Arc<dyn ConfigChangeListener>,
    ) -> Result<String> {
        tokio::time::timeout(
            timeout,
            self.get_config_and_sign_listener(data_id, group, "", listener),
        )
        .await
        .map_err(|_| crate::error::ClientError::Timeout)?
    }

    async fn publish_config(
        &self,
        data_id: &str,
        group: &str,
        content: &str,
    ) -> Result<bool> {
        self.publish_config(data_id, group, "", content).await
    }

    async fn publish_config_with_type(
        &self,
        data_id: &str,
        group: &str,
        content: &str,
        config_type: &str,
    ) -> Result<bool> {
        self.publish_config_with_type(data_id, group, "", content, config_type)
            .await
    }

    async fn publish_config_cas(
        &self,
        data_id: &str,
        group: &str,
        content: &str,
        cas_md5: &str,
    ) -> Result<bool> {
        self.publish_config_cas(data_id, group, "", content, cas_md5)
            .await
    }

    async fn publish_config_cas_with_type(
        &self,
        data_id: &str,
        group: &str,
        content: &str,
        cas_md5: &str,
        config_type: &str,
    ) -> Result<bool> {
        self.publish_config_cas_with_type(data_id, group, "", content, cas_md5, config_type)
            .await
    }

    async fn remove_config(&self, data_id: &str, group: &str) -> Result<bool> {
        self.remove_config(data_id, group, "").await
    }

    async fn add_listener(
        &self,
        data_id: &str,
        group: &str,
        listener: Arc<dyn ConfigChangeListener>,
    ) -> Result<()> {
        self.add_listener(data_id, group, "", listener).await
    }

    async fn add_change_event_listener(
        &self,
        data_id: &str,
        group: &str,
        config_type: &str,
        listener: Arc<dyn listener::ConfigChangeEventListener>,
    ) -> Result<()> {
        self.add_change_event_listener(data_id, group, "", config_type, listener)
            .await
    }

    async fn remove_listener(&self, data_id: &str, group: &str) -> Result<()> {
        self.remove_listener(data_id, group, "").await
    }

    async fn remove_specific_listener(
        &self,
        data_id: &str,
        group: &str,
        listener: &Arc<dyn ConfigChangeListener>,
    ) -> Result<()> {
        self.remove_specific_listener(data_id, group, "", listener)
            .await
    }

    async fn get_server_status(&self) -> String {
        self.get_server_status().await
    }

    async fn shut_down(&self) {
        self.shutdown().await;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::cache::*;

    #[test]
    fn test_build_cache_key_empty_tenant() {
        assert_eq!(build_cache_key("id", "group", ""), "id+group");
    }

    #[test]
    fn test_build_cache_key_with_tenant() {
        assert_eq!(build_cache_key("id", "group", "tenant"), "id+group+tenant");
    }

    #[tokio::test]
    async fn test_shutdown_clears_cache() {
        let config = crate::grpc::GrpcClientConfig::default();
        let client = Arc::new(crate::grpc::GrpcClient::new(config).unwrap());
        let service = super::BatataConfigService::new(client);

        // Populate cache
        service.cache_map.insert(
            build_cache_key("id1", "group1", ""),
            CacheData::new("id1", "group1", ""),
        );
        service.cache_map.insert(
            build_cache_key("id2", "group2", "tenant"),
            CacheData::new("id2", "group2", "tenant"),
        );
        assert_eq!(service.cache_map.len(), 2);

        service.shutdown().await;
        assert_eq!(service.cache_map.len(), 0);
    }
}
