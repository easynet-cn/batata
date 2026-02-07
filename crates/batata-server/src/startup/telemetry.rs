//! Telemetry and logging initialization module.
//!
//! Provides tracing subscriber setup with optional OpenTelemetry integration
//! for distributed tracing and observability.
//!
//! Supported exporters:
//! - OTLP (default): Export to any OTLP-compatible collector (Jaeger, Tempo, etc.)
//! - Jaeger: Direct export to Jaeger via Thrift over HTTP
//! - Zipkin: Direct export to Zipkin
//! - Console: Print spans to console (for debugging)

use std::time::Duration;

use opentelemetry::KeyValue;
use opentelemetry::trace::TracerProvider;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    Resource,
    trace::{RandomIdGenerator, Sampler, SdkTracerProvider},
};
use tracing::{Subscriber, subscriber::set_global_default};
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_log::LogTracer;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{EnvFilter, Registry, fmt::MakeWriter, layer::SubscriberExt};

/// Tracing exporter type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TracingExporter {
    /// OTLP exporter (default) - works with Jaeger, Tempo, etc.
    Otlp,
    /// Jaeger exporter via Thrift over HTTP
    Jaeger,
    /// Zipkin exporter
    Zipkin,
    /// Console exporter (for debugging)
    Console,
}

impl Default for TracingExporter {
    fn default() -> Self {
        Self::Otlp
    }
}

impl std::str::FromStr for TracingExporter {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "otlp" => Ok(Self::Otlp),
            "jaeger" => Ok(Self::Jaeger),
            "zipkin" => Ok(Self::Zipkin),
            "console" => Ok(Self::Console),
            _ => Err(format!("Unknown exporter type: {}", s)),
        }
    }
}

/// OpenTelemetry configuration
#[derive(Debug, Clone)]
pub struct OtelConfig {
    /// Enable OpenTelemetry tracing
    pub enabled: bool,
    /// Exporter type (otlp, jaeger, zipkin, console)
    pub exporter: TracingExporter,
    /// OTLP endpoint URL (e.g., "http://localhost:4317")
    pub otlp_endpoint: String,
    /// Jaeger endpoint URL (e.g., "http://localhost:14268/api/traces")
    pub jaeger_endpoint: String,
    /// Zipkin endpoint URL (e.g., "http://localhost:9411/api/v2/spans")
    pub zipkin_endpoint: String,
    /// Service name for traces
    pub service_name: String,
    /// Service version
    pub service_version: String,
    /// Sampling ratio (0.0 to 1.0)
    pub sampling_ratio: f64,
    /// Export timeout in seconds
    pub export_timeout_secs: u64,
}

impl Default for OtelConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            exporter: TracingExporter::Otlp,
            otlp_endpoint: "http://localhost:4317".to_string(),
            jaeger_endpoint: "http://localhost:14268/api/traces".to_string(),
            zipkin_endpoint: "http://localhost:9411/api/v2/spans".to_string(),
            service_name: "batata".to_string(),
            service_version: env!("CARGO_PKG_VERSION").to_string(),
            sampling_ratio: 1.0,
            export_timeout_secs: 10,
        }
    }
}

impl OtelConfig {
    /// Create config from environment variables
    pub fn from_env() -> Self {
        Self {
            enabled: std::env::var("OTEL_ENABLED")
                .map(|v| v.to_lowercase() == "true" || v == "1")
                .unwrap_or(false),
            exporter: std::env::var("OTEL_EXPORTER_TYPE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(TracingExporter::Otlp),
            otlp_endpoint: std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:4317".to_string()),
            jaeger_endpoint: std::env::var("OTEL_EXPORTER_JAEGER_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:14268/api/traces".to_string()),
            zipkin_endpoint: std::env::var("OTEL_EXPORTER_ZIPKIN_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:9411/api/v2/spans".to_string()),
            service_name: std::env::var("OTEL_SERVICE_NAME")
                .unwrap_or_else(|_| "batata".to_string()),
            service_version: std::env::var("OTEL_SERVICE_VERSION")
                .unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string()),
            sampling_ratio: std::env::var("OTEL_SAMPLING_RATIO")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1.0),
            export_timeout_secs: std::env::var("OTEL_EXPORT_TIMEOUT_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
        }
    }

