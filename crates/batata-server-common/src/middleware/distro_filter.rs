//! Distro filter middleware for AP mode request routing.
//!
//! In cluster mode, ephemeral instance operations (register/deregister/update)
//! should be handled by the responsible node determined by consistent hashing.
//! This middleware intercepts naming write requests and handles them locally,
//! relying on the Distro protocol for cross-node synchronization.

use std::future::{Ready, ready};
use std::sync::Arc;

use actix_service::{Service, Transform};
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::Error;
use futures::future::LocalBoxFuture;

use batata_core::service::distro::DistroProtocol;

/// Actix middleware that intercepts naming write requests in cluster mode.
///
/// For ephemeral instance operations (register/deregister/update), the request
/// is always handled locally. The Distro protocol handles cross-node data
/// synchronization in the background.
pub struct DistroFilter {
    distro: Option<Arc<DistroProtocol>>,
}

impl DistroFilter {
    pub fn new(distro: Option<Arc<DistroProtocol>>) -> Self {
        Self { distro }
    }
}

impl<S, B> Transform<S, ServiceRequest> for DistroFilter
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<actix_web::body::EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = DistroFilterMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(DistroFilterMiddleware {
            service,
            distro: self.distro.clone(),
        }))
    }
}

pub struct DistroFilterMiddleware<S> {
    service: S,
    distro: Option<Arc<DistroProtocol>>,
}

impl<S, B> Service<ServiceRequest> for DistroFilterMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<actix_web::body::EitherBody<B>>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &self,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let Some(ref _distro) = self.distro else {
            // No distro protocol (standalone mode) — pass through
            let fut = self.service.call(req);
            return Box::pin(async move {
                let res = fut.await?;
                Ok(res.map_into_left_body())
            });
        };

        // Handle all requests locally — Distro protocol syncs data in background.
        // This matches gRPC handler behavior where operations are executed
        // locally and distro.sync_data() propagates changes to peers.
        let fut = self.service.call(req);
        Box::pin(async move {
            let res = fut.await?;
            Ok(res.map_into_left_body())
        })
    }
}
