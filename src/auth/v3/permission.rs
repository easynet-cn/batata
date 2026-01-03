use actix_web::{HttpMessage, HttpRequest, HttpResponse, Responder, delete, get, post, web};
use serde::Deserialize;

use crate::{
    ActionTypes, Secured,
    api::model::Page,
    auth::{self, model::PermissionInfo},
    model::common::{self, AppState},
    secured,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchPageParam {
    search: Option<String>,
    role: Option<String>,
    page_no: u64,
    page_size: u64,
}

#[get("/permission")]
async fn exist(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<PermissionInfo>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/permissions")
            .action(ActionTypes::Read)
            .build()
    );

    match auth::service::permission::find_by_id(
        &data.database_connection,
        &params.role,
        &params.resource,
        &params.action,
    )
    .await
    {
        Ok(result) => common::Result::<bool>::http_success(result.is_some()),
        Err(e) => {
            tracing::error!("Failed to check permission: {}", e);
            HttpResponse::InternalServerError().json(common::Result::<bool> {
                code: 500,
                message: e.to_string(),
                data: false,
            })
        }
    }
}

#[get("/permission/list")]
async fn search_page(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<SearchPageParam>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/permissions")
            .action(ActionTypes::Read)
            .build()
    );

    let accurate = params.search.clone().unwrap_or_default() == "accurate";
    let mut role = params.role.clone().unwrap_or_default();

    if role.starts_with("*") {
        role = role.strip_prefix("*").unwrap().to_string();
    }
    if role.ends_with("*") {
        role = role.strip_suffix("*").unwrap().to_string();
    }

    match auth::service::permission::search_page(
        &data.database_connection,
        &role,
        params.page_no,
        params.page_size,
        accurate,
    )
    .await
    {
        Ok(result) => common::Result::<Page<PermissionInfo>>::http_success(result),
        Err(e) => {
            tracing::error!("Failed to search permissions: {}", e);
            HttpResponse::InternalServerError().json(common::Result::<Page<PermissionInfo>> {
                code: 500,
                message: e.to_string(),
                data: Page::default(),
            })
        }
    }
}

#[post("/permission")]
async fn create(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Form<PermissionInfo>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/permissions")
            .action(ActionTypes::Write)
            .build()
    );

    let result = auth::service::permission::create(
        &data.database_connection,
        &params.role,
        &params.resource,
        &params.action,
    )
    .await;

    match result {
        Ok(()) => common::Result::<String>::http_success("add permission ok!"),
        Err(err) => HttpResponse::InternalServerError().json(common::Result::<String> {
            code: 500,
            message: err.to_string(),
            data: err.to_string(),
        }),
    }
}

#[delete("/permission")]
async fn delete(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<PermissionInfo>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/permissions")
            .action(ActionTypes::Write)
            .build()
    );

    let result = auth::service::permission::delete(
        &data.database_connection,
        &params.role,
        &params.resource,
        &params.action,
    )
    .await;

    match result {
        Ok(()) => common::Result::<String>::http_success("delete permission ok!"),
        Err(err) => HttpResponse::InternalServerError().json(common::Result::<String> {
            code: 500,
            message: err.to_string(),
            data: err.to_string(),
        }),
    }
}
