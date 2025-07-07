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
                return model::common::ErrorResult::http_response_forbidden(
                    actix_web::http::StatusCode::UNAUTHORIZED.as_u16() as i32,
                    auth_context.jwt_error.unwrap().to_string().as_str(),
                    $req.path(),
                );
            } else {
                let global_admin = service::role::has_global_admin_role_by_username(
                    &$data.database_connection,
                    &auth_context.username,
                )
                .await
                .ok()
                .unwrap_or_default();

                if !global_admin {
                    return model::common::ErrorResult::http_response_forbidden(
                        actix_web::http::StatusCode::FORBIDDEN.as_u16() as i32,
                        "authorization failed!.",
                        $req.path(),
                    );
                }
            }
        } else {
            return model::common::ErrorResult::http_response_forbidden(
                actix_web::http::StatusCode::UNAUTHORIZED.as_u16() as i32,
                model::auth::USER_NOT_FOUND_MESSAGE,
                $req.path(),
            );
        }
    };
}
