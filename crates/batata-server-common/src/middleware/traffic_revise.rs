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

use crate::model::server_status::{ServerStatus, ServerStatusManager};

/// Middleware factory that rejects requests with 503 when the server is not UP.
pub struct TrafficReviseFilter {
    status_manager: Arc<ServerStatusManager>,
}

impl TrafficReviseFilter {
/// Performs the `new` operation.
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

/// Middleware that adjusts request routing based on the current server status.
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

/// Check whether the User-Agent indicates a cluster peer.
/// Accepts both "Nacos-Server" (for SDK compatibility) and "Batata-Server".
fn is_cluster_peer(user_agent: Option<&str>) -> bool {
    user_agent
        .map(|ua| ua.starts_with("Nacos-Server") || ua.starts_with("Batata-Server"))
        .unwrap_or(false)
}

/// Whether a request may be served under the given lifecycle status.
///
/// Nacos parity (`TrafficReviseFilter`): `UP` serves everything; the operator modes
/// serve one half of the traffic — `READ_ONLY` passes GET, `WRITE_ONLY` passes non-GET.
/// Every other status (STARTING / DOWN / DRAINING / PAUSED) serves nothing here and
/// falls through to the bypass rules before being rejected with 503.
fn traffic_allowed(status: ServerStatus, method: &str) -> bool {
    match status {
        ServerStatus::Up => true,
        ServerStatus::ReadOnly => method.eq_ignore_ascii_case("GET"),
        ServerStatus::WriteOnly => !method.eq_ignore_ascii_case("GET"),
        ServerStatus::Starting
        | ServerStatus::Down
        | ServerStatus::Draining
        | ServerStatus::Paused => false,
    }
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
        let status = self.status_manager.status();

        // Fast path: server is UP → pass through immediately (single atomic load).
        if status == ServerStatus::Up {
            let fut = self.service.call(req);
            return Box::pin(async move { fut.await.map(|res| res.map_into_left_body()) });
        }

        // Operator modes still serve one half of the traffic (Nacos parity):
        // READ_ONLY passes GET, WRITE_ONLY passes non-GET.
        let method = req.method().to_string();
        if traffic_allowed(status, &method) {
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
    fn test_traffic_allowed_by_status() {
        // UP serves everything.
        assert!(traffic_allowed(ServerStatus::Up, "GET"));
        assert!(traffic_allowed(ServerStatus::Up, "POST"));

        // Operator modes serve one half each (Nacos TrafficReviseFilter parity).
        assert!(traffic_allowed(ServerStatus::ReadOnly, "GET"));
        assert!(!traffic_allowed(ServerStatus::ReadOnly, "POST"));
        assert!(!traffic_allowed(ServerStatus::WriteOnly, "GET"));
        assert!(traffic_allowed(ServerStatus::WriteOnly, "PUT"));

        // Lifecycle / paused states serve nothing.
        for status in [
            ServerStatus::Starting,
            ServerStatus::Down,
            ServerStatus::Draining,
            ServerStatus::Paused,
        ] {
            assert!(!traffic_allowed(status, "GET"));
            assert!(!traffic_allowed(status, "POST"));
        }
    }

    #[test]
    fn test_cluster_peer_detection() {
        // Nacos SDK compatibility
        assert!(is_cluster_peer(Some("Nacos-Server")));
        assert!(is_cluster_peer(Some("Nacos-Server/2.3.0")));
        // Batata native
        assert!(is_cluster_peer(Some("Batata-Server")));
        assert!(is_cluster_peer(Some("Batata-Server/1.0.0")));
        // Non-peers
        assert!(!is_cluster_peer(Some("Nacos-Client")));
        assert!(!is_cluster_peer(None));
    }
}
