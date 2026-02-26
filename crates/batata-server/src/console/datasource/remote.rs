// Remote data source implementation
// Provides HTTP-based access to console operations via remote server

use async_trait::async_trait;
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
use batata_maintainer_client::{MaintainerClient, MaintainerClientConfig};

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
    console::v3::cluster::{
        ClusterHealthResponse, ClusterHealthSummaryResponse, SelfMemberResponse,
    },
    model::common::Configuration,
};

use super::ConsoleDataSource;

// ============== Type Conversions ==============

/// Build MaintainerClientConfig from server Configuration.
///
/// Server discovery order (same as Nacos 3.x cluster member lookup):
/// 1. `nacos.member.list` config property
/// 2. `conf/cluster.conf` file
/// 3. `nacos.console.remote.server_addr` (legacy fallback)
///
/// Authentication priority:
/// 1. Server identity headers (no login needed, used for console-server trust)
/// 2. JWT username/password (fallback for backward compatibility)
fn build_maintainer_config(configuration: &Configuration) -> MaintainerClientConfig {
    let server_addrs = configuration.resolve_remote_server_addrs();

    MaintainerClientConfig {
        server_addrs,
        // Server identity (primary auth, no login needed)
        server_identity_key: configuration.server_identity_key(),
        server_identity_value: configuration.server_identity_value(),
        // JWT fallback (kept for backward compatibility)
        username: configuration.console_remote_username(),
        password: configuration.console_remote_password(),
        connect_timeout_ms: configuration.console_remote_connect_timeout_ms(),
        read_timeout_ms: configuration.console_remote_read_timeout_ms(),
        context_path: configuration.server_context_path(),
    }
}

fn convert_namespace(v: batata_maintainer_client::model::Namespace) -> Namespace {
    Namespace {
        namespace: v.namespace,
        namespace_show_name: v.namespace_show_name,
        namespace_desc: v.namespace_desc,
        quota: v.quota,
        config_count: v.config_count,
        type_: v.type_,
    }
}

fn convert_config_basic_info(
    v: batata_maintainer_client::model::ConfigBasicInfo,
) -> ConfigBasicInfo {
    ConfigBasicInfo {
        id: v.id,
        namespace_id: v.namespace_id,
        group_name: v.group_name,
        data_id: v.data_id,
        md5: v.md5,
        r#type: v.r#type,
        app_name: v.app_name,
        create_time: v.create_time,
        modify_time: v.modify_time,
    }
}

fn convert_config_gray_info(v: batata_maintainer_client::model::ConfigGrayInfo) -> ConfigGrayInfo {
    use crate::api::config::model::ConfigDetailInfo;

    ConfigGrayInfo {
        config_detail_info: ConfigDetailInfo {
            config_basic_info: convert_config_basic_info(v.config_detail_info.config_basic_info),
            content: v.config_detail_info.content,
            desc: v.config_detail_info.desc,
            encrypted_data_key: v.config_detail_info.encrypted_data_key,
            create_user: v.config_detail_info.create_user,
            create_ip: v.config_detail_info.create_ip,
            config_tags: v.config_detail_info.config_tags,
        },
        gray_name: v.gray_name,
        gray_rule: v.gray_rule,
    }
}

fn convert_config_history_basic_info(
    v: batata_maintainer_client::model::ConfigHistoryBasicInfo,
) -> ConfigHistoryBasicInfo {
    ConfigHistoryBasicInfo {
        config_basic_info: convert_config_basic_info(v.config_basic_info),
        src_ip: v.src_ip,
        src_user: v.src_user,
        op_type: v.op_type,
        publish_type: v.publish_type,
    }
}

fn convert_config_history_detail_info(
    v: batata_maintainer_client::model::ConfigHistoryDetailInfo,
) -> ConfigHistoryDetailInfo {
    ConfigHistoryDetailInfo {
        config_history_basic_info: convert_config_history_basic_info(v.config_history_basic_info),
        content: v.content,
        encrypted_data_key: v.encrypted_data_key,
        gray_name: v.gray_name,
        ext_info: v.ext_info,
    }
}

fn convert_page<S, T>(v: batata_maintainer_client::model::Page<S>, f: fn(S) -> T) -> Page<T> {
    Page {
        total_count: v.total_count,
        page_number: v.page_number,
        pages_available: v.pages_available,
        page_items: v.page_items.into_iter().map(f).collect(),
    }
}

