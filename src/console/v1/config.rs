use actix_web::{delete, get, post, put, web, HttpResponse, Responder, Scope};
use serde::Deserialize;

use crate::common::model::{ConfigInfo, Page};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchConfigParams {
    show: Option<String>,
    namespace_id: Option<String>,
    check_namespace_id_exist: Option<bool>,
}

#[get("")]
pub async fn search() -> impl Responder {
    let mut page_result = Page::<ConfigInfo> {
        total_count: 1,
        page_number: 1,
        pages_available: 1,
        page_items: vec![],
    };

    return HttpResponse::Ok().json(page_result);
}

pub fn routers() -> Scope {
    web::scope("/cs/configs").service(search)
}
