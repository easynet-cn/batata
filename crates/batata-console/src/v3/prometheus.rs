//! Prometheus query API endpoint
//!
//! Provides a simplified Prometheus-compatible query endpoint that allows
//! filtering metrics by namespace and service name labels.
//! Endpoint: GET /v3/console/prometheus/query

use actix_web::{HttpRequest, HttpResponse, Responder, Scope, get, web};
use serde::Deserialize;

use batata_server_common::{ActionTypes, ApiType, Secured, SignType, model::AppState, secured};

use super::metrics::METRICS;

#[derive(Debug, Deserialize)]
pub struct PrometheusQueryParams {
    #[serde(default)]
    pub namespace: Option<String>,
    #[serde(default, alias = "serviceName")]
    pub service_name: Option<String>,
    #[serde(default)]
    pub metric: Option<String>,
}

/// GET /v3/console/prometheus/query - Query Prometheus-style metrics
///
/// Supports filtering by namespace, service_name, and metric name.
/// Returns metrics in Prometheus JSON format compatible with Grafana.
#[get("/query")]
async fn prometheus_query(
    req: HttpRequest,
    data: web::Data<AppState>,
    query: web::Query<PrometheusQueryParams>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let cluster_size = data.cluster_manager().member_count();
    let is_healthy = data.cluster_manager().is_cluster_healthy();
    let full_metrics = METRICS.to_prometheus_format(cluster_size, is_healthy);

    // Filter metrics by requested criteria
    let filtered: Vec<&str> = full_metrics
        .lines()
        .filter(|line| {
            // Skip comments and empty lines for filtering purposes
            if line.starts_with('#') || line.is_empty() {
                return true;
            }

            // Filter by metric name prefix
            if let Some(ref metric) = query.metric
                && !line.starts_with(metric.as_str())
            {
                return false;
            }

            // Filter by namespace label
            if let Some(ref ns) = query.namespace
                && !ns.is_empty()
                && !line.contains(&format!("namespace=\"{}\"", ns))
                && line.contains("namespace=")
            {
                return false;
            }

            // Filter by service_name label
            if let Some(ref svc) = query.service_name
                && !svc.is_empty()
                && !line.contains(&format!("service=\"{}\"", svc))
                && line.contains("service=")
            {
                return false;
            }

            true
        })
        .collect();

    let result_text = filtered.join("\n");

    // Return in Prometheus JSON query result format
    HttpResponse::Ok().json(serde_json::json!({
        "status": "success",
        "data": {
            "resultType": "text",
            "result": result_text,
        }
    }))
}

/// GET /v3/console/prometheus/metrics - Get raw Prometheus metrics text
#[get("/metrics")]
async fn prometheus_text(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    HttpResponse::Ok()
        .content_type("text/plain; version=0.0.4; charset=utf-8")
        .body(METRICS.to_prometheus_format(1, true))
}

/// Create the Prometheus query routes
pub fn routes() -> Scope {
    web::scope("/prometheus")
        .service(prometheus_query)
        .service(prometheus_text)
}
