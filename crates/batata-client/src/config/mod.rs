//! Configuration management service
//!
//! Provides `BatataConfigService` for config CRUD operations,
//! config listening, and server push handling.

pub mod cache;
pub mod encryption;
pub mod filter;
pub mod listener;

use std::sync::Arc;

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
use tracing::{debug, error, info};

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
use self::listener::{ConfigChangeListener, ConfigResponse};

/// Nacos-compatible config service backed by gRPC.
pub struct BatataConfigService {
    grpc_client: Arc<GrpcClient>,
    cache_map: DashMap<String, CacheData>,
    filter_chain: Arc<tokio::sync::RwLock<ConfigFilterChainManager>>,
    local_processor: Option<Arc<LocalConfigInfoProcessor>>,
}

impl BatataConfigService {
    /// Create a new config service with the given gRPC client.
    pub fn new(grpc_client: Arc<GrpcClient>) -> Self {
        Self {
            grpc_client,
            cache_map: DashMap::new(),
            filter_chain: Arc::new(tokio::sync::RwLock::new(ConfigFilterChainManager::new())),
            local_processor: None,
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
        }
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

    /// Get a config value from the server.
    pub async fn get_config(&self, data_id: &str, group: &str, tenant: &str) -> Result<String> {
        let content = match self.get_config_inner(data_id, group, tenant).await {
            Ok(content) => content,
            Err(e) => {
                // Try failover if available
                if let Some(processor) = &self.local_processor {
                    if let Ok(Some(failover_content)) =
                        processor.get_failover(data_id, group, tenant)
                    {
                        tracing::warn!(
                            "Using failover config for dataId={}, group={}, tenant={}",
                            data_id,
                            group,
                            tenant
                        );
                        return Ok(failover_content);
                    }
                }
                return Err(e);
            }
        };

        Ok(content)
    }

    /// Get config from server with optional failover fallback
    async fn get_config_inner(&self, data_id: &str, group: &str, tenant: &str) -> Result<String> {
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
                tracing::warn!("Filter query failed: {}", e);
            }
        }

        let content = filter_resp.content;

        // Cache the content
        let key = build_cache_key(data_id, group, tenant);
        self.cache_map
            .entry(key)
            .and_modify(|cache| {
                cache.update_content(&content);
            })
            .or_insert_with(|| {
                let mut cache = CacheData::new(data_id, group, tenant);
                cache.update_content(&content);
                cache
            });

        // Save snapshot if available
        if let Some(processor) = &self.local_processor {
            if let Err(e) = processor.save_snapshot(data_id, group, tenant, Some(&content)) {
                tracing::warn!("Failed to save snapshot: {}", e);
            }
        }

        Ok(content)
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
                tracing::warn!("Filter publish failed: {}", e);
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

            if let Some(processor) = &self.local_processor {
                if let Err(e) =
                    processor.save_snapshot(data_id, group, tenant, Some(&filter_req.content))
                {
                    tracing::warn!("Failed to save snapshot: {}", e);
                }
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
            if let Some(processor) = &self.local_processor {
                if let Err(e) = processor.save_snapshot(data_id, group, tenant, None) {
                    tracing::warn!("Failed to remove snapshot: {}", e);
                }
            }
        }

