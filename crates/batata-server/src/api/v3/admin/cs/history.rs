//! V3 Admin config history endpoints

use actix_web::{HttpMessage, HttpRequest, Responder, get, web};
use serde::Deserialize;
use tracing::warn;

use batata_api::Page;

use crate::{
    ActionTypes, ApiType, Secured, SignType,
    api::config::model::{ConfigBasicInfo, ConfigHistoryBasicInfo, ConfigHistoryDetailInfo},
    model::{
        self,
        common::{AppState, DEFAULT_NAMESPACE_ID},
    },
    secured,
};

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
    #[serde(default = "default_page_size")]
    page_size: u64,
}

fn default_page_no() -> u64 {
    1
}

fn default_page_size() -> u64 {
    20
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct HistoryDetailParam {
    data_id: String,
    group_name: String,
    #[serde(default)]
    namespace_id: Option<String>,
    nid: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistoryPreviousParam {
    data_id: String,
    group_name: String,
    #[serde(default)]
    namespace_id: Option<String>,
    id: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NamespaceConfigsParam {
    namespace_id: String,
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

    let mut namespace_id = params.tenant.clone().unwrap_or_default();
    if namespace_id.is_empty() {
        namespace_id = params
            .namespace_id
            .clone()
            .unwrap_or(DEFAULT_NAMESPACE_ID.to_string());
    }

    match data
        .persistence()
        .config_history_search_page(
            &params.data_id,
            &params.group_name,
            &namespace_id,
            params.page_no,
            params.page_size,
        )
        .await
    {
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

/// GET /v3/admin/cs/history/detail
#[get("detail")]
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

    match data
        .persistence()
        .config_history_find_by_id(params.nid)
        .await
    {
        Ok(result) => {
            let config_info = result.map(ConfigHistoryDetailInfo::from);
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

    let namespace_id = params
        .namespace_id
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_NAMESPACE_ID)
        .to_string();

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

    let namespace_id = if params.namespace_id.is_empty() {
        DEFAULT_NAMESPACE_ID.to_string()
    } else {
        params.namespace_id.clone()
    };

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

pub fn routes() -> actix_web::Scope {
    web::scope("/history")
        .service(list_history)
        .service(get_detail)
        .service(get_previous)
        .service(get_configs_by_namespace)
}
