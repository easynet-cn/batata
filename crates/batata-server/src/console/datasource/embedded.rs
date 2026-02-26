// Embedded local data source implementation
// Provides direct PersistenceService access for console operations in embedded mode
// (standalone_embedded and distributed_embedded), avoiding HTTP self-connection.

use async_trait::async_trait;
use std::sync::Arc;

use batata_core::cluster::ServerMemberManager;
use batata_persistence::PersistenceService;

use batata_config::Namespace;

use std::collections::{HashMap, HashSet};

use crate::{
    api::{
        config::model::{
            ConfigBasicInfo, ConfigDetailInfo, ConfigGrayInfo, ConfigHistoryBasicInfo,
            ConfigHistoryDetailInfo, ConfigListenerInfo,
        },
        model::{Member, Page},
        naming::model::Instance,
    },
    config::{
        export_model::{ImportResult, NacosExportItem, SameConfigPolicy},
        model::ConfigAllInfo,
    },
    console::v3::cluster::{
        ClusterHealthResponse, ClusterHealthSummaryResponse, SelfMemberResponse,
    },
    model::common::{
        APOLLO_ENABLED_STATE, APOLLO_PORT_STATE, AUTH_ADMIN_REQUEST, AUTH_ENABLED,
        AUTH_SYSTEM_TYPE, CONFIG_RENTENTION_DAYS_PROPERTY_STATE, CONSUL_ENABLED_STATE,
        CONSUL_PORT_STATE, Configuration, DATASOURCE_PLATFORM_PROPERTY_STATE,
        DEFAULT_CLUSTER_QUOTA, DEFAULT_GROUP_QUOTA, DEFAULT_MAX_AGGR_COUNT, DEFAULT_MAX_AGGR_SIZE,
        DEFAULT_MAX_SIZE, FUNCTION_MODE_STATE, IS_CAPACITY_LIMIT_CHECK, IS_HEALTH_CHECK,
        IS_MANAGE_CAPACITY, MAX_CONTENT, MAX_HEALTH_CHECK_FAIL_COUNT,
        NACOS_PLUGIN_DATASOURCE_LOG_STATE, NACOS_VERSION, NOTIFY_CONNECT_TIMEOUT,
        NOTIFY_SOCKET_TIMEOUT, SERVER_PORT_STATE, STARTUP_MODE_STATE,
    },
    service::naming::NamingService,
};

use super::ConsoleDataSource;

/// Embedded local data source — direct PersistenceService access
///
/// Used in standalone_embedded and distributed_embedded modes.
/// Avoids the HTTP self-connection pattern by calling PersistenceService
/// (backed by RocksDB) directly.
pub struct EmbeddedLocalDataSource {
    persistence: Arc<dyn PersistenceService>,
    server_member_manager: Arc<ServerMemberManager>,
    _config_subscriber_manager: Arc<batata_core::ConfigSubscriberManager>,
    configuration: Configuration,
    naming_service: Option<Arc<NamingService>>,
}

impl EmbeddedLocalDataSource {
    pub fn new(
        persistence: Arc<dyn PersistenceService>,
        server_member_manager: Arc<ServerMemberManager>,
        config_subscriber_manager: Arc<batata_core::ConfigSubscriberManager>,
        configuration: Configuration,
        naming_service: Option<Arc<NamingService>>,
    ) -> Self {
        Self {
            persistence,
            server_member_manager,
            _config_subscriber_manager: config_subscriber_manager,
            configuration,
            naming_service,
        }
    }
}

#[async_trait]
impl ConsoleDataSource for EmbeddedLocalDataSource {
    // ============== Namespace Operations ==============

    async fn namespace_find_all(&self) -> Vec<Namespace> {
        match self.persistence.namespace_find_all().await {
            Ok(infos) => infos.into_iter().map(Namespace::from).collect(),
            Err(e) => {
                tracing::error!("Failed to find all namespaces: {}", e);
                Vec::new()
            }
        }
    }

    async fn namespace_get_by_id(
        &self,
        namespace_id: &str,
        _tenant_id: &str,
    ) -> anyhow::Result<Namespace> {
        match self.persistence.namespace_get_by_id(namespace_id).await? {
            Some(info) => Ok(Namespace::from(info)),
            None => Err(anyhow::anyhow!("Namespace not found: {}", namespace_id)),
        }
    }

