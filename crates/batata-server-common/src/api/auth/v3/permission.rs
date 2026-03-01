use actix_web::{HttpRequest, HttpResponse, Responder, delete, get, post, web};
use serde::Deserialize;

use batata_api::model::Page;

use crate::api::auth::model::PermissionInfo;
use crate::model::app_state::AppState;
use crate::model::response::Result;
use crate::secured::Secured;
use crate::{ActionTypes, ApiType, SignType, secured};

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
            .sign_type(SignType::Specified)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    match data
        .persistence()
        .permission_find_by_id(&params.role, &params.resource, &params.action)
        .await
    {
        Ok(result) => Result::<bool>::http_success(result.is_some()),
        Err(e) => {
            tracing::error!("Failed to check permission: {}", e);
            HttpResponse::InternalServerError().json(Result::<bool> {
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
            .sign_type(SignType::Specified)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let accurate = params.search.clone().unwrap_or_default() == "accurate";
    let mut role = params.role.clone().unwrap_or_default();

    if let Some(stripped) = role.strip_prefix('*') {
        role = stripped.to_string();
    }
    if let Some(stripped) = role.strip_suffix('*') {
        role = stripped.to_string();
    }

    match data
        .persistence()
        .permission_find_page(&role, params.page_no, params.page_size, accurate)
        .await
    {
        Ok(page) => {
            // Convert persistence Page<PermissionInfo> to API Page<PermissionInfo>
            let api_page = Page {
                total_count: page.total_count,
                page_number: page.page_number,
                pages_available: page.pages_available,
                page_items: page
                    .page_items
                    .into_iter()
                    .map(|p| PermissionInfo {
                        role: p.role,
                        resource: p.resource,
                        action: p.action,
                    })
                    .collect(),
            };
            Result::<Page<PermissionInfo>>::http_success(api_page)
        }
        Err(e) => {
            tracing::error!("Failed to search permissions: {}", e);
            HttpResponse::InternalServerError().json(Result::<Page<PermissionInfo>> {
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
            .sign_type(SignType::Specified)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let result = data
        .persistence()
        .permission_grant(&params.role, &params.resource, &params.action)
        .await;

    match result {
        Ok(()) => Result::<String>::http_success("add permission ok!"),
        Err(err) => HttpResponse::InternalServerError().json(Result::<String> {
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
            .sign_type(SignType::Specified)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let result = data
        .persistence()
        .permission_revoke(&params.role, &params.resource, &params.action)
        .await;

    match result {
        Ok(()) => Result::<String>::http_success("delete permission ok!"),
        Err(err) => HttpResponse::InternalServerError().json(Result::<String> {
            code: 500,
            message: err.to_string(),
            data: err.to_string(),
        }),
    }
}