fn convert_import_result(v: batata_maintainer_client::model::ImportResult) -> ImportResult {
    use crate::config::export_model::ImportFailItem;

    ImportResult {
        success_count: v.success_count,
        skip_count: v.skip_count,
        fail_count: v.fail_count,
        fail_data: v
            .fail_data
            .into_iter()
            .map(|item| ImportFailItem {
                data_id: item.data_id,
                group: item.group,
                reason: item.reason,
            })
            .collect(),
    }
}

fn convert_member(v: batata_maintainer_client::model::Member) -> Member {
    use batata_api::model::NodeState;

    Member {
        ip: v.ip,
        port: v.port,
        state: v.state.parse::<NodeState>().unwrap_or_default(),
        extend_info: Default::default(),
        address: v.address,
        fail_access_cnt: v.fail_access_cnt,
    }
}

fn convert_cluster_health(
    v: batata_maintainer_client::model::ClusterHealthResponse,
) -> ClusterHealthResponse {
    ClusterHealthResponse {
        is_healthy: v.is_healthy,
        summary: ClusterHealthSummaryResponse {
            total: v.summary.total,
            up: v.summary.up,
            down: v.summary.down,
            suspicious: v.summary.suspicious,
            starting: v.summary.starting,
            isolation: v.summary.isolation,
        },
        standalone: v.standalone,
    }
}

fn convert_self_member(
    v: batata_maintainer_client::model::SelfMemberResponse,
) -> SelfMemberResponse {
    SelfMemberResponse {
        ip: v.ip,
        port: v.port,
        address: v.address,
        state: v.state,
        is_standalone: v.is_standalone,
        version: v.version,
    }
}

fn convert_config_detail_to_all_info(
    v: batata_maintainer_client::model::ConfigDetailInfo,
) -> ConfigAllInfo {
    use batata_config::model::{ConfigInfo, ConfigInfoBase};

    ConfigAllInfo {
        config_info: ConfigInfo {
            config_info_base: ConfigInfoBase {
                id: v.config_basic_info.id,
                data_id: v.config_basic_info.data_id,
                group: v.config_basic_info.group_name,
                content: v.content,
                md5: v.config_basic_info.md5,
                encrypted_data_key: v.encrypted_data_key,
            },
            tenant: v.config_basic_info.namespace_id,
            app_name: v.config_basic_info.app_name,
            r#type: v.config_basic_info.r#type,
        },
        create_time: v.config_basic_info.create_time,
        modify_time: v.config_basic_info.modify_time,
        create_user: v.create_user,
        create_ip: v.create_ip,
        desc: v.desc,
        r#use: String::new(),
        effect: String::new(),
        schema: String::new(),
        config_tags: v.config_tags,
    }
}

fn convert_instance(v: batata_maintainer_client::model::Instance) -> Instance {
    Instance {
        instance_id: v.instance_id,
        ip: v.ip,
        port: v.port,
        weight: v.weight,
        healthy: v.healthy,
        enabled: v.enabled,
        ephemeral: v.ephemeral,
        cluster_name: v.cluster_name,
        service_name: v.service_name,
        metadata: v.metadata,
        ..Default::default()
    }
}

fn convert_config_listener_info(
    v: batata_maintainer_client::model::ConfigListenerInfo,
) -> ConfigListenerInfo {
    ConfigListenerInfo {
        query_type: v.query_type,
        listeners_status: v.listeners_status,
    }
}

fn convert_same_config_policy(
    policy: SameConfigPolicy,
) -> batata_maintainer_client::model::SameConfigPolicy {
    match policy {
        SameConfigPolicy::Abort => batata_maintainer_client::model::SameConfigPolicy::Abort,
        SameConfigPolicy::Skip => batata_maintainer_client::model::SameConfigPolicy::Skip,
        SameConfigPolicy::Overwrite => batata_maintainer_client::model::SameConfigPolicy::Overwrite,
    }
}