    async fn namespace_create(
        &self,
        namespace_id: &str,
        namespace_name: &str,
        namespace_desc: &str,
    ) -> anyhow::Result<()> {
        self.persistence
            .namespace_create(namespace_id, namespace_name, namespace_desc)
            .await
    }

    async fn namespace_update(
        &self,
        namespace_id: &str,
        namespace_name: &str,
        namespace_desc: &str,
    ) -> anyhow::Result<bool> {
        self.persistence
            .namespace_update(namespace_id, namespace_name, namespace_desc)
            .await
    }

    async fn namespace_delete(&self, namespace_id: &str) -> anyhow::Result<bool> {
        self.persistence.namespace_delete(namespace_id).await
    }

    async fn namespace_check(&self, namespace_id: &str) -> anyhow::Result<bool> {
        self.persistence.namespace_check(namespace_id).await
    }

    // ============== Config Operations ==============

    async fn config_find_one(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
    ) -> anyhow::Result<Option<ConfigAllInfo>> {
        let storage = self
            .persistence
            .config_find_one(data_id, group_name, namespace_id)
            .await?;
        Ok(storage.map(ConfigAllInfo::from))
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
        let result = self
            .persistence
            .config_search_page(
                page_no,
                page_size,
                namespace_id,
                data_id,
                group_name,
                app_name,
                tags,
                types,
                content,
            )
            .await?;

        Ok(Page::new(
            result.total_count,
            result.page_number,
            result.pages_available,
            result
                .page_items
                .into_iter()
                .map(ConfigBasicInfo::from)
                .collect(),
        ))
    }

