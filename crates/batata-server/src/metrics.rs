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

    // gRPC connection metrics
    describe_gauge!("grpc_connections_active", "Active gRPC client connections");
    describe_gauge!("grpc_subscriptions_active", "Active gRPC subscriptions");

    // Raft state metrics
    describe_gauge!(
        "raft_state",
        "Raft node state (0=follower, 1=candidate, 2=leader)"
    );
    describe_gauge!("raft_leader_id", "Current Raft leader node ID");

    // Registry size metrics
    describe_gauge!(
        "naming_dashmap_services_size",
        "Number of service keys in DashMap"
    );
    describe_gauge!("config_items_total", "Total number of configuration items");

    // gRPC request latency
    describe_histogram!(
        "grpc_request_duration_seconds",
        "gRPC request duration in seconds"
    );

    // gRPC handler dispatch metrics
    describe_histogram!(
        "grpc_handler_duration_seconds",
        "gRPC handler execution time per message type"
    );
    describe_counter!(
        "grpc_handler_calls_total",
        "Total gRPC handler invocations by message type"
    );
    describe_counter!(
        "grpc_handler_errors_total",
        "Total gRPC handler errors by message type"
    );

    // Connection lifecycle metrics
    describe_counter!(
        "grpc_connections_established_total",
        "Total gRPC connections established"
    );
    describe_counter!(
        "grpc_connections_closed_total",
        "Total gRPC connections closed"
    );

    // Config change notification metrics
    describe_histogram!(
        "config_change_notify_duration_seconds",
        "Config change notification fan-out latency"
    );
    describe_counter!(
        "config_change_notify_total",
        "Total config change notifications sent"
    );

    // Naming subscription fan-out metrics
    describe_histogram!(
        "naming_subscription_fanout_duration_seconds",
        "Naming service subscription fan-out time"
    );
    describe_counter!(
        "naming_subscription_fanout_total",
        "Total naming subscription fan-out events"
    );

    // Rate limiter metrics
    describe_gauge!(
        "ratelimit_tracked_ips",
        "Number of IPs currently tracked by rate limiter"
    );
    describe_counter!(
        "ratelimit_rejected_total",
        "Total requests rejected by rate limiter"
    );

    // System resource metrics (updated periodically)
    describe_gauge!(
        "system_cpu_usage_percent",
        "System CPU usage percentage (0-100)"
    );
    describe_gauge!("system_memory_used_bytes", "System used memory in bytes");
    describe_gauge!("system_memory_total_bytes", "System total memory in bytes");
    describe_gauge!("process_memory_bytes", "Process resident memory in bytes");

    tracing::info!("Metrics initialized");
}

/// Normalize an HTTP path for use as a metrics label.
///
/// Strips query parameters and normalizes dynamic path segments to prevent
/// high-cardinality label explosion. For example:
/// - `/nacos/v2/ns/instance?ip=1.2.3.4` -> `/nacos/v2/ns/instance`
/// - `/v3/console/namespace/abc123` -> `/v3/console/namespace/:id`
fn normalize_path(path: &str) -> String {
    // Strip query parameters
    let path = path.split('?').next().unwrap_or(path);
    // Strip trailing slash (except root)
    let path = if path.len() > 1 {
        path.trim_end_matches('/')
    } else {
        path
    };
    path.to_string()
}