    /// Create config from application configuration
    /// Environment variables take precedence over config file settings
    pub fn from_config(
        enabled: bool,
        endpoint: String,
        service_name: String,
        sampling_ratio: f64,
        export_timeout_secs: u64,
    ) -> Self {
        Self {
            enabled: std::env::var("OTEL_ENABLED")
                .map(|v| v.to_lowercase() == "true" || v == "1")
                .unwrap_or(enabled),
            exporter: std::env::var("OTEL_EXPORTER_TYPE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(TracingExporter::Otlp),
            otlp_endpoint: std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").unwrap_or(endpoint),
            jaeger_endpoint: std::env::var("OTEL_EXPORTER_JAEGER_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:14268/api/traces".to_string()),
            zipkin_endpoint: std::env::var("OTEL_EXPORTER_ZIPKIN_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:9411/api/v2/spans".to_string()),
            service_name: std::env::var("OTEL_SERVICE_NAME").unwrap_or(service_name),
            service_version: std::env::var("OTEL_SERVICE_VERSION")
                .unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string()),
            sampling_ratio: std::env::var("OTEL_SAMPLING_RATIO")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(sampling_ratio),
            export_timeout_secs: std::env::var("OTEL_EXPORT_TIMEOUT_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(export_timeout_secs),
        }
    }
}

/// Create an OpenTelemetry tracer provider.
///
/// This is used by the logging module to optionally add an OTEL layer.
pub(crate) fn init_tracer_provider(
    config: &OtelConfig,
) -> Result<SdkTracerProvider, Box<dyn std::error::Error>> {
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(&config.otlp_endpoint)
        .with_timeout(Duration::from_secs(config.export_timeout_secs))
        .build()?;

    let resource = Resource::builder_empty()
        .with_attributes([
            KeyValue::new("service.name", config.service_name.clone()),
            KeyValue::new("service.version", config.service_version.clone()),
        ])
        .build();

    let sampler = if config.sampling_ratio >= 1.0 {
        Sampler::AlwaysOn
    } else if config.sampling_ratio <= 0.0 {
        Sampler::AlwaysOff
    } else {
        Sampler::TraceIdRatioBased(config.sampling_ratio)
    };

    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_sampler(sampler)
        .with_id_generator(RandomIdGenerator::default())
        .with_resource(resource)
        .build();

    Ok(provider)
}

/// Creates a tracing subscriber with Bunyan JSON formatting.
///
/// # Arguments
/// * `name` - The application name for log entries
/// * `env_filter` - Default log level filter (e.g., "info", "debug")
/// * `sink` - The output sink for log entries
pub fn get_subscriber(
    name: &str,
    env_filter: &str,
    sink: impl for<'a> MakeWriter<'a> + 'static + Send + Sync,
) -> impl Subscriber + Send + Sync {
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(env_filter));
    let formatting_layer = BunyanFormattingLayer::new(name.into(), sink);

    Registry::default()
        .with(env_filter)
        .with(JsonStorageLayer)
        .with(formatting_layer)
}

/// Initialize tracing with optional OpenTelemetry support.
///
/// # Arguments
/// * `name` - The application name for log entries
/// * `env_filter` - Default log level filter (e.g., "info", "debug")
/// * `otel_config` - OpenTelemetry configuration
///
/// Returns an OtelGuard that must be kept alive for the duration of the application.
pub fn init_tracing_with_otel(
    name: &str,
    env_filter: &str,
    otel_config: &OtelConfig,
) -> Result<OtelGuard, Box<dyn std::error::Error>> {
    let env_filter_layer =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(env_filter));
    let formatting_layer = BunyanFormattingLayer::new(name.into(), std::io::stdout);

    LogTracer::init().map_err(|e| format!("Failed to set logger: {}", e))?;

    if otel_config.enabled {
        let provider = init_tracer_provider(otel_config)?;
        let tracer = provider.tracer("batata");
        let otel_layer = OpenTelemetryLayer::new(tracer);

        let subscriber = Registry::default()
            .with(env_filter_layer)
            .with(JsonStorageLayer)
            .with(formatting_layer)
            .with(otel_layer);

        set_global_default(subscriber).map_err(|e| format!("Failed to set subscriber: {}", e))?;
        Ok(OtelGuard::new(Some(provider)))
    } else {
        let subscriber = Registry::default()
            .with(env_filter_layer)
            .with(JsonStorageLayer)
            .with(formatting_layer);

        set_global_default(subscriber).map_err(|e| format!("Failed to set subscriber: {}", e))?;
        Ok(OtelGuard::new(None))
    }
}

