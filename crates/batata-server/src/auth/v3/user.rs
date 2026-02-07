use actix_web::{HttpMessage, HttpRequest, HttpResponse, Responder, delete, get, post, put, web};
use serde::Deserialize;

use crate::api::model::Page;
use crate::auth::model::{GLOBAL_ADMIN_ROLE, ONLY_IDENTITY, UPDATE_PASSWORD_ENTRY_POINT, User};
use crate::error::BatataError;
use crate::{ActionTypes, ApiType, Secured, SignType, auth, model};
use crate::{
    model::common::{self, AppState},
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
struct UserParam {
    username: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateFormData {
    username: String,
    new_password: String,
}

#[get("/user/list")]
async fn search_page(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<SearchPageParam>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/users")
            .action(ActionTypes::Read)
            .sign_type(SignType::Specified)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let accurate = params.search.clone().unwrap_or_default() == "accurate";
    let mut username = params.username.clone().unwrap_or_default();

    if let Some(stripped) = username.strip_prefix("*") {
        username = stripped.to_string();
    }
    if let Some(stripped) = username.strip_suffix("*") {
        username = stripped.to_string();
    }

    let result = match auth::service::user::search_page(
        data.db(),
        &username,
        params.page_no,
        params.page_size,
        accurate,
    )
    .await
    {
        Ok(page) => page,
        Err(e) => {
            tracing::error!("Failed to search users: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "code": 500,
                "message": "Failed to search users from database",
                "data": null
            }));
        }
    };

    common::Result::<Page<User>>::http_success(result)
}

#[get("/user/search")]
async fn search(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<UserParam>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/users")
            .action(ActionTypes::Read)
            .sign_type(SignType::Specified)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let result = match auth::service::user::search(data.db(), &params.username).await {
        Ok(users) => users,
        Err(e) => {
            tracing::error!("Failed to search users: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "code": 500,
                "message": "Failed to search users from database",
                "data": null
            }));
        }
    };

    common::Result::<Vec<String>>::http_success(result)
}

#[post("/user")]
async fn create(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Form<User>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/users")
            .action(ActionTypes::Write)
            .sign_type(SignType::Specified)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    if params.username.is_empty() || params.password.is_empty() {
        return model::common::ConsoleException::handle_illegal_argument_exception(
            "username or password cann't be empty".to_string(),
        );
    }

    let user = match auth::service::user::find_by_username(data.db(), &params.username).await {
        Ok(u) => u,
        Err(e) => {
            tracing::error!(
                "Failed to check if user '{}' exists: {}",
                params.username,
                e
            );
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "code": 500,
                "message": "Failed to check user existence in database",
                "data": null
            }));
        }
    };

    if user.is_some() {
        return model::common::ConsoleException::handle_illegal_argument_exception(format!(
            "user '{}' already exist!",
            params.username
        ));
    }

    let password = match bcrypt::hash(params.password.clone(), 10u32) {
        Ok(hash) => hash,
        Err(e) => {
            tracing::error!("Failed to hash password: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "code": 500,
                "message": "Failed to hash password",
                "data": null
            }));
        }
    };

    let result = auth::service::user::create(data.db(), &params.username, &password).await;

    match result {
        Ok(()) => model::common::Result::<String>::http_success("create user ok!"),
        Err(err) => model::common::ConsoleException::handle_exception(
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
    secured!(
        Secured::builder(&req, &data, "console/user/password")
            .action(ActionTypes::Write)
            .sign_type(SignType::Specified)
            .api_type(ApiType::ConsoleApi)
            .tags(vec![
                ONLY_IDENTITY.to_string(),
                UPDATE_PASSWORD_ENTRY_POINT.to_string()
            ])
            .build()
    );

    let result =
        auth::service::user::update(data.db(), &params.username, &params.new_password).await;

    match result {
        Ok(()) => common::Result::<String>::http_success("update user ok!"),
        Err(err) => {
            let code = match err.downcast_ref() {
                Some(BatataError::UserNotExist(_)) => 400,
                _ => 500,
            };

            HttpResponse::InternalServerError().json(common::Result::<String> {
                code,
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
    params: web::Query<UserParam>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/users")
            .action(ActionTypes::Write)
            .sign_type(SignType::Specified)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let global_admin = auth::service::role::find_by_username(data.db(), &params.username)
        .await
        .unwrap_or_default()
        .iter()
        .any(|role| role.role == GLOBAL_ADMIN_ROLE);

    if global_admin {
        return HttpResponse::BadRequest().json(common::Result::<String> {
            code: 400,
            message: format!("cannot delete admin: {}", &params.username),
            data: format!("cannot delete admin: {}", &params.username),
        });
    }

    let result = auth::service::user::delete(data.db(), &params.username).await;

    match result {
        Ok(()) => common::Result::<String>::http_success("delete user ok!"),
        Err(err) => {
            let code = match err.downcast_ref() {
                Some(BatataError::UserNotExist(_)) => 400,
                _ => 500,
            };

            HttpResponse::InternalServerError().json(common::Result::<String> {
                code,
                message: err.to_string(),
                data: err.to_string(),
            })
        }
    }
}