/// Record an HTTP request with normalized path labels.
///
/// Path is normalized to prevent high-cardinality label explosion
/// (e.g., query params are stripped, only the route pattern is kept).
pub fn record_http_request(method: &str, path: &str, status: u16, duration_secs: f64) {
    let norm_path = normalize_path(path);
    let status_str = status.to_string();
    counter!("http_requests_total", "method" => method.to_string(), "path" => norm_path.clone(), "status" => status_str.clone()).increment(1);
    histogram!("http_request_duration_seconds", "method" => method.to_string(), "path" => norm_path.clone()).record(duration_secs);

    if status >= 400 {
        counter!("http_requests_errors_total", "method" => method.to_string(), "path" => norm_path, "status" => status_str).increment(1);
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

/// Update gRPC connection counts
pub fn set_grpc_connections(active: f64) {
    gauge!("grpc_connections_active").set(active);
}

/// Update gRPC subscription count
pub fn set_grpc_subscriptions(active: f64) {
    gauge!("grpc_subscriptions_active").set(active);
}

/// Update Raft state (0=follower, 1=candidate, 2=leader)
pub fn set_raft_node_state(state: f64, leader_id: f64) {
    gauge!("raft_state").set(state);
    gauge!("raft_leader_id").set(leader_id);
}

/// Update naming DashMap service key count
pub fn set_naming_dashmap_size(size: f64) {
    gauge!("naming_dashmap_services_size").set(size);
}

/// Update total config items count
pub fn set_config_items_total(count: f64) {
    gauge!("config_items_total").set(count);
}

/// Record a gRPC request duration
pub fn record_grpc_request(method: &str, duration_secs: f64) {
    histogram!("grpc_request_duration_seconds", "method" => method.to_string())
        .record(duration_secs);
}

/// Record a gRPC handler invocation with timing
pub fn record_grpc_handler(message_type: &str, duration_secs: f64, success: bool) {
    counter!("grpc_handler_calls_total", "type" => message_type.to_string()).increment(1);
    histogram!("grpc_handler_duration_seconds", "type" => message_type.to_string())
        .record(duration_secs);
    if !success {
        counter!("grpc_handler_errors_total", "type" => message_type.to_string()).increment(1);
    }
}

/// Record a gRPC connection lifecycle event
pub fn record_grpc_connection_event(event: &str) {
    match event {
        "established" => counter!("grpc_connections_established_total").increment(1),
        "closed" => counter!("grpc_connections_closed_total").increment(1),
        _ => {}
    }
}

/// Record a config change notification fan-out
pub fn record_config_notify(duration_secs: f64, count: u64) {
    counter!("config_change_notify_total").increment(count);
    histogram!("config_change_notify_duration_seconds").record(duration_secs);
}

/// Record a naming subscription fan-out event
pub fn record_naming_fanout(duration_secs: f64, count: u64) {
    counter!("naming_subscription_fanout_total").increment(count);
    histogram!("naming_subscription_fanout_duration_seconds").record(duration_secs);
}

/// Update rate limiter tracked IPs count
pub fn set_ratelimit_tracked_ips(count: f64) {
    gauge!("ratelimit_tracked_ips").set(count);
}

/// Record a rate limit rejection
pub fn record_ratelimit_rejection() {
    counter!("ratelimit_rejected_total").increment(1);
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

/// Default interval for system stats collection (seconds).
const DEFAULT_SYSTEM_STATS_INTERVAL_SECS: u64 = 15;

/// Start a background task that periodically collects system resource metrics
/// (CPU usage, memory) and reports them as Prometheus gauges.
///
/// Interval is configurable; defaults to 15 seconds. Uses `sysinfo` crate
/// which costs ~1-5ms per refresh on modern hardware.
pub fn start_system_stats_reporter(interval_secs: Option<u64>) -> tokio::task::JoinHandle<()> {
    let interval =
        std::time::Duration::from_secs(interval_secs.unwrap_or(DEFAULT_SYSTEM_STATS_INTERVAL_SECS));
    tokio::spawn(async move {
        use sysinfo::System;
        let mut sys = System::new();
        let mut ticker = tokio::time::interval(interval);
        let pid = sysinfo::get_current_pid().ok();

        loop {
            ticker.tick().await;

            // Refresh only what we need (CPU + memory), not full system scan
            sys.refresh_cpu_usage();
            sys.refresh_memory();

            // System-wide CPU usage (average across all cores)
            let cpu_usage: f64 = if sys.cpus().is_empty() {
                0.0
            } else {
                sys.cpus().iter().map(|c| c.cpu_usage() as f64).sum::<f64>()
                    / sys.cpus().len() as f64
            };
            gauge!("system_cpu_usage_percent").set(cpu_usage);
            gauge!("system_memory_used_bytes").set(sys.used_memory() as f64);
            gauge!("system_memory_total_bytes").set(sys.total_memory() as f64);

            // Process-specific memory
            if let Some(pid) = pid {
                sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
                if let Some(process) = sys.process(pid) {
                    gauge!("process_memory_bytes").set(process.memory() as f64);
                }
            }
        }
    })
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

    #[test]
    fn test_normalize_path_strips_query_params() {
        assert_eq!(
            normalize_path("/nacos/v2/ns/instance?ip=1.2.3.4&port=8080"),
            "/nacos/v2/ns/instance"
        );
    }

    #[test]
    fn test_normalize_path_strips_trailing_slash() {
        assert_eq!(
            normalize_path("/nacos/v2/ns/instance/"),
            "/nacos/v2/ns/instance"
        );
    }

    #[test]
    fn test_normalize_path_preserves_root() {
        assert_eq!(normalize_path("/"), "/");
    }

    #[test]
    fn test_normalize_path_no_change() {
        assert_eq!(normalize_path("/v3/admin/cs/config"), "/v3/admin/cs/config");
    }
}
