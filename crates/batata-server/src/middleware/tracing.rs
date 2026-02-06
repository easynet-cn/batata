//! Distributed tracing middleware for OpenTelemetry integration.
//!
//! This module provides:
//! - W3C Trace Context header extraction and injection
//! - Automatic span creation for HTTP requests
//! - Span attributes for request metadata
//! - Trace ID propagation across services

use std::future::{Future, Ready, ready};
use std::pin::Pin;
use std::task::{Context, Poll};

use actix_web::{
    Error, HttpMessage,
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    http::header::HeaderMap,
};
use tracing::{Instrument, Span, info_span};

/// W3C Trace Context header names
pub mod headers {
    /// Traceparent header (W3C Trace Context)
    pub const TRACEPARENT: &str = "traceparent";
    /// Tracestate header (W3C Trace Context)
    pub const TRACESTATE: &str = "tracestate";
    /// B3 single header (Zipkin)
    pub const B3: &str = "b3";
    /// B3 trace ID header (Zipkin)
    pub const B3_TRACE_ID: &str = "x-b3-traceid";
    /// B3 span ID header (Zipkin)
    pub const B3_SPAN_ID: &str = "x-b3-spanid";
    /// B3 sampled header (Zipkin)
    pub const B3_SAMPLED: &str = "x-b3-sampled";
    /// Jaeger trace ID header
    pub const UBER_TRACE_ID: &str = "uber-trace-id";
    /// Request ID header (custom)
    pub const X_REQUEST_ID: &str = "x-request-id";
}

/// Span attribute keys following OpenTelemetry semantic conventions
pub mod attributes {
    pub const HTTP_METHOD: &str = "http.method";
    pub const HTTP_URL: &str = "http.url";
    pub const HTTP_TARGET: &str = "http.target";
    pub const HTTP_HOST: &str = "http.host";
    pub const HTTP_SCHEME: &str = "http.scheme";
    pub const HTTP_STATUS_CODE: &str = "http.status_code";
    pub const HTTP_USER_AGENT: &str = "http.user_agent";
    pub const HTTP_REQUEST_CONTENT_LENGTH: &str = "http.request_content_length";
    pub const HTTP_RESPONSE_CONTENT_LENGTH: &str = "http.response_content_length";
    pub const NET_PEER_IP: &str = "net.peer.ip";
    pub const NET_PEER_PORT: &str = "net.peer.port";
    pub const ENDUSER_ID: &str = "enduser.id";
    pub const BATATA_NAMESPACE: &str = "batata.namespace";
    pub const BATATA_GROUP: &str = "batata.group";
    pub const BATATA_DATA_ID: &str = "batata.data_id";
    pub const BATATA_SERVICE_NAME: &str = "batata.service_name";
}

/// Trace context extracted from incoming request headers
#[derive(Debug, Clone, Default)]
pub struct TraceContext {
    /// W3C traceparent header value
    pub traceparent: Option<String>,
    /// W3C tracestate header value
    pub tracestate: Option<String>,
    /// B3 trace ID
    pub b3_trace_id: Option<String>,
    /// B3 span ID
    pub b3_span_id: Option<String>,
    /// B3 sampled flag
    pub b3_sampled: Option<String>,
    /// Jaeger trace ID
    pub uber_trace_id: Option<String>,
    /// Custom request ID
    pub request_id: Option<String>,
}

impl TraceContext {
    /// Extract trace context from HTTP headers
    pub fn from_headers(headers: &HeaderMap) -> Self {
        Self {
            traceparent: headers
                .get(headers::TRACEPARENT)
                .and_then(|v| v.to_str().ok())
                .map(String::from),
            tracestate: headers
                .get(headers::TRACESTATE)
                .and_then(|v| v.to_str().ok())
                .map(String::from),
            b3_trace_id: headers
                .get(headers::B3_TRACE_ID)
                .and_then(|v| v.to_str().ok())
                .map(String::from),
            b3_span_id: headers
                .get(headers::B3_SPAN_ID)
                .and_then(|v| v.to_str().ok())
                .map(String::from),
            b3_sampled: headers
                .get(headers::B3_SAMPLED)
                .and_then(|v| v.to_str().ok())
                .map(String::from),
            uber_trace_id: headers
                .get(headers::UBER_TRACE_ID)
                .and_then(|v| v.to_str().ok())
                .map(String::from),
            request_id: headers
                .get(headers::X_REQUEST_ID)
                .and_then(|v| v.to_str().ok())
                .map(String::from),
        }
    }

    /// Get the primary trace ID from any available source
    pub fn trace_id(&self) -> Option<&str> {
        // Priority: W3C > B3 > Jaeger > Request ID
        if let Some(ref tp) = self.traceparent {
            // Extract trace ID from traceparent: 00-<trace-id>-<span-id>-<flags>
            if let Some(trace_id) = tp.split('-').nth(1) {
                return Some(trace_id);
            }
        }
        if let Some(ref b3) = self.b3_trace_id {
            return Some(b3.as_str());
        }
        if let Some(ref uber) = self.uber_trace_id {
            // Extract trace ID from uber-trace-id: <trace-id>:<span-id>:<parent-id>:<flags>
            if let Some(trace_id) = uber.split(':').next() {
                return Some(trace_id);
            }
        }
        self.request_id.as_deref()
    }

    /// Check if tracing is sampled
    pub fn is_sampled(&self) -> bool {
        if let Some(ref tp) = self.traceparent {
            // Check flags in traceparent: 00-<trace-id>-<span-id>-<flags>
            if let Some(flags) = tp.split('-').nth(3) {
                return flags == "01";
            }
        }
        if let Some(ref sampled) = self.b3_sampled {
            return sampled == "1" || sampled.to_lowercase() == "true";
        }
        true // Default to sampled
    }
}

