use actix_web::{HttpMessage, HttpRequest, Responder, Scope, get, web};
use serde::Deserialize;

use crate::{
    ActionTypes, ApiType, Secured, SignType,
    api::{
        config::model::{ConfigBasicInfo, ConfigHistoryBasicInfo, ConfigHistoryDetailInfo},
        model::Page,
    },
    model::{
        self,
        common::{AppState, DEFAULT_NAMESPACE_ID},
    },
    secured, service,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FindOneParam {
    data_id: String,
    group_name: String,
    namespace_id: String,
    nid: u64,
}

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

#[get("")]
async fn find_one(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<FindOneParam>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let result = service::history::find_by_id(&data.database_connection, params.nid)
        .await
        .unwrap()
        .map(ConfigHistoryDetailInfo::from);

    model::common::Result::<Option<ConfigHistoryDetailInfo>>::http_success(result)
}

#[get("list")]
async fn search(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<SearchParam>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

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
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<FindConfigsbyNamespaceIdParam>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

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
        .service(find_one)
        .service(search)
        .service(find_configs_by_namespace_id)
}
