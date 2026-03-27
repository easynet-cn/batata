//! API client for Batata/Nacos operations
//!
//! Provides typed methods for each console API endpoint.

use serde::Serialize;

use crate::http::BatataHttpClient;
use crate::model::{
    ApiResponse, CloneResult, ClusterHealthResponse, ConfigAllInfo, ConfigBasicInfo,
    ConfigGrayInfo, ConfigHistoryBasicInfo, ConfigHistoryDetailInfo, ConfigListenerInfo,
    ImportResult, InstanceInfo, Member, Namespace, Page, SelfMemberResponse, ServiceDetail,
    ServiceListItem, SubscriberInfo,
};

/// API client wrapper providing typed access to Batata/Nacos APIs
pub struct BatataApiClient {
    http_client: BatataHttpClient,
}

impl BatataApiClient {
    /// Create a new API client with the given HTTP client
    pub fn new(http_client: BatataHttpClient) -> Self {
        Self { http_client }
    }

    /// Get the underlying HTTP client
    pub fn http_client(&self) -> &BatataHttpClient {
        &self.http_client
    }

    // ============== Namespace APIs ==============

    /// Get all namespaces
    pub async fn namespace_list(&self) -> anyhow::Result<Vec<Namespace>> {
        let response: ApiResponse<Vec<Namespace>> = self
            .http_client
            .get("/v3/console/core/namespace/list")
            .await?;
        Ok(response.data)
    }

