use actix_web::{HttpResponse, Responder, Scope, get, web};
use serde::Deserialize;

use crate::{
    api::{
        config::model::{ConfigBasicInfo, ConfigHistoryBasicInfo},
        model::Page,
    },
    model::{
        self,
        common::{AppState, DEFAULT_NAMESPACE_ID},
    },
    service,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchParam {
    data_id: String,
    group_name: String,
    tenant: Option<String>,
    namespace_id: Option<String>,
    page_no: u64,
    page_size: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FindConfigsbyNamespaceIdParam {
    namespace_id: String,
}

#[get("list")]
async fn search(data: web::Data<AppState>, params: web::Query<SearchParam>) -> impl Responder {
    let data_id = &params.data_id;
    let group_name = &params.group_name;
    let mut namespace_id = params.tenant.clone().unwrap_or_default();

    if namespace_id.is_empty() {
        namespace_id = params
            .namespace_id
            .clone()
            .unwrap_or(DEFAULT_NAMESPACE_ID.to_string());
    }

    let result = service::history::search_page(
        &data.database_connection,
        data_id,
        group_name,
        &namespace_id,
        params.page_no,
        params.page_size,
    )
    .await
    .unwrap();

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

#[get("configs")]
async fn find_configs_by_namespace_id(
    data: web::Data<AppState>,
    params: web::Query<FindConfigsbyNamespaceIdParam>,
) -> impl Responder {
    let config_infos = service::history::find_configs_by_namespace_id(
        &data.database_connection,
        &params.namespace_id,
    )
    .await
    .unwrap()
    .into_iter()
    .map(ConfigBasicInfo::from)
    .collect::<Vec<ConfigBasicInfo>>();

    model::common::Result::<Vec<ConfigBasicInfo>>::http_success(config_infos)
}

pub fn routes() -> Scope {
    web::scope("/cs/history")
        .service(search)
        .service(find_configs_by_namespace_id)
}
