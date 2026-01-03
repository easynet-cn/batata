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
        let mut authenticate_pass: bool = false;

        if Method::OPTIONS == *req.method() {
            authenticate_pass = true;
        }

        if !authenticate_pass && let Some(authen_header) = req.headers().get(ACCESS_TOKEN) {
            let mut auth_context = AuthContext::default();

            if let Ok(authen_str) = authen_header.to_str() {
                let token = authen_str.trim();

                if let Some(app_state) = req.app_data::<Data<AppState>>() {
                    let secret_key = app_state.configuration.token_secret_key().clone();
                    // Use cached token validation for better performance
                    let decode_result = auth::service::auth::decode_jwt_token_cached(token, &secret_key);

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

            req.extensions_mut().insert(auth_context);
        }

        let res = self.service.call(req);

        Box::pin(async move { res.await.map(ServiceResponse::map_into_left_body) })
    }
}
