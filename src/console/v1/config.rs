use std::collections::HashMap;

use actix_web::{get, web, HttpRequest, HttpResponse, Responder, Scope};
use serde::Deserialize;

use chrono::Utc;

use crate::{
    api::model::{AppState, ErrorResult},
    common::model::{ConfigInfo, Page},
    service,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConfigSearchPageParam {
    search: Option<String>,
    show: Option<String>,
    data_id: Option<String>,
    group: Option<String>,
    app_name: Option<String>,
    #[serde(rename = "config_tags")]
    config_tags: Option<String>,
    tenant: Option<String>,
    page_no: Option<u64>,
    page_size: Option<u64>,
}

#[get("")]
pub async fn search(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<ConfigSearchPageParam>,
) -> impl Responder {
    if params.search.is_some() && params.search.as_ref().unwrap() == "blur" {
        let mut config_advance_info = HashMap::<String, String>::new();

        if params.app_name.is_some() && !params.app_name.as_ref().unwrap().to_string().is_empty() {
            config_advance_info.insert(
                "app_name".to_string(),
                params.app_name.as_ref().unwrap().to_string(),
            );
        }

        if params.config_tags.is_some()
            && !params.config_tags.as_ref().unwrap().to_string().is_empty()
        {
            config_advance_info.insert(
                "config_tags".to_string(),
                params.config_tags.as_ref().unwrap().to_string(),
            );
        }

        let result = crate::service::config::search_page(
            &data.database_connection,
            params.page_no.unwrap_or_default(),
            params.page_size.unwrap_or_default(),
            params.data_id.clone().unwrap_or("".to_string()),
            params.group.clone().unwrap_or("".to_string()),
            params.tenant.clone().unwrap_or("".to_string()),
            config_advance_info,
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
    } else if params.show.is_some() && params.show.as_ref().unwrap() == "all" {
        let config_all_info = service::config::find_all(
            &data.database_connection,
            params.data_id.clone().unwrap_or_default().as_str(),
            params.group.clone().unwrap_or_default().as_str(),
            params.tenant.clone().unwrap_or_default().as_str(),
        )
        .await
        .ok();

        return HttpResponse::Ok().json(config_all_info);
    }

    return HttpResponse::Ok().json(Page::<ConfigInfo>::default());
}

pub fn routers() -> Scope {
    web::scope("/cs/configs").service(search)
}
