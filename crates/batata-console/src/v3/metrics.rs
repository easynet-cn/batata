//! Prometheus metrics endpoint for observability

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock};
use std::time::Instant;

use actix_web::{HttpResponse, Responder, Scope, get, web};

use crate::datasource::ConsoleDataSource;

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
    /// Service registration operations
    pub service_register_total: AtomicU64,
    /// Service query operations
    pub service_query_total: AtomicU64,
    /// Active connections count
    pub active_connections: AtomicU64,
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

    pub fn inc_service_register(&self) {
        self.service_register_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_service_query(&self) {
        self.service_query_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_connections(&self) {
        self.active_connections.fetch_add(1, Ordering::Relaxed);
    }

    pub fn dec_connections(&self) {
        self.active_connections.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn uptime_seconds(&self) -> u64 {
        self.start_time.map(|t| t.elapsed().as_secs()).unwrap_or(0)
    }

    /// Format metrics in Prometheus text format
    pub fn to_prometheus_format(&self, cluster_size: usize, is_healthy: bool) -> String {
        let mut output = String::with_capacity(2048);

        // Help and type declarations
        output
            .push_str("# HELP batata_http_requests_total Total number of HTTP requests received\n");
        output.push_str("# TYPE batata_http_requests_total counter\n");
        output.push_str(&format!(
            "batata_http_requests_total {}\n",
            self.http_requests_total.load(Ordering::Relaxed)
        ));

        output
            .push_str("# HELP batata_grpc_requests_total Total number of gRPC requests received\n");
        output.push_str("# TYPE batata_grpc_requests_total counter\n");
        output.push_str(&format!(
            "batata_grpc_requests_total {}\n",
            self.grpc_requests_total.load(Ordering::Relaxed)
        ));

        output.push_str(
            "# HELP batata_config_publish_total Total number of config publish operations\n",
        );
        output.push_str("# TYPE batata_config_publish_total counter\n");
        output.push_str(&format!(
            "batata_config_publish_total {}\n",
            self.config_publish_total.load(Ordering::Relaxed)
        ));

        output
            .push_str("# HELP batata_config_query_total Total number of config query operations\n");
        output.push_str("# TYPE batata_config_query_total counter\n");
        output.push_str(&format!(
            "batata_config_query_total {}\n",
            self.config_query_total.load(Ordering::Relaxed)
        ));

        output.push_str("# HELP batata_service_register_total Total number of service registration operations\n");
        output.push_str("# TYPE batata_service_register_total counter\n");
        output.push_str(&format!(
            "batata_service_register_total {}\n",
            self.service_register_total.load(Ordering::Relaxed)
        ));

        output.push_str(
            "# HELP batata_service_query_total Total number of service query operations\n",
        );
        output.push_str("# TYPE batata_service_query_total counter\n");
        output.push_str(&format!(
            "batata_service_query_total {}\n",
            self.service_query_total.load(Ordering::Relaxed)
        ));

        output.push_str("# HELP batata_active_connections Current number of active connections\n");
        output.push_str("# TYPE batata_active_connections gauge\n");
        output.push_str(&format!(
            "batata_active_connections {}\n",
            self.active_connections.load(Ordering::Relaxed)
        ));

        output
            .push_str("# HELP batata_uptime_seconds Number of seconds since the server started\n");
        output.push_str("# TYPE batata_uptime_seconds gauge\n");
        output.push_str(&format!(
            "batata_uptime_seconds {}\n",
            self.uptime_seconds()
        ));

        output.push_str("# HELP batata_cluster_size Number of nodes in the cluster\n");
        output.push_str("# TYPE batata_cluster_size gauge\n");
        output.push_str(&format!("batata_cluster_size {}\n", cluster_size));

        output.push_str(
            "# HELP batata_healthy Whether the node is healthy (1=healthy, 0=unhealthy)\n",
        );
        output.push_str("# TYPE batata_healthy gauge\n");
        output.push_str(&format!(
            "batata_healthy {}\n",
            if is_healthy { 1 } else { 0 }
        ));

        output
    }
}

/// Global metrics instance
pub static METRICS: LazyLock<Arc<Metrics>> = LazyLock::new(|| Arc::new(Metrics::new()));

#[get("")]
pub async fn metrics(datasource: web::Data<Arc<dyn ConsoleDataSource>>) -> impl Responder {
    let cluster_size = datasource.cluster_member_count();
    let is_healthy = datasource.cluster_health().is_healthy;

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

        assert_eq!(m.http_requests_total.load(Ordering::Relaxed), 2);
        assert_eq!(m.grpc_requests_total.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_metrics_connections() {
        let m = Metrics::new();

        m.inc_connections();
        m.inc_connections();
        assert_eq!(m.active_connections.load(Ordering::Relaxed), 2);

        m.dec_connections();
        assert_eq!(m.active_connections.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_prometheus_format() {
        let m = Metrics::new();
        m.inc_http_requests();

        let output = m.to_prometheus_format(3, true);

        assert!(output.contains("batata_http_requests_total 1"));
        assert!(output.contains("batata_cluster_size 3"));
        assert!(output.contains("batata_healthy 1"));
        assert!(output.contains("# TYPE batata_http_requests_total counter"));
    }
}
