use std::collections::HashMap;

use actix_web::{delete, get, post, put, web, HttpResponse, Responder, Scope};
use serde::Deserialize;

use crate::{
    api::model::AppState,
    common::model::{ConfigInfo, Page},
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConfigSearchPageParam {
    search: Option<String>,
    data_id: Option<String>,
    group: Option<String>,
    app_name: Option<String>,
    #[serde(rename = "config_tags")]
    config_tags: Option<String>,
    tenant: Option<String>,
    page_no: u64,
    page_size: u64,
}

#[get("")]
pub async fn search(
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

        let page_result = crate::service::config::find_config_info_like_4_page(
            data.conns.get(0).unwrap(),
            params.page_no,
            params.page_size,
            params.data_id.clone().unwrap_or("".to_string()),
            params.group.clone().unwrap_or("".to_string()),
            params.tenant.clone().unwrap_or("".to_string()),
            config_advance_info,
        )
        .await;

        return HttpResponse::Ok().json(page_result);
    }

    let page_result = Page::<ConfigInfo> {
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
