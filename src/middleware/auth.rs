use actix_service::forward_ready;
use actix_utils::future::{ok, Ready};
use actix_web::body::EitherBody;
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::http::Method;
use actix_web::web::Data;
use actix_web::Error;
use actix_web::HttpMessage;
use actix_web::HttpResponse;
use chrono::Utc;
use futures_core::future::LocalBoxFuture;

use crate::{api, service};

const IGNORE_ROUTES: [&str; 4] = [
    "/v1/auth/users/login",
    "/v1/console/server/state",
    "/v1/console/server/announcement",
    "/v1/console/server/guide",
];

const ACCESS_TOKEN: &str = "accessToken";

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
        let app_state = req.app_data::<Data<api::model::AppState>>().unwrap();
        let context_path = app_state.context_path.as_str();
        let mut authenticate_pass: bool = false;

        if Method::OPTIONS == *req.method() {
            authenticate_pass = true;
        } else {
            authenticate_pass = IGNORE_ROUTES.iter().any(|ignore_route| {
                let path = format!("{}{}", &context_path, ignore_route);

                req.path().starts_with(&path)
            });
        }

        if !authenticate_pass {
            if let Some(authen_header) = req.headers().get(ACCESS_TOKEN) {
                if let Ok(authen_str) = authen_header.to_str() {
                    let token = authen_str.trim();
                    let secret_key = req
                        .app_data::<Data<api::model::AppState>>()
                        .unwrap()
                        .token_secret_key
                        .clone();

                    let decode_result = service::auth::decode_jwt_token(token, &secret_key);

                    match decode_result {
                        Ok(token_data) => {
                            authenticate_pass = true;
                            req.extensions_mut().insert(token_data.claims);
                        }
                        Err(err) => {
                            let err_msg = match err.kind() {
                                jsonwebtoken::errors::ErrorKind::ExpiredSignature => {
                                    "token expired!"
                                }
                                _ => "token invalid!",
                            };
                            let (request, _pl) = req.into_parts();
                            let response = HttpResponse::Forbidden()
                                .json(api::model::ErrorResult {
                                    timestamp: Utc::now().to_rfc3339(),
                                    status: 403,
                                    message: err_msg.to_string(),
                                    error: String::from("Forbiden"),
                                    path: request.path().to_string(),
                                })
                                .map_into_right_body();

                            return Box::pin(async { Ok(ServiceResponse::new(request, response)) });
                        }
                    }
                }
            }
        }

        if !authenticate_pass {
            let (request, _pl) = req.into_parts();
            let response = HttpResponse::Forbidden()
                .json(api::model::ErrorResult {
                    timestamp: Utc::now().to_rfc3339(),
                    status: 403,
                    message: String::from("user not found!"),
                    error: String::from("Forbiden"),
                    path: request.path().to_string(),
                })
                .map_into_right_body();

            return Box::pin(async { Ok(ServiceResponse::new(request, response)) });
        }

        let res = self.service.call(req);

        Box::pin(async move { res.await.map(ServiceResponse::map_into_left_body) })
    }
}
