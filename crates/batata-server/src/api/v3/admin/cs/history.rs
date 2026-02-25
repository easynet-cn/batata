//! V3 Admin config history endpoints

use actix_web::{HttpMessage, HttpRequest, Responder, get, web};
use serde::Deserialize;
use tracing::warn;

use batata_api::Page;

use crate::{
    ActionTypes, ApiType, Secured, SignType,
    api::config::model::{ConfigHistoryBasicInfo, ConfigHistoryDetailInfo},
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

pub fn routes() -> actix_web::Scope {
    web::scope("/history")
        .service(list_history)
        .service(get_detail)
}
