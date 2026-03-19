//! TPS (Transactions Per Second) control middleware for HTTP endpoints.
//!
//! Provides per-endpoint rate limiting using the control plugin.
//! Maps HTTP method + path to TPS control point names following Nacos conventions.

use std::future::{Ready, ready};
use std::sync::Arc;

use actix_service::{Service, Transform};
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::{Error, HttpResponse};
use futures::future::LocalBoxFuture;

use batata_plugin::{ControlContext, ControlPlugin, ExceedAction};

/// TPS control point names following Nacos conventions.
fn resolve_tps_point(method: &str, path: &str) -> Option<&'static str> {
    // Config listener (check before general config to match more specific path first)
    if path.contains("/cs/config/listener") {
        return Some("HttpConfigListen");
    }

    // Config endpoints
    if path.contains("/cs/config") {
        return match method {
            "GET" => Some("HttpConfigQuery"),
            "POST" => Some("HttpConfigPublish"),
            "DELETE" => Some("HttpConfigRemove"),
            _ => None,
        };
    }

    // Naming instance batch metadata (check before general instance)
    if path.contains("/ns/instance/metadata/batch") {
        return Some("HttpNamingInstanceMetadataBatchUpdate");
    }

    // Naming instance endpoints
    if path.contains("/ns/instance") {
        return match method {
            "POST" => Some("HttpNamingInstanceRegister"),
            "DELETE" => Some("HttpNamingInstanceDeregister"),
            "PUT" => Some("HttpNamingInstanceUpdate"),
            "GET" => Some("HttpNamingInstanceQuery"),
            _ => None,
        };
    }

    // Naming service list (check before general service)
    if path.contains("/ns/service/list") {
        return Some("HttpNamingServiceListQuery");
    }
    if path.contains("/ns/service/subscribers") {
        return Some("HttpNamingServiceSubscribe");
    }

    // Naming service endpoints
    if path.contains("/ns/service") {
        return match method {
            "POST" => Some("HttpNamingServiceRegister"),
            "DELETE" => Some("HttpNamingServiceDeregister"),
            "PUT" => Some("HttpNamingServiceUpdate"),
            "GET" => Some("HttpNamingServiceQuery"),
            _ => None,
        };
    }

    // Health endpoints
    if path.contains("/ns/health") {
        return Some("HttpHealthCheck");
    }

    None
}

/// Actix middleware that enforces per-endpoint TPS control.
///
/// If no control plugin is provided, requests pass through without TPS checking.
pub struct TpsControlMiddleware {
    control_plugin: Option<Arc<dyn ControlPlugin>>,
}

impl TpsControlMiddleware {
    pub fn new(control_plugin: Option<Arc<dyn ControlPlugin>>) -> Self {
        Self { control_plugin }
    }
}

impl<S, B> Transform<S, ServiceRequest> for TpsControlMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<actix_web::body::EitherBody<B>>;
    type Error = Error;
    type Transform = TpsControlService<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(TpsControlService {
            service,
            control_plugin: self.control_plugin.clone(),
        }))
    }
}

pub struct TpsControlService<S> {
    service: S,
    control_plugin: Option<Arc<dyn ControlPlugin>>,
}

impl<S, B> Service<ServiceRequest> for TpsControlService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<actix_web::body::EitherBody<B>>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        // Fast path: no control plugin configured
        let Some(control_plugin) = self.control_plugin.clone() else {
            let fut = self.service.call(req);
            return Box::pin(async move {
                let res = fut.await?;
                Ok(res.map_into_left_body())
            });
        };

        let method = req.method().as_str().to_string();
        let path = req.path().to_string();
        let client_ip = req
            .connection_info()
            .peer_addr()
            .map(|s| s.to_string())
            .unwrap_or_default();

        let fut = self.service.call(req);

        Box::pin(async move {
            // Resolve TPS control point from HTTP method + path
            if let Some(point_name) = resolve_tps_point(&method, &path) {
                let ctx = ControlContext::new().with_ip(&client_ip).with_path(&path);

                let result = control_plugin.check_rate_limit(&ctx).await;

                if !result.allowed {
                    tracing::warn!(
                        point = point_name,
                        client_ip = %client_ip,
                        path = %path,
                        "TPS control: request rejected (limit={}, remaining={})",
                        result.limit,
                        result.remaining
                    );

                    match result.action {
                        ExceedAction::Warn => {
                            // Log warning but allow through
                        }
                        _ => {
                            // Reject with 503 (matching Nacos behavior)
                            let response = HttpResponse::ServiceUnavailable()
                                .insert_header(("Retry-After", "1"))
                                .json(serde_json::json!({
                                    "code": 503,
                                    "message": format!("Over threshold: {}", point_name),
                                    "data": serde_json::Value::Null
                                }));
                            let res = fut.await?;
                            return Ok(res.into_response(response).map_into_right_body());
                        }
                    }
                }
            }

            let res = fut.await?;
            Ok(res.map_into_left_body())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_tps_point_config() {
        assert_eq!(
            resolve_tps_point("GET", "/nacos/v2/cs/config"),
            Some("HttpConfigQuery")
        );
        assert_eq!(
            resolve_tps_point("POST", "/nacos/v2/cs/config"),
            Some("HttpConfigPublish")
        );
        assert_eq!(
            resolve_tps_point("DELETE", "/nacos/v2/cs/config"),
            Some("HttpConfigRemove")
        );
    }

    #[test]
    fn test_resolve_tps_point_naming() {
        assert_eq!(
            resolve_tps_point("POST", "/nacos/v2/ns/instance"),
            Some("HttpNamingInstanceRegister")
        );
        assert_eq!(
            resolve_tps_point("DELETE", "/nacos/v2/ns/instance"),
            Some("HttpNamingInstanceDeregister")
        );
        assert_eq!(
            resolve_tps_point("GET", "/nacos/v2/ns/service/list"),
            Some("HttpNamingServiceListQuery")
        );
    }

    #[test]
    fn test_resolve_tps_point_listener() {
        assert_eq!(
            resolve_tps_point("POST", "/nacos/v2/cs/config/listener"),
            Some("HttpConfigListen")
        );
    }

    #[test]
    fn test_resolve_tps_point_none() {
        assert_eq!(resolve_tps_point("GET", "/nacos/v3/auth/user/login"), None);
        assert_eq!(resolve_tps_point("GET", "/health/liveness"), None);
    }
}
