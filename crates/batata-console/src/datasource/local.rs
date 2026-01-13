//! Local data source implementation
//!
//! Provides direct database access for console operations when running co-located with the server.

use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

use batata_api::Page;
use batata_api::model::Member as ApiMember;
use batata_config::{
    ConfigAllInfo, ConfigBasicInfo, ConfigHistoryInfo, ConfigInfoGrayWrapper, ConfigInfoWrapper,
    ImportResult, Namespace, SameConfigPolicy,
    service::config::CloneResult,
};
use batata_core::cluster::ServerMemberManager;
use batata_naming::NamingService;

use super::{
    ClusterInfo, ConfigListenerInfo, ConsoleDataSource, HealthChecker, InstanceInfo,
    ServiceDetail, ServiceListItem, ServiceSelector, SubscriberInfo,
};
use crate::model::{ClusterHealthResponse, ClusterHealthSummary, Member, SelfMemberResponse};

/// Local data source - direct database access
pub struct LocalDataSource {
    database_connection: DatabaseConnection,
    server_member_manager: Arc<ServerMemberManager>,
    naming_service: Arc<NamingService>,
    config_subscriber_manager: Option<Arc<batata_core::ConfigSubscriberManager>>,
}

impl LocalDataSource {
    pub fn new(
        database_connection: DatabaseConnection,
        server_member_manager: Arc<ServerMemberManager>,
    ) -> Self {
        Self {
            database_connection,
            server_member_manager,
            naming_service: Arc::new(NamingService::new()),
            config_subscriber_manager: None,
        }
    }

    pub fn with_naming_service(
        database_connection: DatabaseConnection,
        server_member_manager: Arc<ServerMemberManager>,
        naming_service: Arc<NamingService>,
    ) -> Self {
        Self {
            database_connection,
            server_member_manager,
            naming_service,
            config_subscriber_manager: None,
        }
    }

    /// Create with config subscriber manager for config listener tracking
    pub fn with_config_subscriber_manager(
        database_connection: DatabaseConnection,
        server_member_manager: Arc<ServerMemberManager>,
        naming_service: Arc<NamingService>,
        config_subscriber_manager: Arc<batata_core::ConfigSubscriberManager>,
    ) -> Self {
        Self {
            database_connection,
            server_member_manager,
            naming_service,
            config_subscriber_manager: Some(config_subscriber_manager),
        }
    }
}

#[async_trait]
impl ConsoleDataSource for LocalDataSource {
    // ============== Namespace Operations ==============

    async fn namespace_list(&self) -> Vec<Namespace> {
        batata_config::service::namespace::find_all(&self.database_connection).await
    }

    async fn namespace_get(&self, namespace_id: &str) -> anyhow::Result<Namespace> {
        batata_config::service::namespace::get_by_namespace_id(
            &self.database_connection,
            namespace_id,
            "",
        )
        .await
    }

    async fn namespace_create(
        &self,
        namespace_id: &str,
        namespace_name: &str,
        namespace_desc: &str,
    ) -> anyhow::Result<bool> {
        batata_config::service::namespace::create(
            &self.database_connection,
            namespace_id,
            namespace_name,
            namespace_desc,
        )
        .await?;
        Ok(true)
    }

    async fn namespace_update(
        &self,
        namespace_id: &str,
        namespace_name: &str,
        namespace_desc: &str,
    ) -> anyhow::Result<bool> {
        batata_config::service::namespace::update(
            &self.database_connection,
            namespace_id,
            namespace_name,
            namespace_desc,
        )
        .await
    }

    async fn namespace_delete(&self, namespace_id: &str) -> anyhow::Result<bool> {
        batata_config::service::namespace::delete(&self.database_connection, namespace_id).await
    }

    async fn namespace_exists(&self, namespace_id: &str) -> anyhow::Result<bool> {
        batata_config::service::namespace::check(&self.database_connection, namespace_id).await
    }

    // ============== Config Operations ==============

    async fn config_get(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
    ) -> anyhow::Result<Option<ConfigAllInfo>> {
        batata_config::service::find_one(
            &self.database_connection,
            data_id,
            group_name,
            namespace_id,
        )
        .await
    }

