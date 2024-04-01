use crate::api::model::AppState;
use actix_web::{post, web, HttpResponse, Responder};
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
    let user = crate::service::user::find_by_username(data.conns.get(0).unwrap(), &form.username)
        .await
        .unwrap();

    let bcrypt_result = bcrypt::verify(&form.password, &user.password).unwrap();

    if bcrypt_result {
        let login_result = LoginResult {
            access_token: "access_token".to_string(),
            token_ttl: 3600,
            global_admin: false,
            username: user.username,
        };

        return HttpResponse::Ok().json(login_result);
    }

    return HttpResponse::Forbidden().json("user not found!");
}
