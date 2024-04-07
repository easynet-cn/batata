use crate::api::model::AppState;
use crate::common::model::{NacosUser, DEFAULT_TOKEN_EXPIRE_SECONDS};
use crate::service::auth::encode_jwt_token;
use actix_web::{post, web, HttpResponse, Responder, Scope};
use serde::{Deserialize, Serialize};

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

#[post("/users/login")]
pub async fn users_login(
    data: web::Data<AppState>,
    form: web::Form<LoginFormData>,
) -> impl Responder {
    let user_option =
        crate::service::user::find_by_username(data.conns.get(0).unwrap(), &form.username).await;

    if user_option.is_none() {
        return HttpResponse::Forbidden().json("user not found!");
    }

    let secret_key_string = data
        .app_config
        .get_string("nacos.core.auth.plugin.nacos.token.secret.key")
        .unwrap();
    let secret_key = secret_key_string.as_str();

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
            secret_key,
            token_expire_seconds,
        )
        .unwrap();

        let login_result = LoginResult {
            access_token: access_token.clone(),
            token_ttl: token_expire_seconds,
            global_admin: false,
            username: user.username,
        };

        return HttpResponse::Ok()
            .append_header(("Authorization", format!("Bearer {}", access_token)))
            .json(login_result);
    }

    return HttpResponse::Forbidden().json("user not found!");
}

pub fn routes() -> Scope {
    return web::scope("/auth").service(users_login);
}