    /// Get namespace by ID
    pub async fn namespace_get(&self, namespace_id: &str) -> anyhow::Result<Namespace> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
        }

        let response: ApiResponse<Namespace> = self
            .http_client
            .get_with_query("/v3/console/core/namespace", &Query { namespace_id })
            .await?;
        Ok(response.data)
    }

    /// Create a new namespace
    pub async fn namespace_create(
        &self,
        namespace_id: &str,
        namespace_name: &str,
        namespace_desc: &str,
    ) -> anyhow::Result<bool> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Form<'a> {
            custom_namespace_id: &'a str,
            namespace_name: &'a str,
            namespace_desc: &'a str,
        }

        let response: ApiResponse<bool> = self
            .http_client
            .post_form(
                "/v3/console/core/namespace",
                &Form {
                    custom_namespace_id: namespace_id,
                    namespace_name,
                    namespace_desc,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Update an existing namespace
    pub async fn namespace_update(
        &self,
        namespace_id: &str,
        namespace_name: &str,
        namespace_desc: &str,
    ) -> anyhow::Result<bool> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Form<'a> {
            namespace_id: &'a str,
            namespace_name: &'a str,
            namespace_desc: &'a str,
        }

        let response: ApiResponse<bool> = self
            .http_client
            .put_form(
                "/v3/console/core/namespace",
                &Form {
                    namespace_id,
                    namespace_name,
                    namespace_desc,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Delete a namespace
    pub async fn namespace_delete(&self, namespace_id: &str) -> anyhow::Result<bool> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
        }

        let response: ApiResponse<bool> = self
            .http_client
            .delete_with_query("/v3/console/core/namespace", &Query { namespace_id })
            .await?;
        Ok(response.data)
    }

    /// Check if namespace exists
    pub async fn namespace_exists(&self, namespace_id: &str) -> anyhow::Result<bool> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            custom_namespace_id: &'a str,
        }

        let response: ApiResponse<bool> = self
            .http_client
            .get_with_query(
                "/v3/console/core/namespace/exist",
                &Query {
                    custom_namespace_id: namespace_id,
                },
            )
            .await?;
        Ok(response.data)
    }

    // ============== Config APIs ==============

    /// Get a single configuration
    pub async fn config_get(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
    ) -> anyhow::Result<Option<ConfigAllInfo>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            data_id: &'a str,
            group_name: &'a str,
            namespace_id: &'a str,
        }

        let response: ApiResponse<Option<ConfigAllInfo>> = self
            .http_client
            .get_with_query(
                "/v3/console/cs/config",
                &Query {
                    data_id,
                    group_name,
                    namespace_id,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Search configurations with pagination
    #[allow(clippy::too_many_arguments)]
    pub async fn config_list(
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
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            page_no: u64,
            page_size: u64,
            namespace_id: &'a str,
            data_id: &'a str,
            group_name: &'a str,
            app_name: &'a str,
            config_tags: &'a str,
            r#type: &'a str,
            content: &'a str,
        }

        let response: ApiResponse<Page<ConfigBasicInfo>> = self
            .http_client
            .get_with_query(
                "/v3/console/cs/config/list",
                &Query {
                    page_no,
                    page_size,
                    namespace_id,
                    data_id,
                    group_name,
                    app_name,
                    config_tags: tags,
                    r#type: types,
                    content,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Create or update a configuration
    #[allow(clippy::too_many_arguments)]
    pub async fn config_publish(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
        content: &str,
        app_name: &str,
        config_tags: &str,
        desc: &str,
        r#use: &str,
        effect: &str,
        r#type: &str,
        schema: &str,
        encrypted_data_key: &str,
    ) -> anyhow::Result<bool> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Form<'a> {
            data_id: &'a str,
            group_name: &'a str,
            namespace_id: &'a str,
            content: &'a str,
            app_name: &'a str,
            config_tags: &'a str,
            desc: &'a str,
            r#use: &'a str,
            effect: &'a str,
            r#type: &'a str,
            schema: &'a str,
            encrypted_data_key: &'a str,
        }

        let response: ApiResponse<bool> = self
            .http_client
            .post_form(
                "/v3/console/cs/config",
                &Form {
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
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Delete a configuration
    pub async fn config_delete(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
    ) -> anyhow::Result<bool> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            data_id: &'a str,
            group_name: &'a str,
            tenant: &'a str,
        }

        let response: ApiResponse<bool> = self
            .http_client
            .delete_with_query(
                "/v3/console/cs/config",
                &Query {
                    data_id,
                    group_name,
                    tenant: namespace_id,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Get gray/beta configuration
    pub async fn config_gray_get(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
    ) -> anyhow::Result<Option<ConfigGrayInfo>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            data_id: &'a str,
            group_name: &'a str,
            namespace_id: &'a str,
        }

        let response: ApiResponse<Option<ConfigGrayInfo>> = self
            .http_client
            .get_with_query(
                "/v3/console/cs/config/beta",
                &Query {
                    data_id,
                    group_name,
                    namespace_id,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Export configurations as ZIP
    pub async fn config_export(
        &self,
        namespace_id: &str,
        group: Option<&str>,
        data_ids: Option<&str>,
        app_name: Option<&str>,
    ) -> anyhow::Result<Vec<u8>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            group: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            data_ids: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            app_name: Option<&'a str>,
        }

        let query = Query {
            namespace_id,
            group,
            data_ids,
            app_name,
        };
        let query_string = serde_urlencoded::to_string(&query)?;
        let path = format!("/v3/console/cs/config/export?{}", query_string);

        self.http_client.get_bytes(&path).await
    }

    /// Import configuration from a ZIP file
    pub async fn config_import(
        &self,
        file_data: Vec<u8>,
        namespace_id: &str,
        policy: &str,
    ) -> anyhow::Result<ImportResult> {
        // Build query parameters
        let query_string =
            serde_urlencoded::to_string([("namespace_id", namespace_id), ("policy", policy)])?;
        let path = format!("/v3/console/cs/config/import?{}", query_string);

        // Create multipart form
        let part = reqwest::multipart::Part::bytes(file_data)
            .file_name("import.zip")
            .mime_str("application/zip")?;
        let form = reqwest::multipart::Form::new().part("file", part);

        let response: ApiResponse<ImportResult> =
            self.http_client.post_multipart(&path, form).await?;
        Ok(response.data)
    }

    /// Batch delete configurations by IDs
    pub async fn config_batch_delete(&self, ids: &[i64]) -> anyhow::Result<usize> {
        let ids_str = ids
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(",");

        #[derive(Serialize)]
        struct Query<'a> {
            ids: &'a str,
        }

        let response: ApiResponse<usize> = self
            .http_client
            .delete_with_query(
                "/v3/console/cs/config/batchDelete",
                &Query { ids: &ids_str },
            )
            .await?;
        Ok(response.data)
    }

    /// Delete gray/beta configuration
    pub async fn config_gray_delete(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
    ) -> anyhow::Result<bool> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            data_id: &'a str,
            group_name: &'a str,
            namespace_id: &'a str,
        }

        let response: ApiResponse<bool> = self
            .http_client
            .delete_with_query(
                "/v3/console/cs/config/beta",
                &Query {
                    data_id,
                    group_name,
                    namespace_id,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Publish gray/beta configuration
    #[allow(clippy::too_many_arguments)]
    pub async fn config_gray_publish(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
        content: &str,
        gray_name: &str,
        gray_rule: &str,
        app_name: &str,
        encrypted_data_key: &str,
    ) -> anyhow::Result<bool> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Body<'a> {
            data_id: &'a str,
            group_name: &'a str,
            namespace_id: &'a str,
            content: &'a str,
            gray_name: &'a str,
            gray_rule: &'a str,
            app_name: &'a str,
            encrypted_data_key: &'a str,
        }

        let response: ApiResponse<bool> = self
            .http_client
            .post_json(
                "/v3/console/cs/config/beta",
                &Body {
                    data_id,
                    group_name,
                    namespace_id,
                    content,
                    gray_name,
                    gray_rule,
                    app_name,
                    encrypted_data_key,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Search gray/beta configurations with pagination
    pub async fn config_gray_list(
        &self,
        page_no: u64,
        page_size: u64,
        namespace_id: &str,
        data_id: &str,
        group_name: &str,
        app_name: &str,
    ) -> anyhow::Result<Page<ConfigGrayInfo>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            page_no: u64,
            page_size: u64,
            namespace_id: &'a str,
            data_id: &'a str,
            group_name: &'a str,
            app_name: &'a str,
        }

        let response: ApiResponse<Page<ConfigGrayInfo>> = self
            .http_client
            .get_with_query(
                "/v3/console/cs/config/beta/list",
                &Query {
                    page_no,
                    page_size,
                    namespace_id,
                    data_id,
                    group_name,
                    app_name,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Find all gray configs for a specific config
    pub async fn config_gray_find_list(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
    ) -> anyhow::Result<Vec<ConfigGrayInfo>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            data_id: &'a str,
            group_name: &'a str,
            namespace_id: &'a str,
        }

        let response: ApiResponse<Vec<ConfigGrayInfo>> = self
            .http_client
            .get_with_query(
                "/v3/console/cs/config/beta/versions",
                &Query {
                    data_id,
                    group_name,
                    namespace_id,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Clone configurations to another namespace
    pub async fn config_clone(
        &self,
        ids: &[i64],
        target_namespace_id: &str,
        policy: &str,
    ) -> anyhow::Result<CloneResult> {
        let ids_str = ids
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(",");

        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            ids: &'a str,
            target_namespace_id: &'a str,
            policy: &'a str,
        }

        let response: ApiResponse<CloneResult> = self
            .http_client
            .post_with_query(
                "/v3/console/cs/config/clone",
                &Query {
                    ids: &ids_str,
                    target_namespace_id,
                    policy,
                },
            )
            .await?;
        Ok(response.data)
    }

    // ============== History APIs ==============

    /// Get history entry by ID
    pub async fn history_get(
        &self,
        nid: u64,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
    ) -> anyhow::Result<Option<ConfigHistoryDetailInfo>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            nid: u64,
            data_id: &'a str,
            group_name: &'a str,
            namespace_id: &'a str,
        }

        let response: ApiResponse<Option<ConfigHistoryDetailInfo>> = self
            .http_client
            .get_with_query(
                "/v3/console/cs/history",
                &Query {
                    nid,
                    data_id,
                    group_name,
                    namespace_id,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Search history with pagination
    pub async fn history_list(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<ConfigHistoryBasicInfo>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            data_id: &'a str,
            group_name: &'a str,
            namespace_id: &'a str,
            page_no: u64,
            page_size: u64,
        }

        let response: ApiResponse<Page<ConfigHistoryBasicInfo>> = self
            .http_client
            .get_with_query(
                "/v3/console/cs/history/list",
                &Query {
                    data_id,
                    group_name,
                    namespace_id,
                    page_no,
                    page_size,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Get configs by namespace ID (from history)
    pub async fn history_configs_by_namespace(
        &self,
        namespace_id: &str,
    ) -> anyhow::Result<Vec<ConfigBasicInfo>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
        }

        let response: ApiResponse<Vec<ConfigBasicInfo>> = self
            .http_client
            .get_with_query("/v3/console/cs/history/configs", &Query { namespace_id })
            .await?;
        Ok(response.data)
    }

    // ============== Cluster APIs ==============

    /// Get all cluster members
    pub async fn cluster_members(&self) -> anyhow::Result<Vec<Member>> {
        let response: ApiResponse<Vec<Member>> = self
            .http_client
            .get("/v3/console/core/cluster/nodes")
            .await?;
        Ok(response.data)
    }

    /// Get healthy cluster members
    pub async fn cluster_healthy_members(&self) -> anyhow::Result<Vec<Member>> {
        let response: ApiResponse<Vec<Member>> = self
            .http_client
            .get("/v3/console/core/cluster/nodes/healthy")
            .await?;
        Ok(response.data)
    }

    /// Get cluster health status
    pub async fn cluster_health(&self) -> anyhow::Result<ClusterHealthResponse> {
        let response: ApiResponse<ClusterHealthResponse> = self
            .http_client
            .get("/v3/console/core/cluster/health")
            .await?;
        Ok(response.data)
    }

    /// Get self member info
    pub async fn cluster_self(&self) -> anyhow::Result<SelfMemberResponse> {
        let response: ApiResponse<SelfMemberResponse> = self
            .http_client
            .get("/v3/console/core/cluster/self")
            .await?;
        Ok(response.data)
    }

    /// Get a specific member by address
    pub async fn cluster_member(&self, address: &str) -> anyhow::Result<Option<Member>> {
        let path = format!("/v3/console/core/cluster/node/{}", address);
        match self.http_client.get::<ApiResponse<Member>>(&path).await {
            Ok(response) => Ok(Some(response.data)),
            Err(_) => Ok(None),
        }
    }

    /// Get cluster member count
    pub async fn cluster_member_count(&self) -> anyhow::Result<usize> {
        let response: ApiResponse<usize> = self
            .http_client
            .get("/v3/console/core/cluster/count")
            .await?;
        Ok(response.data)
    }

    /// Check if running in standalone mode
    pub async fn cluster_is_standalone(&self) -> anyhow::Result<bool> {
        let response: ApiResponse<bool> = self
            .http_client
            .get("/v3/console/core/cluster/standalone")
            .await?;
        Ok(response.data)
    }

    /// Refresh self member info
    pub async fn cluster_refresh_self(&self) -> anyhow::Result<bool> {
        let response: ApiResponse<bool> = self
            .http_client
            .post_form("/v3/console/core/cluster/self/refresh", &())
            .await?;
        Ok(response.data)
    }

    // ============== Service APIs ==============

    /// Create a new service
    #[allow(clippy::too_many_arguments)]
    pub async fn service_create(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        protect_threshold: f32,
        metadata: &str,
        selector: &str,
    ) -> anyhow::Result<bool> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Form<'a> {
            namespace_id: &'a str,
            group_name: &'a str,
            service_name: &'a str,
            protect_threshold: f32,
            metadata: &'a str,
            selector: &'a str,
        }

        let response: ApiResponse<bool> = self
            .http_client
            .post_json(
                "/v3/console/ns/service",
                &Form {
                    namespace_id,
                    group_name,
                    service_name,
                    protect_threshold,
                    metadata,
                    selector,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Delete a service
    pub async fn service_delete(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
    ) -> anyhow::Result<bool> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
            group_name: &'a str,
            service_name: &'a str,
        }

        let response: ApiResponse<bool> = self
            .http_client
            .delete_with_query(
                "/v3/console/ns/service",
                &Query {
                    namespace_id,
                    group_name,
                    service_name,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Update a service
    #[allow(clippy::too_many_arguments)]
    pub async fn service_update(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        protect_threshold: f32,
        metadata: &str,
        selector: &str,
    ) -> anyhow::Result<bool> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Form<'a> {
            namespace_id: &'a str,
            group_name: &'a str,
            service_name: &'a str,
            protect_threshold: f32,
            metadata: &'a str,
            selector: &'a str,
        }

        let response: ApiResponse<bool> = self
            .http_client
            .put_json(
                "/v3/console/ns/service",
                &Form {
                    namespace_id,
                    group_name,
                    service_name,
                    protect_threshold,
                    metadata,
                    selector,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Get service detail
    pub async fn service_get(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
    ) -> anyhow::Result<Option<ServiceDetail>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
            group_name: &'a str,
            service_name: &'a str,
        }

        let response: ApiResponse<ServiceDetail> = self
            .http_client
            .get_with_query(
                "/v3/console/ns/service",
                &Query {
                    namespace_id,
                    group_name,
                    service_name,
                },
            )
            .await?;
        Ok(Some(response.data))
    }

    /// List services with pagination
    #[allow(clippy::too_many_arguments)]
    pub async fn service_list(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name_param: &str,
        page_no: u32,
        page_size: u32,
        with_instances: bool,
    ) -> anyhow::Result<Page<ServiceListItem>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
            group_name: &'a str,
            service_name_param: &'a str,
            page_no: u32,
            page_size: u32,
            with_instances: bool,
        }

        let response: ApiResponse<Page<ServiceListItem>> = self
            .http_client
            .get_with_query(
                "/v3/console/ns/service/list",
                &Query {
                    namespace_id,
                    group_name,
                    service_name_param,
                    page_no,
                    page_size,
                    with_instances,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Get service subscribers
    pub async fn service_subscribers(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        page_no: u32,
        page_size: u32,
    ) -> anyhow::Result<Page<SubscriberInfo>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
            group_name: &'a str,
            service_name: &'a str,
            page_no: u32,
            page_size: u32,
        }

        let response: ApiResponse<Page<SubscriberInfo>> = self
            .http_client
            .get_with_query(
                "/v3/console/ns/service/subscribers",
                &Query {
                    namespace_id,
                    group_name,
                    service_name,
                    page_no,
                    page_size,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Get selector types
    pub async fn service_selector_types(&self) -> anyhow::Result<Vec<String>> {
        let response: ApiResponse<Vec<String>> = self
            .http_client
            .get("/v3/console/ns/service/selector/types")
            .await?;
        Ok(response.data)
    }

    /// Update cluster configuration
    #[allow(clippy::too_many_arguments)]
    pub async fn service_cluster_update(
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
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Form<'a> {
            namespace_id: &'a str,
            group_name: &'a str,
            service_name: &'a str,
            cluster_name: &'a str,
            check_port: i32,
            use_instance_port: bool,
            health_check_type: &'a str,
            metadata: &'a str,
        }

        let response: ApiResponse<bool> = self
            .http_client
            .put_json(
                "/v3/console/ns/service/cluster",
                &Form {
                    namespace_id,
                    group_name,
                    service_name,
                    cluster_name,
                    check_port,
                    use_instance_port,
                    health_check_type,
                    metadata,
                },
            )
            .await?;
        Ok(response.data)
    }

    // ============== Instance APIs ==============

    /// List instances of a service
    pub async fn instance_list(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
        page_no: u32,
        page_size: u32,
    ) -> anyhow::Result<Page<InstanceInfo>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
            group_name: &'a str,
            service_name: &'a str,
            cluster_name: &'a str,
            page_no: u32,
            page_size: u32,
        }

        let response: ApiResponse<Page<InstanceInfo>> = self
            .http_client
            .get_with_query(
                "/v3/console/ns/instance/list",
                &Query {
                    namespace_id,
                    group_name,
                    service_name,
                    cluster_name,
                    page_no,
                    page_size,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Update an instance
    #[allow(clippy::too_many_arguments)]
    pub async fn instance_update(
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
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Form<'a> {
            namespace_id: &'a str,
            group_name: &'a str,
            service_name: &'a str,
            cluster_name: &'a str,
            ip: &'a str,
            port: i32,
            weight: f64,
            healthy: bool,
            enabled: bool,
            ephemeral: bool,
            metadata: &'a str,
        }

        let response: ApiResponse<bool> = self
            .http_client
            .put_json(
                "/v3/console/ns/instance",
                &Form {
                    namespace_id,
                    group_name,
                    service_name,
                    cluster_name,
                    ip,
                    port,
                    weight,
                    healthy,
                    enabled,
                    ephemeral,
                    metadata,
                },
            )
            .await?;
        Ok(response.data)
    }

    // ============== Config Listener APIs ==============

    /// Get config listeners
    pub async fn config_listeners(
        &self,
        data_id: &str,
        group_name: &str,
        namespace_id: &str,
    ) -> anyhow::Result<Vec<ConfigListenerInfo>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            data_id: &'a str,
            group_name: &'a str,
            namespace_id: &'a str,
        }

        let response: ApiResponse<Vec<ConfigListenerInfo>> = self
            .http_client
            .get_with_query(
                "/v3/console/cs/config/listener",
                &Query {
                    data_id,
                    group_name,
                    namespace_id,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Get config listeners by IP
    pub async fn config_listeners_by_ip(
        &self,
        ip: &str,
    ) -> anyhow::Result<Vec<ConfigListenerInfo>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            ip: &'a str,
        }

        let response: ApiResponse<Vec<ConfigListenerInfo>> = self
            .http_client
            .get_with_query("/v3/console/cs/config/listener/ip", &Query { ip })
            .await?;
        Ok(response.data)
    }
    // ============== Instance Admin APIs ==============

    /// Register a new instance via HTTP Admin API
    pub async fn instance_create(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        ip: &str,
        port: i32,
        cluster_name: &str,
        ephemeral: bool,
    ) -> anyhow::Result<String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Body<'a> {
            namespace_id: &'a str,
            group_name: &'a str,
            service_name: &'a str,
            ip: &'a str,
            port: i32,
            cluster_name: &'a str,
            ephemeral: bool,
        }
        let response: ApiResponse<String> = self
            .http_client
            .post_form(
                "/v3/admin/ns/instance",
                &Body {
                    namespace_id,
                    group_name,
                    service_name,
                    ip,
                    port,
                    cluster_name,
                    ephemeral,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Deregister an instance via HTTP Admin API
    pub async fn instance_delete(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        ip: &str,
        port: i32,
        cluster_name: &str,
    ) -> anyhow::Result<bool> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
            group_name: &'a str,
            service_name: &'a str,
            ip: &'a str,
            port: i32,
            cluster_name: &'a str,
        }
        let response: ApiResponse<bool> = self
            .http_client
            .delete_with_query(
                "/v3/admin/ns/instance",
                &Query {
                    namespace_id,
                    group_name,
                    service_name,
                    ip,
                    port,
                    cluster_name,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Get instance detail
    pub async fn instance_get(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        ip: &str,
        port: i32,
        cluster_name: &str,
    ) -> anyhow::Result<InstanceInfo> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
            group_name: &'a str,
            service_name: &'a str,
            ip: &'a str,
            port: i32,
            cluster_name: &'a str,
        }
        let response: ApiResponse<InstanceInfo> = self
            .http_client
            .get_with_query(
                "/v3/admin/ns/instance",
                &Query {
                    namespace_id,
                    group_name,
                    service_name,
                    ip,
                    port,
                    cluster_name,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Update instance health status (for persistent instances)
    pub async fn instance_health_update(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        ip: &str,
        port: i32,
        healthy: bool,
    ) -> anyhow::Result<String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Body<'a> {
            namespace_id: &'a str,
            group_name: &'a str,
            service_name: &'a str,
            ip: &'a str,
            port: i32,
            healthy: bool,
        }
        let response: ApiResponse<String> = self
            .http_client
            .put_form(
                "/v3/admin/ns/health/instance",
                &Body {
                    namespace_id,
                    group_name,
                    service_name,
                    ip,
                    port,
                    healthy,
                },
            )
            .await?;
        Ok(response.data)
    }

    // ============== Client Introspection APIs ==============

    /// List all connected client IDs
    pub async fn client_list(&self) -> anyhow::Result<Vec<serde_json::Value>> {
        let response: ApiResponse<Vec<serde_json::Value>> =
            self.http_client.get("/v3/admin/ns/client/list").await?;
        Ok(response.data)
    }

    /// Get client detail by connection ID
    pub async fn client_detail(&self, client_id: &str) -> anyhow::Result<serde_json::Value> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            client_id: &'a str,
        }
        let response: ApiResponse<serde_json::Value> = self
            .http_client
            .get_with_query("/v3/admin/ns/client", &Query { client_id })
            .await?;
        Ok(response.data)
    }

    /// Get services published by a client
    pub async fn client_published_services(
        &self,
        client_id: &str,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            client_id: &'a str,
        }
        let response: ApiResponse<Vec<serde_json::Value>> = self
            .http_client
            .get_with_query("/v3/admin/ns/client/publish/list", &Query { client_id })
            .await?;
        Ok(response.data)
    }

    /// Get services subscribed by a client
    pub async fn client_subscribed_services(
        &self,
        client_id: &str,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            client_id: &'a str,
        }
        let response: ApiResponse<Vec<serde_json::Value>> = self
            .http_client
            .get_with_query("/v3/admin/ns/client/subscribe/list", &Query { client_id })
            .await?;
        Ok(response.data)
    }

    /// Get publishers of a service
    pub async fn service_publisher_clients(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
            group_name: &'a str,
            service_name: &'a str,
        }
        let response: ApiResponse<Vec<serde_json::Value>> = self
            .http_client
            .get_with_query(
                "/v3/admin/ns/client/service/publisher/list",
                &Query {
                    namespace_id,
                    group_name,
                    service_name,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Get subscribers of a service
    pub async fn service_subscriber_clients(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            namespace_id: &'a str,
            group_name: &'a str,
            service_name: &'a str,
        }
        let response: ApiResponse<Vec<serde_json::Value>> = self
            .http_client
            .get_with_query(
                "/v3/admin/ns/client/service/subscriber/list",
                &Query {
                    namespace_id,
                    group_name,
                    service_name,
                },
            )
            .await?;
        Ok(response.data)
    }

    // ============== Core Server Admin APIs ==============

    /// Get server state (key-value map)
    pub async fn server_state(&self) -> anyhow::Result<serde_json::Value> {
        let response: ApiResponse<serde_json::Value> =
            self.http_client.get("/v3/admin/core/state").await?;
        Ok(response.data)
    }

    /// Liveness probe (for K8s)
    pub async fn liveness(&self) -> anyhow::Result<String> {
        let response: ApiResponse<String> = self
            .http_client
            .get("/v3/admin/core/state/liveness")
            .await?;
        Ok(response.data)
    }

    /// Readiness probe (for K8s)
    pub async fn readiness(&self) -> anyhow::Result<String> {
        let response: ApiResponse<String> = self
            .http_client
            .get("/v3/admin/core/state/readiness")
            .await?;
        Ok(response.data)
    }

    /// Get naming metrics
    pub async fn naming_metrics(&self) -> anyhow::Result<serde_json::Value> {
        let response: ApiResponse<serde_json::Value> =
            self.http_client.get("/v3/admin/ns/ops/metrics").await?;
        Ok(response.data)
    }

    /// Get available health checkers
    pub async fn health_checkers(&self) -> anyhow::Result<Vec<String>> {
        let response: ApiResponse<Vec<String>> =
            self.http_client.get("/v3/admin/ns/health/checkers").await?;
        Ok(response.data)
    }

    // ============== Config Admin Extension APIs ==============

    /// Update config metadata (description, tags) without changing content
    pub async fn config_update_metadata(
        &self,
        data_id: &str,
        group: &str,
        namespace_id: &str,
        desc: &str,
        config_tags: &str,
    ) -> anyhow::Result<bool> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Body<'a> {
            data_id: &'a str,
            group: &'a str,
            namespace_id: &'a str,
            desc: &'a str,
            config_tags: &'a str,
        }
        let response: ApiResponse<bool> = self
            .http_client
            .put_form(
                "/v3/admin/cs/config/metadata",
                &Body {
                    data_id,
                    group,
                    namespace_id,
                    desc,
                    config_tags,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Get previous config history entry
    pub async fn history_previous(
        &self,
        data_id: &str,
        group: &str,
        namespace_id: &str,
        id: i64,
    ) -> anyhow::Result<ConfigHistoryDetailInfo> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query<'a> {
            data_id: &'a str,
            group: &'a str,
            namespace_id: &'a str,
            id: i64,
        }
        let response: ApiResponse<ConfigHistoryDetailInfo> = self
            .http_client
            .get_with_query(
                "/v3/admin/cs/history/previous",
                &Query {
                    data_id,
                    group,
                    namespace_id,
                    id,
                },
            )
            .await?;
        Ok(response.data)
    }

    // ============== Server Loader/Connection Management APIs ==============

    /// Get current connected clients with connection info
    pub async fn loader_current(&self) -> anyhow::Result<serde_json::Value> {
        let response: ApiResponse<serde_json::Value> = self
            .http_client
            .get("/v3/admin/core/loader/current")
            .await?;
        Ok(response.data)
    }

    /// Get cluster-wide loader metrics
    pub async fn loader_cluster_metrics(&self) -> anyhow::Result<serde_json::Value> {
        let response: ApiResponse<serde_json::Value> = self
            .http_client
            .get("/v3/admin/core/loader/cluster")
            .await?;
        Ok(response.data)
    }

    /// Reload SDK connections (migrate connections to balance load)
    pub async fn loader_reload_current(
        &self,
        count: i32,
        redirect_address: &str,
    ) -> anyhow::Result<String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Body<'a> {
            count: i32,
            redirect_address: &'a str,
        }
        let response: ApiResponse<String> = self
            .http_client
            .post_form(
                "/v3/admin/core/loader/reloadCurrent",
                &Body {
                    count,
                    redirect_address,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Smart reload cluster (balance connections across nodes)
    pub async fn loader_smart_reload(&self, loader_factor: &str) -> anyhow::Result<String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Body<'a> {
            loader_factor: &'a str,
        }
        let response: ApiResponse<String> = self
            .http_client
            .post_form(
                "/v3/admin/core/loader/smartReloadCluster",
                &Body { loader_factor },
            )
            .await?;
        Ok(response.data)
    }

    /// Reload a single client connection
    pub async fn loader_reload_client(
        &self,
        connection_id: &str,
        redirect_address: &str,
    ) -> anyhow::Result<String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Body<'a> {
            connection_id: &'a str,
            redirect_address: &'a str,
        }
        let response: ApiResponse<String> = self
            .http_client
            .post_form(
                "/v3/admin/core/loader/reloadClient",
                &Body {
                    connection_id,
                    redirect_address,
                },
            )
            .await?;
        Ok(response.data)
    }

    /// Update log level dynamically
    pub async fn update_log_level(
        &self,
        log_name: &str,
        log_level: &str,
    ) -> anyhow::Result<String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Body<'a> {
            log_name: &'a str,
            log_level: &'a str,
        }
        let response: ApiResponse<String> = self
            .http_client
            .put_form(
                "/v3/admin/core/ops/log",
                &Body {
                    log_name,
                    log_level,
                },
            )
            .await?;
        Ok(response.data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::HttpClientConfig;

    #[test]
    fn test_api_client_creation() {
        let config = HttpClientConfig::new("http://localhost:8848");
        let http_client = BatataHttpClient::new_without_auth(config).unwrap();
        let _api_client = BatataApiClient::new(http_client);
    }
}
