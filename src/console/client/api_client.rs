// API client for remote console operations
// Provides typed methods for each console API endpoint

use serde::{Deserialize, Serialize};

use crate::{
    api::{
        config::model::{
            ConfigBasicInfo, ConfigGrayInfo, ConfigHistoryBasicInfo, ConfigHistoryDetailInfo,
        },
        model::{Member, Page},
    },
    config::model::ConfigAllInfo,
    console::v3::cluster::{ClusterHealthResponse, SelfMemberResponse},
    model::naming::Namespace,
};

use super::ConsoleHttpClient;

/// API client wrapper providing typed access to console APIs
pub struct ConsoleApiClient {
    http_client: ConsoleHttpClient,
}

// Generic API response wrapper
#[derive(Debug, Deserialize)]
struct ApiResponse<T> {
    #[allow(dead_code)]
    pub code: i32,
    #[allow(dead_code)]
    pub message: String,
    pub data: T,
}

impl ConsoleApiClient {
    pub fn new(http_client: ConsoleHttpClient) -> Self {
        Self { http_client }
    }

    // ============== Namespace APIs ==============

    pub async fn namespace_find_all(&self) -> anyhow::Result<Vec<Namespace>> {
        let response: ApiResponse<Vec<Namespace>> = self
            .http_client
            .get("/v3/console/core/namespace/list")
            .await?;
        Ok(response.data)
    }

    pub async fn namespace_get_by_id(&self, namespace_id: &str) -> anyhow::Result<Namespace> {
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

    pub async fn namespace_check(&self, namespace_id: &str) -> anyhow::Result<bool> {
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

    pub async fn config_find_one(
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

    #[allow(clippy::too_many_arguments)]
    pub async fn config_search_page(
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

    #[allow(clippy::too_many_arguments)]
    pub async fn config_create_or_update(
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

    pub async fn config_find_gray_one(
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

        // Build URL with query params manually for binary response
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

    // ============== History APIs ==============

    pub async fn history_find_by_id(
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

    pub async fn history_search_page(
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

    pub async fn history_find_configs_by_namespace_id(
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

    pub async fn cluster_all_members(&self) -> anyhow::Result<Vec<Member>> {
        let response: ApiResponse<Vec<Member>> = self
            .http_client
            .get("/v3/console/core/cluster/nodes")
            .await?;
        Ok(response.data)
    }

    pub async fn cluster_healthy_members(&self) -> anyhow::Result<Vec<Member>> {
        let response: ApiResponse<Vec<Member>> = self
            .http_client
            .get("/v3/console/core/cluster/nodes/healthy")
            .await?;
        Ok(response.data)
    }

    pub async fn cluster_get_health(&self) -> anyhow::Result<ClusterHealthResponse> {
        let response: ApiResponse<ClusterHealthResponse> = self
            .http_client
            .get("/v3/console/core/cluster/health")
            .await?;
        Ok(response.data)
    }

    pub async fn cluster_get_self(&self) -> anyhow::Result<SelfMemberResponse> {
        let response: ApiResponse<SelfMemberResponse> = self
            .http_client
            .get("/v3/console/core/cluster/self")
            .await?;
        Ok(response.data)
    }

    pub async fn cluster_get_member(&self, address: &str) -> anyhow::Result<Option<Member>> {
        let path = format!("/v3/console/core/cluster/node/{}", address);
        match self.http_client.get::<ApiResponse<Member>>(&path).await {
            Ok(response) => Ok(Some(response.data)),
            Err(_) => Ok(None),
        }
    }

    pub async fn cluster_member_count(&self) -> anyhow::Result<usize> {
        let response: ApiResponse<usize> = self
            .http_client
            .get("/v3/console/core/cluster/count")
            .await?;
        Ok(response.data)
    }

    pub async fn cluster_is_standalone(&self) -> anyhow::Result<bool> {
        let response: ApiResponse<bool> = self
            .http_client
            .get("/v3/console/core/cluster/standalone")
            .await?;
        Ok(response.data)
    }

    pub async fn cluster_refresh_self(&self) -> anyhow::Result<bool> {
        let response: ApiResponse<bool> = self
            .http_client
            .post_form("/v3/console/core/cluster/self/refresh", &())
            .await?;
        Ok(response.data)
    }
}
