// Console data source abstraction layer
// Provides a unified interface for console operations in both local and remote modes

pub mod local;
pub mod remote;

use async_trait::async_trait;
use sea_orm::DatabaseConnection;

use crate::{
    api::{
        config::model::{ConfigBasicInfo, ConfigGrayInfo, ConfigHistoryBasicInfo, ConfigHistoryDetailInfo},
        model::{Member, Page},
    },
    config::{
        export_model::{ImportResult, SameConfigPolicy},
        model::ConfigAllInfo,
    },
    console::v3::cluster::{ClusterHealthResponse, SelfMemberResponse},
    core::service::cluster::ServerMemberManager,
    model::{common::Configuration, naming::Namespace},
};
use std::sync::Arc;

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

    // ============== History Operations ==============

    /// Find history by ID
    async fn history_find_by_id(&self, nid: u64) -> anyhow::Result<Option<ConfigHistoryDetailInfo>>;

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

    // ============== Helper Methods ==============

    /// Check if this is a remote data source
    fn is_remote(&self) -> bool;

    /// Get database connection (only available in local mode)
    fn get_database_connection(&self) -> Option<&DatabaseConnection>;

    /// Get server member manager (only available in local mode)
    fn get_server_member_manager(&self) -> Option<Arc<ServerMemberManager>>;
}

/// Create a console data source based on configuration
pub async fn create_datasource(
    configuration: &Configuration,
    database_connection: Option<DatabaseConnection>,
    server_member_manager: Option<Arc<ServerMemberManager>>,
) -> anyhow::Result<Arc<dyn ConsoleDataSource>> {
    if configuration.is_console_remote_mode() {
        // Remote mode: use HTTP client to connect to server
        let remote_datasource = remote::RemoteDataSource::new(configuration).await?;
        Ok(Arc::new(remote_datasource))
    } else {
        // Local mode: direct database access
        let db = database_connection.ok_or_else(|| {
            anyhow::anyhow!("Database connection required for local console mode")
        })?;
        let smm = server_member_manager.ok_or_else(|| {
            anyhow::anyhow!("Server member manager required for local console mode")
        })?;
        let local_datasource = local::LocalDataSource::new(db, smm, configuration.clone());
        Ok(Arc::new(local_datasource))
    }
}
