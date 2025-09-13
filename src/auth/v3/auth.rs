use actix_web::{HttpResponse, Responder, post, web};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{
        self,
        model::{AUTHORIZATION_HEADER, TOKEN_PREFIX, USER_NOT_FOUND_MESSAGE},
        service::auth::encode_jwt_token,
    },
    model::common::AppState,
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
struct LoginData {
    username: Option<String>,
    password: Option<String>,
}

#[post("user/login")]
async fn login(
    data: web::Data<AppState>,
    form: Option<web::Form<LoginData>>,
    query: Option<web::Query<LoginData>>,
) -> impl Responder {
    let mut username: String = "".to_string();
    let mut password: String = "".to_string();

    if let Some(form_data) = form {
        if let Some(v) = &form_data.username {
            username = v.to_string();
        }
        if let Some(v) = &form_data.password {
            password = v.to_string();
        }
    } else if let Some(query_data) = query {
        if let Some(v) = &query_data.username {
            username = v.to_string();
        }
        if let Some(v) = &query_data.password {
            password = v.to_string();
        }
    }

    if username.is_empty() || password.is_empty() {
        return HttpResponse::Forbidden().body(USER_NOT_FOUND_MESSAGE);
    }

    let user_option = auth::service::user::find_by_username(&data.database_connection, &username)
        .await
        .unwrap();

    if user_option.is_none() {
        return HttpResponse::Forbidden().body(USER_NOT_FOUND_MESSAGE);
    }

    let token_secret_key = data.configuration.token_secret_key();

    let user = user_option.unwrap();
    let bcrypt_result = bcrypt::verify(password, &user.password).unwrap();

    if bcrypt_result {
        let token_expire_seconds = data.configuration.auth_token_expire_seconds();

        let access_token =
            encode_jwt_token(&username, token_secret_key.as_str(), token_expire_seconds).unwrap();

        let global_admin = auth::service::role::has_global_admin_role_by_username(
            &data.database_connection,
            &user.username,
        )
        .await
        .ok()
        .unwrap_or_default();

        let login_result = LoginResult {
            access_token: access_token.clone(),
            token_ttl: token_expire_seconds,
            global_admin: global_admin,
            username: user.username,
        };

        return HttpResponse::Ok()
            .append_header((
                AUTHORIZATION_HEADER,
                format!("{}{}", TOKEN_PREFIX, access_token),
            ))
            .json(login_result);
    }

    return HttpResponse::Forbidden().body("USER_NOT_FOUND_MESSAGE");
}
