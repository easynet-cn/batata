//! Prometheus metrics monitoring
//!
//! Provides metrics collection for:
//! - Request latency (histogram)
//! - Service info size (gauge)
//! - Config listener count (gauge)
//! - Failed request count (counter)

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use prometheus::{
    CounterVec, Encoder, GaugeVec, HistogramVec, TextEncoder, register_counter_vec,
    register_gauge_vec, register_histogram_vec,
};

/// Prometheus metrics collector
pub struct MetricsMonitor {
    /// Request latency histogram
    pub request_latency: HistogramVec,

    /// Service info size gauge
    pub service_info_size: GaugeVec,

    /// Config listener count gauge
    pub config_listener_count: GaugeVec,

    /// Failed request count counter
    pub failed_request_count: CounterVec,

    /// Success request count counter
    pub success_request_count: CounterVec,
}

impl MetricsMonitor {
    /// Create a new metrics monitor
    pub fn new() -> Result<Self, prometheus::Error> {
        let request_latency = register_histogram_vec!(
            "batata_request_latency_seconds",
            "Request latency in seconds",
            &["operation", "status"]
        )?;

        let service_info_size = register_gauge_vec!(
            "batata_service_info_size_bytes",
            "Service info size in bytes",
            &["service_name", "namespace_id"]
        )?;

        let config_listener_count = register_gauge_vec!(
            "batata_config_listener_count",
            "Number of config listeners",
            &["data_id", "group", "tenant"]
        )?;

        let failed_request_count = register_counter_vec!(
            "batata_failed_requests_total",
            "Total number of failed requests",
            &["operation", "error_type"]
        )?;

        let success_request_count = register_counter_vec!(
            "batata_success_requests_total",
            "Total number of successful requests",
            &["operation"]
        )?;

        Ok(Self {
            request_latency,
            service_info_size,
            config_listener_count,
            failed_request_count,
            success_request_count,
        })
    }

    /// Record request latency
    pub fn record_latency(&self, operation: &str, status: &str, duration: Duration) {
        self.request_latency
            .with_label_values(&[operation, status])
            .observe(duration.as_secs_f64());
    }

    /// Update service info size
    pub fn update_service_info_size(&self, service_name: &str, namespace_id: &str, size: u64) {
        self.service_info_size
            .with_label_values(&[service_name, namespace_id])
            .set(size as f64);
    }

    /// Update config listener count
    pub fn update_config_listener_count(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
        count: u64,
    ) {
        self.config_listener_count
            .with_label_values(&[data_id, group, tenant])
            .set(count as f64);
    }

    /// Increment failed request count
    pub fn increment_failed_request(&self, operation: &str, error_type: &str) {
        self.failed_request_count
            .with_label_values(&[operation, error_type])
            .inc();
    }

    /// Increment success request count
    pub fn increment_success_request(&self, operation: &str) {
        self.success_request_count
            .with_label_values(&[operation])
            .inc();
    }

    /// Get metrics in Prometheus format
    pub fn gather(&self) -> String {
        let encoder = TextEncoder::new();
        let metric_families = prometheus::gather();
        encoder.encode_to_string(&metric_families).unwrap()
    }
}

impl Default for MetricsMonitor {
    fn default() -> Self {
        Self::new().expect("failed to create metrics monitor")
    }
}

/// Simple metrics timer for measuring operation duration
pub struct Timer<'a> {
    metrics: &'a MetricsMonitor,
    operation: String,
    start: Instant,
}

impl<'a> Timer<'a> {
    /// Create a new timer
    pub fn start(metrics: &'a MetricsMonitor, operation: &str) -> Self {
        Self {
            metrics,
            operation: operation.to_string(),
            start: Instant::now(),
        }
    }

    /// Stop the timer and record success
    pub fn success(self) {
        let duration = self.start.elapsed();
        self.metrics
            .record_latency(&self.operation, "success", duration);
        self.metrics.increment_success_request(&self.operation);
    }

    /// Stop the timer and record failure
    pub fn failure(self, error_type: &str) {
        let duration = self.start.elapsed();
        self.metrics
            .record_latency(&self.operation, "error", duration);
        self.metrics
            .increment_failed_request(&self.operation, error_type);
    }
}

/// Simple atomic counter for metrics without Prometheus dependency
pub struct SimpleCounter {
    value: Arc<AtomicU64>,
}

impl SimpleCounter {
    pub fn new() -> Self {
        Self {
            value: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn increment(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    pub fn add(&self, delta: u64) {
        self.value.fetch_add(delta, Ordering::Relaxed);
    }

    pub fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }

    pub fn reset(&self) {
        self.value.store(0, Ordering::Relaxed);
    }
}

impl Clone for SimpleCounter {
    fn clone(&self) -> Self {
        Self {
            value: Arc::clone(&self.value),
        }
    }
}

impl Default for SimpleCounter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_counter() {
        let counter = SimpleCounter::new();

        assert_eq!(counter.get(), 0);

        counter.increment();
        assert_eq!(counter.get(), 1);

        counter.add(5);
        assert_eq!(counter.get(), 6);

        counter.reset();
        assert_eq!(counter.get(), 0);
    }

    #[test]
    fn test_counter_clone() {
        let counter = SimpleCounter::new();
        let clone = counter.clone();

        counter.increment();
        assert_eq!(clone.get(), 1);
        assert_eq!(counter.get(), 1);
    }

    #[test]
    fn test_metrics_monitor() {
        let metrics = MetricsMonitor::new().unwrap();

        metrics.record_latency("test_op", "success", Duration::from_millis(100));
        metrics.increment_success_request("test_op");

        let output = metrics.gather();
        assert!(output.contains("batata_request_latency_seconds"));
        assert!(output.contains("batata_success_requests_total"));
    }
}
