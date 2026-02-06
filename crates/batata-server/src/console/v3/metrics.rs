// Prometheus metrics endpoint for observability
// Exports application metrics in Prometheus text format

use actix_web::{HttpResponse, Responder, Scope, get, web};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use crate::model::common::AppState;

/// Application metrics collector
#[derive(Default)]
pub struct Metrics {
    /// Total HTTP requests received
    pub http_requests_total: AtomicU64,
    /// Total gRPC requests received
    pub grpc_requests_total: AtomicU64,
    /// Config publish operations
    pub config_publish_total: AtomicU64,
    /// Config query operations
    pub config_query_total: AtomicU64,
    /// Config listen operations
    pub config_listen_total: AtomicU64,
    /// Config delete operations
    pub config_delete_total: AtomicU64,
    /// Service registration operations
    pub service_register_total: AtomicU64,
    /// Service deregistration operations
    pub service_deregister_total: AtomicU64,
    /// Service query operations
    pub service_query_total: AtomicU64,
    /// Service subscribe operations
    pub service_subscribe_total: AtomicU64,
    /// Heartbeat operations
    pub heartbeat_total: AtomicU64,
    /// Active connections count
    pub active_connections: AtomicU64,
    /// Active gRPC connections
    pub grpc_connections: AtomicU64,
    /// Total config count
    pub config_count: AtomicU64,
    /// Total service count
    pub service_count: AtomicU64,
    /// Total instance count
    pub instance_count: AtomicU64,
    /// Healthy instance count
    pub healthy_instance_count: AtomicU64,
    /// HTTP request latency sum (microseconds)
    pub http_latency_sum_us: AtomicU64,
    /// HTTP request count for latency
    pub http_latency_count: AtomicU64,
    /// HTTP error count
    pub http_errors_total: AtomicU64,
    /// gRPC error count
    pub grpc_errors_total: AtomicU64,
    /// Start time for uptime calculation
    start_time: Option<Instant>,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            start_time: Some(Instant::now()),
            ..Default::default()
        }
    }

    pub fn inc_http_requests(&self) {
        self.http_requests_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_grpc_requests(&self) {
        self.grpc_requests_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_config_publish(&self) {
        self.config_publish_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_config_query(&self) {
        self.config_query_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_config_listen(&self) {
        self.config_listen_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_config_delete(&self) {
        self.config_delete_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_service_register(&self) {
        self.service_register_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_service_deregister(&self) {
        self.service_deregister_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_service_query(&self) {
        self.service_query_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_service_subscribe(&self) {
        self.service_subscribe_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_heartbeat(&self) {
        self.heartbeat_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_connections(&self) {
        self.active_connections.fetch_add(1, Ordering::Relaxed);
    }

    pub fn dec_connections(&self) {
        self.active_connections.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn inc_grpc_connections(&self) {
        self.grpc_connections.fetch_add(1, Ordering::Relaxed);
    }

    pub fn dec_grpc_connections(&self) {
        self.grpc_connections.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn set_config_count(&self, count: u64) {
        self.config_count.store(count, Ordering::Relaxed);
    }

    pub fn set_service_count(&self, count: u64) {
        self.service_count.store(count, Ordering::Relaxed);
    }

    pub fn set_instance_count(&self, total: u64, healthy: u64) {
        self.instance_count.store(total, Ordering::Relaxed);
        self.healthy_instance_count
            .store(healthy, Ordering::Relaxed);
    }

    pub fn record_http_latency(&self, latency_us: u64) {
        self.http_latency_sum_us
            .fetch_add(latency_us, Ordering::Relaxed);
        self.http_latency_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_http_errors(&self) {
        self.http_errors_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_grpc_errors(&self) {
        self.grpc_errors_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn uptime_seconds(&self) -> u64 {
        self.start_time.map(|t| t.elapsed().as_secs()).unwrap_or(0)
    }

    pub fn avg_http_latency_ms(&self) -> f64 {
        let count = self.http_latency_count.load(Ordering::Relaxed);
        if count == 0 {
            return 0.0;
        }
        let sum = self.http_latency_sum_us.load(Ordering::Relaxed);
        (sum as f64 / count as f64) / 1000.0 // Convert us to ms
    }

    /// Format metrics in Prometheus text format
    pub fn to_prometheus_format(&self, cluster_size: usize, is_healthy: bool) -> String {
        let mut output = String::with_capacity(8192);

        // ========== HTTP Metrics ==========
        output
            .push_str("# HELP batata_http_requests_total Total number of HTTP requests received\n");
        output.push_str("# TYPE batata_http_requests_total counter\n");
        output.push_str(&format!(
            "batata_http_requests_total {}\n\n",
            self.http_requests_total.load(Ordering::Relaxed)
        ));

        output.push_str("# HELP batata_http_errors_total Total number of HTTP errors\n");
        output.push_str("# TYPE batata_http_errors_total counter\n");
        output.push_str(&format!(
            "batata_http_errors_total {}\n\n",
            self.http_errors_total.load(Ordering::Relaxed)
        ));

        output.push_str(
            "# HELP batata_http_request_latency_ms Average HTTP request latency in milliseconds\n",
        );
        output.push_str("# TYPE batata_http_request_latency_ms gauge\n");
        output.push_str(&format!(
            "batata_http_request_latency_ms {:.3}\n\n",
            self.avg_http_latency_ms()
        ));

        // ========== gRPC Metrics ==========
        output
            .push_str("# HELP batata_grpc_requests_total Total number of gRPC requests received\n");
        output.push_str("# TYPE batata_grpc_requests_total counter\n");
        output.push_str(&format!(
            "batata_grpc_requests_total {}\n\n",
            self.grpc_requests_total.load(Ordering::Relaxed)
        ));

        output.push_str("# HELP batata_grpc_errors_total Total number of gRPC errors\n");
        output.push_str("# TYPE batata_grpc_errors_total counter\n");
        output.push_str(&format!(
            "batata_grpc_errors_total {}\n\n",
            self.grpc_errors_total.load(Ordering::Relaxed)
        ));

        output.push_str("# HELP batata_grpc_connections Current number of gRPC connections\n");
        output.push_str("# TYPE batata_grpc_connections gauge\n");
        output.push_str(&format!(
            "batata_grpc_connections {}\n\n",
            self.grpc_connections.load(Ordering::Relaxed)
        ));

        // ========== Config Service Metrics ==========
        output.push_str(
            "# HELP batata_config_publish_total Total number of config publish operations\n",
        );
        output.push_str("# TYPE batata_config_publish_total counter\n");
        output.push_str(&format!(
            "batata_config_publish_total {}\n\n",
            self.config_publish_total.load(Ordering::Relaxed)
        ));

        output
            .push_str("# HELP batata_config_query_total Total number of config query operations\n");
        output.push_str("# TYPE batata_config_query_total counter\n");
        output.push_str(&format!(
            "batata_config_query_total {}\n\n",
            self.config_query_total.load(Ordering::Relaxed)
        ));

        output.push_str(
            "# HELP batata_config_listen_total Total number of config listen operations\n",
        );
        output.push_str("# TYPE batata_config_listen_total counter\n");
        output.push_str(&format!(
            "batata_config_listen_total {}\n\n",
            self.config_listen_total.load(Ordering::Relaxed)
        ));

        output.push_str(
            "# HELP batata_config_delete_total Total number of config delete operations\n",
        );
        output.push_str("# TYPE batata_config_delete_total counter\n");
        output.push_str(&format!(
            "batata_config_delete_total {}\n\n",
            self.config_delete_total.load(Ordering::Relaxed)
        ));

        output.push_str("# HELP batata_config_count Current number of configurations\n");
        output.push_str("# TYPE batata_config_count gauge\n");
        output.push_str(&format!(
            "batata_config_count {}\n\n",
            self.config_count.load(Ordering::Relaxed)
        ));

        // ========== Naming Service Metrics ==========
        output.push_str("# HELP batata_service_register_total Total number of service registration operations\n");
        output.push_str("# TYPE batata_service_register_total counter\n");
        output.push_str(&format!(
            "batata_service_register_total {}\n\n",
            self.service_register_total.load(Ordering::Relaxed)
        ));

        output.push_str("# HELP batata_service_deregister_total Total number of service deregistration operations\n");
        output.push_str("# TYPE batata_service_deregister_total counter\n");
        output.push_str(&format!(
            "batata_service_deregister_total {}\n\n",
            self.service_deregister_total.load(Ordering::Relaxed)
        ));

        output.push_str(
            "# HELP batata_service_query_total Total number of service query operations\n",
        );
        output.push_str("# TYPE batata_service_query_total counter\n");
        output.push_str(&format!(
            "batata_service_query_total {}\n\n",
            self.service_query_total.load(Ordering::Relaxed)
        ));

        output.push_str(
            "# HELP batata_service_subscribe_total Total number of service subscribe operations\n",
        );
        output.push_str("# TYPE batata_service_subscribe_total counter\n");
        output.push_str(&format!(
            "batata_service_subscribe_total {}\n\n",
            self.service_subscribe_total.load(Ordering::Relaxed)
        ));

        output.push_str("# HELP batata_heartbeat_total Total number of heartbeat operations\n");
        output.push_str("# TYPE batata_heartbeat_total counter\n");
        output.push_str(&format!(
            "batata_heartbeat_total {}\n\n",
            self.heartbeat_total.load(Ordering::Relaxed)
        ));

        output.push_str("# HELP batata_service_count Current number of services\n");
        output.push_str("# TYPE batata_service_count gauge\n");
        output.push_str(&format!(
            "batata_service_count {}\n\n",
            self.service_count.load(Ordering::Relaxed)
        ));

        output.push_str("# HELP batata_instance_count Current number of instances\n");
        output.push_str("# TYPE batata_instance_count gauge\n");
        output.push_str(&format!(
            "batata_instance_count {}\n\n",
            self.instance_count.load(Ordering::Relaxed)
        ));

        output
            .push_str("# HELP batata_healthy_instance_count Current number of healthy instances\n");
        output.push_str("# TYPE batata_healthy_instance_count gauge\n");
        output.push_str(&format!(
            "batata_healthy_instance_count {}\n\n",
            self.healthy_instance_count.load(Ordering::Relaxed)
        ));

        // ========== Connection Metrics ==========
        output.push_str(
            "# HELP batata_active_connections Current number of active HTTP connections\n",
        );
        output.push_str("# TYPE batata_active_connections gauge\n");
        output.push_str(&format!(
            "batata_active_connections {}\n\n",
            self.active_connections.load(Ordering::Relaxed)
        ));

        // ========== Cluster Metrics ==========
        output.push_str("# HELP batata_cluster_size Number of nodes in the cluster\n");
        output.push_str("# TYPE batata_cluster_size gauge\n");
        output.push_str(&format!("batata_cluster_size {}\n\n", cluster_size));

        output.push_str(
            "# HELP batata_healthy Whether the node is healthy (1=healthy, 0=unhealthy)\n",
        );
        output.push_str("# TYPE batata_healthy gauge\n");
        output.push_str(&format!(
            "batata_healthy {}\n\n",
            if is_healthy { 1 } else { 0 }
        ));

        // ========== Server Metrics ==========
        output
            .push_str("# HELP batata_uptime_seconds Number of seconds since the server started\n");
        output.push_str("# TYPE batata_uptime_seconds gauge\n");
        output.push_str(&format!(
            "batata_uptime_seconds {}\n",
            self.uptime_seconds()
        ));

        output
    }
}

/// Global metrics instance
pub static METRICS: std::sync::LazyLock<Arc<Metrics>> =
    std::sync::LazyLock::new(|| Arc::new(Metrics::new()));

#[get("")]
async fn metrics(data: web::Data<AppState>) -> impl Responder {
    let cluster_size = data.member_manager().all_members().len();
    let is_healthy = true; // Simplified - in production would check actual health

    let body = METRICS.to_prometheus_format(cluster_size, is_healthy);

    HttpResponse::Ok()
        .content_type("text/plain; version=0.0.4; charset=utf-8")
        .body(body)
}

pub fn routes() -> Scope {
    web::scope("/metrics").service(metrics)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_increment() {
        let m = Metrics::new();

        m.inc_http_requests();
        m.inc_http_requests();
        m.inc_grpc_requests();
        m.inc_config_publish();
        m.inc_config_query();
        m.inc_config_listen();
        m.inc_service_register();
        m.inc_service_subscribe();
        m.inc_heartbeat();

        assert_eq!(m.http_requests_total.load(Ordering::Relaxed), 2);
        assert_eq!(m.grpc_requests_total.load(Ordering::Relaxed), 1);
        assert_eq!(m.config_publish_total.load(Ordering::Relaxed), 1);
        assert_eq!(m.config_listen_total.load(Ordering::Relaxed), 1);
        assert_eq!(m.heartbeat_total.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_metrics_connections() {
        let m = Metrics::new();

        m.inc_connections();
        m.inc_connections();
        assert_eq!(m.active_connections.load(Ordering::Relaxed), 2);

        m.dec_connections();
        assert_eq!(m.active_connections.load(Ordering::Relaxed), 1);

        m.inc_grpc_connections();
        m.inc_grpc_connections();
        assert_eq!(m.grpc_connections.load(Ordering::Relaxed), 2);

        m.dec_grpc_connections();
        assert_eq!(m.grpc_connections.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_metrics_gauges() {
        let m = Metrics::new();

        m.set_config_count(100);
        m.set_service_count(50);
        m.set_instance_count(200, 180);

        assert_eq!(m.config_count.load(Ordering::Relaxed), 100);
        assert_eq!(m.service_count.load(Ordering::Relaxed), 50);
        assert_eq!(m.instance_count.load(Ordering::Relaxed), 200);
        assert_eq!(m.healthy_instance_count.load(Ordering::Relaxed), 180);
    }

    #[test]
    fn test_metrics_latency() {
        let m = Metrics::new();

        m.record_http_latency(1000); // 1ms
        m.record_http_latency(2000); // 2ms
        m.record_http_latency(3000); // 3ms

        // Average should be 2ms
        let avg = m.avg_http_latency_ms();
        assert!((avg - 2.0).abs() < 0.001);
    }

    #[test]
    fn test_prometheus_format() {
        let m = Metrics::new();
        m.inc_http_requests();
        m.inc_config_publish();
        m.set_service_count(10);
        m.set_instance_count(50, 45);

        let output = m.to_prometheus_format(3, true);

        assert!(output.contains("batata_http_requests_total 1"));
        assert!(output.contains("batata_config_publish_total 1"));
        assert!(output.contains("batata_service_count 10"));
        assert!(output.contains("batata_instance_count 50"));
        assert!(output.contains("batata_healthy_instance_count 45"));
        assert!(output.contains("batata_cluster_size 3"));
        assert!(output.contains("batata_healthy 1"));
        assert!(output.contains("# TYPE batata_http_requests_total counter"));
        assert!(output.contains("# TYPE batata_service_count gauge"));
    }
}