    async fn config_create_or_update(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
        content: &str,
        app_name: &str,
        src_user: &str,
        src_ip: &str,
        config_tags: &str,
        desc: &str,
        r#use: &str,
        effect: &str,
        r#type: &str,
        schema: &str,
        encrypted_data_key: &str,
    ) -> anyhow::Result<()> {
        self.persistence
            .config_create_or_update(
                data_id,
                group_name,
                namespace_id,
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
            .await?;
        Ok(())
    }

    async fn config_delete(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
        tag: &str,
        client_ip: &str,
        src_user: &str,
        _caas_user: &str,
    ) -> anyhow::Result<()> {
        self.persistence
            .config_delete(data_id, group_name, namespace_id, tag, client_ip, src_user)
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
            .persistence
            .config_find_gray_one(data_id, group_name, namespace_id)
            .await?;
        Ok(result.map(|gray| ConfigGrayInfo {
            config_detail_info: ConfigDetailInfo {
                config_basic_info: ConfigBasicInfo {
                    id: 0,
                    namespace_id: gray.tenant,
                    group_name: gray.group,
                    data_id: gray.data_id,
                    md5: gray.md5,
                    r#type: String::new(),
                    app_name: gray.app_name,
                    create_time: gray.created_time,
                    modify_time: gray.modified_time,
                },
                content: gray.content,
                desc: String::new(),
                encrypted_data_key: gray.encrypted_data_key,
                create_user: gray.src_user,
                create_ip: gray.src_ip,
                config_tags: String::new(),
            },
            gray_name: gray.gray_name,
            gray_rule: gray.gray_rule,
        }))
    }

    async fn config_create_or_update_gray(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
        content: &str,
        gray_name: &str,
        gray_rule: &str,
        src_user: &str,
        src_ip: &str,
        app_name: &str,
        encrypted_data_key: &str,
    ) -> anyhow::Result<()> {
        self.persistence
            .config_create_or_update_gray(
                data_id,
                group_name,
                namespace_id,
                content,
                gray_name,
                gray_rule,
                src_user,
                src_ip,
                app_name,
                encrypted_data_key,
            )
            .await?;
        Ok(())
    }

    async fn config_delete_gray(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
        gray_name: &str,
        client_ip: &str,
        src_user: &str,
    ) -> anyhow::Result<()> {
        self.persistence
            .config_delete_gray(
                data_id,
                group_name,
                namespace_id,
                gray_name,
                client_ip,
                src_user,
            )
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
        let configs = self
            .persistence
            .config_find_for_export(namespace_id, group, data_ids, app_name)
            .await?;

        if configs.is_empty() {
            return Err(anyhow::anyhow!("No configurations found to export"));
        }

        let all_infos: Vec<ConfigAllInfo> = configs.into_iter().map(ConfigAllInfo::from).collect();
        batata_config::service::export::create_nacos_export_zip(all_infos)
    }

    async fn config_import(
        &self,
        file_data: Vec<u8>,
        namespace_id: &str,
        policy: SameConfigPolicy,
        src_user: &str,
        src_ip: &str,
    ) -> anyhow::Result<ImportResult> {
        let items = batata_config::service::import::parse_nacos_import_zip(&file_data)?;

        if items.is_empty() {
            return Err(anyhow::anyhow!("No configurations found in ZIP file"));
        }

        // Import via PersistenceService directly (no DatabaseConnection needed)
        self.import_configs_via_persistence(items, namespace_id, policy, src_user, src_ip)
            .await
    }

    // ============== History Operations ==============

    async fn history_find_by_id(
        &self,
        nid: u64,
    ) -> anyhow::Result<Option<ConfigHistoryDetailInfo>> {
        let result = self.persistence.config_history_find_by_id(nid).await?;
        Ok(result.map(ConfigHistoryDetailInfo::from))
    }

    async fn history_search_page(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<ConfigHistoryBasicInfo>> {
        let result = self
            .persistence
            .config_history_search_page(data_id, group_name, namespace_id, page_no, page_size)
            .await?;

        Ok(Page::<ConfigHistoryBasicInfo>::new(
            result.total_count,
            result.page_number,
            result.pages_available,
            result
                .page_items
                .into_iter()
                .map(ConfigHistoryBasicInfo::from)
                .collect(),
        ))
    }

    async fn history_find_configs_by_namespace_id(
        &self,
        namespace_id: &str,
    ) -> anyhow::Result<Vec<ConfigBasicInfo>> {
        // Use config_find_by_namespace instead of querying history
        let configs = self
            .persistence
            .config_find_by_namespace(namespace_id)
            .await?;
        Ok(configs.into_iter().map(ConfigBasicInfo::from).collect())
    }

    // ============== History Operations (Advanced) ==============

    async fn history_search_with_filters(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
        op_type: Option<&str>,
        src_user: Option<&str>,
        start_time: Option<i64>,
        end_time: Option<i64>,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<ConfigHistoryBasicInfo>> {
        let result = self
            .persistence
            .config_history_search_with_filters(
                data_id,
                group_name,
                namespace_id,
                op_type,
                src_user,
                start_time,
                end_time,
                page_no,
                page_size,
            )
            .await?;

        Ok(Page::<ConfigHistoryBasicInfo>::new(
            result.total_count,
            result.page_number,
            result.pages_available,
            result
                .page_items
                .into_iter()
                .map(ConfigHistoryBasicInfo::from)
                .collect(),
        ))
    }

    // ============== Service Operations ==============

    async fn service_list(
        &self,
        namespace_id: &str,
        group_name: &str,
        _service_name: &str,
        page_no: u64,
        page_size: u64,
        _has_ip_count: bool,
    ) -> anyhow::Result<(i32, Vec<serde_json::Value>)> {
        let naming = self
            .naming_service
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("NamingService not available"))?;

        let (total_count, service_names) =
            naming.list_services(namespace_id, group_name, page_no as i32, page_size as i32);

        let service_list: Vec<serde_json::Value> = service_names
            .iter()
            .map(|name| {
                let instances = naming.get_instances(namespace_id, group_name, name, "", false);
                let clusters: HashSet<_> =
                    instances.iter().map(|i| i.cluster_name.clone()).collect();
                let healthy_count = instances.iter().filter(|i| i.healthy && i.enabled).count();
                let metadata_opt = naming.get_service_metadata(namespace_id, group_name, name);
                let (protect_threshold, metadata, selector) = if let Some(meta) = metadata_opt {
                    let sel = if meta.selector_type != "none" && !meta.selector_type.is_empty() {
                        serde_json::json!({
                            "type": meta.selector_type,
                            "expression": meta.selector_expression,
                        })
                    } else {
                        serde_json::Value::Null
                    };
                    (
                        meta.protect_threshold,
                        if meta.metadata.is_empty() {
                            serde_json::Value::Null
                        } else {
                            serde_json::to_value(&meta.metadata).unwrap_or_default()
                        },
                        sel,
                    )
                } else {
                    (0.0, serde_json::Value::Null, serde_json::Value::Null)
                };

                serde_json::json!({
                    "name": name,
                    "groupName": group_name,
                    "clusterCount": clusters.len(),
                    "ipCount": instances.len(),
                    "healthyInstanceCount": healthy_count,
                    "triggerFlag": false,
                    "protectThreshold": protect_threshold,
                    "metadata": metadata,
                    "selector": selector,
                })
            })
            .collect();

        Ok((total_count, service_list))
    }

    async fn service_get(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        let naming = self
            .naming_service
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("NamingService not available"))?;

        if !naming.service_exists(namespace_id, group_name, service_name) {
            return Ok(None);
        }

        let instances = naming.get_instances(namespace_id, group_name, service_name, "", false);
        let clusters: HashSet<_> = instances.iter().map(|i| i.cluster_name.clone()).collect();
        let healthy_count = instances.iter().filter(|i| i.healthy && i.enabled).count();
        let metadata_opt = naming.get_service_metadata(namespace_id, group_name, service_name);

        let (protect_threshold, metadata, selector) = if let Some(meta) = metadata_opt {
            let sel = if meta.selector_type != "none" && !meta.selector_type.is_empty() {
                serde_json::json!({
                    "type": meta.selector_type,
                    "expression": meta.selector_expression,
                })
            } else {
                serde_json::Value::Null
            };
            (
                meta.protect_threshold,
                if meta.metadata.is_empty() {
                    serde_json::Value::Null
                } else {
                    serde_json::to_value(&meta.metadata).unwrap_or_default()
                },
                sel,
            )
        } else {
            (0.0, serde_json::Value::Null, serde_json::Value::Null)
        };

        Ok(Some(serde_json::json!({
            "name": service_name,
            "groupName": group_name,
            "clusterCount": clusters.len(),
            "ipCount": instances.len(),
            "healthyInstanceCount": healthy_count,
            "triggerFlag": false,
            "protectThreshold": protect_threshold,
            "metadata": metadata,
            "selector": selector,
        })))
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
        let naming = self
            .naming_service
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("NamingService not available"))?;

        if naming.service_exists(namespace_id, group_name, service_name) {
            return Err(anyhow::anyhow!("service {} already exists", service_name));
        }

        let metadata_map: HashMap<String, String> =
            serde_json::from_str(metadata).unwrap_or_default();

        let (selector_type, selector_expression) = if !selector.is_empty() {
            let selector_obj: serde_json::Value =
                serde_json::from_str(selector).unwrap_or_default();
            (
                selector_obj["type"].as_str().unwrap_or("none").to_string(),
                selector_obj["expression"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
            )
        } else {
            ("none".to_string(), String::new())
        };

        let service_metadata = batata_naming::service::ServiceMetadata {
            protect_threshold,
            metadata: metadata_map,
            selector_type,
            selector_expression,
        };

        naming.set_service_metadata(namespace_id, group_name, service_name, service_metadata);
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
        let naming = self
            .naming_service
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("NamingService not available"))?;

        if !naming.service_exists(namespace_id, group_name, service_name) {
            return Err(anyhow::anyhow!("service {} not found", service_name));
        }

        if let Some(threshold) = protect_threshold {
            naming.update_service_protect_threshold(
                namespace_id,
                group_name,
                service_name,
                threshold,
            );
        }

        if let Some(metadata_str) = metadata
            && let Ok(metadata_map) = serde_json::from_str::<HashMap<String, String>>(metadata_str)
        {
            naming.update_service_metadata_map(
                namespace_id,
                group_name,
                service_name,
                metadata_map,
            );
        }

        if let Some(selector_str) = selector {
            let selector_obj: serde_json::Value =
                serde_json::from_str(selector_str).unwrap_or_default();
            let selector_type = selector_obj["type"].as_str().unwrap_or("none");
            let selector_expression = selector_obj["expression"].as_str().unwrap_or("");
            naming.update_service_selector(
                namespace_id,
                group_name,
                service_name,
                selector_type,
                selector_expression,
            );
        }

        Ok(true)
    }

