//! HTTP request tracing middleware gating.
//!
//! This mirrors the `consul`/`plugin` convention used elsewhere in Batata:
//! tracing is controlled by BOTH a compile feature (`http-tracing`) and a
//! runtime toggle (`batata.server.http.access_log.enabled`).
//!
//! - When `http-tracing` is **enabled** (default), `access_log_layer(enabled)`
//!   returns `Condition::new(enabled, Logger::default())`, preserving the
//!   existing runtime toggle: with `enabled = false` the middleware
//!   short-circuits and adds no per-request work.
//! - When `http-tracing` is **disabled** at build time, the tracing middleware
//!   is compiled out entirely and replaced by a zero-cost [`NoopMiddleware`],
//!   so there is no tracing code in the request path at all.
//!
//! This is the same dual-gating pattern as `consul`
//! (`consul = ["dep:batata-plugin-consul"]` + `ConsulPlugin::is_enabled()`).

/// Build the HTTP access-log / request-tracing middleware for the current build.
///
/// The `enabled` flag is the runtime toggle; the compile feature decides whether
/// a real tracing middleware or a compiled-out no-op is returned.
#[cfg(feature = "http-tracing")]
pub fn access_log_layer(
    enabled: bool,
) -> actix_web::middleware::Condition<actix_web::middleware::Logger> {
    actix_web::middleware::Condition::new(enabled, actix_web::middleware::Logger::default())
}

/// Compiled-out stand-in for [`access_log_layer`] when `http-tracing` is off.
///
/// Returns a zero-cost middleware that does nothing, so the request path is
/// completely free of tracing overhead.
#[cfg(not(feature = "http-tracing"))]
pub fn access_log_layer(_enabled: bool) -> NoopMiddleware {
    NoopMiddleware
}

// ---------------------------------------------------------------------------
// Zero-cost no-op middleware (only compiled when `http-tracing` is disabled)
// ---------------------------------------------------------------------------

#[cfg(not(feature = "http-tracing"))]
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};

/// A middleware that passes the request through unchanged and adds zero
/// per-request overhead. Used as the compiled-out stand-in for request tracing.
#[cfg(not(feature = "http-tracing"))]
pub struct NoopMiddleware;

#[cfg(not(feature = "http-tracing"))]
impl<S, B> Transform<S, ServiceRequest> for NoopMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = actix_web::Error;
    type Transform = NoopMiddlewareService<S>;
    type InitError = ();
    type Future = std::future::Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        std::future::ready(Ok(NoopMiddlewareService { service }))
    }
}

/// Inner service for [`NoopMiddleware`].
#[cfg(not(feature = "http-tracing"))]
pub struct NoopMiddlewareService<S> {
    /// `service` field.
    service: S,
}

#[cfg(not(feature = "http-tracing"))]
impl<S, B> Service<ServiceRequest> for NoopMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = actix_web::Error;
    type Future = S::Future;

    fn poll_ready(
        &self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        self.service.call(req)
    }
}
