use actix_web::{HttpMessage, HttpRequest, HttpResponse, Responder, delete, get, post, put, web};
use serde::Deserialize;

use crate::model::auth::User;
use crate::model::common::Page;
use crate::{model, service};
use crate::{
    model::{
        auth::GLOBAL_ADMIN_ROLE,
        common::{self, AppState, BusinessError},
    },
    secured,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchPageParam {
    search: Option<String>,
    username: Option<String>,
    page_no: u64,
    page_size: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchParam {
    username: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateFormData {
    username: String,
    password: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateFormData {
    username: String,
    new_password: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteParam {
    username: String,
}

#[get("/user/list")]
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

    let result = service::user::search_page(
        &data.database_connection,
        &username,
        params.page_no,
        params.page_size,
        accurate,
    )
    .await
    .unwrap();

    common::Result::<Page<User>>::http_success(result)
}

#[get("/user/search")]
async fn search(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<SearchParam>,
) -> impl Responder {
    secured!(req, data);

    let result = service::user::search(&data.database_connection, &params.username)
        .await
        .unwrap();

    common::Result::<Vec<String>>::http_success(result)
}

#[post("/user")]
async fn create(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Form<CreateFormData>,
) -> impl Responder {
    secured!(req, data);

    if params.username.is_empty() || params.password.is_empty() {
        return model::common::ConsoleExecption::handle_illegal_argument_exectpion(
            "username or password cann't be empty".to_string(),
        );
    }

    let user = service::user::find_by_username(&data.database_connection, &params.username).await;

    if user.is_some() {
        return model::common::ConsoleExecption::handle_illegal_argument_exectpion(format!(
            "user '{}' already exist!",
            params.username
        ));
    }

    let password = bcrypt::hash(params.password.clone(), 10u32).ok().unwrap();

    let result =
        service::user::create(&data.database_connection, &params.username, &password).await;

    match result {
        Ok(()) => model::common::Result::<String>::http_success("create user ok!"),
        Err(err) => model::common::ConsoleExecption::handle_exectpion(
            req.uri().path().to_string(),
            err.to_string(),
        ),
    }
}

#[put("/user")]
async fn update(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Form<UpdateFormData>,
) -> impl Responder {
    secured!(req, data);

    let result = service::user::update(
        &data.database_connection,
        &params.username,
        &params.new_password,
    )
    .await;

    match result {
        Ok(()) => common::Result::<String>::http_success("update user ok!"),
        Err(err) => {
            let code = match err.downcast_ref() {
                Some(BusinessError::UserNotExist(_)) => 400,
                _ => 500,
            };

            HttpResponse::InternalServerError().json(common::Result::<String> {
                code: code,
                message: err.to_string(),
                data: err.to_string(),
            })
        }
    }
}

#[delete("/user")]
async fn delete(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<DeleteParam>,
) -> impl Responder {
    secured!(req, data);

    let global_admin = service::role::find_by_username(&data.database_connection, &params.username)
        .await
        .ok()
        .unwrap()
        .iter()
        .any(|role| role.role == GLOBAL_ADMIN_ROLE);

    if global_admin {
        return HttpResponse::BadRequest().json(common::Result::<String> {
            code: 400,
            message: format!("cannot delete admin: {}", &params.username),
            data: format!("cannot delete admin: {}", &params.username),
        });
    }

    let result = service::user::delete(&data.database_connection, &params.username).await;

    match result {
        Ok(()) => common::Result::<String>::http_success("delete user ok!"),
        Err(err) => {
            let code = match err.downcast_ref() {
                Some(BusinessError::UserNotExist(_)) => 400,
                _ => 500,
            };

            HttpResponse::InternalServerError().json(common::Result::<String> {
                code: code,
                message: err.to_string(),
                data: err.to_string(),
            })
        }
    }
}
