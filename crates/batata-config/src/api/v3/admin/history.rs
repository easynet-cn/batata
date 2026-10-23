//! V3 Admin config history endpoints

use actix_web::{HttpMessage, HttpRequest, Responder, get, web};
use serde::Deserialize;
use tracing::warn;

use batata_api::Page;

use batata_common::{default_page_no, default_page_size_small, resolve_namespace_id};
use batata_plugin::spi::{ConfigChangeRequest, ConfigPointcut, ExecuteType};
use batata_server_common::{
    ActionTypes, ApiType, Secured, SignType, error, model, model::AppState, secured,
};

use crate::api::config_model::{ConfigBasicInfo, ConfigHistoryBasicInfo, ConfigHistoryDetailInfo};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistoryListParam {
    data_id: String,
    group_name: String,
    #[serde(default)]
    tenant: Option<String>,
    #[serde(default)]
    namespace_id: Option<String>,
    #[serde(default = "default_page_no")]
    page_no: u64,
    #[serde(default = "default_page_size_small")]
    page_size: u64,
}

impl HistoryListParam {
    /// Resolved with the upstream precedence chain `namespaceId` -> `tenant`,
    /// then trimmed and defaulted by `NamespaceUtil.processNamespaceParameter`.
    fn resolved_namespace_id(&self) -> String {
        resolve_namespace_id(self.namespace_id.as_deref(), self.tenant.as_deref(), None)
    }
}

