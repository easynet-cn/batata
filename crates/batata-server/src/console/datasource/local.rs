// Local data source implementation
// Provides direct database access for console operations

use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

use batata_core::cluster::ServerMemberManager;

use batata_config::Namespace;

use crate::{
    api::{
        config::model::{
            ConfigBasicInfo, ConfigGrayInfo, ConfigHistoryBasicInfo, ConfigHistoryDetailInfo,
        },
        model::{Member, Page},
    },
    config::export_model::{ImportResult, SameConfigPolicy},
    config::model::ConfigAllInfo,
    console::v3::cluster::{
        ClusterHealthResponse, ClusterHealthSummaryResponse, SelfMemberResponse,
    },
    model::common::Configuration,
    service,
};

use super::ConsoleDataSource;

/// Local data source - direct database access
pub struct LocalDataSource {
    database_connection: DatabaseConnection,
    server_member_manager: Arc<ServerMemberManager>,
    #[allow(dead_code)]
    configuration: Configuration,
}

impl LocalDataSource {
    pub fn new(
        database_connection: DatabaseConnection,
        server_member_manager: Arc<ServerMemberManager>,
        configuration: Configuration,
    ) -> Self {
        Self {
            database_connection,
            server_member_manager,
            configuration,
        }
    }
}

#[async_trait]
impl ConsoleDataSource for LocalDataSource {
    // ============== Namespace Operations ==============

    async fn namespace_find_all(&self) -> Vec<Namespace> {
        service::namespace::find_all(&self.database_connection).await
    }

    async fn namespace_get_by_id(
        &self,
        namespace_id: &str,
        tenant_id: &str,
    ) -> anyhow::Result<Namespace> {
        service::namespace::get_by_namespace_id(&self.database_connection, namespace_id, tenant_id)
            .await
    }

    async fn namespace_create(
        &self,
        namespace_id: &str,
        namespace_name: &str,
        namespace_desc: &str,
    ) -> anyhow::Result<()> {
        service::namespace::create(
            &self.database_connection,
            namespace_id,
            namespace_name,
            namespace_desc,
        )
        .await?;
        Ok(())
    }

    async fn namespace_update(
        &self,
        namespace_id: &str,
        namespace_name: &str,
        namespace_desc: &str,
    ) -> anyhow::Result<bool> {
        service::namespace::update(
            &self.database_connection,
            namespace_id,
            namespace_name,
            namespace_desc,
        )
        .await
    }

    async fn namespace_delete(&self, namespace_id: &str) -> anyhow::Result<bool> {
        service::namespace::delete(&self.database_connection, namespace_id).await
    }

    async fn namespace_check(&self, namespace_id: &str) -> anyhow::Result<bool> {
        service::namespace::check(&self.database_connection, namespace_id).await
    }

    // ============== Config Operations ==============

    async fn config_find_one(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
    ) -> anyhow::Result<Option<ConfigAllInfo>> {
        service::config::find_one(&self.database_connection, data_id, group_name, namespace_id)
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
        let result = service::config::search_page(
            &self.database_connection,
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

        // Convert batata_config::ConfigBasicInfo to api::config::model::ConfigBasicInfo
        Ok(Page::new(
            result.total_count,
            result.page_number,
            result.pages_available,
            result.page_items.into_iter().map(ConfigBasicInfo::from).collect(),
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
        service::config::create_or_update(
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
        service::config::delete(
            &self.database_connection,
            data_id,
            group_name,
            namespace_id,
            tag,
            client_ip,
            src_user,
        )
        .await?;
        Ok(())
    }

    async fn config_find_gray_one(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
    ) -> anyhow::Result<Option<ConfigGrayInfo>> {
        let result = service::config::find_gray_one(
            &self.database_connection,
            data_id,
            group_name,
            namespace_id,
        )
        .await?;
        Ok(result.map(ConfigGrayInfo::from))
    }

    async fn config_export(
        &self,
        namespace_id: &str,
        group: Option<&str>,
        data_ids: Option<Vec<String>>,
        app_name: Option<&str>,
    ) -> anyhow::Result<Vec<u8>> {
        let configs = service::config_export::find_configs_for_export(
            &self.database_connection,
            namespace_id,
            group,
            data_ids,
            app_name,
        )
        .await?;

        if configs.is_empty() {
            return Err(anyhow::anyhow!("No configurations found to export"));
        }

        service::config_export::create_nacos_export_zip(configs)
    }

    async fn config_import(
        &self,
        file_data: Vec<u8>,
        namespace_id: &str,
        policy: SameConfigPolicy,
        src_user: &str,
        src_ip: &str,
    ) -> anyhow::Result<ImportResult> {
        let items = service::config_import::parse_nacos_import_zip(&file_data)?;

        if items.is_empty() {
            return Err(anyhow::anyhow!("No configurations found in ZIP file"));
        }

        service::config_import::import_nacos_items(
            &self.database_connection,
            items,
            namespace_id,
            policy,
            src_user,
            src_ip,
        )
        .await
    }

    // ============== History Operations ==============

    async fn history_find_by_id(
        &self,
        nid: u64,
    ) -> anyhow::Result<Option<ConfigHistoryDetailInfo>> {
        let result = service::history::find_by_id(&self.database_connection, nid).await?;
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
        let result = service::history::search_page(
            &self.database_connection,
            data_id,
            group_name,
            namespace_id,
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

    async fn history_find_configs_by_namespace_id(
        &self,
        namespace_id: &str,
    ) -> anyhow::Result<Vec<ConfigBasicInfo>> {
        let result =
            service::history::find_configs_by_namespace_id(&self.database_connection, namespace_id)
                .await?;

        Ok(result.into_iter().map(ConfigBasicInfo::from).collect())
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

    // ============== Helper Methods ==============

    fn is_remote(&self) -> bool {
        false
    }

    fn get_database_connection(&self) -> Option<&DatabaseConnection> {
        Some(&self.database_connection)
    }

    fn get_server_member_manager(&self) -> Option<Arc<ServerMemberManager>> {
        Some(self.server_member_manager.clone())
    }
}