/// Initializes the global subscriber for tracing.
///
/// This should only be called once during application startup.
pub fn init_subscriber(
    subscriber: impl Subscriber + Send + Sync,
) -> Result<(), Box<dyn std::error::Error>> {
    LogTracer::init().map_err(|e| format!("Failed to set logger: {}", e))?;
    set_global_default(subscriber).map_err(|e| format!("Failed to set subscriber: {}", e))?;
    Ok(())
}

/// Shutdown OpenTelemetry tracer provider gracefully
pub fn shutdown_tracer_provider(provider: Option<SdkTracerProvider>) {
    if let Some(provider) = provider {
        if let Err(e) = provider.shutdown() {
            tracing::warn!("Failed to shutdown OpenTelemetry tracer provider: {:?}", e);
        }
    }
}

/// OpenTelemetry guard that ensures proper shutdown
pub struct OtelGuard {
    provider: Option<SdkTracerProvider>,
}

impl OtelGuard {
    pub fn new(provider: Option<SdkTracerProvider>) -> Self {
        Self { provider }
    }
}

impl Drop for OtelGuard {
    fn drop(&mut self) {
        if let Some(provider) = self.provider.take() {
            if let Err(e) = provider.shutdown() {
                eprintln!("Failed to shutdown OpenTelemetry tracer provider: {:?}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_otel_config_default() {
        let config = OtelConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.exporter, TracingExporter::Otlp);
        assert_eq!(config.otlp_endpoint, "http://localhost:4317");
        assert_eq!(config.jaeger_endpoint, "http://localhost:14268/api/traces");
        assert_eq!(config.zipkin_endpoint, "http://localhost:9411/api/v2/spans");
        assert_eq!(config.service_name, "batata");
        assert_eq!(config.sampling_ratio, 1.0);
    }

    #[test]
    fn test_tracing_exporter_from_str() {
        assert_eq!(
            "otlp".parse::<TracingExporter>().unwrap(),
            TracingExporter::Otlp
        );
        assert_eq!(
            "jaeger".parse::<TracingExporter>().unwrap(),
            TracingExporter::Jaeger
        );
        assert_eq!(
            "zipkin".parse::<TracingExporter>().unwrap(),
            TracingExporter::Zipkin
        );
        assert_eq!(
            "console".parse::<TracingExporter>().unwrap(),
            TracingExporter::Console
        );
        assert!("invalid".parse::<TracingExporter>().is_err());
    }

    #[test]
    fn test_otel_config_from_env() {
        // Set environment variables
        // SAFETY: This test is run in a single thread and sets/unsets env vars atomically
        unsafe {
            std::env::set_var("OTEL_ENABLED", "true");
            std::env::set_var("OTEL_EXPORTER_OTLP_ENDPOINT", "http://otel:4317");
            std::env::set_var("OTEL_SERVICE_NAME", "test-service");
            std::env::set_var("OTEL_SAMPLING_RATIO", "0.5");
        }

        let config = OtelConfig::from_env();
        assert!(config.enabled);
        assert_eq!(config.otlp_endpoint, "http://otel:4317");
        assert_eq!(config.service_name, "test-service");
        assert_eq!(config.sampling_ratio, 0.5);

        // Clean up
        // SAFETY: This test is run in a single thread and sets/unsets env vars atomically
        unsafe {
            std::env::remove_var("OTEL_ENABLED");
            std::env::remove_var("OTEL_EXPORTER_OTLP_ENDPOINT");
            std::env::remove_var("OTEL_SERVICE_NAME");
            std::env::remove_var("OTEL_SAMPLING_RATIO");
        }
    }
}
