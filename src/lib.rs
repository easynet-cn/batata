pub mod console;
pub mod entity;
pub mod middleware;
pub mod model;
pub mod service;

#[macro_export]
macro_rules! secured {
    ($req: expr,$data: expr) => {
        if let Some(auth_context) = $req.extensions().get::<model::auth::AuthContext>().cloned() {
            if auth_context.jwt_error.is_some() {
                return actix_web::HttpResponse::Forbidden().json(model::common::ErrorResult {
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    status: actix_web::http::StatusCode::FORBIDDEN.as_u16() as i32,
                    message: format!(
                        "Code: {}, Message: {}",
                        401,
                        auth_context.jwt_error.unwrap().to_string()
                    ),
                    error: actix_web::http::StatusCode::FORBIDDEN
                        .canonical_reason()
                        .unwrap_or_default()
                        .to_string(),
                    path: $req.path().to_string(),
                });
            } else {
                let global_admin = service::role::has_global_admin_role_by_username(
                    &$data.database_connection,
                    &auth_context.username,
                )
                .await
                .ok()
                .unwrap_or_default();

                if !global_admin {
                    return actix_web::HttpResponse::Forbidden().json(model::common::ErrorResult {
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        status: actix_web::http::StatusCode::FORBIDDEN.as_u16() as i32,
                        message: format!(
                            "Code: {}, Message: {}",
                            actix_web::http::StatusCode::FORBIDDEN.as_u16(),
                            "authorization failed!."
                        ),
                        error: actix_web::http::StatusCode::FORBIDDEN
                            .canonical_reason()
                            .unwrap_or_default()
                            .to_string(),
                        path: $req.path().to_string(),
                    });
                }
            }
        } else {
            return actix_web::HttpResponse::Forbidden().json(model::common::ErrorResult {
                timestamp: chrono::Utc::now().to_rfc3339(),
                status: actix_web::http::StatusCode::FORBIDDEN.as_u16() as i32,
                message: format!(
                    "Code: {}, Message: {}",
                    401,
                    model::auth::USER_NOT_FOUND_MESSAGE
                ),
                error: actix_web::http::StatusCode::FORBIDDEN
                    .canonical_reason()
                    .unwrap_or_default()
                    .to_string(),
                path: $req.path().to_string(),
            });
        }
    };
}
