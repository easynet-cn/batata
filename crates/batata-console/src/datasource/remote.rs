//! Remote data source implementation
//!
//! Provides HTTP-based access to console operations via a remote Batata server.
//! Uses batata-client for HTTP communication.

use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};
use tracing::warn;

use batata_api::Page;
use batata_client::{BatataApiClient, BatataHttpClient, HttpClientConfig};
use batata_config::{
    ConfigAllInfo, ConfigBasicInfo, ConfigHistoryInfo, ConfigInfoGrayWrapper, ConfigInfoWrapper,
    ImportResult, Namespace, SameConfigPolicy,
};
use batata_core::cluster::ServerMemberManager;

use super::{ConsoleDataSource, ConsoleDataSourceConfig};
use crate::model::{ClusterHealthResponse, ClusterHealthSummary, Member, SelfMemberResponse};

/// Extension trait for RwLock that handles poison recovery gracefully
trait RwLockExt<T> {
    /// Acquire a read lock, recovering from poison if necessary
    fn read_recover(&self, name: &str) -> RwLockReadGuard<'_, T>;
    /// Acquire a write lock, recovering from poison if necessary
    fn write_recover(&self, name: &str) -> RwLockWriteGuard<'_, T>;
}

impl<T> RwLockExt<T> for RwLock<T> {
    fn read_recover(&self, name: &str) -> RwLockReadGuard<'_, T> {
        self.read().unwrap_or_else(|poisoned| {
            // Lock poisoning indicates a panic occurred while holding the lock.
            // Log at warn level to ensure visibility in production.
            warn!(
                lock_name = name,
                "Recovering from poisoned read lock - a thread panicked while holding this lock"
            );
            poisoned.into_inner()
        })
    }

    fn write_recover(&self, name: &str) -> RwLockWriteGuard<'_, T> {
        self.write().unwrap_or_else(|poisoned| {
            // Lock poisoning indicates a panic occurred while holding the lock.
            // Log at warn level to ensure visibility in production.
            warn!(
                lock_name = name,
                "Recovering from poisoned write lock - a thread panicked while holding this lock"
            );
            poisoned.into_inner()
        })
    }
}

/// Remote data source - HTTP-based access to remote server
pub struct RemoteDataSource {
    api_client: BatataApiClient,
    // Cached cluster info (refreshed periodically)
    cached_members: RwLock<Vec<Member>>,
    cached_health: RwLock<Option<ClusterHealthResponse>>,
    cached_self: RwLock<Option<SelfMemberResponse>>,
}

impl RemoteDataSource {
    pub async fn new(config: &ConsoleDataSourceConfig) -> anyhow::Result<Self> {
        let http_config = HttpClientConfig::with_servers(config.server_addrs.clone())
            .with_auth(&config.username, &config.password)
            .with_context_path(&config.context_path);

        let http_client = BatataHttpClient::new(http_config).await?;
        let api_client = BatataApiClient::new(http_client);

        let datasource = Self {
            api_client,
            cached_members: RwLock::new(Vec::new()),
            cached_health: RwLock::new(None),
            cached_self: RwLock::new(None),
        };

        // Pre-fetch cluster info
        datasource.refresh_cluster_cache().await;

        Ok(datasource)
    }

    /// Refresh cached cluster information
    async fn refresh_cluster_cache(&self) {
        // Fetch members
        match self.api_client.cluster_members().await {
            Ok(members) => {
                let mut cache = self.cached_members.write_recover("cached_members");
                *cache = members
                    .into_iter()
                    .map(|m| Member {
                        ip: m.ip,
                        port: m.port as u16,
                        state: m.state,
                        extend_info: m.extend_info,
                        address: m.address,
                        fail_access_cnt: m.fail_access_cnt,
                        abilities: m.abilities,
                    })
                    .collect();
            }
            Err(e) => {
                warn!(error = %e, "Failed to fetch cluster members from remote server");
            }
        }

        // Fetch health
        match self.api_client.cluster_health().await {
            Ok(health) => {
                let mut cache = self.cached_health.write_recover("cached_health");
                *cache = Some(ClusterHealthResponse {
                    is_healthy: health.healthy,
                    summary: ClusterHealthSummary {
                        total: health.member_count,
                        up: health.healthy_count,
                        down: health.unhealthy_count,
                        suspicious: 0,
                        starting: 0,
                        isolation: 0,
                    },
                    standalone: health.member_count <= 1,
                });
            }
            Err(e) => {
                warn!(error = %e, "Failed to fetch cluster health from remote server");
            }
        }

        // Fetch self
        match self.api_client.cluster_self().await {
            Ok(self_member) => {
                let mut cache = self.cached_self.write_recover("cached_self");
                *cache = Some(SelfMemberResponse {
                    ip: self_member.member.ip,
                    port: self_member.member.port as u16,
                    address: self_member.member.address,
                    state: self_member.member.state,
                    is_standalone: !self_member.is_leader, // Approximate
                    version: self_member
                        .member
                        .extend_info
                        .get("version")
                        .cloned()
                        .unwrap_or_default(),
                });
            }
            Err(e) => {
                warn!(error = %e, "Failed to fetch self member info from remote server");
            }
        }
    }
}

