use actix_web::{HttpMessage, HttpRequest, HttpResponse, Responder, delete, get, post, web};
use serde::Deserialize;

use crate::{
    ActionTypes, ApiType, Secured, SignType,
    api::model::Page,
    auth::model::RoleInfo,
    error::BatataError,
    model::common::{self, AppState},
    secured,
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
    secured!(
        Secured::builder(&req, &data, "console/roles")
            .action(ActionTypes::Read)
            .sign_type(SignType::Specified)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let accurate = params.search.clone().unwrap_or_default() == "accurate";
    let mut username = params.username.clone().unwrap_or_default();

    if let Some(stripped) = username.strip_prefix('*') {
        username = stripped.to_string();
    }
    if let Some(stripped) = username.strip_suffix('*') {
        username = stripped.to_string();
    }

    let mut role = params.role.clone().unwrap_or_default();

    if let Some(stripped) = role.strip_prefix('*') {
        role = stripped.to_string();
    }
    if let Some(stripped) = role.strip_suffix('*') {
        role = stripped.to_string();
    }

    let result = data
        .persistence()
        .role_find_page(&username, &role, params.page_no, params.page_size, accurate)
        .await;

    match result {
        Ok(page) => {
            // Convert persistence Page<RoleInfo> to API Page<RoleInfo>
            let api_page = Page {
                total_count: page.total_count,
                page_number: page.page_number,
                pages_available: page.pages_available,
                page_items: page
                    .page_items
                    .into_iter()
                    .map(|r| RoleInfo {
                        role: r.role,
                        username: r.username,
                    })
                    .collect(),
            };
            common::Result::<Page<RoleInfo>>::http_success(api_page)
        }
        Err(e) => HttpResponse::InternalServerError().json(common::Result::<Page<RoleInfo>> {
            code: 500,
            message: e.to_string(),
            data: Page::default(),
        }),
    }
}

#[get("/role/search")]
async fn search(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<SearchParam>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/roles")
            .action(ActionTypes::Read)
            .sign_type(SignType::Specified)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let result = data.persistence().role_search(&params.role).await;

    match result {
        Ok(roles) => common::Result::<Vec<String>>::http_success(roles),
        Err(e) => HttpResponse::InternalServerError().json(common::Result::<Vec<String>> {
            code: 500,
            message: e.to_string(),
            data: Vec::new(),
        }),
    }
}

#[post("role")]
async fn create(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Form<CreateFormData>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/roles")
            .action(ActionTypes::Write)
            .sign_type(SignType::Specified)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let result = data
        .persistence()
        .role_create(&params.role, &params.username)
        .await;

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
    secured!(
        Secured::builder(&req, &data, "console/roles")
            .action(ActionTypes::Write)
            .sign_type(SignType::Specified)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let result = data
        .persistence()
        .role_delete(&params.role, &params.username.clone().unwrap_or_default())
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
