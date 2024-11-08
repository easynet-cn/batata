use actix_web::{get, web, HttpResponse, Responder, Scope};
use serde::Deserialize;

use crate::api::model::AppState;
use crate::service;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchParam {
    search: Option<String>,
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

#[get("")]
pub async fn search(data: web::Data<AppState>, params: web::Query<SearchParam>) -> impl Responder {
    if params.search.is_some() && params.search.as_ref().unwrap() == "accurate" {
        let result = service::history::search_page(
            &data.database_connection,
            params.data_id.clone().unwrap_or_default().as_str(),
            params.group.clone().unwrap_or_default().as_str(),
            params.tenant.clone().unwrap_or_default().as_str(),
            params.page_no.unwrap_or(1),
            params.page_size.unwrap_or(100),
        )
        .await;

        return HttpResponse::Ok().json(result.ok().unwrap());
    } else {
        let result =
            service::history::get_by_id(&data.database_connection, params.nid.unwrap()).await;

        return HttpResponse::Ok().json(result.ok().unwrap());
    }
}

#[get("configs")]
pub async fn get_data_ids(
    data: web::Data<AppState>,
    params: web::Query<GetDataIdsParam>,
) -> impl Responder {
    let config_infos =
        service::history::get_config_list_by_namespace(&data.database_connection, &params.tenant)
            .await;

    return HttpResponse::Ok().json(config_infos.ok().unwrap());
}

pub fn routers() -> Scope {
    web::scope("/cs/history")
        .service(get_data_ids)
        .service(search)
}
