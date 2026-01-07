//! Console data source abstraction layer
//!
//! Provides a unified interface for console operations in both local and remote modes.
//! - LocalDataSource: Direct database access for co-located console
//! - RemoteDataSource: HTTP client-based access to remote Batata server

pub mod local;
pub mod remote;

use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

use batata_api::Page;
use batata_config::{
    ConfigAllInfo, ConfigBasicInfo, ConfigHistoryInfo, ConfigInfoGrayWrapper, ConfigInfoWrapper,
    ImportResult, Namespace, SameConfigPolicy,
};
use batata_core::cluster::ServerMemberManager;

use crate::model::{ClusterHealthResponse, Member, SelfMemberResponse};

/// Console data source trait - abstracts data access for console operations
///
/// This trait allows the console to work in two modes:
/// - Local mode: Direct database access (co-located with server)
/// - Remote mode: HTTP client-based access (separate console deployment)
#[async_trait]
pub trait ConsoleDataSource: Send + Sync {
    // ============== Namespace Operations ==============

    /// Get all namespaces
    async fn namespace_list(&self) -> Vec<Namespace>;

    /// Get namespace by ID
    async fn namespace_get(&self, namespace_id: &str) -> anyhow::Result<Namespace>;

    /// Create a new namespace
    async fn namespace_create(
        &self,
        namespace_id: &str,
        namespace_name: &str,
        namespace_desc: &str,
    ) -> anyhow::Result<bool>;

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
    async fn namespace_exists(&self, namespace_id: &str) -> anyhow::Result<bool>;

    // ============== Config Operations ==============

    /// Find a single config
    async fn config_get(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
    ) -> anyhow::Result<Option<ConfigAllInfo>>;

    /// Search configs with pagination
    #[allow(clippy::too_many_arguments)]
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
    ) -> anyhow::Result<Page<ConfigBasicInfo>>;

    /// Create or update a config
    #[allow(clippy::too_many_arguments)]
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
    ) -> anyhow::Result<bool>;

    /// Delete a config
    async fn config_delete(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
        gray_name: &str,
        client_ip: &str,
        src_user: &str,
    ) -> anyhow::Result<bool>;

    /// Find gray/beta config
    async fn config_gray_get(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
    ) -> anyhow::Result<Option<ConfigInfoGrayWrapper>>;

    /// Export configs as ZIP bytes
    async fn config_export(
        &self,
        namespace_id: &str,
        group: Option<&str>,
        data_ids: Option<&str>,
        app_name: Option<&str>,
    ) -> anyhow::Result<Vec<u8>>;

    /// Import configs from ZIP bytes
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
    async fn history_get(
        &self,
        nid: u64,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
    ) -> anyhow::Result<Option<ConfigHistoryInfo>>;

    /// Search history with pagination
    async fn history_list(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<ConfigHistoryInfo>>;

    /// Find configs by namespace ID (from history)
    async fn history_configs_by_namespace(
        &self,
        namespace_id: &str,
    ) -> anyhow::Result<Vec<ConfigInfoWrapper>>;

    // ============== Cluster Operations ==============

    /// Get all cluster members
    fn cluster_members(&self) -> Vec<Member>;

    /// Get healthy cluster members
    fn cluster_healthy_members(&self) -> Vec<Member>;

    /// Get cluster health status
    fn cluster_health(&self) -> ClusterHealthResponse;

    /// Get self member info
    fn cluster_self(&self) -> SelfMemberResponse;

    /// Get a specific member by address
    fn cluster_member(&self, address: &str) -> Option<Member>;

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
    fn database(&self) -> Option<&DatabaseConnection>;

    /// Get server member manager (only available in local mode)
    fn member_manager(&self) -> Option<Arc<ServerMemberManager>>;
}

/// Console data source configuration
#[derive(Clone, Debug)]
pub struct ConsoleDataSourceConfig {
    /// Whether to use remote mode
    pub remote_mode: bool,
    /// Remote server addresses (for remote mode)
    pub server_addrs: Vec<String>,
    /// Username for authentication
    pub username: String,
    /// Password for authentication
    pub password: String,
    /// Context path
    pub context_path: String,
}

impl Default for ConsoleDataSourceConfig {
    fn default() -> Self {
        Self {
            remote_mode: false,
            server_addrs: vec!["http://127.0.0.1:8848".to_string()],
            username: "nacos".to_string(),
            password: "nacos".to_string(),
            context_path: String::new(),
        }
    }
}

/// Create a console data source based on configuration
pub async fn create_datasource(
    config: &ConsoleDataSourceConfig,
    database_connection: Option<DatabaseConnection>,
    server_member_manager: Option<Arc<ServerMemberManager>>,
) -> anyhow::Result<Arc<dyn ConsoleDataSource>> {
    if config.remote_mode {
        // Remote mode: use HTTP client to connect to server
        let remote_datasource = remote::RemoteDataSource::new(config).await?;
        Ok(Arc::new(remote_datasource))
    } else {
        // Local mode: direct database access
        let db = database_connection.ok_or_else(|| {
            anyhow::anyhow!("Database connection required for local console mode")
        })?;
        let smm = server_member_manager.ok_or_else(|| {
            anyhow::anyhow!("Server member manager required for local console mode")
        })?;
        let local_datasource = local::LocalDataSource::new(db, smm);
        Ok(Arc::new(local_datasource))
    }
}
