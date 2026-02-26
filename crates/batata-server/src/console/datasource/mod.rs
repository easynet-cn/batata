// Console data source abstraction layer
// Provides a unified interface for console operations in both local and remote modes

pub mod embedded;
pub mod local;
pub mod remote;

use async_trait::async_trait;
use sea_orm::DatabaseConnection;

use batata_core::cluster::ServerMemberManager;

use batata_config::Namespace;

use std::collections::HashMap;
use std::sync::Arc;

use crate::{
    api::{
        config::model::{
            ConfigBasicInfo, ConfigGrayInfo, ConfigHistoryBasicInfo, ConfigHistoryDetailInfo,
            ConfigListenerInfo,
        },
        model::{Member, Page},
        naming::model::Instance,
    },
    config::{
        export_model::{ImportResult, SameConfigPolicy},
        model::ConfigAllInfo,
    },
    console::v3::cluster::{ClusterHealthResponse, SelfMemberResponse},
    model::common::Configuration,
};

/// Console data source trait - abstracts data access for console operations
#[async_trait]
pub trait ConsoleDataSource: Send + Sync {
    // ============== Namespace Operations ==============

    /// Get all namespaces
    async fn namespace_find_all(&self) -> Vec<Namespace>;

    /// Get namespace by ID
    async fn namespace_get_by_id(
        &self,
        namespace_id: &str,
        tenant_id: &str,
    ) -> anyhow::Result<Namespace>;

    /// Create a new namespace
    async fn namespace_create(
        &self,
        namespace_id: &str,
        namespace_name: &str,
        namespace_desc: &str,
    ) -> anyhow::Result<()>;

    /// Update an existing namespace
    async fn namespace_update(
        &self,
        namespace_id: &str,
        namespace_name: &str,
        namespace_desc: &str,
    ) -> anyhow::Result<bool>;

    /// Delete a namespace
    async fn namespace_delete(&self, namespace_id: &str) -> anyhow::Result<bool>;

    /// Check if namespace exists
    async fn namespace_check(&self, namespace_id: &str) -> anyhow::Result<bool>;

    // ============== Config Operations ==============

    /// Find a single config
    async fn config_find_one(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
    ) -> anyhow::Result<Option<ConfigAllInfo>>;

