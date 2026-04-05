// HTTP metrics middleware
// Records request count, duration, and error rate for Prometheus export

use std::future::{Future, Ready, ready};
use std::pin::Pin;
use std::time::Instant;

use actix_web::{
    Error,
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
};
use metrics::{counter, histogram};

/// Middleware that records HTTP request metrics (count, duration, errors)
/// using the `metrics` crate facade. Metrics are automatically exported
/// via the `/prometheus` endpoint.
///
/// Path labels are normalized to prevent high-cardinality explosion:
/// - Query parameters are stripped
/// - Trailing slashes are removed
pub struct HttpMetrics;

impl<S, B> Transform<S, ServiceRequest> for HttpMetrics
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = HttpMetricsMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(HttpMetricsMiddleware { service }))
    }
}

pub struct HttpMetricsMiddleware<S> {
    service: S,
}

/// Normalize an HTTP path for use as a metrics label.
/// Strips query parameters and trailing slashes to prevent cardinality explosion.
fn normalize_path(path: &str) -> String {
    let path = path.split('?').next().unwrap_or(path);
    let path = if path.len() > 1 {
        path.trim_end_matches('/')
    } else {
        path
    };
    path.to_string()
}

impl<S, B> Service<ServiceRequest> for HttpMetricsMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(
        &self,
        ctx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(ctx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let method = req.method().to_string();
        let path = normalize_path(req.path());
        let start = Instant::now();

        let fut = self.service.call(req);

        Box::pin(async move {
            let res = fut.await?;
            let duration = start.elapsed().as_secs_f64();
            let status = res.status().as_u16().to_string();

            counter!("http_requests_total", "method" => method.clone(), "path" => path.clone(), "status" => status.clone()).increment(1);
            histogram!("http_request_duration_seconds", "method" => method.clone(), "path" => path.clone()).record(duration);

            if res.status().is_client_error() || res.status().is_server_error() {
                counter!("http_requests_errors_total", "method" => method, "path" => path, "status" => status).increment(1);
            }

            Ok(res)
        })
    }
}
