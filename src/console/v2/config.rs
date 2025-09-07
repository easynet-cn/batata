use actix_web::{HttpRequest, HttpResponse, Responder, Scope, get, web};
use serde::Deserialize;

use chrono::Utc;

use crate::model::{
    common::{AppState, ErrorResult, Page},
    config::ConfigInfo,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchParam {
    search: Option<String>,
    data_id: Option<String>,
    group: Option<String>,
    app_name: Option<String>,
    #[serde(rename = "config_tags")]
    config_tags: Option<String>,
    tenant: Option<String>,
    types: Option<String>,
    #[serde(rename = "config_detail")]
    config_detail: Option<String>,
    page_no: u64,
    page_size: u64,
}

#[get("searchDetail")]
pub async fn search(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<SearchParam>,
) -> impl Responder {
    return HttpResponse::Ok().json(Page::<ConfigInfo>::default());
}

pub fn routes() -> Scope {
    return web::scope("/cs/config").service(search);
}