#[async_trait]
impl ConsoleDataSource for RemoteDataSource {
    // ============== Namespace Operations ==============

    async fn namespace_list(&self) -> Vec<Namespace> {
        match self.api_client.namespace_list().await {
            Ok(namespaces) => namespaces
                .into_iter()
                .map(|n| Namespace {
                    namespace: n.namespace,
                    namespace_show_name: n.namespace_show_name,
                    namespace_desc: n.namespace_desc,
                    quota: n.quota,
                    config_count: n.config_count,
                    type_: n.type_,
                })
                .collect(),
            Err(e) => {
                warn!("Failed to fetch namespaces from remote server: {}", e);
                Vec::new()
            }
        }
    }

    async fn namespace_get(&self, namespace_id: &str) -> anyhow::Result<Namespace> {
        let n = self.api_client.namespace_get(namespace_id).await?;
        Ok(Namespace {
            namespace: n.namespace,
            namespace_show_name: n.namespace_show_name,
            namespace_desc: n.namespace_desc,
            quota: n.quota,
            config_count: n.config_count,
            type_: n.type_,
        })
    }

    async fn namespace_create(
        &self,
        namespace_id: &str,
        namespace_name: &str,
        namespace_desc: &str,
    ) -> anyhow::Result<bool> {
        self.api_client
            .namespace_create(namespace_id, namespace_name, namespace_desc)
            .await
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

    async fn namespace_exists(&self, namespace_id: &str) -> anyhow::Result<bool> {
        self.api_client.namespace_exists(namespace_id).await
    }

    // ============== Config Operations ==============

    async fn config_get(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
    ) -> anyhow::Result<Option<ConfigAllInfo>> {
        let result = self
            .api_client
            .config_get(data_id, group_name, namespace_id)
            .await?;

        Ok(result.map(|c| ConfigAllInfo {
            config_info: batata_config::ConfigInfo {
                config_info_base: batata_config::ConfigInfoBase {
                    id: c.id,
                    data_id: c.data_id,
                    group: c.group,
                    content: c.content,
                    md5: c.md5,
                    encrypted_data_key: c.encrypted_data_key,
                },
                tenant: c.tenant,
                app_name: c.app_name,
                r#type: c.r#type,
            },
            create_time: c.create_time,
            modify_time: c.modify_time,
            create_user: c.create_user,
            create_ip: c.create_ip,
            desc: c.desc,
            r#use: c.r#use,
            effect: c.effect,
            schema: c.schema,
            config_tags: c.config_tags,
        }))
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
        let result = self
            .api_client
            .config_list(
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
                .map(|c| ConfigBasicInfo {
                    id: c.id,
                    namespace_id: c.namespace_id,
                    group_name: c.group_name,
                    data_id: c.data_id,
                    md5: c.md5,
                    r#type: c.r#type,
                    app_name: c.app_name,
                    create_time: c.create_time,
                    modify_time: c.modify_time,
                })
                .collect(),
        ))
    }

    async fn config_publish(
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
    ) -> anyhow::Result<bool> {
        self.api_client
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
            .await
    }

    async fn config_delete(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
        _gray_name: &str,
        _client_ip: &str,
        _src_user: &str,
    ) -> anyhow::Result<bool> {
        self.api_client
            .config_delete(data_id, group_name, namespace_id)
            .await
    }

    async fn config_gray_get(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
    ) -> anyhow::Result<Option<ConfigInfoGrayWrapper>> {
        let result = self
            .api_client
            .config_gray_get(data_id, group_name, namespace_id)
            .await?;

        Ok(result.map(|c| ConfigInfoGrayWrapper {
            config_info: batata_config::ConfigInfo {
                config_info_base: batata_config::ConfigInfoBase {
                    id: c.id,
                    data_id: c.data_id,
                    group: c.group,
                    content: c.content,
                    md5: c.md5,
                    encrypted_data_key: String::new(),
                },
                tenant: c.tenant,
                app_name: String::new(),
                r#type: c.r#type,
            },
            last_modified: 0,
            gray_name: c.gray_name,
            gray_rule: c.gray_rule,
            src_user: c.src_user,
        }))
    }

    async fn config_export(
        &self,
        namespace_id: &str,
        group: Option<&str>,
        data_ids: Option<&str>,
        app_name: Option<&str>,
    ) -> anyhow::Result<Vec<u8>> {
        self.api_client
            .config_export(namespace_id, group, data_ids, app_name)
            .await
    }

