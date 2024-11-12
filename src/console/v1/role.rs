use actix_web::{get, web, HttpResponse, Responder};
use serde::Deserialize;

use crate::api::model::AppState;
use crate::service;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchPageParam {
    search: Option<String>,
    username: Option<String>,
    role: Option<String>,
    page_no: u64,
    page_size: u64,
}

#[get("/roles")]
pub async fn search_page(
    data: web::Data<AppState>,
    params: web::Query<SearchPageParam>,
) -> impl Responder {
    let search = params.search.clone().unwrap();
    let accurate = search == "accurate";
    let mut username = params.username.clone().unwrap_or_default();

    if username.starts_with("*") {
        username = username.strip_prefix("*").unwrap().to_string();
    }
    if username.ends_with("*") {
        username = username.strip_suffix("*").unwrap().to_string();
    }

    let mut role = params.role.clone().unwrap_or_default();

    if role.starts_with("*") {
        role = role.strip_prefix("*").unwrap().to_string();
    }
    if role.ends_with("*") {
        role = role.strip_suffix("*").unwrap().to_string();
    }

    let result = service::role::search_page(
        &data.database_connection,
        &username,
        &role,
        params.page_no,
        params.page_size,
        accurate,
    )
    .await
    .unwrap();

    return HttpResponse::Ok().json(result);
}
