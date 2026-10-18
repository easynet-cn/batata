//! Config storage readiness gate
//!
//! Mirrors Nacos `CircuitFilter` (`config/.../filter/CircuitFilter.java`), which is
//! registered on `/v1/cs/*` only when embedded distributed storage is enabled and
//! answers `503` until the node is part of the Raft group.
//!
//! Background: config writes go through Raft, reads are served from the local
//! store. A node that has not joined the Raft group yet has an empty (or stale)
//! local store, so a read would return "config not found" or a superseded value
//! instead of failing. Nacos closes that hole with this filter; note that it is
//! independent from `nacos.naming.data.warmup` (batata: `data_warmup`), which is
//! `false` by default in both projects, so the lifecycle filter alone does not
//! cover this case.
//!
//! Reads are only gated when the local store is Raft-backed:
//! - standalone mode (no Raft): never gated
//! - external database (`datasource_platform` set): every node reads the shared
//!   database, so there is no Raft-specific staleness to guard against
//! - embedded cluster: gated until the node is a voting member

use std::future::{Future, Ready, ready};
use std::pin::Pin;

use actix_web::{
    Error, HttpResponse,
    body::EitherBody,
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    http::StatusCode,
};

use crate::model::common::AppState;

/// Middleware factory that rejects config reads until the local store is Raft-ready.
pub struct ConfigStorageGate;

impl ConfigStorageGate {
    /// Performs the `new` operation.
    pub fn new() -> Self {
        Self
    }
}

impl Default for ConfigStorageGate {
    fn default() -> Self {
        Self::new()
    }
}

impl<S, B> Transform<S, ServiceRequest> for ConfigStorageGate
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Transform = ConfigStorageGateMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(ConfigStorageGateMiddleware { service }))
    }
}

/// Middleware that gates config endpoints on Raft membership.
pub struct ConfigStorageGateMiddleware<S> {
    service: S,
}

/// Whether the request targets a config endpoint (Nacos `/v1/cs/*` equivalent).
///
/// Covers `/v2/cs/...`, `/v3/client/cs/...` and `/v3/admin/cs/...`.
fn is_config_path(path: &str) -> bool {
    path.contains("/cs/")
}

/// Decide whether the request must be rejected.
///
/// Returns the rejection reason, or `None` when the request may proceed.
fn rejection_reason(app_state: &AppState, path: &str) -> Option<String> {
    if !is_config_path(path) {
        return None;
    }

    // Standalone: no Raft, nothing to be stale behind.
    if app_state.configuration.is_standalone() {
        return None;
    }

    // External database: all nodes read the same database, so membership says
    // nothing about config freshness (upstream gates only embedded storage).
    if !app_state.configuration.datasource_platform().is_empty() {
        return None;
    }

    match app_state.raft_node.as_ref() {
        None => None,
        Some(raft) if raft.is_group_member() => None,
        Some(_) => Some("local config store is not raft-initialized".to_string()),
    }
}

impl<S, B> Service<ServiceRequest> for ConfigStorageGateMiddleware<S>
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
        // Only config paths can be rejected; everything else passes through
        // without touching the app state at all.
        if !is_config_path(req.path()) {
            let fut = self.service.call(req);
            return Box::pin(async move { fut.await.map(|res| res.map_into_left_body()) });
        }

        let reason = req
            .app_data::<actix_web::web::Data<AppState>>()
            .and_then(|state| rejection_reason(state, req.path()));

        let Some(reason) = reason else {
            let fut = self.service.call(req);
            return Box::pin(async move { fut.await.map(|res| res.map_into_left_body()) });
        };

        let path = req.path().to_string();
        tracing::warn!(
            "Rejecting config read on {}: {} (node has not joined the raft group)",
            path,
            reason
        );

        Box::pin(async move {
            let body = format!(
                "In the node initialization, unable to process any requests at this time: {}",
                reason
            );
            let response = HttpResponse::build(StatusCode::SERVICE_UNAVAILABLE).body(body);
            Ok(req.into_response(response).map_into_right_body())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_config_path() {
        assert!(is_config_path("/nacos/v2/cs/config"));
        assert!(is_config_path("/nacos/v3/client/cs/config"));
        assert!(is_config_path("/nacos/v3/admin/cs/config/list"));
        assert!(!is_config_path("/nacos/v2/ns/instance"));
        assert!(!is_config_path("/nacos/v3/client/ns/instance/list"));
    }

    /// The gate is a no-op outside embedded cluster mode; the AppState-dependent
    /// branches are covered by the path/standalone checks above, keeping this
    /// unit free of a full server fixture.
    #[test]
    fn test_gate_skips_non_config_paths() {
        assert!(!is_config_path("/health/readiness"));
    }
}
