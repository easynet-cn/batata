//! Distributed tracing query endpoints
//!
//! Provides a lightweight trace query interface for the console UI.
//! For production use, configure OTLP export to Jaeger or Tempo and
//! query those backends directly. These endpoints report tracing status
//! and provide basic service visibility.

use actix_web::{Responder, get, web};
use serde::Deserialize;

use batata_server_common::model::AppState;
use batata_server_common::model::response::Result;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Query parameters for listing distributed traces.
pub struct TraceListQuery {
    /// Page number (1-based).
    #[serde(default = "default_page_no", alias = "pageNo")]
    pub page_no: u64,
    /// Page size.
    #[serde(default = "default_page_size", alias = "pageSize")]
    pub page_size: u64,
    /// Optional service name filter.
    #[serde(default)]
    pub service: Option<String>,
    /// Optional operation name filter.
    #[serde(default)]
    pub operation: Option<String>,
    /// Optional trace ID filter.
    #[serde(default, alias = "traceId")]
    pub trace_id: Option<String>,
    /// Optional minimum duration filter in milliseconds.
    #[serde(default, alias = "minDurationMs")]
    pub min_duration_ms: Option<u64>,
    /// Optional start time filter.
    #[serde(default, alias = "startTime")]
    pub start_time: Option<String>,
    /// Optional end time filter.
    #[serde(default, alias = "endTime")]
    pub end_time: Option<String>,
}

fn default_page_no() -> u64 {
    1
}
fn default_page_size() -> u64 {
    20
}

/// GET /v3/console/trace/list
/// Query traces. For full tracing, configure OTLP export to Jaeger/Tempo.
#[get("/list")]
pub async fn list_traces(
    data: web::Data<AppState>,
    _query: web::Query<TraceListQuery>,
) -> impl Responder {
    let otel_enabled = data.configuration.otel_enabled();
    let otel_endpoint = data.configuration.otel_endpoint();

    if otel_enabled {
        Result::<()>::http_success(serde_json::json!({
            "totalCount": 0,
            "pageNumber": 1,
            "pagesAvailable": 0,
            "pageItems": [],
            "tracing": {
                "enabled": true,
                "backend": "otlp",
                "endpoint": otel_endpoint,
                "hint": "Traces are exported via OTLP. Query your trace backend (Jaeger/Tempo) directly for full trace data."
            }
        }))
    } else {
        Result::<()>::http_success(serde_json::json!({
            "totalCount": 0,
            "pageNumber": 1,
            "pagesAvailable": 0,
            "pageItems": [],
            "tracing": {
                "enabled": false,
                "hint": "Enable OpenTelemetry tracing in application.yml: batata.otel.enabled=true"
            }
        }))
    }
}

/// GET /v3/console/trace/services
/// List services that generate traces
#[get("/services")]
pub async fn list_trace_services(data: web::Data<AppState>) -> impl Responder {
    let services = vec![serde_json::json!({
        "name": data.configuration.otel_service_name(),
        "operationCount": 0,
        "type": "service"
    })];

    Result::<()>::http_success(services)
}

/// Register the distributed tracing routes under `/trace`.
pub fn routes() -> actix_web::Scope {
    web::scope("/trace")
        .service(list_traces)
        .service(list_trace_services)
}
