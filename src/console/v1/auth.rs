use actix_web::{delete, get, post, put, web, HttpRequest, HttpResponse, Responder, Scope};
use serde::{Deserialize, Serialize};

use crate::api::model::AppState;
use crate::common::model::{
    self, NacosUser, Page, RestResult, User, DEFAULT_TOKEN_EXPIRE_SECONDS, DEFAULT_USER,
};
use crate::service::auth::encode_jwt_token;
use crate::{api, common, service};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct LoginResult {
    access_token: String,
    token_ttl: i64,
    global_admin: bool,
    username: String,
}

#[derive(Deserialize)]
struct LoginFormData {
    username: String,
    password: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchParam {
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

#[post("/users/login")]
pub async fn users_login(
    data: web::Data<AppState>,
    form: web::Form<LoginFormData>,
) -> impl Responder {
    let user_option =
        crate::service::user::find_by_username(&data.database_connection, &form.username).await;

    if user_option.is_none() {
        return HttpResponse::Forbidden().json("user not found!");
    }

    let token_secret_key = data.token_secret_key.as_str();

    let user = user_option.unwrap();
    let bcrypt_result = bcrypt::verify(&form.password, &user.password).unwrap();

    if bcrypt_result {
        let token_expire_seconds = data
            .app_config
            .get_int("nacos.core.auth.plugin.nacos.token.expire.seconds")
            .unwrap_or(DEFAULT_TOKEN_EXPIRE_SECONDS);

        let access_token = encode_jwt_token(
            &NacosUser {
                username: user.username.clone(),
                password: user.password.clone(),
                token: "".to_string(),
                global_admin: false,
            },
            token_secret_key,
            token_expire_seconds,
        )
        .unwrap();

        let global_admin =
            service::role::find_by_username(&data.database_connection, &user.username)
                .await
                .ok()
                .unwrap()
                .iter()
                .any(|role| role.role == common::model::GLOBAL_ADMIN_ROLE);

        let login_result = LoginResult {
            access_token: access_token.clone(),
            token_ttl: token_expire_seconds,
            global_admin: global_admin,
            username: user.username,
        };

        return HttpResponse::Ok()
            .append_header(("Authorization", format!("Bearer {}", access_token)))
            .json(login_result);
    }

    return HttpResponse::Forbidden().json("user not found!");
}

#[get("/users")]
pub async fn search(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<SearchParam>,
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
pub async fn create_user(
    req: HttpRequest,
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
pub async fn update_user(
    req: HttpRequest,
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
pub fn routers() -> Scope {
    return web::scope("/auth")
        .service(users_login)
        .service(search)
        .service(update_user)
        .service(create_user);
}