/// Namespace resolution shared by the history forms.
///
/// Upstream `HistoryControllerV3` reads `ConfigFormV3.getNamespaceId()` and
/// passes it through `NamespaceUtil.processNamespaceParameter`; `tenant` is
/// accepted here only as a legacy fallback, and always at lower precedence.
macro_rules! impl_namespace_resolver {
    () => {
        fn resolved_namespace_id(&self) -> String {
            resolve_namespace_id(self.namespace_id.as_deref(), None, None)
        }
    };
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistoryDetailParam {
    data_id: String,
    group_name: String,
    #[serde(default)]
    namespace_id: Option<String>,
    nid: i64,
}

impl HistoryDetailParam {
    impl_namespace_resolver!();
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistoryPreviousParam {
    data_id: String,
    group_name: String,
    #[serde(default)]
    namespace_id: Option<String>,
    id: i64,
}

impl HistoryPreviousParam {
    impl_namespace_resolver!();
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NamespaceConfigsParam {
    #[serde(default)]
    namespace_id: Option<String>,
}

impl NamespaceConfigsParam {
    fn resolved_namespace_id(&self) -> String {
        resolve_namespace_id(self.namespace_id.as_deref(), None, None)
    }
}

/// GET /v3/admin/cs/history/list
#[get("list")]
async fn list_history(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<HistoryListParam>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let namespace_id = params.resolved_namespace_id();

    // Execute ConfigChangePluginV2 Before hook for History
    let src_user = req
        .extensions()
        .get::<batata_common::IdentityContext>()
        .map(|ctx| ctx.username.clone())
        .unwrap_or_default();
    let src_ip = req
        .connection_info()
        .realip_remote_addr()
        .unwrap_or_default()
        .to_owned();
    let operator = if src_user.is_empty() {
        "anonymous"
    } else {
        &src_user
    };
    if let Some(plugin_mgr) = data.try_plugin_manager() {
        let mut req_plugin = ConfigChangeRequest::new(
            &params.data_id,
            &params.group_name,
            &namespace_id,
            "",
            ConfigPointcut::History,
            ExecuteType::Before,
            operator,
        );
        req_plugin.client_ip = Some(src_ip.to_string());
        let resp = plugin_mgr.execute_config_change_v2(req_plugin).await;
        if !resp.allowed {
            return model::common::Result::<String>::http_response(
                403,
                error::SERVER_ERROR.code,
                resp.message.unwrap_or_else(|| "plugin denied".to_string()),
                String::new(),
            );
        }
    }

    let result = data
        .persistence()
        .config_history_search_page(
            &params.data_id,
            &params.group_name,
            &namespace_id,
            params.page_no,
            params.page_size,
        )
        .await;

    // Execute ConfigChangePluginV2 After hook for History
    if let Some(plugin_mgr) = data.try_plugin_manager() {
        let mut req_plugin = ConfigChangeRequest::new(
            &params.data_id,
            &params.group_name,
            &namespace_id,
            "",
            ConfigPointcut::History,
            ExecuteType::After,
            operator,
        );
        req_plugin.client_ip = Some(src_ip.to_string());
        let _ = plugin_mgr.execute_config_change_v2(req_plugin).await;
    }

    match result {
        Ok(result) => {
            let page_result = Page::<ConfigHistoryBasicInfo>::new(
                result.total_count,
                result.page_number,
                result.pages_available,
                result
                    .page_items
                    .into_iter()
                    .map(ConfigHistoryBasicInfo::from)
                    .collect(),
            );
            model::common::Result::<Page<ConfigHistoryBasicInfo>>::http_success(page_result)
        }
        Err(e) => {
            warn!(error = %e, "Failed to list history");
            model::common::Result::<String>::http_response(
                500,
                500,
                format!("Failed to list history: {}", e),
                String::new(),
            )
        }
    }
}

/// GET /v3/admin/cs/history
#[get("")]
async fn get_detail(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<HistoryDetailParam>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let namespace_id = params.resolved_namespace_id();

    match data
        .persistence()
        .config_history_find_by_id(params.nid)
        .await
    {
        Ok(result) => {
            let config_info = result.map(ConfigHistoryDetailInfo::from);

            // Upstream `HistoryService.getConfigHistoryInfo` fetches by `nid`
            // and then calls `checkHistoryInfoPermission`, which rejects the
            // request when the stored entry belongs to a different
            // dataId/group/namespaceId. Without this the endpoint happily
            // returns another config's history when `nid` is ambiguous.
            let mismatched = config_info.as_ref().is_some_and(|info| {
                let basic = &info.config_history_basic_info.config_basic_info;
                basic.data_id != params.data_id
                    || basic.group_name != params.group_name
                    || resolve_namespace_id(Some(&basic.namespace_id), None, None) != namespace_id
            });
            if mismatched {
                warn!(
                    nid = params.nid,
                    "History detail does not match the requested dataId/group/namespaceId"
                );
                return model::common::Result::<String>::http_response(
                    403,
                    403,
                    "Please check dataId, group or namespaceId.".to_string(),
                    String::new(),
                );
            }

            model::common::Result::<Option<ConfigHistoryDetailInfo>>::http_success(config_info)
        }
        Err(e) => {
            warn!(error = %e, "Failed to get history detail");
            model::common::Result::<String>::http_response(
                500,
                500,
                format!("Failed to get history detail: {}", e),
                String::new(),
            )
        }
    }
}

/// GET /v3/admin/cs/history/previous
#[get("previous")]
async fn get_previous(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<HistoryPreviousParam>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let namespace_id = params.resolved_namespace_id();

    match data
        .persistence()
        .config_history_get_previous(
            &params.data_id,
            &params.group_name,
            &namespace_id,
            params.id,
        )
        .await
    {
        Ok(Some(item)) => {
            let detail = ConfigHistoryDetailInfo::from(item);
            model::common::Result::<ConfigHistoryDetailInfo>::http_success(detail)
        }
        Ok(None) => model::common::Result::<String>::http_response(
            404,
            404,
            "previous config history not found".to_string(),
            String::new(),
        ),
        Err(e) => {
            warn!(error = %e, "Failed to get previous history");
            model::common::Result::<String>::http_response(
                500,
                500,
                format!("Failed to get previous history: {}", e),
                String::new(),
            )
        }
    }
}

/// GET /v3/admin/cs/history/configs
#[get("configs")]
async fn get_configs_by_namespace(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<NamespaceConfigsParam>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let namespace_id = params.resolved_namespace_id();

    match data
        .persistence()
        .config_find_by_namespace(&namespace_id)
        .await
    {
        Ok(configs) => {
            let result: Vec<ConfigBasicInfo> =
                configs.into_iter().map(ConfigBasicInfo::from).collect();
            model::common::Result::<Vec<ConfigBasicInfo>>::http_success(result)
        }
        Err(e) => {
            warn!(error = %e, "Failed to get configs by namespace");
            model::common::Result::<String>::http_response(
                500,
                500,
                format!("Failed to get configs: {}", e),
                String::new(),
            )
        }
    }
}

/// Builds the Actix web `Scope` of routes for this module.
pub fn routes() -> actix_web::Scope {
    web::scope("/history")
        .service(list_history)
        .service(get_detail)
        .service(get_previous)
        .service(get_configs_by_namespace)
}
