use actix_web::{HttpMessage, HttpRequest, HttpResponse, Responder, delete, get, post, web};
use serde::Deserialize;

use crate::{
    model::{
        auth::PermissionInfo,
        common::{self, AppState, Page},
    },
    secured, service,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchPageParam {
    search: Option<String>,
    role: Option<String>,
    page_no: u64,
    page_size: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateFormData {
    role: String,
    resource: String,
    action: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteParam {
    role: String,
    resource: String,
    action: String,
}

#[get("/permission")]
async fn exist(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<CreateFormData>,
) -> impl Responder {
    secured!(req, data);

    let exist = service::permission::find_by_id(
        &data.database_connection,
        &params.role,
        &params.resource,
        &params.action,
    )
    .await
    .unwrap()
    .is_some();

    common::Result::<bool>::http_success(exist)
}

#[get("/permission/list")]
async fn search_page(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<SearchPageParam>,
) -> impl Responder {
    secured!(req, data);

    let accurate = params.search.clone().unwrap_or_default() == "accurate";
    let mut role = params.role.clone().unwrap_or_default();

    if role.starts_with("*") {
        role = role.strip_prefix("*").unwrap().to_string();
    }
    if role.ends_with("*") {
        role = role.strip_suffix("*").unwrap().to_string();
    }

    let result = service::permission::search_page(
        &data.database_connection,
        &role,
        params.page_no,
        params.page_size,
        accurate,
    )
    .await
    .unwrap();

    common::Result::<Page<PermissionInfo>>::http_success(result)
}

#[post("/permission")]
async fn create(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Form<CreateFormData>,
) -> impl Responder {
    secured!(req, data);

    let result = service::permission::create(
        &data.database_connection,
        &params.role,
        &params.resource,
        &params.action,
    )
    .await;

    return match result {
        Ok(()) => HttpResponse::Ok().json(common::Result::<String> {
            code: 200,
            message: String::from("add permission ok!"),
            data: String::from("add permission ok!"),
        }),
        Err(err) => HttpResponse::InternalServerError().json(common::Result::<String> {
            code: 500,
            message: err.to_string(),
            data: err.to_string(),
        }),
    };
}

#[delete("/permission")]
async fn delete(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<DeleteParam>,
) -> impl Responder {
    secured!(req, data);

    let result = service::permission::delete(
        &data.database_connection,
        &params.role,
        &params.resource,
        &params.action,
    )
    .await;

    return match result {
        Ok(()) => HttpResponse::Ok().json(common::Result::<String> {
            code: 200,
            message: String::from("delete permission ok!"),
            data: String::from("delete permission ok!"),
        }),
        Err(err) => {
            return HttpResponse::InternalServerError().json(common::Result::<String> {
                code: 500,
                message: err.to_string(),
                data: err.to_string(),
            });
        }
    };
}
