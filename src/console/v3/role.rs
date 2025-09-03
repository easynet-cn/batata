use actix_web::{HttpMessage, HttpRequest, HttpResponse, Responder, delete, get, post, web};
use serde::Deserialize;

use crate::{
    error::BatataError,
    model::{
        auth::RoleInfo,
        common::{self, AppState, Page},
    },
    secured, service,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchPageParam {
    search: Option<String>,
    username: Option<String>,
    role: Option<String>,
    page_no: u64,
    page_size: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchParam {
    role: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateFormData {
    role: String,
    username: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteParam {
    role: String,
    username: Option<String>,
}

#[get("/role/list")]
async fn search_page(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<SearchPageParam>,
) -> impl Responder {
    secured!(req, data);

    let accurate = params.search.clone().unwrap_or_default() == "accurate";
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

    common::Result::<Page<RoleInfo>>::http_success(result)
}

#[get("/role/search")]
async fn search(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<SearchParam>,
) -> impl Responder {
    secured!(req, data);

    let result = service::role::search(&data.database_connection, &params.role)
        .await
        .unwrap();

    common::Result::<Vec<String>>::http_success(result)
}

#[post("role")]
async fn create(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Form<CreateFormData>,
) -> impl Responder {
    secured!(req, data);

    let result =
        service::role::create(&data.database_connection, &params.role, &params.username).await;

    match result {
        Ok(()) => common::Result::<String>::http_success("add role ok!"),
        Err(err) => {
            if let Some(e) = err.downcast_ref::<BatataError>()
                && let BatataError::IllegalArgument(msg) = e
            {
                return HttpResponse::BadRequest().body(msg.to_string());
            }

            HttpResponse::InternalServerError().json(common::Result::<String> {
                code: 500,
                message: err.to_string(),
                data: err.to_string(),
            })
        }
    }
}

#[delete("role")]
pub async fn delete(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<DeleteParam>,
) -> impl Responder {
    secured!(req, data);

    let result = service::role::delete(
        &data.database_connection,
        &params.role,
        &params.username.clone().unwrap_or_default(),
    )
    .await;

    match result {
        Ok(()) => common::Result::<String>::http_success(format!(
            "delete role of user {} ok!",
            params.username.clone().unwrap_or_default()
        )),
        Err(err) => {
            if let Some(e) = err.downcast_ref::<BatataError>()
                && let BatataError::IllegalArgument(msg) = e
            {
                return HttpResponse::BadRequest().body(msg.to_string());
            }

            HttpResponse::InternalServerError().json(common::Result::<String> {
                code: 500,
                message: err.to_string(),
                data: err.to_string(),
            })
        }
    }
}
