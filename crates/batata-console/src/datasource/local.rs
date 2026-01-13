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
};
use batata_core::cluster::ServerMemberManager;

use super::ConsoleDataSource;
use crate::model::{ClusterHealthResponse, ClusterHealthSummary, Member, SelfMemberResponse};

/// Local data source - direct database access
pub struct LocalDataSource {
    database_connection: DatabaseConnection,
    server_member_manager: Arc<ServerMemberManager>,
}

impl LocalDataSource {
    pub fn new(
        database_connection: DatabaseConnection,
        server_member_manager: Arc<ServerMemberManager>,
    ) -> Self {
        Self {
            database_connection,
            server_member_manager,
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
