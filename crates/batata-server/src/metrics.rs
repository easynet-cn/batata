// Metrics module for observability
// Provides counters, gauges, and histograms for monitoring application performance

use std::time::Instant;

use metrics::{counter, describe_counter, describe_gauge, describe_histogram, gauge, histogram};

/// Initialize all metric descriptions
/// Should be called once at application startup
pub fn init_metrics() {
    // HTTP request metrics
    describe_counter!(
        "http_requests_total",
        "Total number of HTTP requests received"
    );
    describe_histogram!(
        "http_request_duration_seconds",
        "HTTP request duration in seconds"
    );
    describe_counter!(
        "http_requests_errors_total",
        "Total number of HTTP request errors"
    );

    // Database metrics
    describe_histogram!(
        "db_query_duration_seconds",
        "Database query duration in seconds"
    );
    describe_counter!("db_queries_total", "Total number of database queries");
    describe_counter!(
        "db_query_errors_total",
        "Total number of database query errors"
    );

    // Cache metrics
    describe_counter!("cache_hits_total", "Total number of cache hits");
    describe_counter!("cache_misses_total", "Total number of cache misses");
    describe_gauge!("cache_size", "Current number of items in cache");

    // Cluster metrics
    describe_gauge!("cluster_members_total", "Total number of cluster members");
    describe_gauge!(
        "cluster_members_healthy",
        "Number of healthy cluster members"
    );
    describe_counter!(
        "cluster_sync_operations_total",
        "Total number of cluster sync operations"
    );

    // Raft metrics
    describe_gauge!("raft_term", "Current Raft term");
    describe_gauge!("raft_log_index", "Current Raft log index");
    describe_counter!("raft_proposals_total", "Total number of Raft proposals");

    // Config metrics
    describe_counter!(
        "config_publish_total",
        "Total number of config publish operations"
    );
    describe_counter!(
        "config_query_total",
        "Total number of config query operations"
    );

    // Naming metrics
    describe_gauge!(
        "naming_instances_total",
        "Total number of registered instances"
    );
    describe_gauge!(
        "naming_services_total",
        "Total number of registered services"
    );
    describe_counter!(
        "naming_heartbeats_total",
        "Total number of instance heartbeats"
    );

    tracing::info!("Metrics initialized");
}

/// Record an HTTP request
pub fn record_http_request(method: &str, path: &str, status: u16, duration_secs: f64) {
    counter!("http_requests_total", "method" => method.to_string(), "path" => path.to_string(), "status" => status.to_string()).increment(1);
    histogram!("http_request_duration_seconds", "method" => method.to_string(), "path" => path.to_string()).record(duration_secs);

    if status >= 400 {
        counter!("http_requests_errors_total", "method" => method.to_string(), "path" => path.to_string(), "status" => status.to_string()).increment(1);
    }
}

/// Record a database query
pub fn record_db_query(operation: &str, table: &str, duration_secs: f64, success: bool) {
    counter!("db_queries_total", "operation" => operation.to_string(), "table" => table.to_string()).increment(1);
    histogram!("db_query_duration_seconds", "operation" => operation.to_string(), "table" => table.to_string()).record(duration_secs);

    if !success {
        counter!("db_query_errors_total", "operation" => operation.to_string(), "table" => table.to_string()).increment(1);
    }
}

/// Record a cache hit
pub fn record_cache_hit(cache_name: &str) {
    counter!("cache_hits_total", "cache" => cache_name.to_string()).increment(1);
}

/// Record a cache miss
pub fn record_cache_miss(cache_name: &str) {
    counter!("cache_misses_total", "cache" => cache_name.to_string()).increment(1);
}

/// Update cache size
pub fn set_cache_size(cache_name: &str, size: f64) {
    gauge!("cache_size", "cache" => cache_name.to_string()).set(size);
}

/// Update cluster member counts
pub fn set_cluster_members(total: f64, healthy: f64) {
    gauge!("cluster_members_total").set(total);
    gauge!("cluster_members_healthy").set(healthy);
}

/// Record a cluster sync operation
pub fn record_cluster_sync(sync_type: &str) {
    counter!("cluster_sync_operations_total", "type" => sync_type.to_string()).increment(1);
}

/// Update Raft state
pub fn set_raft_state(term: f64, log_index: f64) {
    gauge!("raft_term").set(term);
    gauge!("raft_log_index").set(log_index);
}

/// Record a Raft proposal
pub fn record_raft_proposal(proposal_type: &str) {
    counter!("raft_proposals_total", "type" => proposal_type.to_string()).increment(1);
}

/// Record a config operation
pub fn record_config_operation(operation: &str) {
    match operation {
        "publish" => counter!("config_publish_total").increment(1),
        "query" => counter!("config_query_total").increment(1),
        _ => {}
    }
}

/// Update naming service counts
pub fn set_naming_counts(instances: f64, services: f64) {
    gauge!("naming_instances_total").set(instances);
    gauge!("naming_services_total").set(services);
}

/// Record a naming heartbeat
pub fn record_naming_heartbeat() {
    counter!("naming_heartbeats_total").increment(1);
}

/// Timer helper for measuring operation duration
pub struct Timer {
    start: Instant,
}

impl Timer {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    pub fn elapsed_secs(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }
}

impl Default for Timer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timer() {
        let timer = Timer::new();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let elapsed = timer.elapsed_secs();
        assert!(elapsed >= 0.01);
        assert!(elapsed < 0.1);
    }
}