    async fn service_delete(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
    ) -> anyhow::Result<bool> {
        let naming = self
            .naming_service
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("NamingService not available"))?;

        if !naming.service_exists(namespace_id, group_name, service_name) {
            return Err(anyhow::anyhow!("service {} not found", service_name));
        }

        let instances = naming.get_instances(namespace_id, group_name, service_name, "", false);
        if !instances.is_empty() {
            return Err(anyhow::anyhow!(
                "service {} has {} instances, cannot delete",
                service_name,
                instances.len()
            ));
        }

        naming.delete_service_metadata(namespace_id, group_name, service_name);
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
        let naming = self
            .naming_service
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("NamingService not available"))?;

        let all_subscribers = naming.get_subscribers(namespace_id, group_name, service_name);
        let total = all_subscribers.len() as i32;

        let start = ((page_no.saturating_sub(1)) * page_size) as usize;
        let subscribers: Vec<String> = all_subscribers
            .into_iter()
            .skip(start)
            .take(page_size as usize)
            .collect();

        Ok((total, subscribers))
    }

    // ============== Instance Operations ==============

    async fn instance_list(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
    ) -> anyhow::Result<Vec<Instance>> {
        let naming = self
            .naming_service
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("NamingService not available"))?;

        Ok(naming.get_instances(namespace_id, group_name, service_name, cluster_name, false))
    }

    async fn instance_update(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        instance: Instance,
    ) -> anyhow::Result<bool> {
        let naming = self
            .naming_service
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("NamingService not available"))?;

        naming.register_instance(namespace_id, group_name, service_name, instance);
        Ok(true)
    }

    // ============== Config Listener Operations ==============

    async fn config_listener_list(
        &self,
        _data_id: &str,
        _group_name: &str,
        _namespace_id: &str,
    ) -> anyhow::Result<ConfigListenerInfo> {
        Ok(ConfigListenerInfo {
            query_type: ConfigListenerInfo::QUERY_TYPE_CONFIG.to_string(),
            listeners_status: HashMap::new(),
        })
    }

    // ============== Server State Operations ==============

    async fn server_state(&self) -> HashMap<String, Option<String>> {
        let cfg = &self.configuration;
        let mut state_map = HashMap::with_capacity(30);

        // Console server port
        state_map.insert(
            SERVER_PORT_STATE.to_string(),
            Some(format!("{}", cfg.console_server_port())),
        );

        // Config module state
        state_map.insert(
            DATASOURCE_PLATFORM_PROPERTY_STATE.to_string(),
            Some(cfg.datasource_platform()),
        );
        state_map.insert(
            NACOS_PLUGIN_DATASOURCE_LOG_STATE.to_string(),
            Some(format!("{}", cfg.plugin_datasource_log())),
        );
        state_map.insert(
            NOTIFY_CONNECT_TIMEOUT.to_string(),
            Some(format!("{}", cfg.notify_connect_timeout())),
        );
        state_map.insert(
            NOTIFY_SOCKET_TIMEOUT.to_string(),
            Some(format!("{}", cfg.notify_socket_timeout())),
        );
        state_map.insert(
            IS_HEALTH_CHECK.to_string(),
            Some(format!("{}", cfg.is_health_check())),
        );
        state_map.insert(
            MAX_HEALTH_CHECK_FAIL_COUNT.to_string(),
            Some(format!("{}", cfg.max_health_check_fail_count())),
        );
        state_map.insert(
            MAX_CONTENT.to_string(),
            Some(format!("{}", cfg.max_content())),
        );
        state_map.insert(
            IS_MANAGE_CAPACITY.to_string(),
            Some(format!("{}", cfg.is_manage_capacity())),
        );
        state_map.insert(
            IS_CAPACITY_LIMIT_CHECK.to_string(),
            Some(format!("{}", cfg.is_capacity_limit_check())),
        );
        state_map.insert(
            DEFAULT_CLUSTER_QUOTA.to_string(),
            Some(format!("{}", cfg.default_cluster_quota())),
        );
        state_map.insert(
            DEFAULT_GROUP_QUOTA.to_string(),
            Some(format!("{}", cfg.default_group_quota())),
        );
        state_map.insert(
            DEFAULT_MAX_SIZE.to_string(),
            Some(format!("{}", cfg.default_max_size())),
        );
        state_map.insert(
            DEFAULT_MAX_AGGR_COUNT.to_string(),
            Some(format!("{}", cfg.default_max_aggr_count())),
        );
        state_map.insert(
            DEFAULT_MAX_AGGR_SIZE.to_string(),
            Some(format!("{}", cfg.default_max_aggr_size())),
        );
        state_map.insert(
            CONFIG_RENTENTION_DAYS_PROPERTY_STATE.to_string(),
            Some(format!("{}", cfg.config_rentention_days())),
        );

        // Auth module state — use PersistenceService directly (no DB connection needed)
        let auth_enabled = cfg.auth_enabled();
        let global_admin = self
            .persistence
            .role_has_global_admin()
            .await
            .unwrap_or_else(|e| {
                tracing::error!("Failed to check global admin role: {}", e);
                false
            });
        state_map.insert(AUTH_ENABLED.to_string(), Some(format!("{}", auth_enabled)));
        state_map.insert(AUTH_SYSTEM_TYPE.to_string(), Some(cfg.auth_system_type()));
        state_map.insert(
            AUTH_ADMIN_REQUEST.to_string(),
            Some(format!("{}", auth_enabled && !global_admin)),
        );

        // Env module state
        state_map.insert(STARTUP_MODE_STATE.to_string(), Some(cfg.startup_mode()));
        state_map.insert(FUNCTION_MODE_STATE.to_string(), cfg.function_mode());
        state_map.insert(NACOS_VERSION.to_string(), Some(cfg.version()));

        // Console module state
        state_map.insert(
            "console_ui_enabled".to_string(),
            Some(format!("{}", cfg.console_ui_enabled())),
        );
        state_map.insert(
            "login_page_enabled".to_string(),
            Some(format!("{}", cfg.auth_console_enabled())),
        );

        // Plugin module state
        state_map.insert(
            CONSUL_ENABLED_STATE.to_string(),
            Some(format!("{}", cfg.consul_enabled())),
        );
        state_map.insert(
            CONSUL_PORT_STATE.to_string(),
            Some(format!("{}", cfg.consul_server_port())),
        );
        state_map.insert(
            APOLLO_ENABLED_STATE.to_string(),
            Some(format!("{}", cfg.apollo_enabled())),
        );
        state_map.insert(
            APOLLO_PORT_STATE.to_string(),
            Some(format!("{}", cfg.apollo_server_port())),
        );

        state_map
    }

    async fn server_readiness(&self) -> bool {
        self.persistence.health_check().await.is_ok()
    }

    // ============== Cluster Operations ==============

    fn cluster_all_members(&self) -> Vec<Member> {
        self.server_member_manager.all_members()
    }

    fn cluster_healthy_members(&self) -> Vec<Member> {
        self.server_member_manager.healthy_members()
    }

    fn cluster_get_health(&self) -> ClusterHealthResponse {
        let summary = self.server_member_manager.health_summary();
        let healthy = self.server_member_manager.is_cluster_healthy();
        let standalone_mode = self.server_member_manager.is_standalone();

        ClusterHealthResponse {
            is_healthy: healthy,
            summary: ClusterHealthSummaryResponse::from(summary),
            standalone: standalone_mode,
        }
    }

    fn cluster_get_self(&self) -> SelfMemberResponse {
        let self_member = self.server_member_manager.get_self();
        let version = self_member
            .extend_info
            .read()
            .ok()
            .and_then(|info| {
                info.get(Member::VERSION)
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or_default();

        SelfMemberResponse {
            ip: self_member.ip.clone(),
            port: self_member.port,
            address: self_member.address.clone(),
            state: self_member.state.to_string(),
            is_standalone: self.server_member_manager.is_standalone(),
            version,
        }
    }

    fn cluster_get_member(&self, address: &str) -> Option<Member> {
        self.server_member_manager.get_member(address)
    }

    fn cluster_member_count(&self) -> usize {
        self.server_member_manager.member_count()
    }

    fn cluster_is_standalone(&self) -> bool {
        self.server_member_manager.is_standalone()
    }

    fn cluster_refresh_self(&self) {
        self.server_member_manager.refresh_self();
    }

    async fn cluster_update_member_state(
        &self,
        address: &str,
        state: &str,
    ) -> anyhow::Result<String> {
        use batata_api::model::NodeState;

        let previous_state = match self.server_member_manager.get_member(address) {
            Some(member) => member.state.to_string(),
            None => return Err(anyhow::anyhow!("Member not found: {}", address)),
        };

        let new_state = match state.to_uppercase().as_str() {
            "UP" => NodeState::Up,
            "DOWN" => NodeState::Down,
            "SUSPICIOUS" => NodeState::Suspicious,
            "STARTING" => NodeState::Starting,
            "ISOLATION" => NodeState::Isolation,
            _ => return Err(anyhow::anyhow!("Invalid state: {}", state)),
        };

        self.server_member_manager
            .update_member_state(address, new_state)
            .await;

        Ok(previous_state)
    }

    fn cluster_is_leader(&self) -> bool {
        self.server_member_manager.is_leader()
    }

    fn cluster_leader_address(&self) -> Option<String> {
        self.server_member_manager.leader_address()
    }

    fn cluster_local_address(&self) -> String {
        self.server_member_manager.local_address().to_string()
    }

    // ============== Service Operations (Cluster) ==============

    async fn service_update_cluster(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
        health_checker_type: Option<&str>,
        metadata: Option<HashMap<String, String>>,
    ) -> anyhow::Result<bool> {
        let naming = self
            .naming_service
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("NamingService not available"))?;

        if let Some(checker_type) = health_checker_type {
            naming.update_cluster_health_check(
                namespace_id,
                group_name,
                service_name,
                cluster_name,
                checker_type,
                80,
                true,
            );
        }

        if let Some(meta) = metadata {
            naming.update_cluster_metadata(
                namespace_id,
                group_name,
                service_name,
                cluster_name,
                meta,
            );
        }

        Ok(true)
    }

    // ============== Helper Methods ==============

    fn is_remote(&self) -> bool {
        false
    }

    fn get_server_member_manager(&self) -> Option<Arc<ServerMemberManager>> {
        Some(self.server_member_manager.clone())
    }
}

