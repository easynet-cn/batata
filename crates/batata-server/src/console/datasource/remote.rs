// Remote data source implementation
// Provides HTTP-based access to console operations via remote server

use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use std::{
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tracing::{debug, info, warn};

use batata_core::cluster::ServerMemberManager;

use batata_config::Namespace;

use std::collections::HashMap;

use crate::{
    api::{
        config::model::{
            ConfigBasicInfo, ConfigGrayInfo, ConfigHistoryBasicInfo, ConfigHistoryDetailInfo,
            ConfigListenerInfo,
        },
        model::{Member, Page},
        naming::model::Instance,
    },
    config::export_model::{ImportResult, SameConfigPolicy},
    config::model::ConfigAllInfo,
    console::{
        client::{ConsoleApiClient, ConsoleHttpClient, http_client::RemoteConsoleConfig},
        v3::cluster::{ClusterHealthResponse, ClusterHealthSummaryResponse, SelfMemberResponse},
    },
    model::common::Configuration,
};

use super::ConsoleDataSource;

/// Configuration for auto-refresh behavior
#[derive(Clone, Debug)]
pub struct AutoRefreshConfig {
    /// Whether auto-refresh is enabled
    pub enabled: bool,
    /// Refresh interval
    pub interval: Duration,
    /// Initial delay before starting refresh
    pub initial_delay: Duration,
}

impl Default for AutoRefreshConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval: Duration::from_secs(30),
            initial_delay: Duration::from_secs(5),
        }
    }
}

/// Remote data source - HTTP-based access to remote server
pub struct RemoteDataSource {
    api_client: Arc<ConsoleApiClient>,
    // Cached cluster info (refreshed periodically)
    cached_members: Arc<RwLock<Vec<Member>>>,
    cached_health: Arc<RwLock<Option<ClusterHealthResponse>>>,
    cached_self: Arc<RwLock<Option<SelfMemberResponse>>>,
    // Auto-refresh state
    auto_refresh_config: AutoRefreshConfig,
    running: Arc<AtomicBool>,
}

impl RemoteDataSource {
    pub async fn new(configuration: &Configuration) -> anyhow::Result<Self> {
        Self::with_auto_refresh(configuration, AutoRefreshConfig::default()).await
    }

    /// Create with custom auto-refresh configuration
    pub async fn with_auto_refresh(
        configuration: &Configuration,
        auto_refresh_config: AutoRefreshConfig,
    ) -> anyhow::Result<Self> {
        let remote_config = RemoteConsoleConfig::from_configuration(configuration);
        let http_client = ConsoleHttpClient::new(remote_config).await?;
        let api_client = Arc::new(ConsoleApiClient::new(http_client));

        let datasource = Self {
            api_client,
            cached_members: Arc::new(RwLock::new(Vec::new())),
            cached_health: Arc::new(RwLock::new(None)),
            cached_self: Arc::new(RwLock::new(None)),
            auto_refresh_config,
            running: Arc::new(AtomicBool::new(false)),
        };

        // Pre-fetch cluster info
        datasource.refresh_cluster_cache().await;

        // Start auto-refresh if enabled
        if datasource.auto_refresh_config.enabled {
            datasource.start_auto_refresh();
        }

        Ok(datasource)
    }

    /// Start the auto-refresh background task
    pub fn start_auto_refresh(&self) {
        if self
            .running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            debug!("Auto-refresh already running");
            return;
        }

        info!(
            "Starting remote data source auto-refresh (interval: {:?})",
            self.auto_refresh_config.interval
        );

        let api_client = self.api_client.clone();
        let cached_members = self.cached_members.clone();
        let cached_health = self.cached_health.clone();
        let cached_self = self.cached_self.clone();
        let config = self.auto_refresh_config.clone();
        let running = self.running.clone();

