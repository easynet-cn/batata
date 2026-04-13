//! Prometheus-compatible metrics for Consul plugin endpoints.
//!
//! Uses the `metrics` crate facade so counters are exported via the
//! main server's `/prometheus` endpoint.
//!
//! Naming convention matches Consul's metric names where possible:
//! `consul.<component>.<operation>` (e.g., `consul.catalog.register`).

/// Increment a counter for an HTTP endpoint call.
///
/// `endpoint`: short name (e.g., "catalog_register", "kv_get")
/// `status`: "success" or "error"
#[inline]
pub fn incr_endpoint(endpoint: &'static str, status: &'static str) {
    metrics::counter!(
        "consul_api_calls_total",
        "endpoint" => endpoint,
        "status" => status,
    )
    .increment(1);
}

/// Record a request latency for an endpoint.
#[inline]
pub fn record_latency(endpoint: &'static str, duration_secs: f64) {
    metrics::histogram!(
        "consul_api_latency_seconds",
        "endpoint" => endpoint,
    )
    .record(duration_secs);
}
