use actix_web::{get, web, HttpRequest, HttpResponse, Responder, Scope};
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
    if params.search.is_some() && params.search.as_ref().unwrap() == "blur" {
        let search_param = params.0;

        let result = crate::service::config::search_page(
            &data.database_connection,
            search_param.page_no,
            search_param.page_size,
            search_param.tenant.unwrap_or_default().as_str(),
            search_param.data_id.unwrap_or_default().as_str(),
            search_param.group.unwrap_or_default().as_str(),
            search_param.app_name.unwrap_or_default().as_str(),
            search_param.config_tags.unwrap_or_default().as_str(),
            search_param.types.clone().unwrap_or_default().as_str(),
            search_param.config_detail.unwrap_or_default().as_str(),
        )
        .await;

        return match result {
            Ok(page_result) => HttpResponse::Ok().json(page_result),
            Err(err) => HttpResponse::InternalServerError().json(ErrorResult {
                timestamp: Utc::now().to_rfc3339(),
                status: 403,
                message: err.to_string(),
                error: String::from("Forbiden"),
                path: req.path().to_string(),
            }),
        };
    }

    return HttpResponse::Ok().json(Page::<ConfigInfo>::default());
}

pub fn routers() -> Scope {
    return web::scope("/cs/config").service(search);
}