        tokio::spawn(async move {
            // Initial delay
            tokio::time::sleep(config.initial_delay).await;

            while running.load(Ordering::SeqCst) {
                debug!("Refreshing remote data source cache");

                // Refresh members
                match api_client.cluster_all_members().await {
                    Ok(members) => {
                        if let Ok(mut cache) = cached_members.write() {
                            *cache = members;
                        }
                    }
                    Err(e) => {
                        warn!("Failed to refresh cluster members: {}", e);
                    }
                }

                // Refresh health
                match api_client.cluster_get_health().await {
                    Ok(health) => {
                        if let Ok(mut cache) = cached_health.write() {
                            *cache = Some(health);
                        }
                    }
                    Err(e) => {
                        warn!("Failed to refresh cluster health: {}", e);
                    }
                }

                // Refresh self
                match api_client.cluster_get_self().await {
                    Ok(self_member) => {
                        if let Ok(mut cache) = cached_self.write() {
                            *cache = Some(self_member);
                        }
                    }
                    Err(e) => {
                        warn!("Failed to refresh self member: {}", e);
                    }
                }

                // Wait for next refresh
                tokio::time::sleep(config.interval).await;
            }

            info!("Remote data source auto-refresh stopped");
        });
    }

    /// Stop the auto-refresh background task
    pub fn stop_auto_refresh(&self) {
        self.running.store(false, Ordering::SeqCst);
        info!("Stopping remote data source auto-refresh");
    }

    /// Check if auto-refresh is running
    pub fn is_auto_refresh_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Manually trigger a cache refresh
    pub async fn trigger_refresh(&self) {
        self.refresh_cluster_cache().await;
    }

    /// Refresh cached cluster information
    async fn refresh_cluster_cache(&self) {
        // Fetch members
        if let Ok(members) = self.api_client.cluster_all_members().await {
            let mut cache = self
                .cached_members
                .write()
                .unwrap_or_else(|e| e.into_inner());
            *cache = members;
        }

        // Fetch health
        if let Ok(health) = self.api_client.cluster_get_health().await {
            let mut cache = self
                .cached_health
                .write()
                .unwrap_or_else(|e| e.into_inner());
            *cache = Some(health);
        }

        // Fetch self
        if let Ok(self_member) = self.api_client.cluster_get_self().await {
            let mut cache = self.cached_self.write().unwrap_or_else(|e| e.into_inner());
            *cache = Some(self_member);
        }
    }
}

#[async_trait]
impl ConsoleDataSource for RemoteDataSource {
    // ============== Namespace Operations ==============

    async fn namespace_find_all(&self) -> Vec<Namespace> {
        match self.api_client.namespace_find_all().await {
            Ok(namespaces) => namespaces,
            Err(e) => {
                warn!("Failed to fetch namespaces from remote server: {}", e);
                Vec::new()
            }
        }
    }

    async fn namespace_get_by_id(
        &self,
        namespace_id: &str,
        _tenant_id: &str,
    ) -> anyhow::Result<Namespace> {
        self.api_client.namespace_get_by_id(namespace_id).await
    }

    async fn namespace_create(
        &self,
        namespace_id: &str,
        namespace_name: &str,
        namespace_desc: &str,
    ) -> anyhow::Result<()> {
        self.api_client
            .namespace_create(namespace_id, namespace_name, namespace_desc)
            .await?;
        Ok(())
    }

    async fn namespace_update(
        &self,
        namespace_id: &str,
        namespace_name: &str,
        namespace_desc: &str,
    ) -> anyhow::Result<bool> {
        self.api_client
            .namespace_update(namespace_id, namespace_name, namespace_desc)
            .await
    }

    async fn namespace_delete(&self, namespace_id: &str) -> anyhow::Result<bool> {
        self.api_client.namespace_delete(namespace_id).await
    }

    async fn namespace_check(&self, namespace_id: &str) -> anyhow::Result<bool> {
        self.api_client.namespace_check(namespace_id).await
    }

    // ============== Config Operations ==============

    async fn config_find_one(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
    ) -> anyhow::Result<Option<ConfigAllInfo>> {
        self.api_client
            .config_find_one(data_id, group_name, namespace_id)
            .await
    }

    async fn config_search_page(
        &self,
        page_no: u64,
        page_size: u64,
        namespace_id: &str,
        data_id: &str,
        group_name: &str,
        app_name: &str,
        tags: Vec<String>,
        types: Vec<String>,
        content: &str,
    ) -> anyhow::Result<Page<ConfigBasicInfo>> {
        let tags_str = tags.join(",");
        let types_str = types.join(",");

        self.api_client
            .config_search_page(
                page_no,
                page_size,
                namespace_id,
                data_id,
                group_name,
                app_name,
                &tags_str,
                &types_str,
                content,
            )
            .await
    }

    async fn config_create_or_update(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
        content: &str,
        app_name: &str,
        _src_user: &str,
        _src_ip: &str,
        config_tags: &str,
        desc: &str,
        r#use: &str,
        effect: &str,
        r#type: &str,
        schema: &str,
        encrypted_data_key: &str,
    ) -> anyhow::Result<()> {
        self.api_client
            .config_create_or_update(
                data_id,
                group_name,
                namespace_id,
                content,
                app_name,
                config_tags,
                desc,
                r#use,
                effect,
                r#type,
                schema,
                encrypted_data_key,
            )
            .await?;
        Ok(())
    }

    async fn config_delete(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
        _tag: &str,
        _client_ip: &str,
        _src_user: &str,
        _caas_user: &str,
    ) -> anyhow::Result<()> {
        self.api_client
            .config_delete(data_id, group_name, namespace_id)
            .await?;
        Ok(())
    }

