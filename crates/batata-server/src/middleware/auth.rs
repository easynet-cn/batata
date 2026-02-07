// Authentication middleware for Actix-web
// This middleware handles JWT token validation and authentication context setup

use actix_service::forward_ready;
use actix_utils::future::{Ready, ok};
use actix_web::{
    Error, HttpMessage,
    body::EitherBody,
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    http::Method,
    web::Data,
};

use futures::future::LocalBoxFuture;

use crate::{
    auth::{self, model::AuthContext},
    model::common::AppState,
};

const ACCESS_TOKEN: &str = "accessToken";
const AUTHORIZATION_HEADER: &str = "Authorization";
const BEARER_PREFIX: &str = "Bearer ";

// Authentication middleware transformer
pub struct Authentication;

impl<S, B> Transform<S, ServiceRequest> for Authentication
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = AuthenticationMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(AuthenticationMiddleware { service })
    }
}

pub struct AuthenticationMiddleware<S> {
    service: S,
}

/// Extract token from request using 3 sources in priority order:
/// 1. `accessToken` HTTP header
/// 2. `Authorization: Bearer <token>` header
/// 3. `accessToken` query parameter
fn extract_token(req: &ServiceRequest) -> Option<String> {
    // 1. accessToken header
    if let Some(header_val) = req.headers().get(ACCESS_TOKEN)
        && let Ok(s) = header_val.to_str()
    {
        let trimmed = s.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    // 2. Authorization: Bearer <token> header
    if let Some(header_val) = req.headers().get(AUTHORIZATION_HEADER)
        && let Ok(s) = header_val.to_str()
    {
        let trimmed = s.trim();
        if let Some(token) = trimmed.strip_prefix(BEARER_PREFIX) {
            let token = token.trim();
            if !token.is_empty() {
                return Some(token.to_string());
            }
        }
    }

    // 3. accessToken query parameter
    if let Some(query) = req.uri().query() {
        for pair in query.split('&') {
            if let Some((key, value)) = pair.split_once('=')
                && key == ACCESS_TOKEN
                && !value.is_empty()
            {
                return Some(value.to_string());
            }
        }
    }

    None
}

impl<S, B> Service<ServiceRequest> for AuthenticationMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        if Method::OPTIONS != *req.method() {
            let mut auth_context = AuthContext::default();

            if let Some(token) = extract_token(&req) {
                auth_context.token_provided = true;

                if let Some(app_state) = req.app_data::<Data<AppState>>() {
                    let secret_key = app_state.configuration.token_secret_key();
                    let decode_result =
                        auth::service::auth::decode_jwt_token_cached(&token, &secret_key);

                    match decode_result {
                        Ok(token_data) => {
                            auth_context.username = token_data.claims.sub;
                        }
                        Err(err) => {
                            auth_context.jwt_error = Some(err);
                        }
                    }
                } else {
                    tracing::error!("AppState not found in request app_data");
                }
            }

            // Always insert AuthContext so the secured! macro can inspect it
            req.extensions_mut().insert(auth_context);
        }

        let res = self.service.call(req);

        Box::pin(async move { res.await.map(ServiceResponse::map_into_left_body) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_token_constants() {
        assert_eq!(ACCESS_TOKEN, "accessToken");
        assert_eq!(AUTHORIZATION_HEADER, "Authorization");
        assert_eq!(BEARER_PREFIX, "Bearer ");
    }
}
