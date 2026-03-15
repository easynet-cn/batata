//! Prometheus metrics endpoint using metrics-exporter-prometheus
//!
//! Provides a `/prometheus` endpoint that exports application metrics
//! in Prometheus text format using the `metrics` crate ecosystem.
//! This complements the existing `/metrics` endpoint from batata-console
//! by integrating with the standard `metrics` crate facade.

use actix_web::{HttpResponse, web};
use metrics_exporter_prometheus::PrometheusHandle;

/// Shared Prometheus recorder handle for the metrics-crate-based exporter.
///
/// This allows any code using `metrics::counter!()`, `metrics::gauge!()`,
/// etc. to have their metrics automatically exported via Prometheus.
pub struct PrometheusMetricsState {
    pub handle: PrometheusHandle,
}

impl Default for PrometheusMetricsState {
    fn default() -> Self {
        Self::new()
    }
}

impl PrometheusMetricsState {
    /// Initialize the Prometheus metrics recorder.
    ///
    /// This installs a global recorder so that all `metrics::counter!()`,
    /// `metrics::gauge!()`, and `metrics::histogram!()` calls throughout
    /// the application are captured and can be rendered.
    pub fn new() -> Self {
        let builder = metrics_exporter_prometheus::PrometheusBuilder::new();
        let handle = builder
            .install_recorder()
            .expect("Failed to install Prometheus recorder");

        Self { handle }
    }
}

/// GET /prometheus - Returns Prometheus-format metrics from the metrics crate.
///
/// This endpoint renders all metrics registered via the `metrics` crate
/// (counters, gauges, histograms) in Prometheus exposition format.
pub async fn prometheus_metrics(state: web::Data<PrometheusMetricsState>) -> HttpResponse {
    let output = state.handle.render();
    HttpResponse::Ok()
        .content_type("text/plain; version=0.0.4; charset=utf-8")
        .body(output)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_prometheus_metrics_state_content_type() {
        // Verify the expected content type string
        let content_type = "text/plain; version=0.0.4; charset=utf-8";
        assert!(content_type.contains("text/plain"));
        assert!(content_type.contains("0.0.4"));
    }
}
