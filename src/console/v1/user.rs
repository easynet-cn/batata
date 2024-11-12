use actix_web::{delete, get, post, put, web, HttpResponse, Responder};
use serde::Deserialize;

use crate::api::model::AppState;
use crate::common::model::{RestResult, DEFAULT_USER};
use crate::{common, service};

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

#[get("/users")]
pub async fn search_page(
    data: web::Data<AppState>,
    params: web::Query<SearchPageParam>,
) -> impl Responder {
    let search = params.search.clone().unwrap();
    let accurate = search == "accurate";
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

    return HttpResponse::Ok().json(result);
}

#[post("/users")]
pub async fn create(
    data: web::Data<AppState>,
    params: web::Form<CreateFormData>,
) -> impl Responder {
    if params.username == DEFAULT_USER {
        return HttpResponse::Conflict().json(RestResult::<String> {
            code: 409,
            message:String::from("User `nacos` is default admin user. Please use `/nacos/v1/auth/users/admin` API to init `nacos` users. Detail see `https://nacos.io/docs/latest/manual/admin/auth/#31-%E8%AE%BE%E7%BD%AE%E7%AE%A1%E7%90%86%E5%91%98%E5%AF%86%E7%A0%81`"),
            data:String::from("User `nacos` is default admin user. Please use `/nacos/v1/auth/users/admin` API to init `nacos` users. Detail see `https://nacos.io/docs/latest/manual/admin/auth/#31-%E8%AE%BE%E7%BD%AE%E7%AE%A1%E7%90%86%E5%91%98%E5%AF%86%E7%A0%81`")
        });
    }

    let user = service::user::find_by_username(&data.database_connection, &params.username).await;

    if user.is_some() {
        return HttpResponse::BadRequest()
            .json(format!("user '{}' already exist!", params.username));
    }

    let password = bcrypt::hash(params.password.clone(), 10u32).ok().unwrap();

    let result =
        service::user::create(&data.database_connection, &params.username, &password).await;

    return match result {
        Ok(()) => HttpResponse::Ok().json(RestResult::<String> {
            code: 200,
            message: String::from("create user ok!"),
            data: String::from("create user ok!"),
        }),
        Err(err) => HttpResponse::InternalServerError().json(RestResult::<String> {
            code: 500,
            message: err.to_string(),
            data: err.to_string(),
        }),
    };
}

#[put("/users")]
pub async fn update(
    data: web::Data<AppState>,
    params: web::Form<UpdateFormData>,
) -> impl Responder {
    let result = service::user::update(
        &data.database_connection,
        &params.username,
        &params.new_password,
    )
    .await;

    return match result {
        Ok(()) => HttpResponse::Ok().json(RestResult::<String> {
            code: 200,
            message: String::from("update user ok!"),
            data: String::from("update user ok!"),
        }),
        Err(err) => {
            let code = match err.downcast_ref() {
                Some(common::model::BusinessError::UserNotExist(_)) => 400,
                _ => 500,
            };

            return HttpResponse::InternalServerError().json(RestResult::<String> {
                code: code,
                message: err.to_string(),
                data: err.to_string(),
            });
        }
    };
}

#[delete("/users")]
pub async fn delete(data: web::Data<AppState>, params: web::Query<DeleteParam>) -> impl Responder {
    let global_admin = service::role::find_by_username(&data.database_connection, &params.username)
        .await
        .ok()
        .unwrap()
        .iter()
        .any(|role| role.role == common::model::GLOBAL_ADMIN_ROLE);

    if global_admin {
        return HttpResponse::BadRequest().json(RestResult::<String> {
            code: 400,
            message: format!("cannot delete admin: {}", &params.username),
            data: format!("cannot delete admin: {}", &params.username),
        });
    }

    let result = service::user::delete(&data.database_connection, &params.username).await;

    return match result {
        Ok(()) => HttpResponse::Ok().json(RestResult::<String> {
            code: 200,
            message: String::from("delete user ok!"),
            data: String::from("delete user ok!"),
        }),
        Err(err) => {
            let code = match err.downcast_ref() {
                Some(common::model::BusinessError::UserNotExist(_)) => 400,
                _ => 500,
            };

            return HttpResponse::InternalServerError().json(RestResult::<String> {
                code: code,
                message: err.to_string(),
                data: err.to_string(),
            });
        }
    };
}