// Private helper methods
impl EmbeddedLocalDataSource {
    /// Import configs via PersistenceService (no DatabaseConnection needed)
    async fn import_configs_via_persistence(
        &self,
        items: Vec<NacosExportItem>,
        namespace_id: &str,
        policy: SameConfigPolicy,
        src_user: &str,
        src_ip: &str,
    ) -> anyhow::Result<ImportResult> {
        let mut result = ImportResult::default();

        for item in items {
            let data_id = &item.metadata.data_id;
            let group = &item.metadata.group;
            let content = &item.content;
            let app_name = &item.metadata.app_name;
            let config_type = &item.metadata.content_type;
            let desc = &item.metadata.desc;
            let config_tags = &item.metadata.config_tags;
            let encrypted_data_key = &item.metadata.encrypted_data_key;

            // Check if config already exists
            let existing = self
                .persistence
                .config_find_one(data_id, group, namespace_id)
                .await?;

            if existing.is_some() {
                match policy {
                    SameConfigPolicy::Abort => {
                        result
                            .fail_data
                            .push(crate::config::export_model::ImportFailItem {
                                data_id: data_id.clone(),
                                group: group.clone(),
                                reason: "Config already exists".to_string(),
                            });
                        return Ok(result);
                    }
                    SameConfigPolicy::Skip => {
                        result.skip_count += 1;
                        continue;
                    }
                    SameConfigPolicy::Overwrite => {
                        // Fall through to create_or_update
                    }
                }
            }

            match self
                .persistence
                .config_create_or_update(
                    data_id,
                    group,
                    namespace_id,
                    content,
                    app_name,
                    src_user,
                    src_ip,
                    config_tags,
                    desc,
                    "",
                    "",
                    config_type,
                    "",
                    encrypted_data_key,
                )
                .await
            {
                Ok(_) => result.success_count += 1,
                Err(e) => {
                    result
                        .fail_data
                        .push(crate::config::export_model::ImportFailItem {
                            data_id: data_id.clone(),
                            group: group.clone(),
                            reason: e.to_string(),
                        });
                }
            }
        }

        Ok(result)
    }
}
