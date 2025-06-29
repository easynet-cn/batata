use actix_web::{HttpResponse, Responder, Scope, post, web};
use serde::{Deserialize, Serialize};

use crate::{
    model::{
        auth::{DEFAULT_TOKEN_EXPIRE_SECONDS, GLOBAL_ADMIN_ROLE, NacosUser},
        common::AppState,
    },
    {service, service::auth::encode_jwt_token},
};

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

#[post("/user/login")]
pub async fn user_login(
    data: web::Data<AppState>,
    form: web::Form<LoginFormData>,
) -> impl Responder {
    let user_option =
        service::user::find_by_username(&data.database_connection, &form.username).await;

    if user_option.is_none() {
        return HttpResponse::Forbidden().json("user not found!");
    }

    let token_secret_key = data.token_secret_key.as_str();

    let user = user_option.unwrap();
    let bcrypt_result = bcrypt::verify(&form.password, &user.password).unwrap();

    if bcrypt_result {
        let token_expire_seconds = data
            .configuration
            .config
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
                .any(|role| role.role == GLOBAL_ADMIN_ROLE);

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

pub fn routers() -> Scope {
    return web::scope("/auth")
        .service(user_login)
        .service(super::user::search_page)
        .service(super::user::search)
        .service(super::user::update)
        .service(super::user::create)
        .service(super::user::delete)
        .service(super::role::search_page)
        .service(super::role::create)
        .service(super::role::delete)
        .service(super::role::search)
        .service(super::permission::search_page)
        .service(super::permission::create)
        .service(super::permission::delete);
}
