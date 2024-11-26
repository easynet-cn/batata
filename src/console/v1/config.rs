use std::collections::HashMap;

use actix_web::{get, post, web, HttpMessage, HttpRequest, HttpResponse, Responder, Scope};
use sea_orm::ColIdx;
use serde::Deserialize;

use chrono::Utc;

use crate::{
    api::model::{AppState, ErrorResult},
    common::model::{ConfigInfo, NacosJwtPayload, Page},
    service,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchPageParam {
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateFormParam {
    data_id: String,
    group: String,
    tenant: Option<String>,
    content: String,
    tag: Option<String>,
    app_name: Option<String>,
    #[serde(rename = "src_user")]
    src_user: Option<String>,
    config_tags: Option<String>,
    desc: Option<String>,
    r#use: Option<String>,
    effect: Option<String>,
    r#type: Option<String>,
    schema: Option<String>,
    encrypted_data_key: Option<String>,
}

#[get("")]
pub async fn search(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<SearchPageParam>,
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

#[post("")]
pub async fn create(
    data: web::Data<AppState>,
    req: HttpRequest,
    form: web::Form<CreateFormParam>,
) -> impl Responder {
    let token_data = req
        .extensions_mut()
        .get::<NacosJwtPayload>()
        .unwrap()
        .clone();
    let src_user = form.src_user.clone().unwrap_or(token_data.sub.clone());
    let config_type = form.r#type.clone().unwrap_or(String::from("text"));
    let src_ip = String::from(
        req.connection_info()
            .realip_remote_addr()
            .unwrap_or_default(),
    );

    let _ = service::config::create_or_update(
        &data.database_connection,
        form.data_id.as_str(),
        form.group.as_str(),
        form.tenant.clone().unwrap_or_default().as_str(),
        form.content.as_str(),
        form.tag.clone().unwrap_or_default().as_str(),
        form.app_name.clone().unwrap_or_default().as_str(),
        src_user.as_str(),
        src_ip.as_str(),
        form.config_tags.clone().unwrap_or_default().as_str(),
        form.desc.clone().unwrap_or_default().as_str(),
        form.r#use.clone().unwrap_or_default().as_str(),
        form.effect.clone().unwrap_or_default().as_str(),
        config_type.as_str(),
        form.schema.clone().unwrap_or_default().as_str(),
        form.encrypted_data_key.clone().unwrap_or_default().as_str(),
    )
    .await;

    return HttpResponse::Ok().json(true);
}

pub fn routers() -> Scope {
    web::scope("/cs/configs").service(search).service(create)
}