    async fn config_list(
        &self,
        page_no: u64,
        page_size: u64,
        namespace_id: &str,
        data_id: &str,
        group_name: &str,
        app_name: &str,
        tags: &str,
        types: &str,
        content: &str,
    ) -> anyhow::Result<Page<ConfigBasicInfo>> {
        let tags_vec: Vec<String> = if tags.is_empty() {
            Vec::new()
        } else {
            tags.split(',').map(|s| s.trim().to_string()).collect()
        };
        let types_vec: Vec<String> = if types.is_empty() {
            Vec::new()
        } else {
            types.split(',').map(|s| s.trim().to_string()).collect()
        };

        batata_config::service::search_page(
            &self.database_connection,
            page_no,
            page_size,
            namespace_id,
            data_id,
            group_name,
            app_name,
            tags_vec,
            types_vec,
            content,
        )
        .await
    }

    async fn config_publish(
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
    ) -> anyhow::Result<bool> {
        batata_config::service::create_or_update(
            &self.database_connection,
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
        Ok(true)
    }

    async fn config_delete(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
        gray_name: &str,
        client_ip: &str,
        src_user: &str,
    ) -> anyhow::Result<bool> {
        batata_config::service::delete(
            &self.database_connection,
            data_id,
            group_name,
            namespace_id,
            gray_name,
            client_ip,
            src_user,
        )
        .await
    }

    async fn config_gray_get(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
    ) -> anyhow::Result<Option<ConfigInfoGrayWrapper>> {
        batata_config::service::find_gray_one(
            &self.database_connection,
            data_id,
            group_name,
            namespace_id,
        )
        .await
    }

    async fn config_export(
        &self,
        namespace_id: &str,
        group: Option<&str>,
        data_ids: Option<&str>,
        app_name: Option<&str>,
    ) -> anyhow::Result<Vec<u8>> {
        let data_ids_vec = data_ids.map(|s| {
            s.split(',')
                .map(|id| id.trim().to_string())
                .collect::<Vec<_>>()
        });

        let configs = batata_config::service::export::find_configs_for_export(
            &self.database_connection,
            namespace_id,
            group,
            data_ids_vec,
            app_name,
        )
        .await?;

        if configs.is_empty() {
            return Err(anyhow::anyhow!("No configurations found to export"));
        }

        batata_config::service::export::create_nacos_export_zip(configs)
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

        batata_config::service::import::import_nacos_items(
            &self.database_connection,
            items,
            namespace_id,
            policy,
            src_user,
            src_ip,
        )
        .await
    }

    async fn config_batch_delete(
        &self,
        ids: &[i64],
        client_ip: &str,
        src_user: &str,
    ) -> anyhow::Result<usize> {
        batata_config::service::batch_delete(
            &self.database_connection,
            ids,
            client_ip,
            src_user,
        )
        .await
    }

    async fn config_gray_delete(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
        client_ip: &str,
        src_user: &str,
    ) -> anyhow::Result<bool> {
        batata_config::service::delete_gray(
            &self.database_connection,
            data_id,
            group_name,
            namespace_id,
            client_ip,
            src_user,
        )
        .await
    }

    async fn config_clone(
        &self,
        ids: &[i64],
        target_namespace_id: &str,
        policy: &str,
        src_user: &str,
        src_ip: &str,
    ) -> anyhow::Result<CloneResult> {
        batata_config::service::clone_configs(
            &self.database_connection,
            ids,
            target_namespace_id,
            policy,
            src_user,
            src_ip,
        )
        .await
    }

    async fn config_listeners(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
    ) -> anyhow::Result<Vec<ConfigListenerInfo>> {
        // Use ConfigSubscriberManager if available
        if let Some(ref manager) = self.config_subscriber_manager {
            let config_key =
                batata_core::ConfigKey::new(data_id, group_name, namespace_id);
            let subscribers = manager.get_subscribers(&config_key);

            return Ok(subscribers
                .into_iter()
                .map(|s| ConfigListenerInfo {
                    connection_id: s.connection_id,
                    client_ip: s.client_ip,
                    data_id: data_id.to_string(),
                    group: group_name.to_string(),
                    tenant: namespace_id.to_string(),
                    md5: s.md5,
                })
                .collect());
        }

        // Fallback: ConfigSubscriberManager not configured
        Ok(Vec::new())
    }

    async fn config_listeners_by_ip(
        &self,
        ip: &str,
    ) -> anyhow::Result<Vec<ConfigListenerInfo>> {
        // Use ConfigSubscriberManager if available
        if let Some(ref manager) = self.config_subscriber_manager {
            let subscribers = manager.get_subscribers_by_ip(ip);

            return Ok(subscribers
                .into_iter()
                .map(|(key, s)| ConfigListenerInfo {
                    connection_id: s.connection_id,
                    client_ip: s.client_ip,
                    data_id: key.data_id,
                    group: key.group,
                    tenant: key.tenant,
                    md5: s.md5,
                })
                .collect());
        }

        // Fallback: ConfigSubscriberManager not configured
        Ok(Vec::new())
    }

    // ============== Naming/Service Operations ==============

    async fn service_create(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        protect_threshold: f32,
        metadata: &str,
        selector: &str,
    ) -> anyhow::Result<bool> {
        // Parse metadata JSON
        let metadata_map: std::collections::HashMap<String, String> = if metadata.is_empty() {
            std::collections::HashMap::new()
        } else {
            serde_json::from_str(metadata).unwrap_or_default()
        };

        // Parse selector JSON (expects {"type": "...", "expression": "..."})
        let (selector_type, selector_expression) = if selector.is_empty() {
            ("none".to_string(), String::new())
        } else {
            serde_json::from_str::<serde_json::Value>(selector)
                .ok()
                .map(|v| {
                    (
                        v.get("type")
                            .and_then(|t| t.as_str())
                            .unwrap_or("none")
                            .to_string(),
                        v.get("expression")
                            .and_then(|e| e.as_str())
                            .unwrap_or("")
                            .to_string(),
                    )
                })
                .unwrap_or(("none".to_string(), String::new()))
        };

        // Create service metadata
        let service_metadata = batata_naming::ServiceMetadata {
            protect_threshold,
            metadata: metadata_map,
            selector_type,
            selector_expression,
        };

        // Store service metadata (this creates the service)
        self.naming_service
            .set_service_metadata(namespace_id, group_name, service_name, service_metadata);

        Ok(true)
    }

    async fn service_delete(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
    ) -> anyhow::Result<bool> {
        // Get all instances and deregister them
        let instances =
            self.naming_service
                .get_instances(namespace_id, group_name, service_name, "", false);
        for instance in &instances {
            self.naming_service
                .deregister_instance(namespace_id, group_name, service_name, instance);
        }

        // Delete service metadata
        self.naming_service
            .delete_service_metadata(namespace_id, group_name, service_name);

        Ok(true)
    }

    async fn service_update(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        protect_threshold: f32,
        metadata: &str,
        selector: &str,
    ) -> anyhow::Result<bool> {
        // Parse metadata JSON
        let metadata_map: std::collections::HashMap<String, String> = if metadata.is_empty() {
            std::collections::HashMap::new()
        } else {
            serde_json::from_str(metadata).unwrap_or_default()
        };

        // Parse selector JSON (expects {"type": "...", "expression": "..."})
        let (selector_type, selector_expression) = if selector.is_empty() {
            ("none".to_string(), String::new())
        } else {
            serde_json::from_str::<serde_json::Value>(selector)
                .ok()
                .map(|v| {
                    (
                        v.get("type")
                            .and_then(|t| t.as_str())
                            .unwrap_or("none")
                            .to_string(),
                        v.get("expression")
                            .and_then(|e| e.as_str())
                            .unwrap_or("")
                            .to_string(),
                    )
                })
                .unwrap_or(("none".to_string(), String::new()))
        };

        // Update service metadata
        let service_metadata = batata_naming::ServiceMetadata {
            protect_threshold,
            metadata: metadata_map,
            selector_type,
            selector_expression,
        };

        self.naming_service
            .set_service_metadata(namespace_id, group_name, service_name, service_metadata);

        Ok(true)
    }

    async fn service_get(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
    ) -> anyhow::Result<Option<ServiceDetail>> {
        // Check if service exists (has metadata or instances)
        if !self
            .naming_service
            .service_exists(namespace_id, group_name, service_name)
        {
            return Ok(None);
        }

        let service =
            self.naming_service
                .get_service(namespace_id, group_name, service_name, "", false);

        // Get stored service metadata
        let svc_metadata = self
            .naming_service
            .get_service_metadata(namespace_id, group_name, service_name);

        // Extract unique clusters from instances
        let mut cluster_names: std::collections::HashSet<String> = std::collections::HashSet::new();
        for host in &service.hosts {
            cluster_names.insert(host.cluster_name.clone());
        }

        // Get cluster configs or use defaults
        let clusters: Vec<ClusterInfo> = cluster_names
            .into_iter()
            .map(|name| {
                let cluster_config = self.naming_service.get_cluster_config(
                    namespace_id,
                    group_name,
                    service_name,
                    &name,
                );

                match cluster_config {
                    Some(cfg) => ClusterInfo {
                        name: cfg.name,
                        health_checker: HealthChecker {
                            check_type: cfg.health_check_type,
                            port: cfg.check_port,
                            use_instance_port: cfg.use_instance_port,
                        },
                        metadata: cfg.metadata,
                    },
                    None => ClusterInfo {
                        name,
                        health_checker: HealthChecker {
                            check_type: "TCP".to_string(),
                            port: 80,
                            use_instance_port: true,
                        },
                        metadata: std::collections::HashMap::new(),
                    },
                }
            })
            .collect();

        Ok(Some(ServiceDetail {
            namespace_id: namespace_id.to_string(),
            group_name: group_name.to_string(),
            service_name: service_name.to_string(),
            protect_threshold: svc_metadata.as_ref().map_or(0.0, |m| m.protect_threshold),
            metadata: svc_metadata.as_ref().map_or_else(
                std::collections::HashMap::new,
                |m| m.metadata.clone(),
            ),
            selector: svc_metadata.as_ref().map_or_else(ServiceSelector::default, |m| {
                ServiceSelector {
                    selector_type: m.selector_type.clone(),
                    expression: m.selector_expression.clone(),
                }
            }),
            clusters,
        }))
    }

    async fn service_list(
        &self,
        namespace_id: &str,
        group_name: &str,
        _service_name_pattern: &str,
        page_no: u32,
        page_size: u32,
        with_instances: bool,
    ) -> anyhow::Result<Page<ServiceListItem>> {
        let (total, service_names) = self.naming_service.list_services(
            namespace_id,
            group_name,
            page_no as i32,
            page_size as i32,
        );

        let items: Vec<ServiceListItem> = service_names
            .into_iter()
            .map(|name| {
                let instances = self.naming_service.get_instances(
                    namespace_id,
                    group_name,
                    &name,
                    "",
                    false,
                );
                let healthy_count = instances.iter().filter(|i| i.healthy).count() as u32;

                // Extract unique clusters
                let cluster_names: std::collections::HashSet<String> =
                    instances.iter().map(|i| i.cluster_name.clone()).collect();

                ServiceListItem {
                    name: name.clone(),
                    group_name: group_name.to_string(),
                    cluster_count: cluster_names.len() as u32,
                    ip_count: instances.len() as u32,
                    healthy_instance_count: healthy_count,
                    trigger_flag: false,
                    metadata: std::collections::HashMap::new(),
                    instances: if with_instances {
                        Some(instances)
                    } else {
                        None
                    },
                }
            })
            .collect();

        let pages_available = if page_size > 0 {
            (total as u64 + page_size as u64 - 1) / page_size as u64
        } else {
            1
        };

        Ok(Page::new(
            total as u64,
            page_no as u64,
            pages_available,
            items,
        ))
    }

    async fn service_subscribers(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        page_no: u32,
        page_size: u32,
    ) -> anyhow::Result<Page<SubscriberInfo>> {
        let subscribers =
            self.naming_service
                .get_subscribers(namespace_id, group_name, service_name);

        let total = subscribers.len() as u64;
        let start = ((page_no - 1) * page_size) as usize;
        let items: Vec<SubscriberInfo> = subscribers
            .into_iter()
            .skip(start)
            .take(page_size as usize)
            .map(|conn_id| SubscriberInfo {
                address: conn_id.clone(),
                agent: String::new(),
                app: String::new(),
            })
            .collect();

        let pages_available = if page_size > 0 {
            (total + page_size as u64 - 1) / page_size as u64
        } else {
            1
        };

        Ok(Page::new(total, page_no as u64, pages_available, items))
    }

    fn service_selector_types(&self) -> Vec<String> {
        vec![
            "none".to_string(),
            "label".to_string(),
        ]
    }

    async fn service_cluster_update(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
        check_port: i32,
        use_instance_port: bool,
        health_check_type: &str,
        metadata: &str,
    ) -> anyhow::Result<bool> {
        // Parse metadata JSON
        let metadata_map: std::collections::HashMap<String, String> = if metadata.is_empty() {
            std::collections::HashMap::new()
        } else {
            serde_json::from_str(metadata).unwrap_or_default()
        };

        // Create cluster config
        let cluster_config = batata_naming::ClusterConfig {
            name: cluster_name.to_string(),
            health_check_type: health_check_type.to_string(),
            check_port,
            use_instance_port,
            metadata: metadata_map,
        };

        // Store cluster configuration
        self.naming_service.set_cluster_config(
            namespace_id,
            group_name,
            service_name,
            cluster_name,
            cluster_config,
        );

        Ok(true)
    }

    // ============== Instance Operations ==============

    async fn instance_list(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
        page_no: u32,
        page_size: u32,
    ) -> anyhow::Result<Page<InstanceInfo>> {
        let instances = self.naming_service.get_instances(
            namespace_id,
            group_name,
            service_name,
            cluster_name,
            false, // include unhealthy
        );

        // Convert to InstanceInfo
        let instance_infos: Vec<InstanceInfo> = instances
            .into_iter()
            .map(|inst| InstanceInfo {
                ip: inst.ip.clone(),
                port: inst.port,
                weight: inst.weight,
                healthy: inst.healthy,
                enabled: inst.enabled,
                ephemeral: inst.ephemeral,
                cluster_name: inst.cluster_name.clone(),
                service_name: inst.service_name.clone(),
                metadata: inst.metadata.clone(),
                instance_heart_beat_interval: inst.instance_heart_beat_interval,
                instance_heart_beat_timeout: inst.instance_heart_beat_time_out,
                ip_delete_timeout: inst.ip_delete_timeout,
            })
            .collect();

        // Apply pagination
        let total = instance_infos.len();
        let start = ((page_no.saturating_sub(1)) * page_size) as usize;
        let end = (start + page_size as usize).min(total);
        let page_data = if start < total {
            instance_infos[start..end].to_vec()
        } else {
            vec![]
        };

        Ok(Page::new(total as u64, page_no as u64, page_size as u64, page_data))
    }

    async fn instance_update(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
        ip: &str,
        port: i32,
        weight: f64,
        healthy: bool,
        enabled: bool,
        ephemeral: bool,
        metadata: &str,
    ) -> anyhow::Result<bool> {
        // Parse metadata JSON
        let metadata_map: std::collections::HashMap<String, String> = if metadata.is_empty() {
            std::collections::HashMap::new()
        } else {
            serde_json::from_str(metadata).unwrap_or_default()
        };

        // Create updated instance
        let instance = batata_naming::Instance {
            ip: ip.to_string(),
            port,
            weight,
            healthy,
            enabled,
            ephemeral,
            cluster_name: cluster_name.to_string(),
            service_name: service_name.to_string(),
            metadata: metadata_map,
            ..Default::default()
        };

        // Register will update if instance exists
        self.naming_service.register_instance(
            namespace_id,
            group_name,
            service_name,
            instance,
        );

        Ok(true)
    }

    // ============== History Operations ==============

    async fn history_get(
        &self,
        nid: u64,
        _data_id: &str,
        _group_name: &str,
        _namespace_id: &str,
    ) -> anyhow::Result<Option<ConfigHistoryInfo>> {
        batata_config::service::history::find_by_id(&self.database_connection, nid).await
    }

    async fn history_list(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<ConfigHistoryInfo>> {
        batata_config::service::history::search_page(
            &self.database_connection,
            data_id,
            group_name,
            namespace_id,
            page_no,
            page_size,
        )
        .await
    }

    async fn history_configs_by_namespace(
        &self,
        namespace_id: &str,
    ) -> anyhow::Result<Vec<ConfigInfoWrapper>> {
        batata_config::service::history::find_configs_by_namespace_id(
            &self.database_connection,
            namespace_id,
        )
        .await
    }

    // ============== Cluster Operations ==============

    fn cluster_members(&self) -> Vec<Member> {
        self.server_member_manager
            .all_members()
            .into_iter()
            .map(Member::from)
            .collect()
    }

    fn cluster_healthy_members(&self) -> Vec<Member> {
        self.server_member_manager
            .healthy_members()
            .into_iter()
            .map(Member::from)
            .collect()
    }

    fn cluster_health(&self) -> ClusterHealthResponse {
        let summary = self.server_member_manager.health_summary();
        let healthy = self.server_member_manager.is_cluster_healthy();
        let standalone_mode = self.server_member_manager.is_standalone();

        ClusterHealthResponse {
            is_healthy: healthy,
            summary: ClusterHealthSummary::from(summary),
            standalone: standalone_mode,
        }
    }

    fn cluster_self(&self) -> SelfMemberResponse {
        let self_member = self.server_member_manager.get_self();
        let version = self_member
            .extend_info
            .read()
            .ok()
            .and_then(|info| {
                info.get(ApiMember::VERSION)
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

    fn cluster_member(&self, address: &str) -> Option<Member> {
        self.server_member_manager
            .get_member(address)
            .map(Member::from)
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

    // ============== Helper Methods ==============

    fn is_remote(&self) -> bool {
        false
    }

    fn database(&self) -> Option<&DatabaseConnection> {
        Some(&self.database_connection)
    }

    fn member_manager(&self) -> Option<Arc<ServerMemberManager>> {
        Some(self.server_member_manager.clone())
    }
}
