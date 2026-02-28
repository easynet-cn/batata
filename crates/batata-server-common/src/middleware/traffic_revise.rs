//! Traffic revise middleware
//!
//! Returns HTTP 503 to SDK clients while the server is still starting up or
//! has transitioned to DOWN. Cluster-internal traffic, auth endpoints, and
//! health probes are always allowed through so that operators and peer nodes
//! can still reach the server during initialization.

use std::future::{Future, Ready, ready};
use std::pin::Pin;
use std::sync::Arc;

use actix_web::{
    Error, HttpResponse,
    body::EitherBody,
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    http::StatusCode,
};

use crate::model::server_status::ServerStatusManager;

/// Middleware factory that rejects requests with 503 when the server is not UP.
pub struct TrafficReviseFilter {
    status_manager: Arc<ServerStatusManager>,
}

impl TrafficReviseFilter {
    pub fn new(status_manager: Arc<ServerStatusManager>) -> Self {
        Self { status_manager }
    }
}

impl<S, B> Transform<S, ServiceRequest> for TrafficReviseFilter
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Transform = TrafficReviseMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(TrafficReviseMiddleware {
            service,
            status_manager: self.status_manager.clone(),
        }))
    }
}

pub struct TrafficReviseMiddleware<S> {
    service: S,
    status_manager: Arc<ServerStatusManager>,
}

/// Check whether the request path should bypass the traffic filter.
fn is_bypass_path(path: &str) -> bool {
    path.contains("/auth/")
        || path.contains("/health")
        || path.contains("/liveness")
        || path.contains("/readiness")
}

/// Check whether the User-Agent indicates a Nacos cluster peer.
fn is_cluster_peer(user_agent: Option<&str>) -> bool {
    user_agent
        .map(|ua| ua.starts_with("Nacos-Server"))
        .unwrap_or(false)
}

impl<S, B> Service<ServiceRequest> for TrafficReviseMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(
        &self,
        ctx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(ctx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        // Fast path: server is UP → pass through immediately (single atomic load).
        if self.status_manager.is_up() {
            let fut = self.service.call(req);
            return Box::pin(async move { fut.await.map(|res| res.map_into_left_body()) });
        }

        // Server is not UP — check bypass rules before rejecting.
        let path = req.path().to_string();
        let user_agent = req
            .headers()
            .get("user-agent")
            .and_then(|v| v.to_str().ok())
            .map(String::from);

        if is_bypass_path(&path) || is_cluster_peer(user_agent.as_deref()) {
            let fut = self.service.call(req);
            return Box::pin(async move { fut.await.map(|res| res.map_into_left_body()) });
        }

        // Reject with 503.
        let status = self.status_manager.status();
        let status_manager = self.status_manager.clone();

        Box::pin(async move {
            let body = match status_manager.error_msg().await {
                Some(msg) => format!("server is {} now, {}", status, msg),
                None => format!("server is {} now, please try again later!", status),
            };

            let response = HttpResponse::build(StatusCode::SERVICE_UNAVAILABLE).body(body);
            Ok(req.into_response(response).map_into_right_body())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bypass_paths() {
        assert!(is_bypass_path("/nacos/v3/auth/user/login"));
        assert!(is_bypass_path("/v3/console/health"));
        assert!(is_bypass_path("/v3/admin/core/state/liveness"));
        assert!(is_bypass_path("/v3/admin/core/state/readiness"));
        assert!(!is_bypass_path("/nacos/v2/cs/config"));
        assert!(!is_bypass_path("/nacos/v2/ns/instance"));
    }

    #[test]
    fn test_cluster_peer_detection() {
        assert!(is_cluster_peer(Some("Nacos-Server")));
        assert!(is_cluster_peer(Some("Nacos-Server/2.3.0")));
        assert!(!is_cluster_peer(Some("Nacos-Client")));
        assert!(!is_cluster_peer(None));
    }
}