    async fn config_find_gray_one(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
    ) -> anyhow::Result<Option<ConfigGrayInfo>> {
        self.api_client
            .config_find_gray_one(data_id, group_name, namespace_id)
            .await
    }

    async fn config_export(
        &self,
        namespace_id: &str,
        group: Option<&str>,
        data_ids: Option<Vec<String>>,
        app_name: Option<&str>,
    ) -> anyhow::Result<Vec<u8>> {
        let data_ids_str = data_ids.map(|ids| ids.join(","));
        self.api_client
            .config_export(namespace_id, group, data_ids_str.as_deref(), app_name)
            .await
    }

    async fn config_import(
        &self,
        file_data: Vec<u8>,
        namespace_id: &str,
        policy: SameConfigPolicy,
        _src_user: &str,
        _src_ip: &str,
    ) -> anyhow::Result<ImportResult> {
        // Import configuration via HTTP API
        self.api_client
            .config_import(file_data, namespace_id, policy)
            .await
    }

    // ============== History Operations ==============

    async fn history_find_by_id(
        &self,
        nid: u64,
    ) -> anyhow::Result<Option<ConfigHistoryDetailInfo>> {
        // The remote API requires data_id, group_name, namespace_id
        // For now, we'll pass empty strings and let the API handle it by nid
        self.api_client.history_find_by_id(nid, "", "", "").await
    }