    async fn config_import(
        &self,
        _file_data: Vec<u8>,
        _namespace_id: &str,
        _policy: SameConfigPolicy,
        _src_user: &str,
        _src_ip: &str,
    ) -> anyhow::Result<ImportResult> {
        // Import via multipart is complex, for now return error
        Err(anyhow::anyhow!(
            "Config import is not yet supported in remote console mode"
        ))
    }

    // ============== History Operations ==============

    async fn history_get(
        &self,
        nid: u64,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
    ) -> anyhow::Result<Option<ConfigHistoryInfo>> {
        let result = self
            .api_client
            .history_get(nid, data_id, group_name, namespace_id)
            .await?;

        Ok(result.map(|h| ConfigHistoryInfo {
            id: h.id,
            last_id: -1,
            data_id: h.data_id,
            group: h.group,
            tenant: h.tenant,
            content: h.content,
            md5: h.md5,
            app_name: h.app_name,
            created_time: h.created_time,
            last_modified_time: h.last_modified_time,
            src_user: h.src_user,
            src_ip: h.src_ip,
            op_type: h.op_type,
            publish_type: h.publish_type,
            gray_name: h.gray_name,
            ext_info: h.ext_info,
            encrypted_data_key: h.encrypted_data_key,
        }))
    }

    async fn history_list(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<ConfigHistoryInfo>> {
        let result = self
            .api_client
            .history_list(data_id, group_name, namespace_id, page_no, page_size)
            .await?;

        Ok(Page::new(
            result.total_count,
            result.page_number,
            result.pages_available,
            result
                .page_items
                .into_iter()
                .map(|h| ConfigHistoryInfo {
                    id: h.id,
                    last_id: -1,
                    data_id: h.data_id,
                    group: h.group,
                    tenant: h.tenant,
                    content: String::new(),
                    md5: String::new(),
                    app_name: String::new(),
                    created_time: h.created_time,
                    last_modified_time: h.last_modified_time,
                    src_user: h.src_user,
                    src_ip: h.src_ip,
                    op_type: h.op_type,
                    publish_type: h.publish_type,
                    gray_name: h.gray_name,
                    ext_info: String::new(),
                    encrypted_data_key: String::new(),
                })
                .collect(),
        ))
    }

    async fn history_configs_by_namespace(
        &self,
        namespace_id: &str,
    ) -> anyhow::Result<Vec<ConfigInfoWrapper>> {
        let result = self
            .api_client
            .history_configs_by_namespace(namespace_id)
            .await?;

        Ok(result
            .into_iter()
            .map(|c| ConfigInfoWrapper {
                id: Some(c.id as u64),
                namespace_id: c.namespace_id,
                group_name: c.group_name,
                data_id: c.data_id,
                md5: None,
                r#type: c.r#type,
                app_name: c.app_name,
                create_time: c.create_time,
                modify_time: c.modify_time,
            })
            .collect())
    }

    // ============== Cluster Operations ==============

    fn cluster_members(&self) -> Vec<Member> {
        self.cached_members.read_recover("cached_members").clone()
    }

    fn cluster_healthy_members(&self) -> Vec<Member> {
        self.cached_members
            .read_recover("cached_members")
            .iter()
            .filter(|m| m.state == "UP")
            .cloned()
            .collect()
    }

    fn cluster_health(&self) -> ClusterHealthResponse {
        self.cached_health
            .read_recover("cached_health")
            .clone()
            .unwrap_or(ClusterHealthResponse {
                is_healthy: false,
                summary: ClusterHealthSummary::default(),
                standalone: true,
            })
    }

    fn cluster_self(&self) -> SelfMemberResponse {
        self.cached_self
            .read_recover("cached_self")
            .clone()
            .unwrap_or(SelfMemberResponse {
                ip: "unknown".to_string(),
                port: 0,
                address: "unknown".to_string(),
                state: "unknown".to_string(),
                is_standalone: true,
                version: "unknown".to_string(),
            })
    }

    fn cluster_member(&self, address: &str) -> Option<Member> {
        self.cached_members
            .read_recover("cached_members")
            .iter()
            .find(|m| m.address == address)
            .cloned()
    }

    fn cluster_member_count(&self) -> usize {
        self.cached_members.read_recover("cached_members").len()
    }

    fn cluster_is_standalone(&self) -> bool {
        self.cached_health
            .read_recover("cached_health")
            .as_ref()
            .map(|h| h.standalone)
            .unwrap_or(true)
    }

    fn cluster_refresh_self(&self) {
        warn!("cluster_refresh_self called in remote mode - refresh not implemented");
    }

    // ============== Helper Methods ==============

    fn is_remote(&self) -> bool {
        true
    }

    fn database(&self) -> Option<&DatabaseConnection> {
        None
    }

    fn member_manager(&self) -> Option<Arc<ServerMemberManager>> {
        None
    }
}
