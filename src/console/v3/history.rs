use actix_web::{HttpResponse, Responder, Scope, get, web};
use serde::Deserialize;

use crate::{
    api::{config::model::ConfigHistoryBasicInfo, model::Page},
    model::{self, common::AppState},
    service,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchParam {
    data_id: Option<String>,
    group: Option<String>,
    tenant: Option<String>,
    app_name: Option<String>,
    nid: Option<u64>,
    page_no: Option<u64>,
    page_size: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetDataIdsParam {
    tenant: String,
}

#[get("list")]
async fn search(data: web::Data<AppState>, params: web::Query<SearchParam>) -> impl Responder {
    let result = service::history::search_page(
        &data.database_connection,
        params.data_id.clone().unwrap_or_default().as_str(),
        params.group.clone().unwrap_or_default().as_str(),
        params.tenant.clone().unwrap_or_default().as_str(),
        params.page_no.unwrap_or(1),
        params.page_size.unwrap_or(100),
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
async fn get_data_ids(
    data: web::Data<AppState>,
    params: web::Query<GetDataIdsParam>,
) -> impl Responder {
    let config_infos =
        service::history::get_config_list_by_namespace(&data.database_connection, &params.tenant)
            .await;

    return HttpResponse::Ok().json(config_infos.ok().unwrap());
}

pub fn routes() -> Scope {
    web::scope("/cs/history")
        .service(get_data_ids)
        .service(search)
}