    /// Search configs with pagination
    #[allow(clippy::too_many_arguments)]
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
    ) -> anyhow::Result<Page<ConfigBasicInfo>>;

    /// Create or update a config
    #[allow(clippy::too_many_arguments)]
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
    ) -> anyhow::Result<()>;

    /// Delete a config
    #[allow(clippy::too_many_arguments)]
    async fn config_delete(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
        tag: &str,
        client_ip: &str,
        src_user: &str,
        caas_user: &str,
    ) -> anyhow::Result<()>;

    /// Find gray/beta config
    async fn config_find_gray_one(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
    ) -> anyhow::Result<Option<ConfigGrayInfo>>;

    /// Create or update a gray/beta config
    #[allow(clippy::too_many_arguments)]
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
    ) -> anyhow::Result<()>;

    /// Delete a gray/beta config
    #[allow(clippy::too_many_arguments)]
    async fn config_delete_gray(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
        gray_name: &str,
        client_ip: &str,
        src_user: &str,
    ) -> anyhow::Result<()>;

    /// Export configs
    async fn config_export(
        &self,
        namespace_id: &str,
        group: Option<&str>,
        data_ids: Option<Vec<String>>,
        app_name: Option<&str>,
    ) -> anyhow::Result<Vec<u8>>;

    /// Import configs
    #[allow(clippy::too_many_arguments)]
    async fn config_import(
        &self,
        file_data: Vec<u8>,
        namespace_id: &str,
        policy: SameConfigPolicy,
        src_user: &str,
        src_ip: &str,
    ) -> anyhow::Result<ImportResult>;

    /// Batch delete configs by IDs
    async fn config_batch_delete(
        &self,
        ids: &[i64],
        client_ip: &str,
        src_user: &str,
    ) -> anyhow::Result<()>;

    /// Clone configs to another namespace
    #[allow(clippy::too_many_arguments)]
    async fn config_clone(
        &self,
        ids: &[i64],
        target_namespace_id: &str,
        policy: &str,
        src_user: &str,
        src_ip: &str,
    ) -> anyhow::Result<ImportResult>;

    /// Get config listener info by IP address
    async fn config_listener_list_by_ip(
        &self,
        ip: &str,
        all: bool,
        namespace_id: &str,
    ) -> anyhow::Result<ConfigListenerInfo>;

    // ============== History Operations ==============

    /// Find history by ID
    async fn history_find_by_id(&self, nid: u64)
    -> anyhow::Result<Option<ConfigHistoryDetailInfo>>;

    /// Search history with pagination
    async fn history_search_page(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<ConfigHistoryBasicInfo>>;

    /// Find configs by namespace ID (from history)
    async fn history_find_configs_by_namespace_id(
        &self,
        namespace_id: &str,
    ) -> anyhow::Result<Vec<ConfigBasicInfo>>;

    /// Find the previous history version before a given history ID
    async fn history_find_previous(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
        id: u64,
    ) -> anyhow::Result<Option<ConfigHistoryDetailInfo>>;

    // ============== Service Operations ==============

    /// List services with pagination
    #[allow(clippy::too_many_arguments)]
    async fn service_list(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        page_no: u64,
        page_size: u64,
        has_ip_count: bool,
    ) -> anyhow::Result<(i32, Vec<serde_json::Value>)>;

    /// Get service detail
    async fn service_get(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
    ) -> anyhow::Result<Option<serde_json::Value>>;

    /// Create a service
    async fn service_create(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        protect_threshold: f32,
        metadata: &str,
        selector: &str,
    ) -> anyhow::Result<bool>;

    /// Update a service
    async fn service_update(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        protect_threshold: Option<f32>,
        metadata: Option<&str>,
        selector: Option<&str>,
    ) -> anyhow::Result<bool>;

    /// Delete a service
    async fn service_delete(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
    ) -> anyhow::Result<bool>;

    /// Get service subscribers with pagination
    async fn service_subscriber_list(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<(i32, Vec<String>)>;

    // ============== Instance Operations ==============

    /// List instances for a service
    async fn instance_list(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
    ) -> anyhow::Result<Vec<Instance>>;

    /// Update an instance
    #[allow(clippy::too_many_arguments)]
    async fn instance_update(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        instance: Instance,
    ) -> anyhow::Result<bool>;

    // ============== Config Listener Operations ==============

    /// Get config listener info
    async fn config_listener_list(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
    ) -> anyhow::Result<ConfigListenerInfo>;

    // ============== Server State Operations ==============

    /// Get server state map
    async fn server_state(&self) -> HashMap<String, Option<String>>;

    /// Check server readiness
    async fn server_readiness(&self) -> bool;

    // ============== History Operations (Advanced) ==============

    /// Search history with advanced filters
    #[allow(clippy::too_many_arguments)]
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
    ) -> anyhow::Result<Page<ConfigHistoryBasicInfo>>;

    // ============== Cluster Operations ==============

    /// Get all cluster members
    fn cluster_all_members(&self) -> Vec<Member>;

    /// Get healthy cluster members
    fn cluster_healthy_members(&self) -> Vec<Member>;

    /// Get cluster health status
    fn cluster_get_health(&self) -> ClusterHealthResponse;

    /// Get self member info
    fn cluster_get_self(&self) -> SelfMemberResponse;

    /// Get a specific member by address
    fn cluster_get_member(&self, address: &str) -> Option<Member>;

    /// Get member count
    fn cluster_member_count(&self) -> usize;

    /// Check if standalone mode
    fn cluster_is_standalone(&self) -> bool;

    /// Refresh self member
    fn cluster_refresh_self(&self);

    /// Update member state, returns previous state
    async fn cluster_update_member_state(
        &self,
        address: &str,
        state: &str,
    ) -> anyhow::Result<String>;

    /// Check if this node is the leader
    fn cluster_is_leader(&self) -> bool;

    /// Get leader address
    fn cluster_leader_address(&self) -> Option<String>;

    /// Get local address
    fn cluster_local_address(&self) -> String;

    // ============== Service Operations (Cluster) ==============

    /// Update cluster health check and metadata
    #[allow(clippy::too_many_arguments)]
    async fn service_update_cluster(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
        health_checker_type: Option<&str>,
        metadata: Option<HashMap<String, String>>,
    ) -> anyhow::Result<bool>;

    // ============== Helper Methods ==============

    /// Check if this is a remote data source
    fn is_remote(&self) -> bool;

    /// Get server member manager (only available in local mode)
    fn get_server_member_manager(&self) -> Option<Arc<ServerMemberManager>>;
}

/// Create a console data source based on configuration
pub async fn create_datasource(
    configuration: &Configuration,
    database_connection: Option<DatabaseConnection>,
    server_member_manager: Option<Arc<ServerMemberManager>>,
    config_subscriber_manager: Arc<batata_core::ConfigSubscriberManager>,
    naming_service: Option<Arc<crate::service::naming::NamingService>>,
    persistence: Option<Arc<dyn batata_persistence::PersistenceService>>,
) -> anyhow::Result<Arc<dyn ConsoleDataSource>> {
    if configuration.is_console_remote_mode() {
        // Remote mode: use HTTP client to connect to server
        let remote_datasource = remote::RemoteDataSource::new(configuration).await?;
        Ok(Arc::new(remote_datasource))
    } else if let Some(db) = database_connection {
        // Local mode with external DB: direct database access
        let smm = server_member_manager.ok_or_else(|| {
            anyhow::anyhow!("Server member manager required for local console mode")
        })?;
        let local_datasource = local::LocalDataSource::new(
            db,
            smm,
            config_subscriber_manager,
            configuration.clone(),
            naming_service,
        );
        Ok(Arc::new(local_datasource))
    } else {
        // Embedded mode (standalone/distributed): use direct PersistenceService access
        let smm = server_member_manager.ok_or_else(|| {
            anyhow::anyhow!("Server member manager required for embedded console mode")
        })?;
        let persist = persistence.ok_or_else(|| {
            anyhow::anyhow!("Persistence service required for embedded console mode")
        })?;
        let embedded_ds = embedded::EmbeddedLocalDataSource::new(
            persist,
            smm,
            config_subscriber_manager,
            configuration.clone(),
            naming_service,
        );
        Ok(Arc::new(embedded_ds))
    }
}