        Ok(resp.response.success)
    }

    /// Add a listener for config changes.
    ///
    /// If this is the first listener for the config, a `ConfigBatchListenRequest`
    /// is sent to the server.
    pub async fn add_listener(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
        listener: Arc<dyn ConfigChangeListener>,
    ) -> Result<()> {
        let key = build_cache_key(data_id, group, tenant);
        let should_subscribe;

        {
            let mut entry = self
                .cache_map
                .entry(key.clone())
                .or_insert_with(|| CacheData::new(data_id, group, tenant));
            entry.add_listener(listener);
            should_subscribe = !entry.is_listening;
            if should_subscribe {
                entry.is_listening = true;
            }
        }

        if should_subscribe {
            self.send_listen_request(data_id, group, tenant, true)
                .await?;
            debug!(
                "Started listening for config: data_id={}, group={}, tenant={}",
                data_id, group, tenant
            );
        }

        Ok(())
    }

    /// Remove all listeners for a config.
    ///
    /// If no listeners remain, a `ConfigBatchListenRequest` (listen=false)
    /// is sent to the server.
    pub async fn remove_listener(&self, data_id: &str, group: &str, tenant: &str) -> Result<()> {
        let key = build_cache_key(data_id, group, tenant);
        let should_unsubscribe;

        {
            if let Some(mut entry) = self.cache_map.get_mut(&key) {
                entry.remove_all_listeners();
                should_unsubscribe = entry.is_listening;
                entry.is_listening = false;
            } else {
                return Ok(());
            }
        }

        if should_unsubscribe {
            self.send_listen_request(data_id, group, tenant, false)
                .await?;
            debug!(
                "Stopped listening for config: data_id={}, group={}, tenant={}",
                data_id, group, tenant
            );
        }

        Ok(())
    }

    /// Handle a `ConfigChangeNotifyRequest` from the server.
    ///
    /// Re-fetches the config and notifies listeners if the content changed.
    pub async fn handle_config_change_notify(&self, data_id: &str, group: &str, tenant: &str) {
        info!(
            "Config change notification: data_id={}, group={}, tenant={}",
            data_id, group, tenant
        );

        // Re-fetch from server
        match self.get_config(data_id, group, tenant).await {
            Ok(content) => {
                let key = build_cache_key(data_id, group, tenant);
                if let Some(entry) = self.cache_map.get(&key) {
                    let response = ConfigResponse {
                        data_id: data_id.to_string(),
                        group: group.to_string(),
                        tenant: tenant.to_string(),
                        content,
                    };

                    for listener in &entry.listeners {
                        listener.receive_config_info(response.clone());
                    }
                }
            }
            Err(e) => {
                error!(
                    "Failed to re-fetch config on change notify: data_id={}, error={}",
                    data_id, e
                );
            }
        }
    }

    /// Re-establish all config listen subscriptions (called after reconnect).
    pub async fn redo_listeners(&self) -> Result<()> {
        let listen_entries: Vec<(String, String, String, String)> = self
            .cache_map
            .iter()
            .filter(|entry| entry.is_listening)
            .map(|entry| {
                (
                    entry.data_id.clone(),
                    entry.group.clone(),
                    entry.tenant.clone(),
                    entry.md5.clone(),
                )
            })
            .collect();

        if listen_entries.is_empty() {
            return Ok(());
        }

        let contexts: Vec<ConfigListenContext> = listen_entries
            .iter()
            .map(|(data_id, group, tenant, md5)| ConfigListenContext {
                data_id: data_id.clone(),
                group: group.clone(),
                tenant: tenant.clone(),
                md5: md5.clone(),
            })
            .collect();

        let mut req = ConfigBatchListenRequest {
            config_request: ConfigRequest::default(),
            listen: true,
            config_listen_contexts: contexts,
        };
        req.config_request.request.request_id = uuid::Uuid::new_v4().to_string();

        let _resp: ConfigChangeBatchListenResponse = self.grpc_client.request_typed(&req).await?;

        info!(
            "Re-established {} config listen subscriptions",
            listen_entries.len()
        );

        Ok(())
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
/// When the server pushes a config change notification, this handler
/// delegates to `BatataConfigService` to re-fetch and notify listeners.
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
        let data_id = req.data_id.clone();
        let group = req.group.clone();
        let tenant = req.tenant.clone();

        let service = self.config_service.clone();
        tokio::spawn(async move {
            service
                .handle_config_change_notify(&data_id, &group, &tenant)
                .await;
        });

        // Send acknowledgment
        let resp = ConfigChangeNotifyResponse::new();
        Some(resp.build_payload())
    }
}

#[cfg(test)]
mod tests {
    use super::cache::*;

    #[test]
    fn test_build_cache_key_empty_tenant() {
        assert_eq!(build_cache_key("id", "group", ""), "id+group");
    }

    #[test]
    fn test_build_cache_key_with_tenant() {
        assert_eq!(build_cache_key("id", "group", "tenant"), "id+group+tenant");
    }
}
