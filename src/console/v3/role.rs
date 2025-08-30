use actix_web::{HttpResponse, Responder, delete, get, post, web};
use serde::Deserialize;

use crate::{
    model::{common, common::AppState},
    service,
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
    data: web::Data<AppState>,
    params: web::Query<SearchPageParam>,
) -> impl Responder {
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

    return HttpResponse::Ok().json(result);
}

#[get("/role/search")]
async fn search(data: web::Data<AppState>, params: web::Query<SearchParam>) -> impl Responder {
    let result = service::role::search(&data.database_connection, &params.role)
        .await
        .unwrap();

    return HttpResponse::Ok().json(result);
}

#[post("role")]
async fn create(data: web::Data<AppState>, params: web::Form<CreateFormData>) -> impl Responder {
    let result =
        service::role::create(&data.database_connection, &params.role, &params.username).await;

    return match result {
        Ok(()) => HttpResponse::Ok().json(common::Result::<String> {
            code: 200,
            message: String::from("add role ok!"),
            data: String::from("add role ok!"),
        }),
        Err(err) => HttpResponse::InternalServerError().json(common::Result::<String> {
            code: 500,
            message: err.to_string(),
            data: err.to_string(),
        }),
    };
}

#[delete("role")]
pub async fn delete(data: web::Data<AppState>, params: web::Query<DeleteParam>) -> impl Responder {
    let result = service::role::delete(
        &data.database_connection,
        &params.role,
        &params.username.clone().unwrap_or_default(),
    )
    .await;

    return match result {
        Ok(()) => HttpResponse::Ok().json(common::Result::<String> {
            code: 200,
            message: format!(
                "delete role of user {} ok!",
                params.username.clone().unwrap_or_default()
            ),
            data: format!(
                "delete role of user {} ok!",
                params.username.clone().unwrap_or_default()
            ),
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