// ============== RemoteDataSource ==============

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
    client: Arc<MaintainerClient>,
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
        let maintainer_config = build_maintainer_config(configuration);
        let client = Arc::new(MaintainerClient::new(maintainer_config).await?);

        let datasource = Self {
            client,
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

        let client = self.client.clone();
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
                match client.cluster_members().await {
                    Ok(members) => {
                        if let Ok(mut cache) = cached_members.write() {
                            *cache = members.into_iter().map(convert_member).collect();
                        }
                    }
                    Err(e) => {
                        warn!("Failed to refresh cluster members: {}", e);
                    }
                }

                // Refresh health
                match client.cluster_health().await {
                    Ok(health) => {
                        if let Ok(mut cache) = cached_health.write() {
                            *cache = Some(convert_cluster_health(health));
                        }
                    }
                    Err(e) => {
                        warn!("Failed to refresh cluster health: {}", e);
                    }
                }

                // Refresh self
                match client.cluster_self().await {
                    Ok(self_member) => {
                        if let Ok(mut cache) = cached_self.write() {
                            *cache = Some(convert_self_member(self_member));
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
        if let Ok(members) = self.client.cluster_members().await {
            let mut cache = self
                .cached_members
                .write()
                .unwrap_or_else(|e| e.into_inner());
            *cache = members.into_iter().map(convert_member).collect();
        }

        // Fetch health
        if let Ok(health) = self.client.cluster_health().await {
            let mut cache = self
                .cached_health
                .write()
                .unwrap_or_else(|e| e.into_inner());
            *cache = Some(convert_cluster_health(health));
        }

        // Fetch self
        if let Ok(self_member) = self.client.cluster_self().await {
            let mut cache = self.cached_self.write().unwrap_or_else(|e| e.into_inner());
            *cache = Some(convert_self_member(self_member));
        }
    }
}

#[async_trait]
impl ConsoleDataSource for RemoteDataSource {
    // ============== Namespace Operations ==============

    async fn namespace_find_all(&self) -> Vec<Namespace> {
        match self.client.namespace_list().await {
            Ok(namespaces) => namespaces.into_iter().map(convert_namespace).collect(),
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
        Ok(convert_namespace(
            self.client.namespace_get(namespace_id).await?,
        ))
    }

    async fn namespace_create(
        &self,
        namespace_id: &str,
        namespace_name: &str,
        namespace_desc: &str,
    ) -> anyhow::Result<()> {
        self.client
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
        self.client
            .namespace_update(namespace_id, namespace_name, namespace_desc)
            .await
    }

    async fn namespace_delete(&self, namespace_id: &str) -> anyhow::Result<bool> {
        self.client.namespace_delete(namespace_id).await
    }

    async fn namespace_check(&self, namespace_id: &str) -> anyhow::Result<bool> {
        self.client.namespace_exists(namespace_id).await
    }

    // ============== Config Operations ==============

    async fn config_find_one(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
    ) -> anyhow::Result<Option<ConfigAllInfo>> {
        let data = self
            .client
            .config_get(data_id, group_name, namespace_id)
            .await?;

        Ok(data.map(convert_config_detail_to_all_info))
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

        let page = self
            .client
            .config_search(
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
            .await?;
        Ok(convert_page(page, convert_config_basic_info))
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
        self.client
            .config_publish(
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
        self.client
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
        let result = self
            .client
            .config_get_beta(data_id, group_name, namespace_id)
            .await?;
        Ok(result.map(convert_config_gray_info))
    }

    async fn config_create_or_update_gray(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
        content: &str,
        _gray_name: &str,
        _gray_rule: &str,
        src_user: &str,
        _src_ip: &str,
        app_name: &str,
        _encrypted_data_key: &str,
    ) -> anyhow::Result<()> {
        // Remote mode delegates to MaintainerClient which calls the admin API
        // The admin API builds gray_rule from betaIps on the server side
        self.client
            .config_publish_beta(
                data_id,
                group_name,
                namespace_id,
                content,
                app_name,
                src_user,
                "", // config_tags
                "", // desc
                "", // type
                "", // beta_ips - the admin server reconstructs from gray_rule
            )
            .await?;
        Ok(())
    }

    async fn config_delete_gray(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
        _gray_name: &str,
        _client_ip: &str,
        _src_user: &str,
    ) -> anyhow::Result<()> {
        self.client
            .config_stop_beta(data_id, group_name, namespace_id)
            .await?;
        Ok(())
    }

    async fn config_export(
        &self,
        namespace_id: &str,
        group: Option<&str>,
        data_ids: Option<Vec<String>>,
        app_name: Option<&str>,
    ) -> anyhow::Result<Vec<u8>> {
        let data_ids_str = data_ids.map(|ids| ids.join(","));
        self.client
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
        let result = self
            .client
            .config_import(file_data, namespace_id, convert_same_config_policy(policy))
            .await?;
        Ok(convert_import_result(result))
    }

    async fn config_batch_delete(
        &self,
        _ids: &[i64],
        _client_ip: &str,
        _src_user: &str,
    ) -> anyhow::Result<()> {
        // Remote mode: not yet supported via client API
        Ok(())
    }

    async fn config_clone(
        &self,
        _ids: &[i64],
        _target_namespace_id: &str,
        _policy: &str,
        _src_user: &str,
        _src_ip: &str,
    ) -> anyhow::Result<ImportResult> {
        // Remote mode: not yet supported via client API
        Ok(ImportResult::default())
    }

    async fn config_listener_list_by_ip(
        &self,
        _ip: &str,
        _all: bool,
        _namespace_id: &str,
    ) -> anyhow::Result<ConfigListenerInfo> {
        Ok(ConfigListenerInfo {
            query_type: ConfigListenerInfo::QUERY_TYPE_IP.to_string(),
            listeners_status: std::collections::HashMap::new(),
        })
    }

    // ============== History Operations ==============

    async fn history_find_by_id(
        &self,
        nid: u64,
    ) -> anyhow::Result<Option<ConfigHistoryDetailInfo>> {
        let result = self.client.history_get(nid, "", "", "").await?;
        Ok(result.map(convert_config_history_detail_info))
    }

    async fn history_search_page(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<ConfigHistoryBasicInfo>> {
        let page = self
            .client
            .history_list(data_id, group_name, namespace_id, page_no, page_size)
            .await?;
        Ok(convert_page(page, convert_config_history_basic_info))
    }

    async fn history_find_configs_by_namespace_id(
        &self,
        namespace_id: &str,
    ) -> anyhow::Result<Vec<ConfigBasicInfo>> {
        let items = self
            .client
            .history_configs_by_namespace(namespace_id)
            .await?;
        Ok(items.into_iter().map(convert_config_basic_info).collect())
    }

    async fn history_find_previous(
        &self,
        _data_id: &str,
        _group_name: &str,
        _namespace_id: &str,
        _id: u64,
    ) -> anyhow::Result<Option<ConfigHistoryDetailInfo>> {
        // Remote mode: not yet supported via client API
        Ok(None)
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
        let page = self
            .client
            .history_list(data_id, group_name, namespace_id, page_no, page_size)
            .await?;
        Ok(convert_page(page, convert_config_history_basic_info))
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
        let page = self
            .client
            .service_list(namespace_id, group_name, service_name, page_no, page_size)
            .await?;

        let count = page.total_count as i32;
        let service_list = page
            .page_items
            .into_iter()
            .filter_map(|item| serde_json::to_value(item).ok())
            .collect();

        Ok((count, service_list))
    }

    async fn service_get(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        match self
            .client
            .service_get(namespace_id, group_name, service_name)
            .await
        {
            Ok(data) => {
                let value = serde_json::to_value(data)?;
                Ok(Some(value))
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
        self.client
            .service_create(
                namespace_id,
                group_name,
                service_name,
                protect_threshold,
                metadata,
                selector,
            )
            .await?;
        Ok(true)
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
        self.client
            .service_update(
                namespace_id,
                group_name,
                service_name,
                protect_threshold.unwrap_or(0.0),
                metadata.unwrap_or(""),
                selector.unwrap_or(""),
            )
            .await?;
        Ok(true)
    }

    async fn service_delete(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
    ) -> anyhow::Result<bool> {
        self.client
            .service_delete(namespace_id, group_name, service_name)
            .await?;
        Ok(true)
    }

    async fn service_subscriber_list(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<(i32, Vec<String>)> {
        let page = self
            .client
            .service_subscribers(namespace_id, group_name, service_name, page_no, page_size)
            .await?;

        let count = page.total_count as i32;
        let subscribers = page
            .page_items
            .into_iter()
            .map(|s| format!("{}:{}", s.ip, s.port))
            .collect();

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
        let instances = self
            .client
            .instance_list(namespace_id, group_name, service_name, cluster_name)
            .await?;

        Ok(instances.into_iter().map(convert_instance).collect())
    }

    async fn instance_update(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        instance: Instance,
    ) -> anyhow::Result<bool> {
        let metadata_str = serde_json::to_string(&instance.metadata).unwrap_or_default();
        self.client
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
            .await?;
        Ok(true)
    }

    // ============== Config Listener Operations ==============

    async fn config_listener_list(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
    ) -> anyhow::Result<ConfigListenerInfo> {
        let data = self
            .client
            .config_listeners(data_id, group_name, namespace_id)
            .await?;

        Ok(convert_config_listener_info(data))
    }

    // ============== Server State Operations ==============

    async fn server_state(&self) -> HashMap<String, Option<String>> {
        match self.client.server_state().await {
            Ok(state) => state,
            Err(e) => {
                warn!("Failed to fetch server state from remote server: {}", e);
                HashMap::new()
            }
        }
    }

    async fn server_readiness(&self) -> bool {
        // In remote mode, check if we can reach the server
        self.client.server_readiness().await
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

    fn get_server_member_manager(&self) -> Option<Arc<ServerMemberManager>> {
        None
    }
}