    async fn history_search_page(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<ConfigHistoryBasicInfo>> {
        self.api_client
            .history_search_page(data_id, group_name, namespace_id, page_no, page_size)
            .await
    }

    async fn history_find_configs_by_namespace_id(
        &self,
        namespace_id: &str,
    ) -> anyhow::Result<Vec<ConfigBasicInfo>> {
        self.api_client
            .history_find_configs_by_namespace_id(namespace_id)
            .await
    }

    // ============== History Operations (Advanced) ==============

    async fn history_search_with_filters(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
        _op_type: Option<&str>,
        _src_user: Option<&str>,
        _start_time: Option<i64>,
        _end_time: Option<i64>,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<ConfigHistoryBasicInfo>> {
        // Remote mode: fall back to basic search (admin API may not support all filters)
        self.api_client
            .history_search_page(data_id, group_name, namespace_id, page_no, page_size)
            .await
    }

    // ============== Service Operations ==============

    async fn service_list(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        page_no: u64,
        page_size: u64,
        _has_ip_count: bool,
    ) -> anyhow::Result<(i32, Vec<serde_json::Value>)> {
        let data = self
            .api_client
            .service_list(namespace_id, group_name, service_name, page_no, page_size)
            .await?;

        let count = data.get("count").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let service_list = data
            .get("serviceList")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        Ok((count, service_list))
    }

    async fn service_get(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        match self
            .api_client
            .service_get(namespace_id, group_name, service_name)
            .await
        {
            Ok(data) => {
                if data.is_null() {
                    Ok(None)
                } else {
                    Ok(Some(data))
                }
            }
            Err(_) => Ok(None),
        }
    }

    async fn service_create(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        protect_threshold: f32,
        metadata: &str,
        selector: &str,
    ) -> anyhow::Result<bool> {
        self.api_client
            .service_create(
                namespace_id,
                group_name,
                service_name,
                protect_threshold,
                metadata,
                selector,
            )
            .await
    }

    async fn service_update(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        protect_threshold: Option<f32>,
        metadata: Option<&str>,
        selector: Option<&str>,
    ) -> anyhow::Result<bool> {
        self.api_client
            .service_update(
                namespace_id,
                group_name,
                service_name,
                protect_threshold.unwrap_or(0.0),
                metadata.unwrap_or(""),
                selector.unwrap_or(""),
            )
            .await
    }

    async fn service_delete(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
    ) -> anyhow::Result<bool> {
        self.api_client
            .service_delete(namespace_id, group_name, service_name)
            .await
    }

    async fn service_subscriber_list(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<(i32, Vec<String>)> {
        let data = self
            .api_client
            .service_subscriber_list(namespace_id, group_name, service_name, page_no, page_size)
            .await?;

        let count = data.get("count").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let subscribers = data
            .get("subscribers")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| {
                        v.get("addrStr")
                            .and_then(|s| s.as_str())
                            .map(|s| s.to_string())
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok((count, subscribers))
    }

    // ============== Instance Operations ==============

    async fn instance_list(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
    ) -> anyhow::Result<Vec<Instance>> {
        let data = self
            .api_client
            .instance_list(namespace_id, group_name, service_name, cluster_name)
            .await?;

        let hosts = data
            .get("hosts")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| serde_json::from_value::<Instance>(v.clone()).ok())
                    .collect()
            })
            .unwrap_or_default();

        Ok(hosts)
    }

    async fn instance_update(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        instance: Instance,
    ) -> anyhow::Result<bool> {
        let metadata_str = serde_json::to_string(&instance.metadata).unwrap_or_default();
        self.api_client
            .instance_update(
                namespace_id,
                group_name,
                service_name,
                &instance.ip,
                instance.port,
                &instance.cluster_name,
                instance.weight,
                instance.enabled,
                &metadata_str,
            )
            .await
    }

    // ============== Config Listener Operations ==============

    async fn config_listener_list(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
    ) -> anyhow::Result<ConfigListenerInfo> {
        let data = self
            .api_client
            .config_listener_list(data_id, group_name, namespace_id)
            .await?;

        // Try to deserialize from remote response, fall back to empty
        let listener_info: ConfigListenerInfo =
            serde_json::from_value(data).unwrap_or(ConfigListenerInfo {
                query_type: ConfigListenerInfo::QUERY_TYPE_CONFIG.to_string(),
                listeners_status: HashMap::new(),
            });

        Ok(listener_info)
    }

    // ============== Server State Operations ==============

    async fn server_state(&self) -> HashMap<String, Option<String>> {
        match self.api_client.server_state().await {
            Ok(state) => state,
            Err(e) => {
                warn!("Failed to fetch server state from remote server: {}", e);
                HashMap::new()
            }
        }
    }

    async fn server_readiness(&self) -> bool {
        // In remote mode, check if we can reach the server
        self.api_client.cluster_get_health().await.is_ok()
    }

    // ============== Cluster Operations ==============

    fn cluster_all_members(&self) -> Vec<Member> {
        self.cached_members
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    fn cluster_healthy_members(&self) -> Vec<Member> {
        self.cached_members
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .filter(|m| m.state.to_string() == "UP")
            .cloned()
            .collect()
    }

    fn cluster_get_health(&self) -> ClusterHealthResponse {
        self.cached_health
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .unwrap_or(ClusterHealthResponse {
                is_healthy: false,
                summary: ClusterHealthSummaryResponse {
                    total: 0,
                    up: 0,
                    down: 0,
                    suspicious: 0,
                    starting: 0,
                    isolation: 0,
                },
                standalone: true,
            })
    }

    fn cluster_get_self(&self) -> SelfMemberResponse {
        self.cached_self
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .unwrap_or(SelfMemberResponse {
                ip: "0.0.0.0".to_string(),
                port: 0,
                address: "not-initialized".to_string(),
                state: "STARTING".to_string(),
                is_standalone: true,
                version: env!("CARGO_PKG_VERSION").to_string(),
            })
    }

    fn cluster_get_member(&self, address: &str) -> Option<Member> {
        self.cached_members
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .find(|m| m.address == address)
            .cloned()
    }

    fn cluster_member_count(&self) -> usize {
        self.cached_members
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .len()
    }

    fn cluster_is_standalone(&self) -> bool {
        self.cached_health
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(|h| h.standalone)
            .unwrap_or(true)
    }

    fn cluster_refresh_self(&self) {
        // Remote data source uses HTTP API, cache is refreshed periodically via other means
        // This method is a no-op for remote sources as the HTTP client handles connection failover
        // and the cluster cache is refreshed in the constructor and by other operations
    }

    async fn cluster_update_member_state(
        &self,
        _address: &str,
        _state: &str,
    ) -> anyhow::Result<String> {
        // In remote mode, member state updates are not supported directly
        Err(anyhow::anyhow!(
            "Member state update not supported in remote console mode"
        ))
    }

    fn cluster_is_leader(&self) -> bool {
        false
    }

    fn cluster_leader_address(&self) -> Option<String> {
        None
    }

    fn cluster_local_address(&self) -> String {
        self.cluster_get_self().address
    }

    // ============== Service Operations (Cluster) ==============

    async fn service_update_cluster(
        &self,
        _namespace_id: &str,
        _group_name: &str,
        _service_name: &str,
        _cluster_name: &str,
        _health_checker_type: Option<&str>,
        _metadata: Option<HashMap<String, String>>,
    ) -> anyhow::Result<bool> {
        // In remote mode, cluster updates would go through the admin API
        Err(anyhow::anyhow!(
            "Service cluster update not supported in remote console mode"
        ))
    }

    // ============== Helper Methods ==============

    fn is_remote(&self) -> bool {
        true
    }

    fn get_database_connection(&self) -> Option<&DatabaseConnection> {
        None
    }

    fn get_server_member_manager(&self) -> Option<Arc<ServerMemberManager>> {
        None
    }
}
