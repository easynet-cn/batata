use actix_web::{delete, get, post, put, web, HttpResponse, Responder, Scope};
use serde::Deserialize;

use crate::api::model::AppState;
use crate::service;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetDataIdsParams {
    tenant: String,
}

#[get("configs")]
pub async fn get_data_ids(
    data: web::Data<AppState>,
    params: web::Query<GetDataIdsParams>,
) -> impl Responder {
    let tenant = params.tenant.clone();

    let config_infos =
        service::history::get_config_list_by_namespace(data.conns.get(0).unwrap(), tenant).await;

    return HttpResponse::Ok().json(config_infos.ok().unwrap());
}

pub fn routers() -> Scope {
    web::scope("/cs/history").service(get_data_ids)
}