/// Tracing middleware factory
pub struct TracingMiddleware;

impl TracingMiddleware {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TracingMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

impl<S, B> Transform<S, ServiceRequest> for TracingMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = TracingMiddlewareService<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(TracingMiddlewareService { service }))
    }
}

/// Tracing middleware service
pub struct TracingMiddlewareService<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for TracingMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        // Extract trace context from headers
        let trace_ctx = TraceContext::from_headers(req.headers());

        // Extract request metadata for span attributes
        let method = req.method().to_string();
        let path = req.path().to_string();
        let uri = req.uri().to_string();
        let host = req
            .headers()
            .get("host")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("unknown")
            .to_string();
        let user_agent = req
            .headers()
            .get("user-agent")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("unknown")
            .to_string();
        let peer_ip = req
            .connection_info()
            .realip_remote_addr()
            .unwrap_or("unknown")
            .to_string();

        // Get trace ID for span
        let trace_id = trace_ctx.trace_id().unwrap_or("none").to_string();

        // Store trace context in request extensions for downstream use
        req.extensions_mut().insert(trace_ctx);

        // Create span with request attributes
        let span = info_span!(
            "http_request",
            otel.name = %format!("{} {}", method, path),
            trace_id = %trace_id,
            http.method = %method,
            http.target = %path,
            http.url = %uri,
            http.host = %host,
            http.user_agent = %user_agent,
            net.peer.ip = %peer_ip,
            http.status_code = tracing::field::Empty,
        );

        let fut = self.service.call(req);

        Box::pin(
            async move {
                let res = fut.await?;

                // Record response status code
                let status = res.status().as_u16();
                Span::current().record("http.status_code", status);

                Ok(res)
            }
            .instrument(span),
        )
    }
}

/// Helper trait to extract trace context from request
pub trait RequestTraceExt {
    /// Get the trace context from the request
    fn trace_context(&self) -> Option<TraceContext>;

    /// Get the trace ID from the request
    fn trace_id(&self) -> Option<String>;
}

impl RequestTraceExt for actix_web::HttpRequest {
    fn trace_context(&self) -> Option<TraceContext> {
        self.extensions().get::<TraceContext>().cloned()
    }

    fn trace_id(&self) -> Option<String> {
        self.extensions()
            .get::<TraceContext>()
            .and_then(|ctx| ctx.trace_id().map(String::from))
    }
}

/// Create a child span with Batata-specific attributes
#[macro_export]
macro_rules! batata_span {
    ($name:expr, $($field:tt)*) => {
        tracing::info_span!(
            $name,
            otel.name = $name,
            $($field)*
        )
    };
}

/// Record Batata-specific attributes on the current span
pub fn record_config_attributes(namespace: &str, group: &str, data_id: &str) {
    let span = Span::current();
    span.record(attributes::BATATA_NAMESPACE, namespace);
    span.record(attributes::BATATA_GROUP, group);
    span.record(attributes::BATATA_DATA_ID, data_id);
}

/// Record service discovery attributes on the current span
pub fn record_naming_attributes(namespace: &str, group: &str, service_name: &str) {
    let span = Span::current();
    span.record(attributes::BATATA_NAMESPACE, namespace);
    span.record(attributes::BATATA_GROUP, group);
    span.record(attributes::BATATA_SERVICE_NAME, service_name);
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::http::header::HeaderValue;

    #[test]
    fn test_trace_context_from_traceparent() {
        let mut headers = HeaderMap::new();
        headers.insert(
            headers::TRACEPARENT.parse().unwrap(),
            HeaderValue::from_static("00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01"),
        );

        let ctx = TraceContext::from_headers(&headers);
        assert!(ctx.traceparent.is_some());
        assert_eq!(
            ctx.trace_id(),
            Some("0af7651916cd43dd8448eb211c80319c")
        );
        assert!(ctx.is_sampled());
    }

    #[test]
    fn test_trace_context_from_b3() {
        let mut headers = HeaderMap::new();
        headers.insert(
            headers::B3_TRACE_ID.parse().unwrap(),
            HeaderValue::from_static("463ac35c9f6413ad48485a3953bb6124"),
        );
        headers.insert(
            headers::B3_SPAN_ID.parse().unwrap(),
            HeaderValue::from_static("0020000000000001"),
        );
        headers.insert(
            headers::B3_SAMPLED.parse().unwrap(),
            HeaderValue::from_static("1"),
        );

        let ctx = TraceContext::from_headers(&headers);
        assert_eq!(ctx.trace_id(), Some("463ac35c9f6413ad48485a3953bb6124"));
        assert!(ctx.is_sampled());
    }

    #[test]
    fn test_trace_context_from_request_id() {
        let mut headers = HeaderMap::new();
        headers.insert(
            headers::X_REQUEST_ID.parse().unwrap(),
            HeaderValue::from_static("my-custom-request-id"),
        );

        let ctx = TraceContext::from_headers(&headers);
        assert_eq!(ctx.trace_id(), Some("my-custom-request-id"));
    }

    #[test]
    fn test_trace_context_not_sampled() {
        let mut headers = HeaderMap::new();
        headers.insert(
            headers::TRACEPARENT.parse().unwrap(),
            HeaderValue::from_static("00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-00"),
        );

        let ctx = TraceContext::from_headers(&headers);
        assert!(!ctx.is_sampled());
    }
}
